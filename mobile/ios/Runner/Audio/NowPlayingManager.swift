import MediaPlayer
import Foundation

class NowPlayingManager {
    private let nowPlayingInfoCenter = MPNowPlayingInfoCenter.default()
    private var currentArtwork: MPMediaItemArtwork?

    func updateNowPlaying(
        title: String,
        artist: String,
        artworkUrl: String?,
        duration: Double,
        playbackRate: Float,
        elapsedTime: Double
    ) {
        var nowPlayingInfo = [String: Any]()

        nowPlayingInfo[MPMediaItemPropertyTitle] = title
        nowPlayingInfo[MPMediaItemPropertyArtist] = artist
        nowPlayingInfo[MPMediaItemPropertyPlaybackDuration] = duration
        nowPlayingInfo[MPNowPlayingInfoPropertyPlaybackRate] = playbackRate
        nowPlayingInfo[MPNowPlayingInfoPropertyElapsedPlaybackTime] = elapsedTime

        // Load artwork if URL provided
        if let urlString = artworkUrl,
           let url = URL(string: urlString) {
            loadArtwork(from: url) { [weak self] artwork in
                if let artwork = artwork {
                    self?.currentArtwork = artwork
                    nowPlayingInfo[MPMediaItemPropertyArtwork] = artwork
                    self?.nowPlayingInfoCenter.nowPlayingInfo = nowPlayingInfo
                }
            }
        }

        // Set now playing info immediately (artwork will update later if needed)
        if let artwork = currentArtwork {
            nowPlayingInfo[MPMediaItemPropertyArtwork] = artwork
        }
        nowPlayingInfoCenter.nowPlayingInfo = nowPlayingInfo

        print("Now playing updated: \(title) by \(artist)")
    }

    func updatePlaybackInfo(elapsedTime: Double, playbackRate: Float) {
        guard var nowPlayingInfo = nowPlayingInfoCenter.nowPlayingInfo else { return }

        nowPlayingInfo[MPNowPlayingInfoPropertyElapsedPlaybackTime] = elapsedTime
        nowPlayingInfo[MPNowPlayingInfoPropertyPlaybackRate] = playbackRate

        nowPlayingInfoCenter.nowPlayingInfo = nowPlayingInfo
    }

    func clearNowPlaying() {
        nowPlayingInfoCenter.nowPlayingInfo = nil
        currentArtwork = nil
        print("Now playing cleared")
    }

    private func loadArtwork(from url: URL, completion: @escaping (MPMediaItemArtwork?) -> Void) {
        // Try to load artwork from URL
        URLSession.shared.dataTask(with: url) { data, response, error in
            guard let data = data,
                  let image = UIImage(data: data) else {
                completion(nil)
                return
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
