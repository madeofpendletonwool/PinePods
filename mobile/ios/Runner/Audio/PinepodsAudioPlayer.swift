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
    // Fires as playback reaches the start of a skip range, so it is skipped as
    // it begins rather than at the next 1s tick.
    private var skipBoundaryObserver: Any?
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
    // A skip seek still running, the range it last skipped and when, and ranges given up on
    // because the playhead landed back inside them (a stream that can't seek there).
    private var skipSeekInFlight = false
    private var lastSkippedIndex: Int?
    private var lastSkipTime: TimeInterval = 0
    private var abandonedSkipIndices = Set<Int>()
    private var playbackIntent = PlaybackIntent()

    // The last playEpisode request, so play() can rebuild a failed item, and
    // whether its asset is still loading (so play() doesn't restart that load).
    private var lastRequest: (url: String, isLocal: Bool)?
    private var loadInFlight = false

    // Last healthy playhead (ms), used as the resume point when rebuilding.
    private var lastKnownPositionMs = 0

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
            if time.isValid && !time.isIndefinite && time.seconds > 0 {
                self?.lastKnownPositionMs = Int(time.seconds * 1000)
            }
            self?.applySilenceSkipIfNeeded(at: time.seconds)
            self?.sendPlaybackStateEvent(updateNowPlaying: false)
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
        if let observer = skipBoundaryObserver {
            player?.removeTimeObserver(observer)
            skipBoundaryObserver = nil
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
            loadInFlight = false
            let seconds = CMTimeGetSeconds(item.duration)
            if seconds.isFinite && seconds > 0 {
                nowPlayingManager?.updateDuration(seconds)
            }
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
            loadInFlight = false
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
            playbackIntent.interruptionBegan()
            player?.pause()
            updateNowPlayingPlaybackInfo()

        case .ended:
            let optionsValue = userInfo[AVAudioSessionInterruptionOptionKey] as? UInt ?? 0
            let options = AVAudioSession.InterruptionOptions(rawValue: optionsValue)
            if playbackIntent.interruptionEnded(shouldResume: options.contains(.shouldResume)) {
                do {
                    try AVAudioSession.sharedInstance().setActive(true)
                    resumeRequestedPlayback()
                } catch {
                    pause()
                    NSLog("[PinepodsAudioPlayer] Could not reactivate audio session: \(error)")
                }
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

    func playEpisode(url: String, startPosition: Int, isLocal: Bool, metadata: [String: Any]?, speed: Float? = nil) {
        let generation = playbackIntent.beginEpisode()
        player?.pause()
        lastRequest = (url: url, isLocal: isLocal)
        lastKnownPositionMs = startPosition
        loadInFlight = true
        if let speed = speed { playbackIntent.setRate(speed) }
        NSLog("[PinepodsAudioPlayer] playEpisode: \(url), startPosition: \(startPosition)ms, isLocal: \(isLocal)")

        // CRITICAL: Ensure all components are set up before playback
        // This prevents issues where now playing info isn't displayed
        ensureSetupComplete()

        // Clear any silence-skip ranges from the previous episode; the Dart layer
        // re-supplies them (or disables) via setSkipSegments after load.
        skipSilenceEnabled = false
        setSkipRanges([])

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
        }

        guard let finalUrl = audioUrl else {
            NSLog("[PinepodsAudioPlayer] Invalid URL: \(url)")
            loadInFlight = false
            sendError(code: -1, message: "Invalid URL")
            return
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

        let asset = AVURLAsset(url: finalUrl, options: assetOptions.isEmpty ? nil : assetOptions)

        // Hand the asset straight to the player. Loading "duration" up front can
        // read a large part of a remote MP3 before anything plays; load failures
        // and unplayable formats surface through the item's status instead.
        continuePlayback(asset: asset, startPosition: startPosition, isLocal: isLocal,
                         metadata: metadata, generation: generation)
    }

    private func continuePlayback(asset: AVURLAsset, startPosition: Int, isLocal: Bool,
                                  metadata: [String: Any]?, generation: UInt64) {
        if let oldItem = playerItem {
            oldItem.removeObserver(self, forKeyPath: #keyPath(AVPlayerItem.status), context: &itemContext)
        }
        playerItem = AVPlayerItem(asset: asset)

        // Configure buffering for streaming
        playerItem?.preferredForwardBufferDuration = 180  // 3 min forward buffer
        playerItem?.canUseNetworkResourcesForLiveStreamingWhilePaused = true

        // Observe player item status
        playerItem?.addObserver(self, forKeyPath: #keyPath(AVPlayerItem.status), options: [.new], context: &itemContext)

        // A file on disk has nothing to stall on, so start it immediately.
        player?.automaticallyWaitsToMinimizeStalling = !isLocal
        player?.replaceCurrentItem(with: playerItem)

        if let meta = metadata {
            let title = meta["title"] as? String ?? "Unknown"
            let artist = meta["artist"] as? String ?? "Unknown"
            let artworkUrl = meta["artwork"] as? String
            // The item's real length replaces this once it's ready to play.
            let duration = (meta["duration"] as? Int).map { Double($0) / 1000.0 } ?? 0
            let elapsed = Double(startPosition) / 1000.0

            nowPlayingManager?.updateNowPlaying(
                title: title,
                artist: artist,
                artworkUrl: artworkUrl,
                duration: duration,
                playbackRate: player?.rate ?? 0,
                elapsedTime: elapsed
            )
        } else {
            NSLog("[PinepodsAudioPlayer] WARNING: No metadata provided for Now Playing")
        }

        // Seek to start position if needed. An exact seek into a remote MP3 can
        // mean reading far into the file; a second early is fine for a resume.
        if startPosition > 0 {
            let time = CMTime(value: CMTimeValue(startPosition), timescale: 1000)
            let before = isLocal ? CMTime.zero : CMTime(seconds: 1, preferredTimescale: 1000)
            player?.seek(to: time, toleranceBefore: before, toleranceAfter: .zero) { [weak self] finished in
                DispatchQueue.main.async {
                    guard let self = self else { return }
                    self.playbackIntent.completeLoad(generation: generation)
                    if finished && self.playbackIntent.canStart(generation: generation) {
                        self.resumeRequestedPlayback()
                    }
                }
            }
        } else {
            playbackIntent.completeLoad(generation: generation)
            resumeRequestedPlayback()
        }
    }

    func play() {
        // A failed (or cleared) item can't resume; rebuild it from the last
        // request at the last known position instead of silently doing nothing.
        if let request = lastRequest, !loadInFlight,
           player?.currentItem == nil || player?.currentItem?.status == .failed {
            NSLog("[PinepodsAudioPlayer] play: current item unusable, reloading")
            playEpisode(url: request.url, startPosition: lastKnownPositionMs,
                        isLocal: request.isLocal, metadata: currentMetadata)
            return
        }
        playbackIntent.play()
        resumeRequestedPlayback()
    }

    private func resumeRequestedPlayback() {
        guard playbackIntent.canResumeLoadedEpisode else { return }
        player?.play()
        // AVPlayer.play() resets rate to 1.0, including lock-screen resume.
        if playbackIntent.rate != 1 { player?.rate = playbackIntent.rate }
        // Slight delay to allow rate to update before refreshing Now Playing
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { [weak self] in
            self?.updateNowPlayingPlaybackInfo()
        }
    }

    func pause() {
        playbackIntent.pause()
        NSLog("[PinepodsAudioPlayer] pause")
        player?.pause()
        // Update Now Playing immediately with rate 0 to indicate paused state
        let currentPosition = Double(getCurrentPosition()) / 1000.0
        nowPlayingManager?.updatePlaybackInfo(elapsedTime: currentPosition, playbackRate: 0.0)
    }

    func stop() {
        playbackIntent.stop()
        lastRequest = nil
        loadInFlight = false
        NSLog("[PinepodsAudioPlayer] stop")
        player?.pause()
        player?.replaceCurrentItem(with: nil)
        nowPlayingManager?.clearNowPlaying()
    }

    func seek(toMilliseconds milliseconds: Int) {
        NSLog("[PinepodsAudioPlayer] seek to \(milliseconds)ms")
        let time = CMTime(value: CMTimeValue(milliseconds), timescale: 1000)
        player?.seek(to: time, toleranceBefore: .zero, toleranceAfter: .zero) { [weak self] _ in
            DispatchQueue.main.async { self?.updateNowPlayingPlaybackInfo() }
        }
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

    /// Player output volume (0-1), used to fade out at the end of a sleep timer.
    func setVolume(_ volume: Float) {
        player?.volume = max(0, min(1, volume))
    }

    func setPlaybackSpeed(_ speed: Float) {
        NSLog("[PinepodsAudioPlayer] setPlaybackSpeed: \(speed)")
        playbackIntent.setRate(speed)
        // Changing a preference while paused must not start playback.
        if playbackIntent.canPlay, let player = player, player.rate != 0 {
            player.rate = playbackIntent.rate
        }
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
        setSkipRanges(segments.sorted { $0.start < $1.start })
    }

    private func setSkipRanges(_ ranges: [(start: Double, end: Double)]) {
        skipSegments = ranges
        skipSeekInFlight = false
        lastSkippedIndex = nil
        abandonedSkipIndices.removeAll()

        if let observer = skipBoundaryObserver {
            player?.removeTimeObserver(observer)
            skipBoundaryObserver = nil
        }
        guard let player = player, !ranges.isEmpty else { return }
        let times = ranges.map {
            NSValue(time: CMTime(seconds: $0.start + 0.05, preferredTimescale: CMTimeScale(NSEC_PER_SEC)))
        }
        skipBoundaryObserver = player.addBoundaryTimeObserver(forTimes: times, queue: .main) { [weak self] in
            guard let self = self, let now = self.player?.currentTime(), now.isValid else { return }
            self.applySilenceSkipIfNeeded(at: now.seconds)
        }
    }

    /// Called from the 1s periodic observer: if the playhead is inside a skip
    /// range, seek to its end. A small tolerance avoids seeking when we're
    /// essentially already past the range.
    ///
    /// Waits until the episode has loaded (including its start-position seek) and
    /// is ready: an episode that opens on an ad would otherwise seek while still
    /// loading, cancelling the start seek and leaving playback stuck.
    private func applySilenceSkipIfNeeded(at seconds: Double) {
        guard skipSilenceEnabled, !skipSegments.isEmpty, seconds.isFinite, !skipSeekInFlight,
              !loadInFlight, playbackIntent.isLoaded,
              let player = player, let item = player.currentItem, item.status == .readyToPlay else { return }
        let duration = item.duration.seconds
        guard duration.isFinite, duration > 0 else { return }

        guard let index = skipSegments.indices.first(where: {
            !abandonedSkipIndices.contains($0)
                && seconds >= skipSegments[$0].start && seconds < skipSegments[$0].end - 0.25
        }) else { return }

        // Back inside the range just skipped: the seek didn't take (or the user
        // went back to hear it). Leave this range alone rather than loop.
        let now = Date().timeIntervalSince1970
        if index == lastSkippedIndex && now - lastSkipTime < 5 {
            abandonedSkipIndices.insert(index)
            return
        }
        lastSkippedIndex = index
        lastSkipTime = now

        let end = min(skipSegments[index].end, duration)
        let target = CMTime(seconds: end, preferredTimescale: CMTimeScale(NSEC_PER_SEC))
        skipSeekInFlight = true
        player.seek(to: target, toleranceBefore: .zero, toleranceAfter: .zero) { [weak self] _ in
            DispatchQueue.main.async { self?.skipSeekInFlight = false }
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

    private func sendPlaybackStateEvent(updateNowPlaying: Bool = true) {
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

        // The lock screen extrapolates elapsed time from the rate, so it only
        // needs refreshing on state changes, not every tick.
        if updateNowPlaying { updateNowPlayingPlaybackInfo() }
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
        // A regular event (not a FlutterError) so the Dart side leaves the
        // buffering state instead of only logging a stream error.
        sendEvent(["type": "error", "code": code, "message": message])
    }
}
