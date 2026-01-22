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
        const val FEED_ID = "__FEED__"
        const val SAVED_ID = "__SAVED__"
        const val DOWNLOADS_ID = "__DOWNLOADS__"
        const val SUBSCRIPTIONS_ID = "__SUBSCRIPTIONS__"  // Keep for podcast browsing
        const val QUEUE_ID = "__QUEUE__"  // Hidden from root but available
        const val RECENT_ID = "__RECENT__"  // Hidden from root but available

        // Media ID prefixes
        const val PREFIX_PODCAST = "__PODCAST__|"
        const val PREFIX_EPISODE = "__EPISODE__|"
        const val PREFIX_CURRENT_ITEM = "__CURRENT__|"
        const val PREFIX_FEED_ITEM = "__FEED__|"
        const val PREFIX_SAVED_ITEM = "__SAVED__|"
        const val PREFIX_DOWNLOAD = "__DOWNLOAD__|"
        const val PREFIX_QUEUE_ITEM = "__QUEUE__|"
        const val PREFIX_RECENT_ITEM = "__RECENT__|"
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
                mediaId = FEED_ID,
                title = "Feed",
                subtitle = "Latest episodes",
                iconUri = null
            ),
            createBrowsableItem(
                mediaId = SAVED_ID,
                title = "Saved",
                subtitle = "Your saved episodes",
                iconUri = null
            ),
            createBrowsableItem(
                mediaId = DOWNLOADS_ID,
                title = "Downloads",
                subtitle = "Downloaded episodes",
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
            parentId == CURRENT_ID -> {
                AudioPlayerPlugin.logToFlutter("INFO", TAG, "Fetching current episodes from Flutter")
                getCurrent()
            }
            parentId == FEED_ID -> {
                AudioPlayerPlugin.logToFlutter("INFO", TAG, "Fetching feed episodes from Flutter")
                getFeed()
            }
            parentId == SAVED_ID -> {
                AudioPlayerPlugin.logToFlutter("INFO", TAG, "Fetching saved episodes from Flutter")
                getSaved()
            }
            parentId == DOWNLOADS_ID -> {
                AudioPlayerPlugin.logToFlutter("INFO", TAG, "Fetching downloads from Flutter")
                getDownloads()
            }
            parentId == SUBSCRIPTIONS_ID -> {
                AudioPlayerPlugin.logToFlutter("INFO", TAG, "Fetching subscriptions from Flutter")
                getSubscriptions()
            }
            parentId == QUEUE_ID -> {
                AudioPlayerPlugin.logToFlutter("INFO", TAG, "Fetching queue from Flutter")
                getQueue()
            }
            parentId == RECENT_ID -> {
                AudioPlayerPlugin.logToFlutter("INFO", TAG, "Fetching recent from Flutter")
                getRecent()
            }
            parentId.startsWith(PREFIX_PODCAST) -> {
                val podcastId = parentId.removePrefix(PREFIX_PODCAST)
                AudioPlayerPlugin.logToFlutter("INFO", TAG, "Fetching episodes for podcast: $podcastId")
                getPodcastEpisodes(parentId)
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
    private suspend fun getSubscriptions(): List<MediaBrowserCompat.MediaItem> {
        return suspendCancellableCoroutine { continuation ->
            methodChannel.invokeMethod("getSubscriptions", null, object : MethodChannel.Result {
                override fun success(result: Any?) {
                    val items = mutableListOf<MediaBrowserCompat.MediaItem>()

                    if (result is List<*>) {
                        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Flutter returned ${result.size} subscriptions")
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
                        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Created ${items.size} subscription items")
                    } else {
                        AudioPlayerPlugin.logToFlutter("WARN", TAG, "Flutter returned non-list for subscriptions: ${result?.javaClass?.simpleName}")
                    }

                    if (continuation.isActive) {
                        continuation.resume(items)
                    }
                }

                override fun error(errorCode: String, errorMessage: String?, errorDetails: Any?) {
                    Log.e(TAG, "Failed to get subscriptions: $errorMessage")
                    AudioPlayerPlugin.logToFlutter("ERROR", TAG, "Failed to get subscriptions: $errorMessage")
                    if (continuation.isActive) {
                        continuation.resume(emptyList())
                    }
                }

                override fun notImplemented() {
                    Log.w(TAG, "getSubscriptions not implemented")
                    AudioPlayerPlugin.logToFlutter("WARN", TAG, "getSubscriptions not implemented in Flutter")
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
     * Get recently played episodes
     */
    private suspend fun getRecent(): List<MediaBrowserCompat.MediaItem> {
        return suspendCancellableCoroutine { continuation ->
            methodChannel.invokeMethod("getRecent", null, object : MethodChannel.Result {
                override fun success(result: Any?) {
                    val items = mutableListOf<MediaBrowserCompat.MediaItem>()

                    if (result is List<*>) {
                        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Flutter returned ${result.size} recent items")
                        for (episode in result) {
                            if (episode is Map<*, *>) {
                                items.add(createPlayableEpisodeItem(episode, PREFIX_RECENT_ITEM))
                            }
                        }
                        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Created ${items.size} recent items")
                    } else {
                        AudioPlayerPlugin.logToFlutter("WARN", TAG, "Flutter returned non-list for recent: ${result?.javaClass?.simpleName}")
                    }

                    if (continuation.isActive) {
                        continuation.resume(items)
                    }
                }

                override fun error(errorCode: String, errorMessage: String?, errorDetails: Any?) {
                    Log.e(TAG, "Failed to get recent: $errorMessage")
                    AudioPlayerPlugin.logToFlutter("ERROR", TAG, "Failed to get recent: $errorMessage")
                    if (continuation.isActive) {
                        continuation.resume(emptyList())
                    }
                }

                override fun notImplemented() {
                    AudioPlayerPlugin.logToFlutter("WARN", TAG, "getRecent not implemented in Flutter")
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
     * Get feed episodes (latest episodes)
     */
    private suspend fun getFeed(): List<MediaBrowserCompat.MediaItem> {
        return suspendCancellableCoroutine { continuation ->
            methodChannel.invokeMethod("getFeed", null, object : MethodChannel.Result {
                override fun success(result: Any?) {
                    val items = mutableListOf<MediaBrowserCompat.MediaItem>()

                    if (result is List<*>) {
                        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Flutter returned ${result.size} feed items")
                        for (episode in result) {
                            if (episode is Map<*, *>) {
                                items.add(createPlayableEpisodeItem(episode, PREFIX_FEED_ITEM))
                            }
                        }
                        AudioPlayerPlugin.logToFlutter("INFO", TAG, "Created ${items.size} feed items")
                    } else {
                        AudioPlayerPlugin.logToFlutter("WARN", TAG, "Flutter returned non-list for feed: ${result?.javaClass?.simpleName}")
                    }

                    if (continuation.isActive) {
                        continuation.resume(items)
                    }
                }

                override fun error(errorCode: String, errorMessage: String?, errorDetails: Any?) {
                    Log.e(TAG, "Failed to get feed: $errorMessage")
                    AudioPlayerPlugin.logToFlutter("ERROR", TAG, "Failed to get feed: $errorMessage")
                    if (continuation.isActive) {
                        continuation.resume(emptyList())
                    }
                }

                override fun notImplemented() {
                    AudioPlayerPlugin.logToFlutter("WARN", TAG, "getFeed not implemented in Flutter")
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
                mediaId.startsWith(PREFIX_FEED_ITEM) -> mediaId.removePrefix(PREFIX_FEED_ITEM)
                mediaId.startsWith(PREFIX_SAVED_ITEM) -> mediaId.removePrefix(PREFIX_SAVED_ITEM)
                mediaId.startsWith(PREFIX_EPISODE) -> mediaId.removePrefix(PREFIX_EPISODE)
                mediaId.startsWith(PREFIX_DOWNLOAD) -> mediaId.removePrefix(PREFIX_DOWNLOAD)
                mediaId.startsWith(PREFIX_RECENT_ITEM) -> mediaId.removePrefix(PREFIX_RECENT_ITEM)
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
