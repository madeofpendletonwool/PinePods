package com.gooseberrydevelopment.pinepods.audio

import android.os.Bundle
import android.util.Log
import androidx.media3.session.MediaSession
import androidx.media3.session.SessionCommand
import androidx.media3.session.SessionResult
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture

class PinepodsMediaSessionCallback(
    private val service: PinepodsMediaService,
    private var eventStreamHandler: AudioEventStreamHandler?
) : MediaSession.Callback {

    fun updateEventStreamHandler(handler: AudioEventStreamHandler) {
        this.eventStreamHandler = handler
    }

    override fun onCustomCommand(
        session: MediaSession,
        controller: MediaSession.ControllerInfo,
        customCommand: SessionCommand,
        args: Bundle
    ): ListenableFuture<SessionResult> {
        Log.d(TAG, "onCustomCommand: ${customCommand.customAction}")

        when (customCommand.customAction) {
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
        private const val TAG = "MediaSessionCallback"
    }
}
