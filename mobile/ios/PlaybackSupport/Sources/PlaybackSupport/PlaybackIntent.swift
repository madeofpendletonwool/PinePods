struct PlaybackIntent {
    private(set) var rate: Float = 1
    private(set) var generation: UInt64 = 0
    private var requested = false
    private var interrupted = false
    private var loadedGeneration: UInt64?

    var canPlay: Bool { requested && !interrupted }
    var canResumeLoadedEpisode: Bool { canPlay && loadedGeneration == generation }
    /// The current episode finished loading (including its start-position seek).
    var isLoaded: Bool { loadedGeneration == generation }

    mutating func completeLoad(generation: UInt64) {
        guard self.generation == generation else { return }
        loadedGeneration = generation
    }

    mutating func beginEpisode() -> UInt64 {
        generation &+= 1
        requested = true
        return generation
    }

    mutating func play() { requested = true }
    mutating func pause() { requested = false }

    mutating func stop() {
        requested = false
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

    func canStart(generation: UInt64) -> Bool {
        self.generation == generation && canPlay
    }
}
