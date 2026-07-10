package com.gooseberrydevelopment.pinepods.audio

import android.app.Notification
import android.app.PendingIntent
import android.content.Intent
import android.media.audiofx.LoudnessEnhancer
import android.net.Uri
import java.io.File
import android.os.Binder
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import androidx.media3.common.ForwardingPlayer
import androidx.media3.exoplayer.DefaultLoadControl
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.upstream.DefaultAllocator
import androidx.media3.session.CommandButton
import androidx.media3.session.MediaLibraryService
import androidx.media3.session.MediaSession
import androidx.media3.session.SessionCommand
import androidx.media3.ui.PlayerNotificationManager
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import com.gooseberrydevelopment.pinepods.MainActivity
import com.gooseberrydevelopment.pinepods.R
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.guava.future
import kotlinx.coroutines.launch

class PinepodsMediaService : MediaLibraryService() {
    private var mediaSession: MediaLibrarySession? = null
    private val serviceScope = CoroutineScope(Dispatchers.Main)
    private var player: ExoPlayer? = null
    // Configurable skip intervals (ms) used by the media session's rewind /
    // fast-forward buttons in Android Auto and the notification. Kept in sync
    // with the in-app setting via setSkipIntervals().
    private var seekForwardMs: Long = 30000
    private var seekBackMs: Long = 10000
    private var eventStreamHandler: AudioEventStreamHandler? = null
    private var sessionCallback: PinepodsLibrarySessionCallback? = null
    private var loudnessEnhancer: LoudnessEnhancer? = null
    private var mediaBrowserHelper: MediaBrowserHelper? = null
    private val binder = LocalBinder()
    private val handler = Handler(Looper.getMainLooper())
    private var positionUpdateRunnable: Runnable? = null
    // Ad-skip ranges (#790), in SECONDS. ExoPlayer's skipSilenceEnabled DSP can
    // only remove silence, not ads (which are content), so ads are skipped by
    // seeking past these ranges from the 1s position poll. Cleared per-episode.
    private var adSkipSegments: List<Pair<Double, Double>> = emptyList()

    inner class LocalBinder : Binder() {
        fun getService(): PinepodsMediaService = this@PinepodsMediaService
    }


    override fun onCreate() {
        super.onCreate()
        Log.d(TAG, "PinepodsMediaService onCreate - thread: ${Thread.currentThread().name}")
        AudioPlayerPlugin.logToFlutter("INFO", TAG, "PinepodsMediaService onCreate - service starting")

        // Create notification channel for Android O+
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.O) {
            val channel = android.app.NotificationChannel(
                NOTIFICATION_CHANNEL_ID,
                "Pinepods Playback",
                android.app.NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = "Pinepods podcast playback controls"
                setShowBadge(false)
            }
            val notificationManager = getSystemService(android.app.NotificationManager::class.java)
            notificationManager.createNotificationChannel(channel)
        }

