import Flutter
import Foundation
import AVFoundation
import MediaPlayer
import CarPlay

/// Flutter plugin that bridges native iOS audio playback to Flutter
/// Uses MethodChannel for commands and EventChannel for playback state updates
public class AudioPlayerPlugin: NSObject, FlutterPlugin, FlutterStreamHandler {
    private static let METHOD_CHANNEL = "com.pinepods/audio_player"
    private static let EVENT_CHANNEL = "com.pinepods/audio_events"

    private var eventSink: FlutterEventSink?
    private var audioPlayer: PinepodsAudioPlayer?
    private var isInitialized = false

    public static func register(with registrar: FlutterPluginRegistrar) {
        let instance = AudioPlayerPlugin()

        let methodChannel = FlutterMethodChannel(
            name: METHOD_CHANNEL,
            binaryMessenger: registrar.messenger()
        )
        registrar.addMethodCallDelegate(instance, channel: methodChannel)

        let eventChannel = FlutterEventChannel(
            name: EVENT_CHANNEL,
            binaryMessenger: registrar.messenger()
        )
        eventChannel.setStreamHandler(instance)

        // CRITICAL: Defer audio player initialization to avoid blocking main thread
        // during app startup. This prevents the iOS black screen issue.
        // But use a very short delay and ensure initialization completes.
        DispatchQueue.main.async {
            NSLog("[AudioPlayerPlugin] Initializing audio player on main thread...")
            instance.audioPlayer = PinepodsAudioPlayer(eventSink: instance.eventSink)
            instance.isInitialized = true
            NSLog("[AudioPlayerPlugin] Audio player initialized successfully")
        }

        NSLog("[AudioPlayerPlugin] Plugin registered")
    }

    // MARK: - FlutterStreamHandler

    public func onListen(withArguments arguments: Any?, eventSink events: @escaping FlutterEventSink) -> FlutterError? {
        NSLog("[AudioPlayerPlugin] Event stream listener registered")
        self.eventSink = events
        audioPlayer?.updateEventSink(events)
        return nil
    }

    public func onCancel(withArguments arguments: Any?) -> FlutterError? {
        NSLog("[AudioPlayerPlugin] Event stream listener cancelled")
        self.eventSink = nil
        audioPlayer?.updateEventSink(nil)
        return nil
    }

    // MARK: - FlutterPlugin Method Handling

