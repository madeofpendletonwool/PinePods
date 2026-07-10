package com.gooseberrydevelopment.pinepods.audio

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.os.IBinder
import android.util.Log
import io.flutter.embedding.engine.plugins.FlutterPlugin
import io.flutter.embedding.engine.plugins.activity.ActivityAware
import io.flutter.embedding.engine.plugins.activity.ActivityPluginBinding
import io.flutter.plugin.common.EventChannel
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import io.flutter.plugin.common.MethodChannel.MethodCallHandler
import io.flutter.plugin.common.MethodChannel.Result

class AudioPlayerPlugin : FlutterPlugin, MethodCallHandler, ActivityAware {
    private lateinit var methodChannel: MethodChannel
    private lateinit var eventChannel: EventChannel
    private lateinit var nativeLogChannel: MethodChannel
    private lateinit var eventStreamHandler: AudioEventStreamHandler
    private var context: Context? = null
    private var mediaService: PinepodsMediaService? = null
    private var serviceBound = false

    companion object {
        private const val TAG = "AudioPlayerPlugin"
        private const val METHOD_CHANNEL = "com.pinepods/audio_player"
        private const val EVENT_CHANNEL = "com.pinepods/audio_events"
        private const val NATIVE_LOG_CHANNEL = "com.pinepods/native_logs"

        // Singleton reference for logging from anywhere
        private var logChannelInstance: MethodChannel? = null

        fun logToFlutter(level: String, tag: String, message: String) {
            logChannelInstance?.invokeMethod("log", mapOf(
                "level" to level,
                "tag" to tag,
                "message" to message
            ))
        }
    }

