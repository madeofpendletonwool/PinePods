package com.gooseberrydevelopment.pinepods

import android.content.Context
import com.gooseberrydevelopment.pinepods.audio.AudioPlayerPlugin
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.embedding.engine.FlutterEngineCache
import io.flutter.embedding.engine.dart.DartExecutor

/**
 * The app's one Flutter engine, shared by the activity and the media service.
 * Android Auto can start the media service with the app closed; the service
 * then starts this engine without UI so the car gets real content, and the
 * activity attaches to the same engine when the app is opened.
 */
object PinepodsEngine {
    private const val ID = "pinepods_main"

    val isRunning: Boolean
        get() = FlutterEngineCache.getInstance().contains(ID)

    fun obtain(context: Context): FlutterEngine {
        FlutterEngineCache.getInstance().get(ID)?.let { return it }
        val engine = FlutterEngine(context.applicationContext)
        engine.plugins.add(AudioPlayerPlugin())
        engine.dartExecutor.executeDartEntrypoint(DartExecutor.DartEntrypoint.createDefault())
        FlutterEngineCache.getInstance().put(ID, engine)
        return engine
    }
}