    public func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
        // Allow getPosition and getDuration even if player isn't fully initialized
        // to prevent errors during early Flutter lifecycle
        if !isInitialized {
            switch call.method {
            case "getPosition", "getDuration":
                result(0)
                return
            default:
                // For other methods, wait a bit and retry
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) { [weak self] in
                    if self?.isInitialized == true {
                        self?.handlePlayerCall(call, result: result)
                    } else {
                        result(FlutterError(
                            code: "NOT_INITIALIZED",
                            message: "Audio player not yet initialized. Please try again.",
                            details: nil
                        ))
                    }
                }
                return
            }
        }

        handlePlayerCall(call, result: result)
    }

    private func handlePlayerCall(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
        guard let player = audioPlayer else {
            result(FlutterError(code: "NOT_INITIALIZED", message: "Audio player not initialized", details: nil))
            return
        }

        switch call.method {
        case "playEpisode":
            handlePlayEpisode(call, player: player, result: result)

        case "play":
            player.play()
            result(nil)

        case "pause":
            player.pause()
            result(nil)

        case "stop":
            player.stop()
            result(nil)

        case "seek":
            if let args = call.arguments as? [String: Any],
               let position = args["position"] as? Int {
                player.seek(toMilliseconds: position)
                result(nil)
            } else {
                result(FlutterError(code: "INVALID_ARGS", message: "Position required", details: nil))
            }

        case "fastForward":
            let milliseconds = (call.arguments as? [String: Any])?["milliseconds"] as? Int ?? 30000
            player.fastForward(milliseconds: milliseconds)
            result(nil)

        case "rewind":
            let milliseconds = (call.arguments as? [String: Any])?["milliseconds"] as? Int ?? 10000
            player.rewind(milliseconds: milliseconds)
            result(nil)

        case "setPlaybackSpeed":
            if let args = call.arguments as? [String: Any],
               let speed = args["speed"] as? Double {
                player.setPlaybackSpeed(Float(speed))
                result(nil)
            } else {
                result(FlutterError(code: "INVALID_ARGS", message: "Speed required", details: nil))
            }

        case "setSkipIntervals":
            let args = call.arguments as? [String: Any]
            let forwardMs = (args?["forwardMs"] as? Int) ?? 30000
            let backwardMs = (args?["backwardMs"] as? Int) ?? 10000
            player.setSkipIntervals(forwardMs: forwardMs, backwardMs: backwardMs)
            result(nil)

        case "setSkipSegments":
            let args = call.arguments as? [String: Any]
            let enabled = (args?["enabled"] as? Bool) ?? false
            let rawSegments = (args?["segments"] as? [[String: Any]]) ?? []
            let segments: [(start: Double, end: Double)] = rawSegments.compactMap { seg in
                guard let start = (seg["start"] as? NSNumber)?.doubleValue,
                      let end = (seg["end"] as? NSNumber)?.doubleValue else { return nil }
                return (start: start, end: end)
            }
            player.setSkipSegments(enabled: enabled, segments: segments)
            result(nil)

        case "getPosition":
            result(player.getCurrentPosition())

        case "getDuration":
            result(player.getDuration())

        case "isPlaying":
            result(player.isPlaying())

        case "getNowPlayingInfo":
            // Debug method to check what's in MPNowPlayingInfoCenter
            result(getNowPlayingInfoDebug())

        case "configureCarPlayNowPlaying":
            // Ensure CarPlay Now Playing template is configured
            configureCarPlayNowPlaying()
            result(nil)

        default:
            result(FlutterMethodNotImplemented)
        }
    }

    private func handlePlayEpisode(_ call: FlutterMethodCall, player: PinepodsAudioPlayer, result: @escaping FlutterResult) {
        guard let args = call.arguments as? [String: Any],
              let url = args["url"] as? String else {
            result(FlutterError(code: "INVALID_ARGS", message: "URL required", details: nil))
            return
        }

        let startPosition = args["startPosition"] as? Int ?? 0
        let isLocal = args["isLocal"] as? Bool ?? false
        let metadata = args["metadata"] as? [String: Any]

        NSLog("[AudioPlayerPlugin] playEpisode called: url=\(url.prefix(50))..., startPosition=\(startPosition)ms, isLocal=\(isLocal)")

        // Log metadata for debugging
        if let meta = metadata {
            NSLog("[AudioPlayerPlugin] Metadata received: title=\(meta["title"] ?? "nil"), artist=\(meta["artist"] ?? "nil"), duration=\(meta["duration"] ?? "nil"), artwork=\(meta["artwork"] != nil ? "present" : "nil")")
        } else {
            NSLog("[AudioPlayerPlugin] WARNING: No metadata received!")
        }

        // Send log event to Flutter for debugging
        eventSink?([
            "type": "log",
            "message": "Native playEpisode called with metadata: \(metadata != nil ? "present" : "nil")"
        ])

        player.playEpisode(
            url: url,
            startPosition: startPosition,
            isLocal: isLocal,
            metadata: metadata
        )
        result(nil)
    }

    // MARK: - CarPlay Now Playing Debug

    private func getNowPlayingInfoDebug() -> [String: Any] {
        let infoCenter = MPNowPlayingInfoCenter.default()

        guard let info = infoCenter.nowPlayingInfo else {
            NSLog("[AudioPlayerPlugin] getNowPlayingInfo: NO INFO SET")
            return [
                "hasInfo": false,
                "message": "No now playing info set in MPNowPlayingInfoCenter"
            ]
        }

        let title = info[MPMediaItemPropertyTitle] as? String ?? "nil"
        let artist = info[MPMediaItemPropertyArtist] as? String ?? "nil"
        let duration = info[MPMediaItemPropertyPlaybackDuration] as? Double ?? 0
        let elapsed = info[MPNowPlayingInfoPropertyElapsedPlaybackTime] as? Double ?? 0
        let rate = info[MPNowPlayingInfoPropertyPlaybackRate] as? Float ?? 0
        let hasArtwork = info[MPMediaItemPropertyArtwork] != nil

        NSLog("[AudioPlayerPlugin] getNowPlayingInfo: title='\(title)', artist='\(artist)', duration=\(duration)s, elapsed=\(elapsed)s, rate=\(rate), artwork=\(hasArtwork)")

        return [
            "hasInfo": true,
            "title": title,
            "artist": artist,
            "duration": duration,
            "elapsed": elapsed,
            "rate": rate,
            "hasArtwork": hasArtwork
        ]
    }

    private func configureCarPlayNowPlaying() {
        if #available(iOS 14.0, *) {
            NSLog("[AudioPlayerPlugin] Configuring CarPlay Now Playing template")

            // Use our helper to configure the shared Now Playing template
            CarPlayNowPlayingHelper.shared.configureNowPlayingTemplate()

            // Also log current Now Playing info for debugging
            let info = getNowPlayingInfoDebug()
            eventSink?([
                "type": "log",
                "message": "CarPlay Now Playing configured. Current info: \(info)"
            ])
        } else {
            NSLog("[AudioPlayerPlugin] CarPlay Now Playing not available (iOS 14+ required)")
        }
    }
}
