import MediaPlayer
import Foundation

/// Manages remote command center controls for iOS
/// Handles play/pause from lock screen, headphones, CarPlay, etc.
class RemoteCommandManager {
    private weak var player: PinepodsAudioPlayer?
    private let commandCenter = MPRemoteCommandCenter.shared()

    init(player: PinepodsAudioPlayer) {
        self.player = player
        setupRemoteCommands()
    }

    deinit {
        disableRemoteCommands()
    }

    private func setupRemoteCommands() {
        // Play command
        commandCenter.playCommand.isEnabled = true
        commandCenter.playCommand.addTarget { [weak self] _ in
            NSLog("[RemoteCommandManager] Remote command: play")
            self?.player?.play()
            return .success
        }

        // Pause command
        commandCenter.pauseCommand.isEnabled = true
        commandCenter.pauseCommand.addTarget { [weak self] _ in
            NSLog("[RemoteCommandManager] Remote command: pause")
            self?.player?.pause()
            return .success
        }

        // Toggle play/pause command (headphone button single press)
        commandCenter.togglePlayPauseCommand.isEnabled = true
        commandCenter.togglePlayPauseCommand.addTarget { [weak self] _ in
            NSLog("[RemoteCommandManager] Remote command: toggle play/pause")
            guard let player = self?.player else { return .commandFailed }

            if player.isPlaying() {
                player.pause()
            } else {
                player.play()
            }
            return .success
        }

        // Skip forward command (fast forward 30s - standard for podcasts)
        commandCenter.skipForwardCommand.isEnabled = true
        commandCenter.skipForwardCommand.preferredIntervals = [30]
        commandCenter.skipForwardCommand.addTarget { [weak self] event in
            NSLog("[RemoteCommandManager] Remote command: skip forward")
            if let skipEvent = event as? MPSkipIntervalCommandEvent {
                let milliseconds = Int(skipEvent.interval * 1000)
                self?.player?.fastForward(milliseconds: milliseconds)
            } else {
                self?.player?.fastForward(milliseconds: 30000)
            }
            return .success
        }

        // Skip backward command (rewind 10s - standard for podcasts)
        commandCenter.skipBackwardCommand.isEnabled = true
        commandCenter.skipBackwardCommand.preferredIntervals = [10]
        commandCenter.skipBackwardCommand.addTarget { [weak self] event in
            NSLog("[RemoteCommandManager] Remote command: skip backward")
            if let skipEvent = event as? MPSkipIntervalCommandEvent {
                let milliseconds = Int(skipEvent.interval * 1000)
                self?.player?.rewind(milliseconds: milliseconds)
            } else {
                self?.player?.rewind(milliseconds: 10000)
            }
            return .success
        }

        // Seek command (scrubbing on lock screen / Now Playing)
        commandCenter.changePlaybackPositionCommand.isEnabled = true
        commandCenter.changePlaybackPositionCommand.addTarget { [weak self] event in
            if let seekEvent = event as? MPChangePlaybackPositionCommandEvent {
                let positionMs = Int(seekEvent.positionTime * 1000)
                NSLog("[RemoteCommandManager] Remote command: seek to \(positionMs)ms")
                self?.player?.seek(toMilliseconds: positionMs)
                return .success
            }
            return .commandFailed
        }

        // Playback rate change (for podcast speed control)
        commandCenter.changePlaybackRateCommand.isEnabled = true
        commandCenter.changePlaybackRateCommand.supportedPlaybackRates = [0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0]
        commandCenter.changePlaybackRateCommand.addTarget { [weak self] event in
            if let rateEvent = event as? MPChangePlaybackRateCommandEvent {
                NSLog("[RemoteCommandManager] Remote command: change playback rate to \(rateEvent.playbackRate)")
                self?.player?.setPlaybackSpeed(rateEvent.playbackRate)
                return .success
            }
            return .commandFailed
        }

        // Disable commands we don't support for podcasts
        commandCenter.nextTrackCommand.isEnabled = false
        commandCenter.previousTrackCommand.isEnabled = false
        commandCenter.seekForwardCommand.isEnabled = false
        commandCenter.seekBackwardCommand.isEnabled = false

        NSLog("[RemoteCommandManager] Remote commands configured")
    }

    private func disableRemoteCommands() {
        commandCenter.playCommand.isEnabled = false
        commandCenter.pauseCommand.isEnabled = false
        commandCenter.togglePlayPauseCommand.isEnabled = false
        commandCenter.skipForwardCommand.isEnabled = false
        commandCenter.skipBackwardCommand.isEnabled = false
        commandCenter.changePlaybackPositionCommand.isEnabled = false
        commandCenter.changePlaybackRateCommand.isEnabled = false

        commandCenter.playCommand.removeTarget(nil)
        commandCenter.pauseCommand.removeTarget(nil)
        commandCenter.togglePlayPauseCommand.removeTarget(nil)
        commandCenter.skipForwardCommand.removeTarget(nil)
        commandCenter.skipBackwardCommand.removeTarget(nil)
        commandCenter.changePlaybackPositionCommand.removeTarget(nil)
        commandCenter.changePlaybackRateCommand.removeTarget(nil)

        NSLog("[RemoteCommandManager] Remote commands disabled")
    }
}
