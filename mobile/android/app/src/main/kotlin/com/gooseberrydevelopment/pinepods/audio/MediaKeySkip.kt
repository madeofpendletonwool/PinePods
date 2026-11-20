package com.gooseberrydevelopment.pinepods.audio

import android.view.KeyEvent

/**
 * Hardware media keys that should seek by the user's skip interval.
 *
 * Headsets, watches and Bluetooth car stereos (AVRCP passthrough arrives as media
 * keys) send NEXT / PREVIOUS for their skip buttons. The session hides
 * episode-level next/previous so Android Auto and the notification show rewind and
 * fast-forward instead, which makes Media3 drop those keys. For podcasts they skip
 * by the configured interval, as they already do on iOS.
 */
internal object MediaKeySkip {
    enum class Direction { FORWARD, BACK }

    fun direction(keyCode: Int): Direction? = when (keyCode) {
        KeyEvent.KEYCODE_MEDIA_NEXT, KeyEvent.KEYCODE_MEDIA_SKIP_FORWARD -> Direction.FORWARD
        KeyEvent.KEYCODE_MEDIA_PREVIOUS, KeyEvent.KEYCODE_MEDIA_SKIP_BACKWARD -> Direction.BACK
        else -> null
    }
}
