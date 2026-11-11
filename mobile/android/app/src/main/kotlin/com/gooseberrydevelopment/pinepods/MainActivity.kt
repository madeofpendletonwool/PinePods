package com.gooseberrydevelopment.pinepods

import android.content.Context
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine

class MainActivity: FlutterActivity() {
    // Shared with the media service, so it isn't destroyed with the activity.
    override fun provideFlutterEngine(context: Context): FlutterEngine = PinepodsEngine.obtain(context)
}
