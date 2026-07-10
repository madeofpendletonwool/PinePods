import AVFoundation
import MediaPlayer
import Foundation
import UIKit
import Flutter

/// Core audio player using AVPlayer for native iOS playback
/// Handles audio session, playback controls, and position tracking
class PinepodsAudioPlayer: NSObject {
    private var player: AVPlayer?
    private var playerItem: AVPlayerItem?
    private var eventSink: FlutterEventSink?
    private var timeObserver: Any?
    private var remoteCommandManager: RemoteCommandManager?
    private var nowPlayingManager: NowPlayingManager?

    private var currentMetadata: [String: Any]?
    private var isAudioSessionConfigured = false
    private var isPlayerSetup = false
    private var isRemoteCommandsSetup = false

    // KVO context
    private var playerContext = 0
    private var itemContext = 0

    // Silence trim (#727): AVPlayer has no native skip-silence DSP, so we apply
    // server-detected silent ranges by seeking past them from the periodic
    // time observer. Times are in seconds.
    private var skipSilenceEnabled = false
    private var skipSegments: [(start: Double, end: Double)] = []

    init(eventSink: FlutterEventSink?) {
        self.eventSink = eventSink
        super.init()

        // Initialize synchronously but on the main thread
        // The plugin already defers creation, so we can setup directly here
        NSLog("[PinepodsAudioPlayer] Initializing...")
        setupAudioSession()
        setupPlayer()
        setupRemoteCommands()
        NSLog("[PinepodsAudioPlayer] Initialization complete")
    }

    /// Ensure all components are initialized before playback
    private func ensureSetupComplete() {
        NSLog("[PinepodsAudioPlayer] ensureSetupComplete called - audioSession:\(isAudioSessionConfigured), player:\(isPlayerSetup), remoteCommands:\(isRemoteCommandsSetup)")

        if !isAudioSessionConfigured {
            NSLog("[PinepodsAudioPlayer] Setting up audio session...")
            setupAudioSession()
        }
        if !isPlayerSetup {
            NSLog("[PinepodsAudioPlayer] Setting up player...")
            setupPlayer()
        }
        if !isRemoteCommandsSetup {
            NSLog("[PinepodsAudioPlayer] Setting up remote commands...")
            setupRemoteCommands()
        }

        // Send status to Flutter for debugging
        sendEvent([
            "type": "log",
            "message": "Setup complete - nowPlayingManager:\(nowPlayingManager != nil), remoteCommandManager:\(remoteCommandManager != nil)"
        ])
    }

    deinit {
        cleanup()
    }

    // MARK: - Setup

    private func setupAudioSession() {
        guard !isAudioSessionConfigured else { return }

        do {
            let audioSession = AVAudioSession.sharedInstance()
            try audioSession.setCategory(
                .playback,
                mode: .spokenAudio,
                options: [.allowBluetooth, .allowAirPlay, .allowBluetoothA2DP]
            )
            try audioSession.setActive(true)
            isAudioSessionConfigured = true
            NSLog("[PinepodsAudioPlayer] Audio session configured successfully")
        } catch {
            NSLog("[PinepodsAudioPlayer] Failed to configure audio session: \(error)")
            sendError(code: -1, message: "Failed to configure audio session: \(error.localizedDescription)")
        }
    }

