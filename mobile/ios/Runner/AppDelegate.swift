import UIKit
import Flutter
import flutter_downloader
import AVFoundation
import MediaPlayer

@main
@objc class AppDelegate: FlutterAppDelegate {

    override func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
    ) -> Bool {
        // Register flutter_downloader callback
        FlutterDownloaderPlugin.setPluginRegistrantCallback(registerPlugins)

        // CRITICAL: Set up audio session and remote control events EARLY
        // This must happen before CarPlay connects for Now Playing to work
        setupAudioForCarPlay()

        // NOTE: CarPlayNowPlayingChannel is registered in SceneDelegate
        // because that's where the active Flutter engine lives in scene-based apps

        return super.application(application, didFinishLaunchingWithOptions: launchOptions)
    }

    private func setupAudioForCarPlay() {
        do {
            let audioSession = AVAudioSession.sharedInstance()
            try audioSession.setCategory(.playback, mode: .spokenAudio, options: [.allowBluetooth, .allowAirPlay])
            try audioSession.setActive(true)
            NSLog("[AppDelegate] Audio session configured for CarPlay")
        } catch {
            NSLog("[AppDelegate] Failed to configure audio session: \(error)")
        }

        // Begin receiving remote control events - required for CarPlay Now Playing
        UIApplication.shared.beginReceivingRemoteControlEvents()
        NSLog("[AppDelegate] Remote control events enabled")

        // Set up placeholder now playing info so CarPlay recognizes us as an audio app
        var nowPlayingInfo = [String: Any]()
        nowPlayingInfo[MPMediaItemPropertyTitle] = "PinePods"
        nowPlayingInfo[MPMediaItemPropertyArtist] = "Podcast Player"
        nowPlayingInfo[MPNowPlayingInfoPropertyPlaybackRate] = 0.0
        MPNowPlayingInfoCenter.default().nowPlayingInfo = nowPlayingInfo
        NSLog("[AppDelegate] Placeholder now playing info set")
    }
}

private func registerPlugins(registry: FlutterPluginRegistry) {
    if (!registry.hasPlugin("FlutterDownloaderPlugin")) {
        FlutterDownloaderPlugin.register(with: registry.registrar(forPlugin: "FlutterDownloaderPlugin")!)
    }
}
