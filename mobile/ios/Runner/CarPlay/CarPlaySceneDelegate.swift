import CarPlay
import MediaPlayer
import UIKit

/// Helper class to push CPNowPlayingTemplate.shared onto flutter_carplay's navigation stack
/// This accesses the interface controller from flutter_carplay's plugin
@available(iOS 14.0, *)
class CarPlayNowPlayingHelper: NSObject, CPNowPlayingTemplateObserver {

    static let shared = CarPlayNowPlayingHelper()

    private var isConfigured = false

    override init() {
        super.init()
        configureNowPlayingTemplate()
    }

    // MARK: - Configuration

    /// Configure the Now Playing template with proper buttons
    /// This only needs to be done once
    func configureNowPlayingTemplate() {
        guard !isConfigured else { return }

        let template = CPNowPlayingTemplate.shared

        NSLog("[CarPlayNowPlayingHelper] Configuring CPNowPlayingTemplate.shared")

        // Create playback rate button (1x, 1.5x, 2x)
        let rateButton = CPNowPlayingPlaybackRateButton { _ in
            NSLog("[CarPlayNowPlayingHelper] Rate button pressed")
            // The actual rate change is handled by MPRemoteCommandCenter.changePlaybackRateCommand
        }

        // Set the buttons - play/pause/skip are automatic from MPRemoteCommandCenter
        // Keep it simple with just the rate button for podcasts
        template.updateNowPlayingButtons([rateButton])

        // Configure options - disable album/artist button since we don't have album navigation
        template.isUpNextButtonEnabled = false
        template.isAlbumArtistButtonEnabled = false

        // Register as observer
        template.add(self)

        isConfigured = true
        NSLog("[CarPlayNowPlayingHelper] Now Playing template configured")
    }

    // MARK: - CPNowPlayingTemplateObserver

    func nowPlayingTemplateUpNextButtonTapped(_ nowPlayingTemplate: CPNowPlayingTemplate) {
        NSLog("[CarPlayNowPlayingHelper] Up Next button tapped")
        // Could show queue here if implemented
    }

    func nowPlayingTemplateAlbumArtistButtonTapped(_ nowPlayingTemplate: CPNowPlayingTemplate) {
        NSLog("[CarPlayNowPlayingHelper] Album/Artist button tapped")
        // Could navigate to podcast details here
    }

    // MARK: - Push Now Playing

    /// Push the Now Playing template using the CarPlay scene's interface controller
    /// Returns true if push was initiated (actual success is async)
    func pushNowPlaying() -> Bool {
        NSLog("[CarPlayNowPlayingHelper] Attempting to push Now Playing template")

        // Make sure template is configured first
        configureNowPlayingTemplate()

        // Get the interface controller from the CarPlay scene
        guard let controller = getCarPlayInterfaceController() else {
            NSLog("[CarPlayNowPlayingHelper] Could not get interface controller")
            return false
        }

        // Log current now playing info for debugging
        if let info = MPNowPlayingInfoCenter.default().nowPlayingInfo {
            let title = info[MPMediaItemPropertyTitle] as? String ?? "Unknown"
            let artist = info[MPMediaItemPropertyArtist] as? String ?? "Unknown"
            let rate = info[MPNowPlayingInfoPropertyPlaybackRate] as? Float ?? 0
            let duration = info[MPMediaItemPropertyPlaybackDuration] as? Double ?? 0
            let elapsed = info[MPNowPlayingInfoPropertyElapsedPlaybackTime] as? Double ?? 0
            let hasArtwork = info[MPMediaItemPropertyArtwork] != nil
            NSLog("[CarPlayNowPlayingHelper] Now Playing: '\(title)' by '\(artist)', rate: \(rate), elapsed: \(elapsed)/\(duration), artwork: \(hasArtwork)")
        } else {
            NSLog("[CarPlayNowPlayingHelper] WARNING: No Now Playing info set - CPNowPlayingTemplate may show blank!")
        }

        // Get the shared template
        let template = CPNowPlayingTemplate.shared

        // Check if now playing is already being shown (on top of stack)
        if let topTemplate = controller.topTemplate, topTemplate === template {
            NSLog("[CarPlayNowPlayingHelper] Now Playing template is already showing")
            return true
        }

        // Push the Now Playing template
        controller.pushTemplate(template, animated: true) { success, error in
            if let error = error {
                NSLog("[CarPlayNowPlayingHelper] Push failed: \(error.localizedDescription), code: \((error as NSError).code)")
            } else {
                NSLog("[CarPlayNowPlayingHelper] Now Playing template pushed successfully")
            }
        }

        return true
    }

    // MARK: - Get Interface Controller

    /// Get the CPInterfaceController from the active CarPlay scene
    private func getCarPlayInterfaceController() -> CPInterfaceController? {
        // Get all connected scenes and find the CarPlay template application scene
        let connectedScenes = UIApplication.shared.connectedScenes

        NSLog("[CarPlayNowPlayingHelper] Connected scenes: \(connectedScenes.count)")

        for scene in connectedScenes {
            NSLog("[CarPlayNowPlayingHelper] Scene: \(type(of: scene)), state: \(scene.activationState.rawValue)")

            if let carPlayScene = scene as? CPTemplateApplicationScene {
                let controller = carPlayScene.interfaceController
                NSLog("[CarPlayNowPlayingHelper] Found CarPlay scene, interface controller: \(controller), top template: \(String(describing: controller.topTemplate))")
                return controller
            }
        }

        NSLog("[CarPlayNowPlayingHelper] No CarPlay scene found")
        return nil
    }
}