    private val serviceConnection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName?, service: IBinder?) {
            val binder = service as PinepodsMediaService.LocalBinder
            mediaService = binder.getService()
            mediaService?.setEventStreamHandler(eventStreamHandler)

            // Initialize media library session for Android Auto / CarPlay browsing
            mediaService?.initializeMediaLibrarySession(methodChannel)

            serviceBound = true
            Log.d(TAG, "Media service connected and media library session initialized")
        }

        override fun onServiceDisconnected(name: ComponentName?) {
            mediaService = null
            serviceBound = false
            Log.d(TAG, "Media service disconnected")
        }
    }

    override fun onAttachedToEngine(binding: FlutterPlugin.FlutterPluginBinding) {
        context = binding.applicationContext

        methodChannel = MethodChannel(binding.binaryMessenger, METHOD_CHANNEL)
        methodChannel.setMethodCallHandler(this)

        eventStreamHandler = AudioEventStreamHandler()
        eventChannel = EventChannel(binding.binaryMessenger, EVENT_CHANNEL)
        eventChannel.setStreamHandler(eventStreamHandler)

        // Set up native log channel for forwarding Android logs to Flutter
        nativeLogChannel = MethodChannel(binding.binaryMessenger, NATIVE_LOG_CHANNEL)
        logChannelInstance = nativeLogChannel

        Log.d(TAG, "AudioPlayerPlugin attached to engine")
        logToFlutter("INFO", TAG, "AudioPlayerPlugin attached to engine - native logging active")
    }

    override fun onDetachedFromEngine(binding: FlutterPlugin.FlutterPluginBinding) {
        methodChannel.setMethodCallHandler(null)
        eventChannel.setStreamHandler(null)
        context = null
        Log.d(TAG, "AudioPlayerPlugin detached from engine")
    }

    override fun onAttachedToActivity(binding: ActivityPluginBinding) {
        // Bind to the media service
        val intent = Intent(context, PinepodsMediaService::class.java)
        context?.bindService(intent, serviceConnection, Context.BIND_AUTO_CREATE)
        Log.d(TAG, "Binding to media service")
    }

    override fun onDetachedFromActivityForConfigChanges() {
        // Don't unbind during config changes
    }

    override fun onReattachedToActivityForConfigChanges(binding: ActivityPluginBinding) {
        // Nothing needed
    }

    override fun onDetachedFromActivity() {
        if (serviceBound) {
            context?.unbindService(serviceConnection)
            serviceBound = false
            Log.d(TAG, "Unbound from media service")
        }
    }

    override fun onMethodCall(call: MethodCall, result: Result) {
        if (mediaService == null) {
            result.error("SERVICE_NOT_READY", "Media service is not ready", null)
            return
        }

        try {
            when (call.method) {
                "playEpisode" -> {
                    val url = call.argument<String>("url")
                    val startPosition = call.argument<Int>("startPosition") ?: 0
                    val isLocal = call.argument<Boolean>("isLocal") ?: false
                    val metadata = call.argument<Map<String, Any>>("metadata")

                    if (url != null) {
                        mediaService?.playEpisode(url, startPosition, isLocal, metadata)
                        result.success(null)
                    } else {
                        result.error("INVALID_ARGS", "URL is required", null)
                    }
                }

                "play" -> {
                    mediaService?.play()
                    result.success(null)
                }

                "pause" -> {
                    mediaService?.pause()
                    result.success(null)
                }

                "stop" -> {
                    mediaService?.stop()
                    result.success(null)
                }

                "seek" -> {
                    val position = call.argument<Int>("position")
                    if (position != null) {
                        mediaService?.seek(position)
                        result.success(null)
                    } else {
                        result.error("INVALID_ARGS", "Position is required", null)
                    }
                }

                "fastForward" -> {
                    val milliseconds = call.argument<Int>("milliseconds") ?: 30000
                    mediaService?.fastForward(milliseconds)
                    result.success(null)
                }

                "rewind" -> {
                    val milliseconds = call.argument<Int>("milliseconds") ?: 10000
                    mediaService?.rewind(milliseconds)
                    result.success(null)
                }

                "setPlaybackSpeed" -> {
                    val speed = call.argument<Double>("speed")
                    if (speed != null) {
                        mediaService?.setPlaybackSpeed(speed.toFloat())
                        result.success(null)
                    } else {
                        result.error("INVALID_ARGS", "Speed is required", null)
                    }
                }

                "setTrimSilence" -> {
                    val enabled = call.argument<Boolean>("enabled") ?: false
                    mediaService?.setTrimSilence(enabled)
                    result.success(null)
                }

                "setAdSkipSegments" -> {
                    val raw = call.argument<List<Map<String, Double>>>("segments") ?: emptyList()
                    val segments = raw.mapNotNull { s ->
                        val start = s["start"]
                        val end = s["end"]
                        if (start != null && end != null) Pair(start, end) else null
                    }
                    mediaService?.setAdSkipSegments(segments)
                    result.success(null)
                }

                "setVolumeBoost" -> {
                    val enabled = call.argument<Boolean>("enabled") ?: false
                    mediaService?.setVolumeBoost(enabled)
                    result.success(null)
                }

                "setSkipIntervals" -> {
                    val forwardMs = (call.argument<Int>("forwardMs") ?: 30000).toLong()
                    val backwardMs = (call.argument<Int>("backwardMs") ?: 10000).toLong()
                    mediaService?.setSkipIntervals(forwardMs, backwardMs)
                    result.success(null)
                }

                "getPosition" -> {
                    val position = mediaService?.getCurrentPosition() ?: 0
                    result.success(position)
                }

                "getDuration" -> {
                    val duration = mediaService?.getDuration() ?: 0
                    result.success(duration)
                }

                // Android Auto / CarPlay browsing methods
                // These will be handled by delegating to Flutter's repository
                "getSubscriptions",
                "getPodcastEpisodes",
                "getDownloads",
                "getQueue",
                "getRecent",
                "playFromMediaId",
                "search" -> {
                    // These methods are handled by MediaBrowserHelper which will call
                    // back to Flutter via method channel - just acknowledge receipt
                    result.success(null)
                }

                else -> {
                    result.notImplemented()
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Error handling method call: ${call.method}", e)
            result.error("EXCEPTION", e.message, e.toString())
        }
    }
}
