import Foundation

struct PlaybackIntent {
    /// Seconds after which an episode load that never finished counts as stuck.
    static let loadTimeout: TimeInterval = 30

    private(set) var rate: Float = 1
    private(set) var generation: UInt64 = 0
    private var requested = false
    private var interrupted = false
    private var loadedGeneration: UInt64?
    private var loadStartedAt: TimeInterval?

    var canPlay: Bool { requested && !interrupted }
    var canResumeLoadedEpisode: Bool { canPlay && loadedGeneration == generation }
    /// The current episode finished loading (including its start-position seek).
    var isLoaded: Bool { loadedGeneration == generation }

    mutating func completeLoad(generation: UInt64) {
        guard self.generation == generation else { return }
        loadedGeneration = generation
    }

    // An explicit play or episode load clears any interruption: iOS often never
    // sends an interruption-ended notification, which would leave play dead.
    mutating func beginEpisode(now: TimeInterval = Date().timeIntervalSince1970) -> UInt64 {
        generation &+= 1
        loadStartedAt = now
        requested = true
        interrupted = false
        return generation
    }

    mutating func play() {
        requested = true
        interrupted = false
    }
    mutating func pause() { requested = false }

    mutating func stop() {
        requested = false
        loadStartedAt = nil
        generation &+= 1
    }

    mutating func setRate(_ value: Float) {
        guard value.isFinite, value > 0 else { return }
        rate = value
    }

    mutating func interruptionBegan() { interrupted = true }

    mutating func interruptionEnded(shouldResume: Bool) -> Bool {
        let resume = interrupted && requested && shouldResume
        interrupted = false
        requested = resume
        return resume
    }

    /// The current episode started loading long ago and never finished (its
    /// start seek never completed), so play would otherwise wait on it forever.
    func loadStalled(now: TimeInterval = Date().timeIntervalSince1970) -> Bool {
        guard let started = loadStartedAt, !isLoaded else { return false }
        return now - started > Self.loadTimeout
    }

    func canStart(generation: UInt64) -> Bool {
        self.generation == generation && canPlay
    }

    /// Whether a play/pause toggle (a headset's single button) should pause.
    ///
    /// Goes by what the user asked for, not by what AVPlayer reports at that
    /// instant: while a stream buffers or stalls AVPlayer says it is waiting, not
    /// playing, and a toggle that went by that resumed instead of pausing.
    /// `transportActive` is AVPlayer playing or waiting to play. An unusable item
    /// (none loaded, or failed) always plays, which reloads it.
    func toggleShouldPause(transportActive: Bool, itemUsable: Bool) -> Bool {
        itemUsable && (canPlay || transportActive)
    }
}
