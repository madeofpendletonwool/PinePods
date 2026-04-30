import Flutter
import UIKit
import MediaPlayer

/// Flutter method channel for CarPlay Now Playing control
/// Allows Flutter to trigger pushing the native CPNowPlayingTemplate.shared
class CarPlayNowPlayingChannel: NSObject, FlutterPlugin {

    static let channelName = "com.pinepods/carplay_now_playing"

    static func register(with registrar: FlutterPluginRegistrar) {
        let channel = FlutterMethodChannel(
            name: channelName,
            binaryMessenger: registrar.messenger()
        )
        let instance = CarPlayNowPlayingChannel()
        registrar.addMethodCallDelegate(instance, channel: channel)
        NSLog("[CarPlayNowPlayingChannel] Registered")

        // Initialize the helper early to configure the Now Playing template
        if #available(iOS 14.0, *) {
            _ = CarPlayNowPlayingHelper.shared
        }
    }

    func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
        NSLog("[CarPlayNowPlayingChannel] Method called: \(call.method)")

        switch call.method {
        case "pushNowPlaying":
            pushNowPlaying(result: result)
        case "canShowNowPlaying":
            canShowNowPlaying(result: result)
        default:
            result(FlutterMethodNotImplemented)
        }
    }

    private func pushNowPlaying(result: @escaping FlutterResult) {
        if #available(iOS 14.0, *) {
            let success = CarPlayNowPlayingHelper.shared.pushNowPlaying()
            result(success)
        } else {
            result(FlutterError(code: "UNSUPPORTED", message: "iOS 14+ required for CarPlay", details: nil))
        }
    }

    private func canShowNowPlaying(result: @escaping FlutterResult) {
        if #available(iOS 14.0, *) {
            let hasNowPlayingInfo = MPNowPlayingInfoCenter.default().nowPlayingInfo != nil
            result(hasNowPlayingInfo)
        } else {
            result(false)
        }
    }
}
