import AVFoundation
import MediaPlayer
import Foundation

class PinepodsAudioPlayer: NSObject {
    private var player: AVPlayer?
    private var playerItem: AVPlayerItem?
    private var eventSink: FlutterEventSink?
    private var timeObserver: Any?
    private var remoteCommandManager: RemoteCommandManager?
    private var nowPlayingManager: NowPlayingManager?

    private var currentMetadata: [String: Any]?

    // KVO context
    private var playerContext = 0

    init(eventSink: FlutterEventSink?) {
        self.eventSink = eventSink
        super.init()

        setupAudioSession()
        setupPlayer()
        setupRemoteCommands()
    }

    deinit {
        cleanup()
    }

    // MARK: - Setup

    private func setupAudioSession() {
        do {
            let audioSession = AVAudioSession.sharedInstance()
            try audioSession.setCategory(
                .playback,
                mode: .spokenAudio,
                options: [.allowBluetooth, .allowAirPlay, .allowBluetoothA2DP]
            )
            try audioSession.setActive(true)
            print("Audio session configured successfully")
        } catch {
            print("Failed to configure audio session: \(error)")
            sendError(code: -1, message: "Failed to configure audio session: \(error.localizedDescription)")
        }
    }

    private func setupPlayer() {
        player = AVPlayer()
        player?.automaticallyWaitsToMinimizeStalling = true

        // Observe player status
        player?.addObserver(self, forKeyPath: #keyPath(AVPlayer.timeControlStatus), options: [.new], context: &playerContext)
        player?.addObserver(self, forKeyPath: #keyPath(AVPlayer.rate), options: [.new], context: &playerContext)

        // Setup periodic time observer for position updates
        let interval = CMTime(seconds: 0.5, preferredTimescale: CMTimeScale(NSEC_PER_SEC))
        timeObserver = player?.addPeriodicTimeObserver(forInterval: interval, queue: .main) { [weak self] time in
            self?.sendPlaybackStateEvent()
        }

        // Setup notification for playback end
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(playerDidFinishPlaying),
            name: .AVPlayerItemDidPlayToEndTime,
            object: nil
        )

        // Setup interruption handling (phone calls, Siri, etc.)
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleInterruption),
            name: AVAudioSession.interruptionNotification,
            object: nil
        )
    }

    private func setupRemoteCommands() {
        remoteCommandManager = RemoteCommandManager(player: self)
        nowPlayingManager = NowPlayingManager()
    }

    private func cleanup() {
        if let observer = timeObserver {
            player?.removeTimeObserver(observer)
            timeObserver = nil
        }

        player?.removeObserver(self, forKeyPath: #keyPath(AVPlayer.timeControlStatus), context: &playerContext)
        player?.removeObserver(self, forKeyPath: #keyPath(AVPlayer.rate), context: &playerContext)

        NotificationCenter.default.removeObserver(self)

        player?.pause()
        player?.replaceCurrentItem(with: nil)
        player = nil

        remoteCommandManager = nil
        nowPlayingManager = nil
    }

    // MARK: - KVO

    override func observeValue(forKeyPath keyPath: String?, of object: Any?, change: [NSKeyValueChangeKey : Any]?, context: UnsafeMutableRawPointer?) {
        guard context == &playerContext else {
            super.observeValue(forKeyPath: keyPath, of: object, change: change, context: context)
            return
        }

        if keyPath == #keyPath(AVPlayer.timeControlStatus) || keyPath == #keyPath(AVPlayer.rate) {
            sendPlaybackStateEvent()
        }
    }

    // MARK: - Notifications

    @objc private func playerDidFinishPlaying() {
        print("Player finished playing")
        sendEvent([
            "type": "completed"
        ])
    }

    @objc private func handleInterruption(notification: Notification) {
        guard let userInfo = notification.userInfo,
              let typeValue = userInfo[AVAudioSessionInterruptionTypeKey] as? UInt,
              let type = AVAudioSession.InterruptionType(rawValue: typeValue) else {
            return
        }

        switch type {
        case .began:
            // Interruption began (phone call, Siri, etc.)
            print("Audio interruption began")
            pause()

        case .ended:
            // Interruption ended
            guard let optionsValue = userInfo[AVAudioSessionInterruptionOptionKey] as? UInt else {
                return
            }
            let options = AVAudioSession.InterruptionOptions(rawValue: optionsValue)
            if options.contains(.shouldResume) {
                print("Audio interruption ended - should resume")
                // Don't auto-resume for podcasts - let user decide
            }

        @unknown default:
            break
        }
    }

    // MARK: - Public API

    func updateEventSink(_ sink: FlutterEventSink?) {
        self.eventSink = sink
    }

    func playEpisode(url: String, startPosition: Int, isLocal: Bool, metadata: [String: Any]?) {
        print("playEpisode: url=\(url), startPosition=\(startPosition), isLocal=\(isLocal)")

        self.currentMetadata = metadata

        guard let audioUrl = URL(string: url) else {
            sendError(code: -1, message: "Invalid URL")
            return
        }

        let asset = AVURLAsset(url: audioUrl)
        playerItem = AVPlayerItem(asset: asset)

        // Configure buffering
        playerItem?.preferredForwardBufferDuration = 180  // 3 min forward buffer
        playerItem?.canUseNetworkResourcesForLiveStreamingWhilePaused = true

        // Observe player item status
        playerItem?.addObserver(self, forKeyPath: #keyPath(AVPlayerItem.status), options: [.new], context: &playerContext)

        player?.replaceCurrentItem(with: playerItem)

        // Seek to start position if needed
        if startPosition > 0 {
            let time = CMTime(value: CMTimeValue(startPosition), timescale: 1000)
            player?.seek(to: time, toleranceBefore: .zero, toleranceAfter: .zero)
        }

        // Update now playing info
        if let meta = metadata {
            nowPlayingManager?.updateNowPlaying(
                title: meta["title"] as? String ?? "Unknown",
                artist: meta["artist"] as? String ?? "Unknown",
                artworkUrl: meta["artwork"] as? String,
                duration: (meta["duration"] as? Int).map { Double($0) / 1000.0 } ?? 0,
                playbackRate: player?.rate ?? 1.0,
                elapsedTime: Double(startPosition) / 1000.0
            )
        }

        // Start playback
        player?.play()
    }

    func play() {
        print("play")
        player?.play()
        updateNowPlayingPlaybackInfo()
    }

    func pause() {
        print("pause")
        player?.pause()
        updateNowPlayingPlaybackInfo()
    }

    func stop() {
        print("stop")
        player?.pause()
        player?.replaceCurrentItem(with: nil)
        nowPlayingManager?.clearNowPlaying()
    }

    func seek(toMilliseconds milliseconds: Int) {
        let time = CMTime(value: CMTimeValue(milliseconds), timescale: 1000)
        player?.seek(to: time, toleranceBefore: .zero, toleranceAfter: .zero)
        updateNowPlayingPlaybackInfo()
    }

    func fastForward(milliseconds: Int) {
        guard let currentTime = player?.currentTime() else { return }
        let currentMs = Int(CMTimeGetSeconds(currentTime) * 1000)
        let newPosition = currentMs + milliseconds

        if let duration = player?.currentItem?.duration,
           duration.isValid {
            let maxMs = Int(CMTimeGetSeconds(duration) * 1000)
            seek(toMilliseconds: min(newPosition, maxMs))
        } else {
            seek(toMilliseconds: newPosition)
        }
    }

    func rewind(milliseconds: Int) {
        guard let currentTime = player?.currentTime() else { return }
        let currentMs = Int(CMTimeGetSeconds(currentTime) * 1000)
        let newPosition = max(0, currentMs - milliseconds)
        seek(toMilliseconds: newPosition)
    }

    func setPlaybackSpeed(_ speed: Float) {
        print("setPlaybackSpeed: \(speed)")
        player?.rate = speed
        updateNowPlayingPlaybackInfo()
    }

    func getCurrentPosition() -> Int {
        guard let currentTime = player?.currentTime() else { return 0 }
        return Int(CMTimeGetSeconds(currentTime) * 1000)
    }

    func getDuration() -> Int {
        guard let duration = player?.currentItem?.duration,
              duration.isValid else { return 0 }
        return Int(CMTimeGetSeconds(duration) * 1000)
    }

    // MARK: - Private Helpers

    private func sendPlaybackStateEvent() {
        guard let player = player else { return }

        let position = getCurrentPosition()
        let duration = getDuration()
        let bufferedPosition = getBufferedPosition()

        let state: String
        switch player.timeControlStatus {
        case .playing:
            state = "playing"
        case .paused:
            state = "paused"
        case .waitingToPlayAtSpecifiedRate:
            state = "buffering"
        @unknown default:
            state = "stopped"
        }

        sendEvent([
            "type": "playbackState",
            "state": state,
            "position": position,
            "bufferedPosition": bufferedPosition,
            "duration": duration,
            "speed": player.rate
        ])

        // Update now playing info
        updateNowPlayingPlaybackInfo()
    }

    private func getBufferedPosition() -> Int {
        guard let timeRanges = player?.currentItem?.loadedTimeRanges,
              let first = timeRanges.first else { return 0 }

        let timeRange = first.timeRangeValue
        let bufferedTime = CMTimeGetSeconds(CMTimeAdd(timeRange.start, timeRange.duration))
        return Int(bufferedTime * 1000)
    }

    private func updateNowPlayingPlaybackInfo() {
        let currentPosition = Double(getCurrentPosition()) / 1000.0
        let rate = player?.rate ?? 0.0
        nowPlayingManager?.updatePlaybackInfo(elapsedTime: currentPosition, playbackRate: rate)
    }

    private func sendEvent(_ event: [String: Any]) {
        DispatchQueue.main.async { [weak self] in
            self?.eventSink?(event)
        }
    }

    private func sendError(code: Int, message: String) {
        DispatchQueue.main.async { [weak self] in
            self?.eventSink?(FlutterError(
                code: String(code),
                message: message,
                details: nil
            ))
        }
    }
}
