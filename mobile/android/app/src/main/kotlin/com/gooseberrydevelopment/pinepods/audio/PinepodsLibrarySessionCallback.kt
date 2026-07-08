package com.gooseberrydevelopment.pinepods.audio

import android.os.Bundle
import android.util.Log
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.session.LibraryResult
import androidx.media3.session.MediaLibraryService
import androidx.media3.session.MediaSession
import androidx.media3.session.SessionCommand
import androidx.media3.session.SessionResult
import com.google.common.collect.ImmutableList
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.guava.future

class PinepodsLibrarySessionCallback(
    private val service: PinepodsMediaService,
    private var mediaBrowserHelper: MediaBrowserHelper?,
    private var eventStreamHandler: AudioEventStreamHandler?
) : MediaLibraryService.MediaLibrarySession.Callback {

    private val scope = CoroutineScope(Dispatchers.Main)

    fun updateEventStreamHandler(handler: AudioEventStreamHandler) {
        this.eventStreamHandler = handler
    }

    fun updateMediaBrowserHelper(helper: MediaBrowserHelper) {
        this.mediaBrowserHelper = helper
        Log.d(TAG, "MediaBrowserHelper updated with Flutter connectivity")
    }

    override fun onConnect(
        session: MediaSession,
        controller: MediaSession.ControllerInfo
    ): MediaSession.ConnectionResult {
        Log.d(TAG, "onConnect called by ${controller.packageName}")
        AudioPlayerPlugin.logToFlutter("INFO", TAG, "onConnect: accepting connection from ${controller.packageName}")

        // Accept all connections (including Android Auto) and additionally expose
        // our custom rewind / fast-forward session commands so the custom-layout
        // buttons (added in PinepodsMediaService.applyCustomLayout) are enabled.
        val sessionCommands = MediaSession.ConnectionResult.DEFAULT_SESSION_AND_LIBRARY_COMMANDS
            .buildUpon()
            .add(SessionCommand(ACTION_REWIND, Bundle.EMPTY))
            .add(SessionCommand(ACTION_FAST_FORWARD, Bundle.EMPTY))
            .build()

        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Connection accepted for ${controller.packageName} with seek commands")

        return MediaSession.ConnectionResult.accept(
            sessionCommands,
            MediaSession.ConnectionResult.DEFAULT_PLAYER_COMMANDS
        )
    }

    override fun onGetLibraryRoot(
        session: MediaLibraryService.MediaLibrarySession,
        browser: MediaSession.ControllerInfo,
        params: MediaLibraryService.LibraryParams?
    ): ListenableFuture<LibraryResult<MediaItem>> {
        Log.d(TAG, "onGetLibraryRoot called by ${browser.packageName}")
        AudioPlayerPlugin.logToFlutter("INFO", TAG, "onGetLibraryRoot called by ${browser.packageName}, params=$params")

        try {
            // Return root item for Android Auto browsing
            val rootItem = MediaItem.Builder()
                .setMediaId(MediaBrowserHelper.ROOT_ID)
                .setMediaMetadata(
                    MediaMetadata.Builder()
                        .setIsPlayable(false)
                        .setIsBrowsable(true)
                        .setMediaType(MediaMetadata.MEDIA_TYPE_FOLDER_MIXED)
                        .setTitle("Pinepods")
                        .build()
                )
                .build()

            AudioPlayerPlugin.logToFlutter("INFO", TAG, "onGetLibraryRoot returning root item with ID=${MediaBrowserHelper.ROOT_ID}")
            return Futures.immediateFuture(LibraryResult.ofItem(rootItem, params))
        } catch (e: Exception) {
            Log.e(TAG, "Error in onGetLibraryRoot", e)
            AudioPlayerPlugin.logToFlutter("ERROR", TAG, "Error in onGetLibraryRoot: ${e.message}")
            return Futures.immediateFuture(LibraryResult.ofError(LibraryResult.RESULT_ERROR_UNKNOWN))
        }
    }

    override fun onGetChildren(
        session: MediaLibraryService.MediaLibrarySession,
        browser: MediaSession.ControllerInfo,
        parentId: String,
        page: Int,
        pageSize: Int,
        params: MediaLibraryService.LibraryParams?
    ): ListenableFuture<LibraryResult<ImmutableList<MediaItem>>> {
        Log.d(TAG, "onGetChildren: $parentId")
        AudioPlayerPlugin.logToFlutter("INFO", TAG, "onGetChildren called for parentId=$parentId, helper=${if (mediaBrowserHelper != null) "ready" else "NULL"}")

        // The top-level menu (root + "More") is static and needs no Flutter, so
        // always serve it directly. This keeps the Android Auto front page from
        // showing "no episodes" when the car connects before the Flutter engine
        // has bound the service (cold start with the phone app closed).
        buildStaticMenu(parentId)?.let { staticItems ->
            AudioPlayerPlugin.logToFlutter("INFO", TAG, "onGetChildren returning ${staticItems.size} static items for parentId=$parentId")
            return Futures.immediateFuture(LibraryResult.ofItemList(staticItems, params))
        }

        // For dynamic content, if Flutter hasn't connected yet, return empty
        // list. Once Flutter connects, notifyChildrenChanged() (in the service)
        // invalidates Android Auto's cache so it re-queries these ids.
        if (mediaBrowserHelper == null) {
            Log.w(TAG, "MediaBrowserHelper not ready yet, returning empty list")
            AudioPlayerPlugin.logToFlutter("WARN", TAG, "MediaBrowserHelper not ready yet, returning empty list for parentId=$parentId")
            return Futures.immediateFuture(LibraryResult.ofItemList(ImmutableList.of(), params))
        }

        return scope.future {
            try {
                val children = mediaBrowserHelper!!.getChildren(parentId)

                // Convert MediaBrowserCompat.MediaItem to Media3 MediaItem
                val mediaItems = children.map { compatItem ->
                    val desc = compatItem.description
                    val isPlayable = compatItem.isBrowsable.not()

                    MediaItem.Builder()
                        .setMediaId(desc.mediaId ?: "")
                        .setMediaMetadata(
                            MediaMetadata.Builder()
                                .setIsPlayable(isPlayable)
                                .setIsBrowsable(!isPlayable)
                                .setMediaType(
                                    if (isPlayable) MediaMetadata.MEDIA_TYPE_PODCAST
                                    else MediaMetadata.MEDIA_TYPE_FOLDER_PODCASTS
                                )
                                .setTitle(desc.title)
                                .setSubtitle(desc.subtitle)
                                .setDescription(desc.description)
                                .setArtworkUri(desc.iconUri)
                                .build()
                        )
                        .build()
                }

                AudioPlayerPlugin.logToFlutter("INFO", TAG, "onGetChildren returning ${mediaItems.size} items to Android Auto for parentId=$parentId")
                LibraryResult.ofItemList(mediaItems, params)
            } catch (e: Exception) {
                Log.e(TAG, "Error getting children for $parentId", e)
                AudioPlayerPlugin.logToFlutter("ERROR", TAG, "Error getting children for $parentId: ${e.message}")
                LibraryResult.ofError(LibraryResult.RESULT_ERROR_UNKNOWN)
            }
        }
    }

    /**
     * Builds the static, Flutter-independent browse menus (the root list and the
     * "More" submenu). Returns null for any other parent id (which requires
     * live data from Flutter). These mirror MediaBrowserHelper.getRootMenuItems /
     * getMoreMenuItems but produce Media3 items so they can be served even
     * before Flutter has connected.
     */
    private fun buildStaticMenu(parentId: String): ImmutableList<MediaItem>? {
        val items: List<MediaItem> = when (parentId) {
            MediaBrowserHelper.ROOT_ID -> listOf(
                browsableFolder(MediaBrowserHelper.CURRENT_ID, "Current", "Currently listening"),
                browsableFolder(MediaBrowserHelper.QUEUE_ID, "Queue", "Queued episodes"),
                browsableFolder(MediaBrowserHelper.DOWNLOADS_ID, "Downloads", "Downloaded episodes"),
                browsableFolder(MediaBrowserHelper.MORE_ID, "More", "More options")
            )
            MediaBrowserHelper.MORE_ID -> listOf(
                browsableFolder(MediaBrowserHelper.SAVED_ID, "Saved", "Saved episodes"),
                browsableFolder(MediaBrowserHelper.HISTORY_ID, "History", "Recently played"),
                browsableFolder(MediaBrowserHelper.PODCASTS_ID, "Podcasts", "Your subscriptions"),
                browsableFolder(MediaBrowserHelper.PLAYLISTS_ID, "Playlists", "Your playlists")
            )
            else -> return null
        }
        return ImmutableList.copyOf(items)
    }

    private fun browsableFolder(mediaId: String, title: String, subtitle: String): MediaItem {
        return MediaItem.Builder()
            .setMediaId(mediaId)
            .setMediaMetadata(
                MediaMetadata.Builder()
                    .setIsPlayable(false)
                    .setIsBrowsable(true)
                    .setMediaType(MediaMetadata.MEDIA_TYPE_FOLDER_MIXED)
                    .setTitle(title)
                    .setSubtitle(subtitle)
                    .build()
            )
            .build()
    }

    override fun onGetItem(
        session: MediaLibraryService.MediaLibrarySession,
        browser: MediaSession.ControllerInfo,
        mediaId: String
    ): ListenableFuture<LibraryResult<MediaItem>> {
        Log.d(TAG, "onGetItem: $mediaId")
        AudioPlayerPlugin.logToFlutter("INFO", TAG, "onGetItem called for mediaId=$mediaId by ${browser.packageName}")

        // For now, return error - we handle items through onGetChildren
        return Futures.immediateFuture(
            LibraryResult.ofError(LibraryResult.RESULT_ERROR_NOT_SUPPORTED)
        )
    }

    override fun onSearch(
        session: MediaLibraryService.MediaLibrarySession,
        browser: MediaSession.ControllerInfo,
        query: String,
        params: MediaLibraryService.LibraryParams?
    ): ListenableFuture<LibraryResult<Void>> {
        Log.d(TAG, "onSearch: $query")

        return scope.future {
            try {
                // Trigger search - results will be returned via onGetSearchResult
                LibraryResult.ofVoid()
            } catch (e: Exception) {
                Log.e(TAG, "Search error", e)
                LibraryResult.ofError(LibraryResult.RESULT_ERROR_UNKNOWN)
            }
        }
    }

    override fun onGetSearchResult(
        session: MediaLibraryService.MediaLibrarySession,
        browser: MediaSession.ControllerInfo,
        query: String,
        page: Int,
        pageSize: Int,
        params: MediaLibraryService.LibraryParams?
    ): ListenableFuture<LibraryResult<ImmutableList<MediaItem>>> {
        Log.d(TAG, "onGetSearchResult: $query")

        // If Flutter hasn't connected yet, return empty results
        if (mediaBrowserHelper == null) {
            Log.w(TAG, "MediaBrowserHelper not ready yet, returning empty search results")
            return Futures.immediateFuture(LibraryResult.ofItemList(ImmutableList.of(), params))
        }

        return scope.future {
            try {
                val results = mediaBrowserHelper!!.search(query)

                // Convert to Media3 MediaItem
                val mediaItems = results.map { compatItem ->
                    val desc = compatItem.description

                    MediaItem.Builder()
                        .setMediaId(desc.mediaId ?: "")
                        .setMediaMetadata(
                            MediaMetadata.Builder()
                                .setIsPlayable(true)
                                .setIsBrowsable(false)
                                .setMediaType(MediaMetadata.MEDIA_TYPE_PODCAST)
                                .setTitle(desc.title)
                                .setSubtitle(desc.subtitle)
                                .setDescription(desc.description)
                                .setArtworkUri(desc.iconUri)
                                .build()
                        )
                        .build()
                }

                LibraryResult.ofItemList(mediaItems, params)
            } catch (e: Exception) {
                Log.e(TAG, "Error getting search results", e)
                LibraryResult.ofError(LibraryResult.RESULT_ERROR_UNKNOWN)
            }
        }
    }

    override fun onAddMediaItems(
        mediaSession: MediaSession,
        controller: MediaSession.ControllerInfo,
        mediaItems: MutableList<MediaItem>
    ): ListenableFuture<MutableList<MediaItem>> {
        Log.d(TAG, "onAddMediaItems: ${mediaItems.size} items")

        // If Flutter hasn't connected yet, can't play anything
        if (mediaBrowserHelper == null) {
            Log.w(TAG, "MediaBrowserHelper not ready yet, cannot play items")
            return super.onAddMediaItems(mediaSession, controller, mediaItems)
        }

        // When Android Auto selects an item to play, handle it here
        mediaItems.forEach { mediaItem ->
            val mediaId = mediaItem.mediaId
            Log.d(TAG, "Playing media ID: $mediaId")
            mediaBrowserHelper!!.playFromMediaId(mediaId)
        }

        return super.onAddMediaItems(mediaSession, controller, mediaItems)
    }

    override fun onCustomCommand(
        session: MediaSession,
        controller: MediaSession.ControllerInfo,
        customCommand: SessionCommand,
        args: Bundle
    ): ListenableFuture<SessionResult> {
        Log.d(TAG, "onCustomCommand: ${customCommand.customAction}")

        when (customCommand.customAction) {
            ACTION_REWIND -> {
                // Delegate to the session player (the ForwardingPlayer), whose
                // seekBack() honors the configurable rewind interval.
                session.player.seekBack()
                return Futures.immediateFuture(SessionResult(SessionResult.RESULT_SUCCESS))
            }
            ACTION_FAST_FORWARD -> {
                session.player.seekForward()
                return Futures.immediateFuture(SessionResult(SessionResult.RESULT_SUCCESS))
            }
            "setPlaybackSpeed" -> {
                val speed = args.getFloat("speed", 1.0f)
                service.setPlaybackSpeed(speed)
                return Futures.immediateFuture(SessionResult(SessionResult.RESULT_SUCCESS))
            }
            "setTrimSilence" -> {
                val enabled = args.getBoolean("enabled", false)
                service.setTrimSilence(enabled)
                return Futures.immediateFuture(SessionResult(SessionResult.RESULT_SUCCESS))
            }
            "setVolumeBoost" -> {
                val enabled = args.getBoolean("enabled", false)
                service.setVolumeBoost(enabled)
                return Futures.immediateFuture(SessionResult(SessionResult.RESULT_SUCCESS))
            }
        }

        return super.onCustomCommand(session, controller, customCommand, args)
    }

    companion object {
        private const val TAG = "LibrarySessionCallback"

        // Custom session commands backing the rewind / fast-forward buttons in
        // the media session custom layout (Android Auto + notification).
        const val ACTION_REWIND = "com.gooseberrydevelopment.pinepods.REWIND"
        const val ACTION_FAST_FORWARD = "com.gooseberrydevelopment.pinepods.FAST_FORWARD"
    }
}