        // Wrap init so a failure here can never leave the service without a
        // valid media session. Android Auto drops apps whose onGetSession
        // returns null, which is a likely cause of the app intermittently
        // disappearing from the car's app list.
        try {
            initializePlayer()
            initializeMediaLibrarySessionEarly()
        } catch (e: Exception) {
            Log.e(TAG, "Error during media service initialization", e)
            AudioPlayerPlugin.logToFlutter("ERROR", TAG, "Error during media service initialization: ${e.message}")
        }
    }

    /**
     * Best-effort (re)initialization used to guarantee a non-null session for
     * onGetSession. Safe to call repeatedly.
     */
    private fun ensureInitialized() {
        try {
            if (player == null) {
                initializePlayer()
            }
            if (mediaSession == null) {
                initializeMediaLibrarySessionEarly()
            }
        } catch (e: Exception) {
            Log.e(TAG, "ensureInitialized failed", e)
            AudioPlayerPlugin.logToFlutter("ERROR", TAG, "ensureInitialized failed: ${e.message}")
        }
    }

    /**
     * Initialize media library session early for Android Auto
     * This ensures the session is ready when Android Auto connects,
     * even before Flutter binds to the service
     */
    private fun initializeMediaLibrarySessionEarly() {
        if (mediaSession != null) {
            return
        }

        Log.d(TAG, "Early initialization of media library session for Android Auto")
        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Early initialization of media library session for Android Auto")

        // Create session activity intent
        val sessionActivityIntent = packageManager?.getLaunchIntentForPackage(packageName)?.let {
            PendingIntent.getActivity(
                this,
                0,
                it,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
            )
        }

        // Create minimal callback for early initialization
        sessionCallback = PinepodsLibrarySessionCallback(this, null, eventStreamHandler)

        // Build media library session using a ForwardingPlayer that hides the
        // "skip to next/previous track" commands so Android Auto / bluetooth /
        // the notification render REWIND and FAST-FORWARD buttons instead. With
        // a single-item podcast timeline the default next/prev buttons either do
        // nothing or seek to the very start of the episode ("back to start"),
        // which is what users were hitting. Rewind / fast-forward honor the
        // configurable increments below.
        val builder = MediaLibrarySession.Builder(this, createSeekOnlyPlayer(player!!), sessionCallback!!)
            .setId("pinepods_media_library_session")
            .also { sessionActivityIntent?.let { intent -> it.setSessionActivity(intent) } }

        mediaSession = builder.build()

        // Because we hide the skip-to-next/previous commands (see
        // createSeekOnlyPlayer), Media3's default transport UI would only leave
        // play/pause. Add explicit rewind / fast-forward buttons to the custom
        // layout so Android Auto and the notification show them and they trigger
        // the player's seekBack() / seekForward() (which honor the configurable
        // interval).
        applyCustomLayout()

        Log.d(TAG, "MediaLibrarySession built successfully - sessionId: ${mediaSession?.id}, token: ${mediaSession?.sessionCompatToken}")

        Log.d(TAG, "Media library session initialized early - ready for Android Auto")
        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Media library session initialized early - ready for Android Auto")
    }

    private fun initializePlayer() {
        // Configure load control for better buffering (fixes rewind rebuffering issue)
        val loadControl = DefaultLoadControl.Builder()
            .setAllocator(DefaultAllocator(true, 16))
            .setBufferDurationsMs(
                60_000,    // min buffer: 60s (increased from buggy 45s)
                180_000,   // max buffer: 3min
                5_000,     // buffer for playback start: 5s
                10_000     // buffer for rebuffer: 10s
            )
            .setBackBuffer(60_000, true)  // Keep 60s back buffer for rewind without rebuffering
            .setPrioritizeTimeOverSizeThresholds(true)
            .build()

        // Configure audio attributes for podcast playback
        val audioAttributes = AudioAttributes.Builder()
            .setContentType(C.AUDIO_CONTENT_TYPE_SPEECH)
            .setUsage(C.USAGE_MEDIA)
            .build()

        // Create ExoPlayer with optimized configuration
        player = ExoPlayer.Builder(this)
            .setLoadControl(loadControl)
            .setAudioAttributes(audioAttributes, true)
            .setHandleAudioBecomingNoisy(true)  // Auto-pause when headphones disconnect
            // Default wake mode; overridden per-episode in playEpisode() so that
            // downloaded/local playback uses WAKE_MODE_LOCAL (CPU only) instead of
            // holding a WifiLock for the whole episode. Streams need the network lock.
            .setWakeMode(C.WAKE_MODE_NETWORK)
            .setSeekBackIncrementMs(10000)  // 10 seconds rewind
            .setSeekForwardIncrementMs(30000)  // 30 seconds forward
            .build()

        // Initialize loudness enhancer for volume boost
        player?.let { p ->
            try {
                loudnessEnhancer = LoudnessEnhancer(p.audioSessionId)
                loudnessEnhancer?.enabled = false
            } catch (e: Exception) {
                Log.w(TAG, "Failed to initialize loudness enhancer", e)
            }
        }

        // Listen to player events
        player?.addListener(object : Player.Listener {
            override fun onPlaybackStateChanged(playbackState: Int) {
                Log.d(TAG, "Playback state changed: $playbackState")
                sendPlaybackStateEvent()

                // Handle completion
                if (playbackState == Player.STATE_ENDED) {
                    sendEvent(mapOf(
                        "type" to "completed"
                    ))
                }
            }

            override fun onIsPlayingChanged(isPlaying: Boolean) {
                Log.d(TAG, "Is playing changed: $isPlaying")
                sendPlaybackStateEvent()

                // Start/stop position updates and foreground service
                if (isPlaying) {
                    startPositionUpdates()
                    startForeground(NOTIFICATION_ID, createNotification())
                } else {
                    stopPositionUpdates()
                }
            }

            override fun onPlayerError(error: androidx.media3.common.PlaybackException) {
                Log.e(TAG, "Player error", error)
                sendEvent(mapOf(
                    "type" to "error",
                    "code" to error.errorCode,
                    "message" to (error.message ?: "Unknown error")
                ))
            }

            override fun onMediaMetadataChanged(mediaMetadata: MediaMetadata) {
                Log.d(TAG, "Media metadata changed: ${mediaMetadata.title}")
            }
        })

        // Create media session
        val sessionActivityIntent = packageManager?.getLaunchIntentForPackage(packageName)?.let {
            PendingIntent.getActivity(
                this,
                0,
                it,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
            )
        }

        Log.d(TAG, "Player initialized, media session will be created when plugin connects")
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaLibrarySession? {
        Log.d(TAG, "onGetSession called - package: ${controllerInfo.packageName}, uid: ${controllerInfo.uid}, session exists: ${mediaSession != null}")
        AudioPlayerPlugin.logToFlutter("INFO", TAG, "onGetSession called by ${controllerInfo.packageName}, session=${if (mediaSession != null) "initialized" else "NULL"}")

        // Guarantee a session exists before returning; a null return makes
        // Android Auto drop the app from its list.
        if (mediaSession == null) {
            ensureInitialized()
        }

        if (mediaSession != null) {
            Log.d(TAG, "Returning MediaLibrarySession - id: ${mediaSession?.id}")
            AudioPlayerPlugin.logToFlutter("INFO", TAG, "Returning valid MediaLibrarySession to ${controllerInfo.packageName}")
        } else {
            Log.e(TAG, "ERROR: mediaSession is NULL when ${controllerInfo.packageName} requested it!")
            AudioPlayerPlugin.logToFlutter("ERROR", TAG, "mediaSession is NULL when ${controllerInfo.packageName} tried to connect!")
        }

        return mediaSession
    }

    /**
     * Initialize the media library session with browsing support
     * Called by AudioPlayerPlugin when it connects with the method channel
     * This replaces the early initialization with full Flutter connectivity
     */
    fun initializeMediaLibrarySession(methodChannel: io.flutter.plugin.common.MethodChannel) {
        Log.d(TAG, "Connecting Flutter method channel to media library session")
        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Flutter connecting - updating MediaBrowserHelper")

        // Create MediaBrowserHelper with method channel
        mediaBrowserHelper = MediaBrowserHelper(this, methodChannel)

        // Update the callback with the real helper
        sessionCallback?.updateMediaBrowserHelper(mediaBrowserHelper!!)

        // Now that Flutter can supply real data, invalidate any empty results
        // Android Auto cached while the helper was null (cold start). This makes
        // Auto re-query these nodes and replace the "no episodes" placeholders.
        notifyBrowseContentChanged()

        Log.d(TAG, "Media library session connected to Flutter - Android Auto browsing now fully functional")
        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Flutter connected - MediaBrowserHelper ready, Android Auto should now have data")
    }

    /**
     * Ask any connected browsers (Android Auto) to re-fetch the content nodes.
     * Called once Flutter connects so stale empty results are refreshed.
     */
    private fun notifyBrowseContentChanged() {
        val session = mediaSession ?: return
        val contentIds = listOf(
            MediaBrowserHelper.ROOT_ID,
            MediaBrowserHelper.CURRENT_ID,
            MediaBrowserHelper.QUEUE_ID,
            MediaBrowserHelper.DOWNLOADS_ID,
            MediaBrowserHelper.MORE_ID,
            MediaBrowserHelper.SAVED_ID,
            MediaBrowserHelper.HISTORY_ID,
            MediaBrowserHelper.PODCASTS_ID,
            MediaBrowserHelper.PLAYLISTS_ID
        )
        for (id in contentIds) {
            try {
                session.notifyChildrenChanged(id, Int.MAX_VALUE, null)
            } catch (e: Exception) {
                Log.w(TAG, "notifyChildrenChanged failed for $id", e)
            }
        }
    }

    /**
     * Update the MediaBrowserHelper in the callback
     */
    fun updateMediaBrowserHelper(helper: MediaBrowserHelper) {
        sessionCallback?.updateMediaBrowserHelper(helper)
    }

    override fun onBind(intent: Intent?): IBinder? {
        val superBinder = super.onBind(intent)
        val action = intent?.action
        val packageName = intent?.`package`

        Log.d(TAG, "onBind called - action: $action, package: $packageName, superBinder: ${superBinder != null}")
        AudioPlayerPlugin.logToFlutter("INFO", TAG, "onBind: action=$action, package=$packageName, returningSuper=${superBinder != null}")

        // If this is a MediaBrowserService bind request, return the superclass binder
        // Otherwise return our LocalBinder
        return if (superBinder != null) {
            AudioPlayerPlugin.logToFlutter("INFO", TAG, "Returning MediaLibraryService binder for browsing")
            superBinder
        } else {
            AudioPlayerPlugin.logToFlutter("INFO", TAG, "Returning LocalBinder for Flutter")
            binder
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val action = intent?.action
        Log.d(TAG, "onStartCommand - action: $action, flags: $flags, startId: $startId")
        AudioPlayerPlugin.logToFlutter("INFO", TAG, "onStartCommand: action=$action")
        return super.onStartCommand(intent, flags, startId)
    }

    override fun onTaskRemoved(rootIntent: Intent?) {
        Log.d(TAG, "onTaskRemoved")
        AudioPlayerPlugin.logToFlutter("INFO", TAG, "onTaskRemoved - app task removed")
        super.onTaskRemoved(rootIntent)
    }

    override fun onDestroy() {
        Log.d(TAG, "onDestroy called - cleaning up service")
        AudioPlayerPlugin.logToFlutter("INFO", TAG, "PinepodsMediaService onDestroy - service shutting down")

        stopPositionUpdates()
        mediaSession?.run {
            player.release()
            release()
            mediaSession = null
        }
        loudnessEnhancer?.release()
        loudnessEnhancer = null
        super.onDestroy()

        Log.d(TAG, "PinepodsMediaService destroyed")
        AudioPlayerPlugin.logToFlutter("INFO", TAG, "PinepodsMediaService destroyed successfully")
    }

    fun setEventStreamHandler(handler: AudioEventStreamHandler) {
        this.eventStreamHandler = handler
        // Update callback with event handler
        sessionCallback?.updateEventStreamHandler(handler)
        Log.d(TAG, "Event stream handler set")
    }

    private fun sendEvent(event: Map<String, Any>) {
        eventStreamHandler?.sendEvent(event)
    }

    private fun sendPlaybackStateEvent() {
        player?.let { p ->
            val state = when {
                p.playbackState == Player.STATE_BUFFERING -> "buffering"
                p.isPlaying -> "playing"
                p.playbackState == Player.STATE_ENDED -> "completed"
                p.playbackState == Player.STATE_IDLE -> "stopped"
                else -> "paused"
            }

            val duration = if (p.duration != C.TIME_UNSET) p.duration else 0L

            sendEvent(mapOf(
                "type" to "playbackState",
                "state" to state,
                "position" to p.currentPosition,
                "bufferedPosition" to p.bufferedPosition,
                "duration" to duration,
                "speed" to p.playbackParameters.speed
            ))
        }
    }

    private fun startPositionUpdates() {
        stopPositionUpdates()
        positionUpdateRunnable = object : Runnable {
            override fun run() {
                applyAdSkipIfNeeded()
                sendPlaybackStateEvent()
                handler.postDelayed(this, 1000)  // Update every 1s (battery: halves channel/UI work)
            }
        }
        handler.post(positionUpdateRunnable!!)
    }

    private fun stopPositionUpdates() {
        positionUpdateRunnable?.let {
            handler.removeCallbacks(it)
            positionUpdateRunnable = null
        }
    }

    // Public API for Flutter

    fun playEpisode(url: String, startPosition: Int, isLocal: Boolean, metadata: Map<String, Any>?) {
        Log.d(TAG, "playEpisode: url=$url, startPosition=$startPosition, isLocal=$isLocal")

        player?.let { p ->
            try {
                // For local downloads `url` is a raw filesystem path (which may
                // contain spaces and has no scheme). Uri.parse() leaves it
                // scheme-less and unencoded, so ExoPlayer fails to load it and
                // never reports a duration (player shows 00:00 and won't scrub).
                // Uri.fromFile() builds a properly-encoded file:// URI.
                val uri = if (isLocal) Uri.fromFile(File(url)) else Uri.parse(url)

                // Use a CPU-only wakelock for local files; only streamed episodes
                // need the WifiLock that WAKE_MODE_NETWORK adds. Holding the WiFi
                // radio awake for downloaded playback is a major battery drain.
                p.setWakeMode(if (isLocal) C.WAKE_MODE_LOCAL else C.WAKE_MODE_NETWORK)

                // Build media metadata
                val mediaMetadataBuilder = MediaMetadata.Builder()
                if (metadata != null) {
                    (metadata["title"] as? String)?.let { mediaMetadataBuilder.setTitle(it) }
                    (metadata["artist"] as? String)?.let { mediaMetadataBuilder.setArtist(it) }
                    (metadata["artwork"] as? String)?.let { artwork ->
                        try {
                            val artworkUri = Uri.parse(artwork)
                            mediaMetadataBuilder.setArtworkUri(artworkUri)
                        } catch (e: Exception) {
                            Log.w(TAG, "Failed to parse artwork URI: $artwork", e)
                        }
                    }
                }

                val builtMetadata = mediaMetadataBuilder.build()
                val mediaItem = MediaItem.Builder()
                    .setUri(uri)
                    .setMediaMetadata(builtMetadata)
                    .build()

                Log.d(TAG, "Playing: ${builtMetadata.title} by ${builtMetadata.artist}")
                Log.d(TAG, "Media item URI: $uri, startPosition: $startPosition")

                // New episode: drop the previous episode's ad-skip ranges. The
                // Dart layer re-supplies them via setAdSkipSegments after load.
                adSkipSegments = emptyList()

                p.setMediaItem(mediaItem)
                p.prepare()

                if (startPosition > 0) {
                    p.seekTo(startPosition.toLong())
                }

                p.play()

                Log.d(TAG, "Episode playback started, player state: ${p.playbackState}, isPlaying: ${p.isPlaying}")
            } catch (e: Exception) {
                Log.e(TAG, "Error playing episode", e)
                sendEvent(mapOf(
                    "type" to "error",
                    "code" to -1,
                    "message" to (e.message ?: "Failed to play episode")
                ))
            }
        }
    }

    fun play() {
        Log.d(TAG, "play")
        player?.play()
    }

    fun pause() {
        Log.d(TAG, "pause")
        player?.pause()
    }

    fun stop() {
        Log.d(TAG, "stop")
        player?.stop()
        player?.clearMediaItems()
        adSkipSegments = emptyList()
    }

    fun seek(positionMs: Int) {
        Log.d(TAG, "seek to $positionMs ms")
        player?.seekTo(positionMs.toLong())
    }

    fun fastForward(milliseconds: Int) {
        player?.let { p ->
            val newPosition = (p.currentPosition + milliseconds).coerceAtMost(p.duration)
            p.seekTo(newPosition)
            Log.d(TAG, "fastForward by $milliseconds ms to $newPosition")
        }
    }

    fun rewind(milliseconds: Int) {
        player?.let { p ->
            val newPosition = (p.currentPosition - milliseconds).coerceAtLeast(0)
            p.seekTo(newPosition)
            Log.d(TAG, "rewind by $milliseconds ms to $newPosition")
        }
    }

    /**
     * Update the skip intervals used by the media session's rewind /
     * fast-forward buttons (Android Auto, bluetooth, notification) so they
     * match the user's in-app setting.
     */
    fun setSkipIntervals(forwardMs: Long, backwardMs: Long) {
        seekForwardMs = if (forwardMs > 0) forwardMs else seekForwardMs
        seekBackMs = if (backwardMs > 0) backwardMs else seekBackMs
        Log.d(TAG, "setSkipIntervals: forward=${seekForwardMs}ms back=${seekBackMs}ms")
    }

    /**
     * Publishes rewind / fast-forward buttons to the media session's custom
     * layout so Android Auto and the media notification render them. They use
     * the built-in seek player commands, which the ForwardingPlayer maps to the
     * configurable increments.
     */
    private fun applyCustomLayout() {
        val session = mediaSession ?: return
        val rewindButton = CommandButton.Builder()
            .setSessionCommand(SessionCommand(PinepodsLibrarySessionCallback.ACTION_REWIND, android.os.Bundle.EMPTY))
            .setIconResId(R.drawable.ic_car_rewind)
            .setDisplayName("Rewind")
            .build()
        val fastForwardButton = CommandButton.Builder()
            .setSessionCommand(SessionCommand(PinepodsLibrarySessionCallback.ACTION_FAST_FORWARD, android.os.Bundle.EMPTY))
            .setIconResId(R.drawable.ic_car_fast_forward)
            .setDisplayName("Fast forward")
            .build()
        session.setCustomLayout(listOf(rewindButton, fastForwardButton))
    }

    /**
     * Wraps the ExoPlayer for use by the MediaLibrarySession so that only
     * time-based seeking (rewind / fast-forward) is exposed to Android Auto and
     * media controllers — not episode-level skip-to-next/previous. Increments
     * are read live from [seekForwardMs] / [seekBackMs] so the configurable
     * setting is honored without rebuilding the session.
     */
    private fun createSeekOnlyPlayer(base: Player): Player {
        return object : ForwardingPlayer(base) {
            override fun getAvailableCommands(): Player.Commands {
                return super.getAvailableCommands().buildUpon()
                    .remove(Player.COMMAND_SEEK_TO_NEXT)
                    .remove(Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM)
                    .remove(Player.COMMAND_SEEK_TO_PREVIOUS)
                    .remove(Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM)
                    .add(Player.COMMAND_SEEK_BACK)
                    .add(Player.COMMAND_SEEK_FORWARD)
                    .build()
            }

            override fun getSeekForwardIncrement(): Long = seekForwardMs

            override fun getSeekBackIncrement(): Long = seekBackMs

            override fun seekForward() {
                val max = if (duration == C.TIME_UNSET) Long.MAX_VALUE else duration
                seekTo((currentPosition + seekForwardMs).coerceAtMost(max))
            }

            override fun seekBack() {
                seekTo((currentPosition - seekBackMs).coerceAtLeast(0))
            }
        }
    }

    fun setPlaybackSpeed(speed: Float) {
        Log.d(TAG, "setPlaybackSpeed: $speed")
        player?.setPlaybackSpeed(speed)
    }

    fun setTrimSilence(enabled: Boolean) {
        Log.d(TAG, "setTrimSilence: $enabled")
        player?.skipSilenceEnabled = enabled
    }

    // Ad-skip (#790): replace the active ad ranges (seconds). Empty list clears.
    // The 1s position poll seeks past any range the playhead enters.
    fun setAdSkipSegments(segments: List<Pair<Double, Double>>) {
        Log.d(TAG, "setAdSkipSegments: ${segments.size} range(s)")
        adSkipSegments = segments.sortedBy { it.first }
    }

    // Called every ~1s from the position poll. If the playhead is inside an
    // active ad range, seek to its end. Times are seconds; ExoPlayer is ms.
    // Uses a 0.25s tail tolerance (matching iOS) to avoid boundary seek-loops.
    private fun applyAdSkipIfNeeded() {
        if (adSkipSegments.isEmpty()) return
        val p = player ?: return
        val posMs = p.currentPosition
        if (posMs < 0) return
        for ((start, end) in adSkipSegments) {
            val startMs = (start * 1000).toLong()
            val endMs = (end * 1000).toLong()
            if (posMs >= startMs && posMs < endMs - 250) {
                Log.d(TAG, "Ad-skip: seeking from ${posMs}ms past ad ending ${endMs}ms")
                p.seekTo(endMs)
                break
            }
        }
    }

    fun setVolumeBoost(enabled: Boolean) {
        Log.d(TAG, "setVolumeBoost: $enabled")
        try {
            loudnessEnhancer?.let { enhancer ->
                if (enabled) {
                    enhancer.setTargetGain(800)  // 0.8 gain (same as old implementation)
                    enhancer.enabled = true
                } else {
                    enhancer.enabled = false
                }
            }
        } catch (e: Exception) {
            Log.w(TAG, "Failed to set volume boost", e)
        }
    }

    fun getCurrentPosition(): Long {
        return player?.currentPosition ?: 0
    }

    fun getDuration(): Long {
        val duration = player?.duration ?: C.TIME_UNSET
        return if (duration != C.TIME_UNSET) duration else 0L
    }

    private fun createNotification(): Notification {
        val sessionActivity = packageManager?.getLaunchIntentForPackage(packageName)?.let {
            PendingIntent.getActivity(
                this,
                0,
                it,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
            )
        }

        val currentMetadata = player?.currentMediaItem?.mediaMetadata
        val title = currentMetadata?.title?.toString() ?: "Pinepods"
        val artist = currentMetadata?.artist?.toString() ?: "Loading..."

        val builder = NotificationCompat.Builder(this, NOTIFICATION_CHANNEL_ID)
            .setContentTitle(title)
            .setContentText(artist)
            .setSmallIcon(android.R.drawable.ic_media_play)
            .setContentIntent(sessionActivity)
            .setOngoing(true)
            .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)

        // Use MediaStyle if we have a media session
        mediaSession?.let { session ->
            builder.setStyle(
                androidx.media.app.NotificationCompat.MediaStyle()
                    .setMediaSession(session.sessionCompatToken)
            )
        }

        return builder.build()
    }

    companion object {
        private const val TAG = "PinepodsMediaService"
        private const val NOTIFICATION_CHANNEL_ID = "pinepods_playback"
        private const val NOTIFICATION_ID = 1
    }
}
