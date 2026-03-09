package com.gooseberrydevelopment.pinepods

import com.gooseberrydevelopment.pinepods.audio.AudioPlayerPlugin
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine

class MainActivity: FlutterActivity() {
    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)

        // Register native audio player plugin
        flutterEngine.plugins.add(AudioPlayerPlugin())
    }
}