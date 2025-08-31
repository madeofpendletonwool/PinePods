# Add project specific ProGuard rules here.

# Keep audio service classes
-keep class com.ryanheise.audioservice.** { *; }
-keep class com.ryanheise.just_audio.** { *; }

# Keep audio player classes
-keep class com.google.android.exoplayer2.** { *; }

# Keep media metadata classes
-keep class android.support.v4.media.** { *; }
-keep class androidx.media.** { *; }

# Keep Flutter audio plugin classes
-keep class io.flutter.plugins.audioplayers.** { *; }

# Prevent obfuscation of duration and metadata fields
-keepclassmembers class * {
    ** duration;
    ** mediaMetadata;
    ** mediaItem;
}

# Keep native audio libraries
-keep class com.google.android.gms.** { *; }