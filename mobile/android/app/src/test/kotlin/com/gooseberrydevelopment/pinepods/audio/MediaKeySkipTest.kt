package com.gooseberrydevelopment.pinepods.audio

import android.view.KeyEvent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class MediaKeySkipTest {
    @Test
    fun trackKeysSkipByInterval() {
        assertEquals(MediaKeySkip.Direction.FORWARD, MediaKeySkip.direction(KeyEvent.KEYCODE_MEDIA_NEXT))
        assertEquals(MediaKeySkip.Direction.FORWARD, MediaKeySkip.direction(KeyEvent.KEYCODE_MEDIA_SKIP_FORWARD))
        assertEquals(MediaKeySkip.Direction.BACK, MediaKeySkip.direction(KeyEvent.KEYCODE_MEDIA_PREVIOUS))
        assertEquals(MediaKeySkip.Direction.BACK, MediaKeySkip.direction(KeyEvent.KEYCODE_MEDIA_SKIP_BACKWARD))
    }

    @Test
    fun otherKeysAreLeftToMedia3() {
        // Play/pause and the seek keys Media3 already maps to the skip interval.
        for (key in listOf(
            KeyEvent.KEYCODE_MEDIA_PLAY_PAUSE,
            KeyEvent.KEYCODE_HEADSETHOOK,
            KeyEvent.KEYCODE_MEDIA_PLAY,
            KeyEvent.KEYCODE_MEDIA_PAUSE,
            KeyEvent.KEYCODE_MEDIA_STOP,
            KeyEvent.KEYCODE_MEDIA_FAST_FORWARD,
            KeyEvent.KEYCODE_MEDIA_REWIND,
        )) {
            assertNull(MediaKeySkip.direction(key))
        }
    }
}
