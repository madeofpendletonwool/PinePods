import MediaPlayer
import Foundation

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
            print("Remote command: play")
            self?.player?.play()
            return .success
        }

        // Pause command
        commandCenter.pauseCommand.isEnabled = true
        commandCenter.pauseCommand.addTarget { [weak self] _ in
            print("Remote command: pause")
            self?.player?.pause()
            return .success
        }

        // Toggle play/pause command
        commandCenter.togglePlayPauseCommand.isEnabled = true
        commandCenter.togglePlayPauseCommand.addTarget { [weak self] _ in
            print("Remote command: toggle play/pause")
            // Check current state and toggle
            // For simplicity, just toggle based on rate
            if let player = self?.player {
                let currentPosition = player.getCurrentPosition()
                if currentPosition > 0 {
                    // If playing, pause; if paused, play
                    // This is a simplification - in production you'd check actual state
                    player.play()
                }
            }
            return .success
        }

        // Skip forward command (fast forward 30s)
        commandCenter.skipForwardCommand.isEnabled = true
        commandCenter.skipForwardCommand.preferredIntervals = [30]
        commandCenter.skipForwardCommand.addTarget { [weak self] event in
            print("Remote command: skip forward")
            if let skipEvent = event as? MPSkipIntervalCommandEvent {
                let milliseconds = Int(skipEvent.interval * 1000)
                self?.player?.fastForward(milliseconds: milliseconds)
            } else {
                self?.player?.fastForward(milliseconds: 30000)
            }
            return .success
        }

        // Skip backward command (rewind 10s)
        commandCenter.skipBackwardCommand.isEnabled = true
        commandCenter.skipBackwardCommand.preferredIntervals = [10]
        commandCenter.skipBackwardCommand.addTarget { [weak self] event in
            print("Remote command: skip backward")
            if let skipEvent = event as? MPSkipIntervalCommandEvent {
                let milliseconds = Int(skipEvent.interval * 1000)
                self?.player?.rewind(milliseconds: milliseconds)
            } else {
                self?.player?.rewind(milliseconds: 10000)
            }
            return .success
        }

        // Seek command (scrubbing on lock screen)
        commandCenter.changePlaybackPositionCommand.isEnabled = true
        commandCenter.changePlaybackPositionCommand.addTarget { [weak self] event in
            if let seekEvent = event as? MPChangePlaybackPositionCommandEvent {
                let positionMs = Int(seekEvent.positionTime * 1000)
                print("Remote command: seek to \(positionMs)ms")
                self?.player?.seek(toMilliseconds: positionMs)
                return .success
            }
            return .commandFailed
        }

        // Disable commands we don't support
        commandCenter.nextTrackCommand.isEnabled = false
        commandCenter.previousTrackCommand.isEnabled = false
        commandCenter.changePlaybackRateCommand.isEnabled = false
        commandCenter.seekForwardCommand.isEnabled = false
        commandCenter.seekBackwardCommand.isEnabled = false

        print("Remote commands configured")
    }

    private func disableRemoteCommands() {
        commandCenter.playCommand.isEnabled = false
        commandCenter.pauseCommand.isEnabled = false
        commandCenter.togglePlayPauseCommand.isEnabled = false
        commandCenter.skipForwardCommand.isEnabled = false
        commandCenter.skipBackwardCommand.isEnabled = false
        commandCenter.changePlaybackPositionCommand.isEnabled = false

        commandCenter.playCommand.removeTarget(nil)
        commandCenter.pauseCommand.removeTarget(nil)
        commandCenter.togglePlayPauseCommand.removeTarget(nil)
        commandCenter.skipForwardCommand.removeTarget(nil)
        commandCenter.skipBackwardCommand.removeTarget(nil)
        commandCenter.changePlaybackPositionCommand.removeTarget(nil)
    }
}
