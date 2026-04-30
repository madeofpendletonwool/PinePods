package com.gooseberrydevelopment.pinepods.audio

import android.content.Context
import android.net.Uri
import android.os.Bundle
import android.support.v4.media.MediaBrowserCompat
import android.support.v4.media.MediaDescriptionCompat
import android.util.Log
import androidx.media3.session.MediaSession
import io.flutter.plugin.common.MethodChannel
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlin.coroutines.resume

/**
 * Helper class to manage Android Auto / car media browsing
 * Provides hierarchical content structure for in-car browsing
 */
class MediaBrowserHelper(
    private val context: Context,
    private val methodChannel: MethodChannel
) {
    companion object {
        private const val TAG = "MediaBrowserHelper"

        // Root media IDs
        const val ROOT_ID = "__ROOT__"
        const val CURRENT_ID = "__CURRENT__"
        const val QUEUE_ID = "__QUEUE__"
        const val DOWNLOADS_ID = "__DOWNLOADS__"
        const val MORE_ID = "__MORE__"

        // More submenu IDs
        const val SAVED_ID = "__SAVED__"
        const val HISTORY_ID = "__HISTORY__"
        const val PODCASTS_ID = "__PODCASTS__"
        const val PLAYLISTS_ID = "__PLAYLISTS__"

        // Media ID prefixes
        const val PREFIX_PODCAST = "__PODCAST__|"
        const val PREFIX_EPISODE = "__EPISODE__|"
        const val PREFIX_CURRENT_ITEM = "__CURRENT__|"
        const val PREFIX_QUEUE_ITEM = "__QUEUE__|"
        const val PREFIX_DOWNLOAD = "__DOWNLOAD__|"
        const val PREFIX_SAVED_ITEM = "__SAVED__|"
        const val PREFIX_HISTORY_ITEM = "__HISTORY__|"
        const val PREFIX_PLAYLIST = "__PLAYLIST__|"
        const val PREFIX_PLAYLIST_EPISODE = "__PLAYLIST_EP__|"
    }

    private val scope = CoroutineScope(Dispatchers.Main)

    /**
     * Get the root menu items for Android Auto
     */
    fun getRootMenuItems(): List<MediaBrowserCompat.MediaItem> {
        return listOf(
            createBrowsableItem(
                mediaId = CURRENT_ID,
                title = "Current",
                subtitle = "Currently listening",
                iconUri = null
            ),
            createBrowsableItem(
                mediaId = QUEUE_ID,
                title = "Queue",
                subtitle = "Queued episodes",
                iconUri = null
            ),
            createBrowsableItem(
                mediaId = DOWNLOADS_ID,
                title = "Downloads",
                subtitle = "Downloaded episodes",
                iconUri = null
            ),
            createBrowsableItem(
                mediaId = MORE_ID,
                title = "More",
                subtitle = "More options",
                iconUri = null
            )
        )
    }

    /**
     * Get the "More" submenu items
     */
    fun getMoreMenuItems(): List<MediaBrowserCompat.MediaItem> {
        return listOf(
            createBrowsableItem(
                mediaId = SAVED_ID,
                title = "Saved",
                subtitle = "Saved episodes",
                iconUri = null
            ),
            createBrowsableItem(
                mediaId = HISTORY_ID,
                title = "History",
                subtitle = "Recently played",
                iconUri = null
            ),
            createBrowsableItem(
                mediaId = PODCASTS_ID,
                title = "Podcasts",
                subtitle = "Your subscriptions",
                iconUri = null
            ),
            createBrowsableItem(
                mediaId = PLAYLISTS_ID,
                title = "Playlists",
                subtitle = "Your playlists",
                iconUri = null
            )
        )
    }

    /**
     * Get children for a given parent ID
     */
    suspend fun getChildren(parentId: String): List<MediaBrowserCompat.MediaItem> {
        Log.d(TAG, "getChildren: $parentId")
        AudioPlayerPlugin.logToFlutter("INFO", TAG, "getChildren called for parentId=$parentId")

        val children = when {
            parentId == ROOT_ID -> {
                AudioPlayerPlugin.logToFlutter("INFO", TAG, "Returning root menu items")
                getRootMenuItems()
            }
            parentId == MORE_ID -> {
                AudioPlayerPlugin.logToFlutter("INFO", TAG, "Returning More submenu")
                getMoreMenuItems()
            }
            parentId == CURRENT_ID -> {
                AudioPlayerPlugin.logToFlutter("INFO", TAG, "Fetching current episodes from Flutter")
                getCurrent()
            }
            parentId == QUEUE_ID -> {
                AudioPlayerPlugin.logToFlutter("INFO", TAG, "Fetching queue from Flutter")
                getQueue()
            }
            parentId == DOWNLOADS_ID -> {
                AudioPlayerPlugin.logToFlutter("INFO", TAG, "Fetching downloads from Flutter")
                getDownloads()
            }
            parentId == SAVED_ID -> {
                AudioPlayerPlugin.logToFlutter("INFO", TAG, "Fetching saved episodes from Flutter")
                getSaved()
            }
            parentId == HISTORY_ID -> {
                AudioPlayerPlugin.logToFlutter("INFO", TAG, "Fetching history from Flutter")
                getHistory()
            }
            parentId == PODCASTS_ID -> {
                AudioPlayerPlugin.logToFlutter("INFO", TAG, "Fetching podcasts from Flutter")
                getPodcasts()
            }
            parentId == PLAYLISTS_ID -> {
                AudioPlayerPlugin.logToFlutter("INFO", TAG, "Fetching playlists from Flutter")
                getPlaylists()
            }
            parentId.startsWith(PREFIX_PODCAST) -> {
                AudioPlayerPlugin.logToFlutter("INFO", TAG, "Fetching episodes for podcast")
                getPodcastEpisodes(parentId)
            }
            parentId.startsWith(PREFIX_PLAYLIST) -> {
                AudioPlayerPlugin.logToFlutter("INFO", TAG, "Fetching episodes for playlist")
                getPlaylistEpisodes(parentId)
            }
            else -> {
                Log.w(TAG, "Unknown parent ID: $parentId")
                AudioPlayerPlugin.logToFlutter("WARN", TAG, "Unknown parent ID: $parentId")
                emptyList()
            }
        }

        AudioPlayerPlugin.logToFlutter("INFO", TAG, "getChildren returning ${children.size} items for parentId=$parentId")
        return children
    }

    /**
     * Get user's podcast subscriptions
     */
    private suspend fun getPodcasts(): List<MediaBrowserCompat.MediaItem> {
        return suspendCancellableCoroutine { continuation ->
            methodChannel.invokeMethod("getPodcasts", null, object : MethodChannel.Result {
                override fun success(result: Any?) {
                    val items = mutableListOf<MediaBrowserCompat.MediaItem>()

                    if (result is List<*>) {
                        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Flutter returned ${result.size} podcasts")
                        for (podcast in result) {
                            if (podcast is Map<*, *>) {
                                val id = podcast["id"] as? String ?: continue
                                val title = podcast["title"] as? String ?: "Unknown Podcast"
                                val imageUrl = podcast["imageUrl"] as? String
                                val episodeCount = podcast["episodeCount"] as? Int ?: 0

                                items.add(createBrowsableItem(
                                    mediaId = "$PREFIX_PODCAST$id",
                                    title = title,
                                    subtitle = "$episodeCount episodes",
                                    iconUri = imageUrl
                                ))
                            }
                        }
                        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Created ${items.size} podcast items")
                    } else {
                        AudioPlayerPlugin.logToFlutter("WARN", TAG, "Flutter returned non-list for podcasts: ${result?.javaClass?.simpleName}")
                    }

                    if (continuation.isActive) {
                        continuation.resume(items)
                    }
                }

                override fun error(errorCode: String, errorMessage: String?, errorDetails: Any?) {
                    Log.e(TAG, "Failed to get podcasts: $errorMessage")
                    AudioPlayerPlugin.logToFlutter("ERROR", TAG, "Failed to get podcasts: $errorMessage")
                    if (continuation.isActive) {
                        continuation.resume(emptyList())
                    }
                }

                override fun notImplemented() {
                    Log.w(TAG, "getPodcasts not implemented")
                    AudioPlayerPlugin.logToFlutter("WARN", TAG, "getPodcasts not implemented in Flutter")
                    if (continuation.isActive) {
                        continuation.resume(emptyList())
                    }
                }
            })
        }
    }

    /**
     * Get episodes for a specific podcast
     */
    private suspend fun getPodcastEpisodes(parentId: String): List<MediaBrowserCompat.MediaItem> {
        val podcastId = parentId.removePrefix(PREFIX_PODCAST)

        return suspendCancellableCoroutine { continuation ->
            methodChannel.invokeMethod("getPodcastEpisodes", mapOf("podcastId" to podcastId), object : MethodChannel.Result {
                override fun success(result: Any?) {
                    val items = mutableListOf<MediaBrowserCompat.MediaItem>()

                    if (result is List<*>) {
                        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Flutter returned ${result.size} episodes for podcast $podcastId")
                        for (episode in result) {
                            if (episode is Map<*, *>) {
                                items.add(createPlayableEpisodeItem(episode, PREFIX_EPISODE))
                            }
                        }
                        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Created ${items.size} episode items")
                    } else {
                        AudioPlayerPlugin.logToFlutter("WARN", TAG, "Flutter returned non-list for podcast episodes: ${result?.javaClass?.simpleName}")
                    }

                    if (continuation.isActive) {
                        continuation.resume(items)
                    }
                }

                override fun error(errorCode: String, errorMessage: String?, errorDetails: Any?) {
                    Log.e(TAG, "Failed to get podcast episodes: $errorMessage")
                    AudioPlayerPlugin.logToFlutter("ERROR", TAG, "Failed to get podcast episodes: $errorMessage")
                    if (continuation.isActive) {
                        continuation.resume(emptyList())
                    }
                }

                override fun notImplemented() {
                    AudioPlayerPlugin.logToFlutter("WARN", TAG, "getPodcastEpisodes not implemented in Flutter")
                    if (continuation.isActive) {
                        continuation.resume(emptyList())
                    }
                }
            })
        }
    }

    /**
     * Get downloaded episodes
     */
    private suspend fun getDownloads(): List<MediaBrowserCompat.MediaItem> {
        return suspendCancellableCoroutine { continuation ->
            methodChannel.invokeMethod("getDownloads", null, object : MethodChannel.Result {
                override fun success(result: Any?) {
                    val items = mutableListOf<MediaBrowserCompat.MediaItem>()

                    if (result is List<*>) {
                        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Flutter returned ${result.size} downloads")
                        for (episode in result) {
                            if (episode is Map<*, *>) {
                                items.add(createPlayableEpisodeItem(episode, PREFIX_DOWNLOAD))
                            }
                        }
                        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Created ${items.size} download items")
                    } else {
                        AudioPlayerPlugin.logToFlutter("WARN", TAG, "Flutter returned non-list for downloads: ${result?.javaClass?.simpleName}")
                    }

                    if (continuation.isActive) {
                        continuation.resume(items)
                    }
                }

                override fun error(errorCode: String, errorMessage: String?, errorDetails: Any?) {
                    Log.e(TAG, "Failed to get downloads: $errorMessage")
                    AudioPlayerPlugin.logToFlutter("ERROR", TAG, "Failed to get downloads: $errorMessage")
                    if (continuation.isActive) {
                        continuation.resume(emptyList())
                    }
                }

                override fun notImplemented() {
                    AudioPlayerPlugin.logToFlutter("WARN", TAG, "getDownloads not implemented in Flutter")
                    if (continuation.isActive) {
                        continuation.resume(emptyList())
                    }
                }
            })
        }
    }

    /**
     * Get queue
     */
    private suspend fun getQueue(): List<MediaBrowserCompat.MediaItem> {
        return suspendCancellableCoroutine { continuation ->
            methodChannel.invokeMethod("getQueue", null, object : MethodChannel.Result {
                override fun success(result: Any?) {
                    val items = mutableListOf<MediaBrowserCompat.MediaItem>()

                    if (result is List<*>) {
                        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Flutter returned ${result.size} queue items")
                        result.forEachIndexed { index, episode ->
                            if (episode is Map<*, *>) {
                                items.add(createPlayableEpisodeItem(episode, "$PREFIX_QUEUE_ITEM$index|"))
                            }
                        }
                        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Created ${items.size} queue items")
                    } else {
                        AudioPlayerPlugin.logToFlutter("WARN", TAG, "Flutter returned non-list for queue: ${result?.javaClass?.simpleName}")
                    }

                    if (continuation.isActive) {
                        continuation.resume(items)
                    }
                }

                override fun error(errorCode: String, errorMessage: String?, errorDetails: Any?) {
                    Log.e(TAG, "Failed to get queue: $errorMessage")
                    AudioPlayerPlugin.logToFlutter("ERROR", TAG, "Failed to get queue: $errorMessage")
                    if (continuation.isActive) {
                        continuation.resume(emptyList())
                    }
                }

                override fun notImplemented() {
                    AudioPlayerPlugin.logToFlutter("WARN", TAG, "getQueue not implemented in Flutter")
                    if (continuation.isActive) {
                        continuation.resume(emptyList())
                    }
                }
            })
        }
    }

    /**
     * Get home episodes (all episodes)
     */
    private suspend fun getCurrent(): List<MediaBrowserCompat.MediaItem> {
        return suspendCancellableCoroutine { continuation ->
            methodChannel.invokeMethod("getCurrent", null, object : MethodChannel.Result {
                override fun success(result: Any?) {
                    val items = mutableListOf<MediaBrowserCompat.MediaItem>()

                    if (result is List<*>) {
                        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Flutter returned ${result.size} current items")
                        for (episode in result) {
                            if (episode is Map<*, *>) {
                                items.add(createPlayableEpisodeItem(episode, PREFIX_CURRENT_ITEM))
                            }
                        }
                        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Created ${items.size} current items")
                    } else {
                        AudioPlayerPlugin.logToFlutter("WARN", TAG, "Flutter returned non-list for current: ${result?.javaClass?.simpleName}")
                    }

                    if (continuation.isActive) {
                        continuation.resume(items)
                    }
                }

                override fun error(errorCode: String, errorMessage: String?, errorDetails: Any?) {
                    Log.e(TAG, "Failed to get current: $errorMessage")
                    AudioPlayerPlugin.logToFlutter("ERROR", TAG, "Failed to get current: $errorMessage")
                    if (continuation.isActive) {
                        continuation.resume(emptyList())
                    }
                }

                override fun notImplemented() {
                    AudioPlayerPlugin.logToFlutter("WARN", TAG, "getCurrent not implemented in Flutter")
                    if (continuation.isActive) {
                        continuation.resume(emptyList())
                    }
                }
            })
        }
    }

    /**
     * Get saved episodes (bookmarked/favorited)
     */
    private suspend fun getSaved(): List<MediaBrowserCompat.MediaItem> {
        return suspendCancellableCoroutine { continuation ->
            methodChannel.invokeMethod("getSaved", null, object : MethodChannel.Result {
                override fun success(result: Any?) {
                    val items = mutableListOf<MediaBrowserCompat.MediaItem>()

                    if (result is List<*>) {
                        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Flutter returned ${result.size} saved items")
                        for (episode in result) {
                            if (episode is Map<*, *>) {
                                items.add(createPlayableEpisodeItem(episode, PREFIX_SAVED_ITEM))
                            }
                        }
                        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Created ${items.size} saved items")
                    } else {
                        AudioPlayerPlugin.logToFlutter("WARN", TAG, "Flutter returned non-list for saved: ${result?.javaClass?.simpleName}")
                    }

                    if (continuation.isActive) {
                        continuation.resume(items)
                    }
                }

                override fun error(errorCode: String, errorMessage: String?, errorDetails: Any?) {
                    Log.e(TAG, "Failed to get saved: $errorMessage")
                    AudioPlayerPlugin.logToFlutter("ERROR", TAG, "Failed to get saved: $errorMessage")
                    if (continuation.isActive) {
                        continuation.resume(emptyList())
                    }
                }

                override fun notImplemented() {
                    AudioPlayerPlugin.logToFlutter("WARN", TAG, "getSaved not implemented in Flutter")
                    if (continuation.isActive) {
                        continuation.resume(emptyList())
                    }
                }
            })
        }
    }

    /**
     * Get user's history
     */
    private suspend fun getHistory(): List<MediaBrowserCompat.MediaItem> {
        return suspendCancellableCoroutine { continuation ->
            methodChannel.invokeMethod("getHistory", null, object : MethodChannel.Result {
                override fun success(result: Any?) {
                    val items = mutableListOf<MediaBrowserCompat.MediaItem>()

                    if (result is List<*>) {
                        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Flutter returned ${result.size} history items")
                        for (episode in result) {
                            if (episode is Map<*, *>) {
                                items.add(createPlayableEpisodeItem(episode, PREFIX_HISTORY_ITEM))
                            }
                        }
                        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Created ${items.size} history items")
                    } else {
                        AudioPlayerPlugin.logToFlutter("WARN", TAG, "Flutter returned non-list for history: ${result?.javaClass?.simpleName}")
                    }

                    if (continuation.isActive) {
                        continuation.resume(items)
                    }
                }

                override fun error(errorCode: String, errorMessage: String?, errorDetails: Any?) {
                    Log.e(TAG, "Failed to get history: $errorMessage")
                    AudioPlayerPlugin.logToFlutter("ERROR", TAG, "Failed to get history: $errorMessage")
                    if (continuation.isActive) {
                        continuation.resume(emptyList())
                    }
                }

                override fun notImplemented() {
                    Log.w(TAG, "getHistory not implemented")
                    AudioPlayerPlugin.logToFlutter("WARN", TAG, "getHistory not implemented in Flutter")
                    if (continuation.isActive) {
                        continuation.resume(emptyList())
                    }
                }
            })
        }
    }

    /**
     * Get user's playlists
     */
    private suspend fun getPlaylists(): List<MediaBrowserCompat.MediaItem> {
        return suspendCancellableCoroutine { continuation ->
            methodChannel.invokeMethod("getPlaylists", null, object : MethodChannel.Result {
                override fun success(result: Any?) {
                    val items = mutableListOf<MediaBrowserCompat.MediaItem>()

                    if (result is List<*>) {
                        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Flutter returned ${result.size} playlists")
                        for (playlist in result) {
                            if (playlist is Map<*, *>) {
                                val id = playlist["id"] as? Int ?: continue
                                val name = playlist["name"] as? String ?: "Unknown Playlist"
                                val episodeCount = playlist["episodeCount"] as? Int ?: 0

                                items.add(createBrowsableItem(
                                    mediaId = "$PREFIX_PLAYLIST$id",
                                    title = name,
                                    subtitle = "$episodeCount episodes",
                                    iconUri = null
                                ))
                            }
                        }
                        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Created ${items.size} playlist items")
                    } else {
                        AudioPlayerPlugin.logToFlutter("WARN", TAG, "Flutter returned non-list for playlists: ${result?.javaClass?.simpleName}")
                    }

                    if (continuation.isActive) {
                        continuation.resume(items)
                    }
                }

                override fun error(errorCode: String, errorMessage: String?, errorDetails: Any?) {
                    Log.e(TAG, "Failed to get playlists: $errorMessage")
                    AudioPlayerPlugin.logToFlutter("ERROR", TAG, "Failed to get playlists: $errorMessage")
                    if (continuation.isActive) {
                        continuation.resume(emptyList())
                    }
                }

                override fun notImplemented() {
                    Log.w(TAG, "getPlaylists not implemented")
                    AudioPlayerPlugin.logToFlutter("WARN", TAG, "getPlaylists not implemented in Flutter")
                    if (continuation.isActive) {
                        continuation.resume(emptyList())
                    }
                }
            })
        }
    }

    /**
     * Get episodes for a specific playlist
     */
    private suspend fun getPlaylistEpisodes(parentId: String): List<MediaBrowserCompat.MediaItem> {
        val playlistId = parentId.removePrefix(PREFIX_PLAYLIST)

        return suspendCancellableCoroutine { continuation ->
            methodChannel.invokeMethod("getPlaylistEpisodes", mapOf("playlistId" to playlistId), object : MethodChannel.Result {
                override fun success(result: Any?) {
                    val items = mutableListOf<MediaBrowserCompat.MediaItem>()

                    if (result is List<*>) {
                        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Flutter returned ${result.size} episodes for playlist $playlistId")
                        for (episode in result) {
                            if (episode is Map<*, *>) {
                                items.add(createPlayableEpisodeItem(episode, PREFIX_PLAYLIST_EPISODE))
                            }
                        }
                        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Created ${items.size} playlist episode items")
                    } else {
                        AudioPlayerPlugin.logToFlutter("WARN", TAG, "Flutter returned non-list for playlist episodes: ${result?.javaClass?.simpleName}")
                    }

                    if (continuation.isActive) {
                        continuation.resume(items)
                    }
                }

                override fun error(errorCode: String, errorMessage: String?, errorDetails: Any?) {
                    Log.e(TAG, "Failed to get playlist episodes: $errorMessage")
                    AudioPlayerPlugin.logToFlutter("ERROR", TAG, "Failed to get playlist episodes: $errorMessage")
                    if (continuation.isActive) {
                        continuation.resume(emptyList())
                    }
                }

                override fun notImplemented() {
                    AudioPlayerPlugin.logToFlutter("WARN", TAG, "getPlaylistEpisodes not implemented in Flutter")
                    if (continuation.isActive) {
                        continuation.resume(emptyList())
                    }
                }
            })
        }
    }

    /**
     * Create a browsable media item (folder)
     */
    private fun createBrowsableItem(
        mediaId: String,
        title: String,
        subtitle: String?,
        iconUri: String?
    ): MediaBrowserCompat.MediaItem {
        val description = MediaDescriptionCompat.Builder()
            .setMediaId(mediaId)
            .setTitle(title)
            .setSubtitle(subtitle)
            .apply {
                if (iconUri != null) {
                    setIconUri(Uri.parse(iconUri))
                }
            }
            .build()

        return MediaBrowserCompat.MediaItem(description, MediaBrowserCompat.MediaItem.FLAG_BROWSABLE)
    }

    /**
     * Create a playable episode media item
     */
    private fun createPlayableEpisodeItem(
        episode: Map<*, *>,
        idPrefix: String
    ): MediaBrowserCompat.MediaItem {
        val guid = episode["guid"] as? String ?: ""
        val title = episode["title"] as? String ?: "Unknown Episode"
        val podcast = episode["podcast"] as? String ?: "Unknown Podcast"
        val imageUrl = episode["imageUrl"] as? String
        val duration = episode["duration"] as? Int ?: 0
        val position = episode["position"] as? Int ?: 0
        val pubDate = episode["pubDate"] as? String

        // Format duration and progress
        val durationStr = if (duration > 0) {
            val minutes = duration / 60
            val seconds = duration % 60
            if (position > 0) {
                val positionSec = position / 1000
                val posMinutes = positionSec / 60
                val posSeconds = positionSec % 60
                val percent = ((position.toDouble() / (duration * 1000)) * 100).toInt()
                String.format("%d:%02d / %d:%02d (%d%%)", posMinutes, posSeconds, minutes, seconds, percent)
            } else {
                String.format("%d:%02d", minutes, seconds)
            }
        } else {
            ""
        }

        // Format publication date (e.g., "2024-01-21T20:00:00" -> "Jan 21")
        val dateStr = if (!pubDate.isNullOrEmpty()) {
            try {
                // Remove time part if present (split on 'T' or space)
                val datePart = pubDate.split("T")[0].split(" ")[0]
                val parts = datePart.split("-")
                if (parts.size >= 3) {
                    val month = when (parts[1].toIntOrNull()) {
                        1 -> "Jan"; 2 -> "Feb"; 3 -> "Mar"; 4 -> "Apr"
                        5 -> "May"; 6 -> "Jun"; 7 -> "Jul"; 8 -> "Aug"
                        9 -> "Sep"; 10 -> "Oct"; 11 -> "Nov"; 12 -> "Dec"
                        else -> ""
                    }
                    val day = parts[2].toIntOrNull()?.toString() ?: parts[2]
                    "$month $day"
                } else {
                    ""
                }
            } catch (e: Exception) {
                ""
            }
        } else {
            ""
        }

        // Build compact subtitle: "Podcast • Duration • Date"
        val subtitleParts = mutableListOf<String>()
        subtitleParts.add(podcast)
        if (durationStr.isNotEmpty()) subtitleParts.add(durationStr)
        if (dateStr.isNotEmpty()) subtitleParts.add(dateStr)
        val subtitle = subtitleParts.joinToString(" • ")

        val description = MediaDescriptionCompat.Builder()
            .setMediaId("$idPrefix$guid")
            .setTitle(title)
            .setSubtitle(subtitle)
            .apply {
                if (imageUrl != null) {
                    setIconUri(Uri.parse(imageUrl))
                }
            }
            .build()

        return MediaBrowserCompat.MediaItem(description, MediaBrowserCompat.MediaItem.FLAG_PLAYABLE)
    }

    /**
     * Play an episode from a media ID
     */
    fun playFromMediaId(mediaId: String) {
        scope.launch {
            Log.d(TAG, "playFromMediaId: $mediaId")
            AudioPlayerPlugin.logToFlutter("INFO", TAG, "playFromMediaId: $mediaId")

            // Extract the episode GUID from the media ID
            val guid = when {
                mediaId.startsWith(PREFIX_CURRENT_ITEM) -> mediaId.removePrefix(PREFIX_CURRENT_ITEM)
                mediaId.startsWith(PREFIX_SAVED_ITEM) -> mediaId.removePrefix(PREFIX_SAVED_ITEM)
                mediaId.startsWith(PREFIX_HISTORY_ITEM) -> mediaId.removePrefix(PREFIX_HISTORY_ITEM)
                mediaId.startsWith(PREFIX_EPISODE) -> mediaId.removePrefix(PREFIX_EPISODE)
                mediaId.startsWith(PREFIX_DOWNLOAD) -> mediaId.removePrefix(PREFIX_DOWNLOAD)
                mediaId.startsWith(PREFIX_PLAYLIST_EPISODE) -> mediaId.removePrefix(PREFIX_PLAYLIST_EPISODE)
                mediaId.startsWith(PREFIX_QUEUE_ITEM) -> {
                    // Queue items have format: __QUEUE__|<index>|<guid>
                    val parts = mediaId.split("|")
                    if (parts.size >= 3) parts[2] else {
                        AudioPlayerPlugin.logToFlutter("ERROR", TAG, "Invalid queue media ID format: $mediaId")
                        return@launch
                    }
                }
                else -> {
                    Log.w(TAG, "Cannot play non-episode media ID: $mediaId")
                    AudioPlayerPlugin.logToFlutter("WARN", TAG, "Cannot play non-episode media ID: $mediaId")
                    return@launch
                }
            }

            AudioPlayerPlugin.logToFlutter("INFO", TAG, "Telling Flutter to play episode with guid: $guid")
            methodChannel.invokeMethod("playFromMediaId", mapOf("guid" to guid))
        }
    }

    /**
     * Search for episodes/podcasts
     */
    suspend fun search(query: String): List<MediaBrowserCompat.MediaItem> {
        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Searching for: $query")

        return suspendCancellableCoroutine { continuation ->
            methodChannel.invokeMethod("search", mapOf("query" to query), object : MethodChannel.Result {
                override fun success(result: Any?) {
                    val items = mutableListOf<MediaBrowserCompat.MediaItem>()

                    if (result is Map<*, *>) {
                        val episodes = result["episodes"] as? List<*>
                        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Flutter returned ${episodes?.size ?: 0} search results for '$query'")
                        episodes?.forEach { episode ->
                            if (episode is Map<*, *>) {
                                items.add(createPlayableEpisodeItem(episode, PREFIX_EPISODE))
                            }
                        }
                        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Created ${items.size} search result items")
                    } else {
                        AudioPlayerPlugin.logToFlutter("WARN", TAG, "Flutter returned non-map for search: ${result?.javaClass?.simpleName}")
                    }

                    if (continuation.isActive) {
                        continuation.resume(items)
                    }
                }

                override fun error(errorCode: String, errorMessage: String?, errorDetails: Any?) {
                    Log.e(TAG, "Search failed: $errorMessage")
                    AudioPlayerPlugin.logToFlutter("ERROR", TAG, "Search failed: $errorMessage")
                    if (continuation.isActive) {
                        continuation.resume(emptyList())
                    }
                }

                override fun notImplemented() {
                    AudioPlayerPlugin.logToFlutter("WARN", TAG, "search not implemented in Flutter")
                    if (continuation.isActive) {
                        continuation.resume(emptyList())
                    }
                }
            })
        }
    }
}
