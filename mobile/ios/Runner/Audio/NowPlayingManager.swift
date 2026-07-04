import MediaPlayer
import Foundation
import UIKit

/// Manages the Now Playing Info Center for lock screen and control center display
/// Shows podcast artwork, title, artist, and playback progress
class NowPlayingManager {
    private let nowPlayingInfoCenter = MPNowPlayingInfoCenter.default()
    private var currentArtwork: MPMediaItemArtwork?
    private var artworkCache: [String: UIImage] = [:]
    private var defaultArtwork: MPMediaItemArtwork?

    init() {
        // Create a default placeholder artwork for when no image is available
        // This helps ensure the Now Playing controls appear even without artwork
        createDefaultArtwork()
    }

    private func createDefaultArtwork() {
        // Try to use the app icon as default artwork
        if let appIcon = UIImage(named: "AppIcon") ?? UIImage(named: "pinepods-logo") {
            defaultArtwork = MPMediaItemArtwork(boundsSize: appIcon.size) { _ in
                return appIcon
            }
        } else {
            // Create a simple colored placeholder if no app icon is available
            let size = CGSize(width: 300, height: 300)
            UIGraphicsBeginImageContextWithOptions(size, true, 0)
            UIColor(red: 0.33, green: 0.62, blue: 0.54, alpha: 1.0).setFill()  // PinePods green
            UIRectFill(CGRect(origin: .zero, size: size))
            if let placeholderImage = UIGraphicsGetImageFromCurrentImageContext() {
                defaultArtwork = MPMediaItemArtwork(boundsSize: placeholderImage.size) { _ in
                    return placeholderImage
                }
            }
            UIGraphicsEndImageContext()
        }
    }

    func updateNowPlaying(
        title: String,
        artist: String,
        artworkUrl: String?,
        duration: Double,
        playbackRate: Float,
        elapsedTime: Double
    ) {
        var nowPlayingInfo = [String: Any]()

        // Basic metadata
        nowPlayingInfo[MPMediaItemPropertyTitle] = title
        nowPlayingInfo[MPMediaItemPropertyArtist] = artist
        nowPlayingInfo[MPMediaItemPropertyAlbumTitle] = artist  // Show podcast name in album field too
        nowPlayingInfo[MPMediaItemPropertyPlaybackDuration] = duration

        // Playback info - CRITICAL for lock screen controls to appear
        nowPlayingInfo[MPNowPlayingInfoPropertyPlaybackRate] = playbackRate
        nowPlayingInfo[MPNowPlayingInfoPropertyDefaultPlaybackRate] = 1.0
        nowPlayingInfo[MPNowPlayingInfoPropertyElapsedPlaybackTime] = elapsedTime

        // Media type - indicate this is audio
        nowPlayingInfo[MPMediaItemPropertyMediaType] = MPMediaType.podcast.rawValue

        // Use cached artwork if available, otherwise use default
        if let artwork = currentArtwork {
            nowPlayingInfo[MPMediaItemPropertyArtwork] = artwork
        } else if let defaultArt = defaultArtwork {
            nowPlayingInfo[MPMediaItemPropertyArtwork] = defaultArt
        }

        // Set now playing info immediately (artwork will update later if needed)
        nowPlayingInfoCenter.nowPlayingInfo = nowPlayingInfo

        // CarPlay's CPNowPlayingTemplate reads playbackState, NOT just the
        // playback-rate key. Without this it shows "Nothing Playing" even when
        // nowPlayingInfo is fully populated.
        nowPlayingInfoCenter.playbackState = playbackRate > 0 ? .playing : .paused

        NSLog("[NowPlayingManager] Now playing updated: '\(title)' by '\(artist)', duration: \(duration)s, rate: \(playbackRate)")

        // Load artwork asynchronously if URL provided
        if let urlString = artworkUrl,
           !urlString.isEmpty,
           let url = URL(string: urlString) {
            loadArtwork(from: url) { [weak self] artwork in
                if let artwork = artwork {
                    self?.currentArtwork = artwork
                    var info = self?.nowPlayingInfoCenter.nowPlayingInfo ?? [:]
                    info[MPMediaItemPropertyArtwork] = artwork
                    self?.nowPlayingInfoCenter.nowPlayingInfo = info
                    NSLog("[NowPlayingManager] Artwork loaded and updated")
                }
            }
        }
    }

    func updatePlaybackInfo(elapsedTime: Double, playbackRate: Float) {
        guard var nowPlayingInfo = nowPlayingInfoCenter.nowPlayingInfo else { return }

        nowPlayingInfo[MPNowPlayingInfoPropertyElapsedPlaybackTime] = elapsedTime
        nowPlayingInfo[MPNowPlayingInfoPropertyPlaybackRate] = playbackRate

        nowPlayingInfoCenter.nowPlayingInfo = nowPlayingInfo

        // Keep CarPlay / lock-screen playback state in sync with the rate.
        nowPlayingInfoCenter.playbackState = playbackRate > 0 ? .playing : .paused
    }

    func clearNowPlaying() {
        nowPlayingInfoCenter.nowPlayingInfo = nil
        nowPlayingInfoCenter.playbackState = .stopped
        currentArtwork = nil
        NSLog("[NowPlayingManager] Now playing cleared")
    }

    private func loadArtwork(from url: URL, completion: @escaping (MPMediaItemArtwork?) -> Void) {
        // Check cache first
        let cacheKey = url.absoluteString
        if let cachedImage = artworkCache[cacheKey] {
            let artwork = MPMediaItemArtwork(boundsSize: cachedImage.size) { _ in
                return cachedImage
            }
            DispatchQueue.main.async {
                completion(artwork)
            }
            return
        }

        // Download artwork
        URLSession.shared.dataTask(with: url) { [weak self] data, response, error in
            guard let data = data,
                  let image = UIImage(data: data) else {
                NSLog("[NowPlayingManager] Failed to load artwork from \(url): \(error?.localizedDescription ?? "unknown error")")
                DispatchQueue.main.async {
                    completion(nil)
                }
                return
            }

            // Cache the image
            self?.artworkCache[cacheKey] = image

            // Limit cache size to 10 images
            if let self = self, self.artworkCache.count > 10 {
                // Remove oldest entries (simple approach - remove first)
                if let firstKey = self.artworkCache.keys.first {
                    self.artworkCache.removeValue(forKey: firstKey)
                }
            }

            let artwork = MPMediaItemArtwork(boundsSize: image.size) { _ in
                return image
            }

            DispatchQueue.main.async {
                completion(artwork)
            }
        }.resume()
    }
}
