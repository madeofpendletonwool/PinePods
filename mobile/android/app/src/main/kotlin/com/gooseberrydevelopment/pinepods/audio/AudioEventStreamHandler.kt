package com.gooseberrydevelopment.pinepods.audio

import io.flutter.plugin.common.EventChannel
import android.os.Handler
import android.os.Looper

class AudioEventStreamHandler : EventChannel.StreamHandler {
    private var eventSink: EventChannel.EventSink? = null
    private val handler = Handler(Looper.getMainLooper())

    override fun onListen(arguments: Any?, events: EventChannel.EventSink?) {
        eventSink = events
    }

    override fun onCancel(arguments: Any?) {
        eventSink = null
    }

    fun sendEvent(event: Map<String, Any>) {
        handler.post {
            eventSink?.success(event)
        }
    }

    fun sendError(code: String, message: String, details: Any?) {
        handler.post {
            eventSink?.error(code, message, details)
        }
    }
}
