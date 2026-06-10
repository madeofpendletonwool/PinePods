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
import androidx.media3.exoplayer.DefaultLoadControl
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.upstream.DefaultAllocator
import androidx.media3.session.MediaLibraryService
import androidx.media3.session.MediaSession
import androidx.media3.ui.PlayerNotificationManager
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import com.gooseberrydevelopment.pinepods.MainActivity
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.guava.future
import kotlinx.coroutines.launch

class PinepodsMediaService : MediaLibraryService() {
    private var mediaSession: MediaLibrarySession? = null
    private val serviceScope = CoroutineScope(Dispatchers.Main)
    private var player: ExoPlayer? = null
    private var eventStreamHandler: AudioEventStreamHandler? = null
    private var sessionCallback: PinepodsLibrarySessionCallback? = null
    private var loudnessEnhancer: LoudnessEnhancer? = null
    private var mediaBrowserHelper: MediaBrowserHelper? = null
    private val binder = LocalBinder()
    private val handler = Handler(Looper.getMainLooper())
    private var positionUpdateRunnable: Runnable? = null

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

        initializePlayer()
        initializeMediaLibrarySessionEarly()
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

        // Build media library session
        val builder = MediaLibrarySession.Builder(this, player!!, sessionCallback!!)
            .setId("pinepods_media_library_session")
            .also { sessionActivityIntent?.let { intent -> it.setSessionActivity(intent) } }

        mediaSession = builder.build()

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

        Log.d(TAG, "Media library session connected to Flutter - Android Auto browsing now fully functional")
        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Flutter connected - MediaBrowserHelper ready, Android Auto should now have data")
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
                sendPlaybackStateEvent()
                handler.postDelayed(this, 500)  // Update every 500ms
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

    fun setPlaybackSpeed(speed: Float) {
        Log.d(TAG, "setPlaybackSpeed: $speed")
        player?.setPlaybackSpeed(speed)
    }

    fun setTrimSilence(enabled: Boolean) {
        Log.d(TAG, "setTrimSilence: $enabled")
        player?.skipSilenceEnabled = enabled
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