    private func setupPlayer() {
        guard !isPlayerSetup else { return }

        player = AVPlayer()
        player?.automaticallyWaitsToMinimizeStalling = true

        // Observe player status
        player?.addObserver(self, forKeyPath: #keyPath(AVPlayer.timeControlStatus), options: [.new], context: &playerContext)
        player?.addObserver(self, forKeyPath: #keyPath(AVPlayer.rate), options: [.new], context: &playerContext)

        // Setup periodic time observer for position updates (every 1s; battery)
        let interval = CMTime(seconds: 1.0, preferredTimescale: CMTimeScale(NSEC_PER_SEC))
        timeObserver = player?.addPeriodicTimeObserver(forInterval: interval, queue: .main) { [weak self] time in
            self?.applySilenceSkipIfNeeded(at: time.seconds)
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

        // Handle route changes (headphones disconnected, etc.)
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleRouteChange),
            name: AVAudioSession.routeChangeNotification,
            object: nil
        )

        isPlayerSetup = true
        NSLog("[PinepodsAudioPlayer] Player setup complete")
    }

    private func setupRemoteCommands() {
        guard !isRemoteCommandsSetup else { return }

        // Ensure audio session is active before setting up remote commands
        // This is required for the Now Playing info to appear on lock screen
        if !isAudioSessionConfigured {
            setupAudioSession()
        }

        // Enable receiving remote control events
        UIApplication.shared.beginReceivingRemoteControlEvents()

        remoteCommandManager = RemoteCommandManager(player: self)
        nowPlayingManager = NowPlayingManager()
        isRemoteCommandsSetup = true
        NSLog("[PinepodsAudioPlayer] Remote commands and now playing manager configured")
    }

    private func cleanup() {
        NSLog("[PinepodsAudioPlayer] Cleaning up...")

        if let observer = timeObserver {
            player?.removeTimeObserver(observer)
            timeObserver = nil
        }

        // Safely remove observers
        if player != nil {
            player?.removeObserver(self, forKeyPath: #keyPath(AVPlayer.timeControlStatus), context: &playerContext)
            player?.removeObserver(self, forKeyPath: #keyPath(AVPlayer.rate), context: &playerContext)
        }

        if playerItem != nil {
            playerItem?.removeObserver(self, forKeyPath: #keyPath(AVPlayerItem.status), context: &itemContext)
        }

        NotificationCenter.default.removeObserver(self)

        // Stop receiving remote control events
        UIApplication.shared.endReceivingRemoteControlEvents()

        // Clear now playing info
        nowPlayingManager?.clearNowPlaying()

        player?.pause()
        player?.replaceCurrentItem(with: nil)
        player = nil

        remoteCommandManager = nil
        nowPlayingManager = nil
    }

    // MARK: - KVO

    override func observeValue(forKeyPath keyPath: String?, of object: Any?, change: [NSKeyValueChangeKey : Any]?, context: UnsafeMutableRawPointer?) {
        if context == &playerContext {
            if keyPath == #keyPath(AVPlayer.timeControlStatus) || keyPath == #keyPath(AVPlayer.rate) {
                sendPlaybackStateEvent()
            }
        } else if context == &itemContext {
            if keyPath == #keyPath(AVPlayerItem.status) {
                handleItemStatusChange()
            }
        } else {
            super.observeValue(forKeyPath: keyPath, of: object, change: change, context: context)
        }
    }

    private func handleItemStatusChange() {
        guard let item = playerItem else { return }

        switch item.status {
        case .readyToPlay:
            NSLog("[PinepodsAudioPlayer] Player item ready to play")
            sendEvent(["type": "log", "message": "Player ready to play"])
            sendPlaybackStateEvent()
        case .failed:
            var errorMessage = "Unknown error"
            var errorCode = -2

            if let error = item.error as NSError? {
                errorMessage = error.localizedDescription
                errorCode = error.code

                // Provide more specific error information for common issues
                NSLog("[PinepodsAudioPlayer] Player item failed - Domain: \(error.domain), Code: \(error.code)")
                NSLog("[PinepodsAudioPlayer] Error details: \(error.localizedDescription)")

                if let underlyingError = error.userInfo[NSUnderlyingErrorKey] as? NSError {
                    NSLog("[PinepodsAudioPlayer] Underlying error: \(underlyingError.domain) - \(underlyingError.code): \(underlyingError.localizedDescription)")
                }

                // Common CoreMedia errors
                if error.domain == "CoreMediaErrorDomain" {
                    switch error.code {
                    case -12640:
                        errorMessage = "Network error: Could not load media. Check URL accessibility."
                    case -12939:
                        errorMessage = "Media format not supported."
                    case -12318:
                        errorMessage = "Network connection lost."
                    case -12660:
                        errorMessage = "Cannot open the URL."
                    default:
                        errorMessage = "CoreMedia error \(error.code): \(error.localizedDescription)"
                    }
                }
            }

            NSLog("[PinepodsAudioPlayer] Player item failed: \(errorMessage)")
            sendEvent(["type": "log", "message": "Playback error: \(errorMessage)"])
            sendError(code: errorCode, message: "Playback failed: \(errorMessage)")
        case .unknown:
            NSLog("[PinepodsAudioPlayer] Player item status unknown")
        @unknown default:
            break
        }
    }

    // MARK: - Notifications

    @objc private func playerDidFinishPlaying() {
        NSLog("[PinepodsAudioPlayer] Episode finished playing")
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
            NSLog("[PinepodsAudioPlayer] Audio interruption began (phone call, Siri, etc.)")
            pause()

        case .ended:
            guard let optionsValue = userInfo[AVAudioSessionInterruptionOptionKey] as? UInt else {
                return
            }
            let options = AVAudioSession.InterruptionOptions(rawValue: optionsValue)
            if options.contains(.shouldResume) {
                NSLog("[PinepodsAudioPlayer] Audio interruption ended - should resume available")
                // Don't auto-resume for podcasts - let user decide
            }

        @unknown default:
            break
        }
    }

    @objc private func handleRouteChange(notification: Notification) {
        guard let userInfo = notification.userInfo,
              let reasonValue = userInfo[AVAudioSessionRouteChangeReasonKey] as? UInt,
              let reason = AVAudioSession.RouteChangeReason(rawValue: reasonValue) else {
            return
        }

        switch reason {
        case .oldDeviceUnavailable:
            // Headphones disconnected - pause playback
            NSLog("[PinepodsAudioPlayer] Audio route changed - old device unavailable (headphones disconnected)")
            pause()
        default:
            break
        }
    }

    // MARK: - Public API

    func updateEventSink(_ sink: FlutterEventSink?) {
        self.eventSink = sink
    }

    func playEpisode(url: String, startPosition: Int, isLocal: Bool, metadata: [String: Any]?) {
        NSLog("[PinepodsAudioPlayer] playEpisode called")
        NSLog("[PinepodsAudioPlayer] URL: \(url)")
        NSLog("[PinepodsAudioPlayer] startPosition: \(startPosition)ms, isLocal: \(isLocal)")

        // Send URL info to Flutter for debugging
        sendEvent([
            "type": "log",
            "message": "playEpisode URL: \(url.prefix(100))..."
        ])

        // CRITICAL: Ensure all components are set up before playback
        // This prevents issues where now playing info isn't displayed
        ensureSetupComplete()

        // Clear any silence-skip ranges from the previous episode; the Dart layer
        // re-supplies them (or disables) via setSkipSegments after load.
        skipSilenceEnabled = false
        skipSegments = []

        self.currentMetadata = metadata

        // Handle both remote URLs and local file paths
        var audioUrl: URL?
        if isLocal {
            // For local files, create file URL
            if url.hasPrefix("file://") {
                audioUrl = URL(string: url)
            } else {
                audioUrl = URL(fileURLWithPath: url)
            }
            NSLog("[PinepodsAudioPlayer] Using local file URL: \(audioUrl?.absoluteString ?? "nil")")
        } else {
            // For remote URLs, try creating URL directly first
            // Only attempt encoding if direct creation fails
            audioUrl = URL(string: url)
            if audioUrl == nil {
                // Try percent encoding only if URL creation failed
                // This handles URLs with special characters
                if let encodedUrl = url.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) {
                    audioUrl = URL(string: encodedUrl)
                    NSLog("[PinepodsAudioPlayer] URL required encoding")
                }
            }
            NSLog("[PinepodsAudioPlayer] Using remote URL: \(audioUrl?.absoluteString ?? "nil")")
        }

        guard let finalUrl = audioUrl else {
            NSLog("[PinepodsAudioPlayer] Invalid URL: \(url)")
            sendEvent(["type": "log", "message": "ERROR: Invalid URL - could not parse"])
            sendError(code: -1, message: "Invalid URL")
            return
        }

        // Remove observer from previous item
        if let oldItem = playerItem {
            oldItem.removeObserver(self, forKeyPath: #keyPath(AVPlayerItem.status), context: &itemContext)
        }

        // Create asset with options for better streaming support
        // This helps handle redirects and various server configurations
        var assetOptions: [String: Any] = [:]
        if !isLocal {
            // Set HTTP headers for podcast URLs
            // Some servers require a User-Agent header
            assetOptions["AVURLAssetHTTPHeaderFieldsKey"] = [
                "User-Agent": "PinePods/1.0 (iOS; Podcast Client)",
                "Accept": "audio/*,*/*;q=0.9"
            ]
        }

        NSLog("[PinepodsAudioPlayer] Creating AVURLAsset with URL: \(finalUrl.absoluteString)")
        let asset = AVURLAsset(url: finalUrl, options: assetOptions.isEmpty ? nil : assetOptions)

        // Check if the asset is playable before proceeding
        NSLog("[PinepodsAudioPlayer] Loading asset keys...")
        sendEvent(["type": "log", "message": "Loading asset for playback..."])

        // Load asset asynchronously to avoid blocking
        asset.loadValuesAsynchronously(forKeys: ["playable", "duration"]) { [weak self] in
            DispatchQueue.main.async {
                guard let self = self else { return }

                var error: NSError?
                let playableStatus = asset.statusOfValue(forKey: "playable", error: &error)

                if playableStatus == .failed {
                    let errorMsg = error?.localizedDescription ?? "Unknown error"
                    NSLog("[PinepodsAudioPlayer] Asset failed to load: \(errorMsg)")
                    self.sendEvent(["type": "log", "message": "Asset load failed: \(errorMsg)"])
                    self.sendError(code: -3, message: "Failed to load audio: \(errorMsg)")
                    return
                }

                if playableStatus == .loaded {
                    if !asset.isPlayable {
                        NSLog("[PinepodsAudioPlayer] Asset is not playable")
                        self.sendEvent(["type": "log", "message": "ERROR: Asset is not playable"])
                        self.sendError(code: -4, message: "Audio format not supported")
                        return
                    }
                }

                NSLog("[PinepodsAudioPlayer] Asset loaded, creating player item...")
                self.sendEvent(["type": "log", "message": "Asset loaded successfully, starting playback..."])

                self.continuePlayback(asset: asset, startPosition: startPosition, metadata: metadata)
            }
        }
    }

    private func continuePlayback(asset: AVURLAsset, startPosition: Int, metadata: [String: Any]?) {
        playerItem = AVPlayerItem(asset: asset)

        // Configure buffering for streaming
        playerItem?.preferredForwardBufferDuration = 180  // 3 min forward buffer
        playerItem?.canUseNetworkResourcesForLiveStreamingWhilePaused = true

        // Observe player item status
        playerItem?.addObserver(self, forKeyPath: #keyPath(AVPlayerItem.status), options: [.new], context: &itemContext)

        player?.replaceCurrentItem(with: playerItem)

        // Set up now playing info BEFORE starting playback
        // Use playbackRate 1.0 to indicate we're about to play (not 0 which indicates paused)
        if let meta = metadata {
            let title = meta["title"] as? String ?? "Unknown"
            let artist = meta["artist"] as? String ?? "Unknown"
            let artworkUrl = meta["artwork"] as? String
            // Prefer the real asset duration (the "duration" key was just loaded
            // above) over the metadata duration. The metadata duration's unit has
            // historically been unreliable across play sources (PinepodsEpisode vs
            // queue/resume from the DB), which produced a wildly inflated lock
            // screen length. Fall back to metadata only if the asset duration
            // isn't known yet (e.g. live/indefinite streams).
            let assetSeconds = CMTimeGetSeconds(asset.duration)
            let metaSeconds = (meta["duration"] as? Int).map { Double($0) / 1000.0 } ?? 0
            let duration = (assetSeconds.isFinite && assetSeconds > 0) ? assetSeconds : metaSeconds
            let elapsed = Double(startPosition) / 1000.0

            NSLog("[PinepodsAudioPlayer] Setting Now Playing: title='\(title)', artist='\(artist)', duration=\(duration)s, elapsed=\(elapsed)s")
            NSLog("[PinepodsAudioPlayer] nowPlayingManager is \(nowPlayingManager != nil ? "available" : "NIL!")")

            if let npm = nowPlayingManager {
                npm.updateNowPlaying(
                    title: title,
                    artist: artist,
                    artworkUrl: artworkUrl,
                    duration: duration,
                    playbackRate: 1.0,  // Set to 1.0 since we're about to play
                    elapsedTime: elapsed
                )
                NSLog("[PinepodsAudioPlayer] Now Playing info set successfully")

                // Send confirmation to Flutter
                sendEvent([
                    "type": "log",
                    "message": "Now Playing set: '\(title)' by '\(artist)'"
                ])
            } else {
                NSLog("[PinepodsAudioPlayer] ERROR: nowPlayingManager is nil!")
                sendEvent([
                    "type": "log",
                    "message": "ERROR: nowPlayingManager is nil, cannot set Now Playing info"
                ])
            }
        } else {
            NSLog("[PinepodsAudioPlayer] WARNING: No metadata provided for Now Playing")
            sendEvent([
                "type": "log",
                "message": "WARNING: No metadata for Now Playing"
            ])
        }

        // Seek to start position if needed
        if startPosition > 0 {
            let time = CMTime(value: CMTimeValue(startPosition), timescale: 1000)
            player?.seek(to: time, toleranceBefore: .zero, toleranceAfter: .zero) { [weak self] finished in
                if finished {
                    NSLog("[PinepodsAudioPlayer] Seeked to start position: \(startPosition)ms")
                }
                self?.player?.play()
                // Update now playing info again after playback starts
                self?.updateNowPlayingPlaybackInfo()
            }
        } else {
            player?.play()
            // Update now playing info after playback starts
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { [weak self] in
                self?.updateNowPlayingPlaybackInfo()
            }
        }
    }

    func play() {
        NSLog("[PinepodsAudioPlayer] play")
        player?.play()
        // Slight delay to allow rate to update before refreshing Now Playing
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { [weak self] in
            self?.updateNowPlayingPlaybackInfo()
        }
    }

    func pause() {
        NSLog("[PinepodsAudioPlayer] pause")
        player?.pause()
        // Update Now Playing immediately with rate 0 to indicate paused state
        let currentPosition = Double(getCurrentPosition()) / 1000.0
        nowPlayingManager?.updatePlaybackInfo(elapsedTime: currentPosition, playbackRate: 0.0)
    }

    func stop() {
        NSLog("[PinepodsAudioPlayer] stop")
        player?.pause()
        player?.replaceCurrentItem(with: nil)
        nowPlayingManager?.clearNowPlaying()
    }

    func seek(toMilliseconds milliseconds: Int) {
        NSLog("[PinepodsAudioPlayer] seek to \(milliseconds)ms")
        let time = CMTime(value: CMTimeValue(milliseconds), timescale: 1000)
        player?.seek(to: time, toleranceBefore: .zero, toleranceAfter: .zero)
        updateNowPlayingPlaybackInfo()
    }

    func fastForward(milliseconds: Int) {
        guard let currentTime = player?.currentTime() else { return }
        let currentMs = Int(CMTimeGetSeconds(currentTime) * 1000)
        let newPosition = currentMs + milliseconds

        if let duration = player?.currentItem?.duration,
           duration.isValid && !duration.isIndefinite {
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
        NSLog("[PinepodsAudioPlayer] setPlaybackSpeed: \(speed)")
        player?.rate = speed
        updateNowPlayingPlaybackInfo()
    }

    /// Push the user-configured skip intervals to the remote command center so
    /// lock-screen / CarPlay / head-unit controls skip by the same amount as the
    /// in-app buttons.
    func setSkipIntervals(forwardMs: Int, backwardMs: Int) {
        NSLog("[PinepodsAudioPlayer] setSkipIntervals: forward=\(forwardMs)ms back=\(backwardMs)ms")
        remoteCommandManager?.updateSkipIntervals(forwardMs: forwardMs, backwardMs: backwardMs)
    }

    /// Configure server-detected silence ranges for the current episode (#727).
    /// Passing enabled=false or an empty list disables skipping.
    func setSkipSegments(enabled: Bool, segments: [(start: Double, end: Double)]) {
        NSLog("[PinepodsAudioPlayer] setSkipSegments: enabled=\(enabled), count=\(segments.count)")
        skipSilenceEnabled = enabled
        skipSegments = segments.sorted { $0.start < $1.start }
    }

    /// Called from the 1s periodic observer: if the playhead is inside a silence
    /// range, seek to its end. A small tolerance avoids seeking when we're
    /// essentially already past the range.
    private func applySilenceSkipIfNeeded(at seconds: Double) {
        guard skipSilenceEnabled, !skipSegments.isEmpty, seconds.isFinite else { return }
        if let seg = skipSegments.first(where: { seconds >= $0.start && seconds < $0.end - 0.25 }) {
            let target = CMTime(seconds: seg.end, preferredTimescale: CMTimeScale(NSEC_PER_SEC))
            player?.seek(to: target, toleranceBefore: .zero, toleranceAfter: .zero)
        }
    }

    func getCurrentPosition() -> Int {
        guard let currentTime = player?.currentTime(), currentTime.isValid && !currentTime.isIndefinite else {
            return 0
        }
        return Int(CMTimeGetSeconds(currentTime) * 1000)
    }

    func getDuration() -> Int {
        guard let duration = player?.currentItem?.duration,
              duration.isValid && !duration.isIndefinite else {
            return 0
        }
        return Int(CMTimeGetSeconds(duration) * 1000)
    }

    func isPlaying() -> Bool {
        return player?.timeControlStatus == .playing
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
