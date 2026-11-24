import XCTest
@testable import PlaybackSupport

final class PlaybackIntentTests: XCTestCase {
    func testPlayDuringLoadCannotResumeThePreviousEpisode() {
        var intent = PlaybackIntent()
        let first = intent.beginEpisode()
        intent.completeLoad(generation: first)
        XCTAssertTrue(intent.canResumeLoadedEpisode)
        let second = intent.beginEpisode()
        intent.play()
        XCTAssertFalse(intent.canResumeLoadedEpisode)
        intent.completeLoad(generation: first)
        XCTAssertFalse(intent.canResumeLoadedEpisode)
        intent.completeLoad(generation: second)
        XCTAssertTrue(intent.canResumeLoadedEpisode)
    }

    func testLoadedOnlyOnceTheCurrentEpisodeFinishesLoading() {
        var intent = PlaybackIntent()
        let first = intent.beginEpisode()
        XCTAssertFalse(intent.isLoaded)
        intent.completeLoad(generation: first)
        XCTAssertTrue(intent.isLoaded)
        intent.pause()
        XCTAssertTrue(intent.isLoaded)
        _ = intent.beginEpisode()
        intent.completeLoad(generation: first)
        XCTAssertFalse(intent.isLoaded)
    }

    func testLoadWhilePausedCanBeResumedExplicitly() {
        var intent = PlaybackIntent()
        let generation = intent.beginEpisode()
        intent.pause()
        intent.completeLoad(generation: generation)
        XCTAssertFalse(intent.canResumeLoadedEpisode)
        intent.play()
        XCTAssertTrue(intent.canResumeLoadedEpisode)
        intent.stop()
        intent.completeLoad(generation: generation)
        intent.play()
        XCTAssertFalse(intent.canResumeLoadedEpisode)
    }

    func testSpeedSurvivesPauseResumeAndEpisodeChanges() {
        var intent = PlaybackIntent()
        intent.setRate(1.75)
        XCTAssertFalse(intent.canPlay)
        _ = intent.beginEpisode()
        intent.pause()
        intent.play()
        _ = intent.beginEpisode()
        XCTAssertEqual(intent.rate, 1.75)
        XCTAssertTrue(intent.canPlay)
    }

    func testInterruptionResumesOnlyPreviouslyRequestedPlayback() {
        for playing in [false, true] {
            for shouldResume in [false, true] {
                var intent = PlaybackIntent()
                if playing { intent.play() }
                intent.interruptionBegan()
                XCTAssertFalse(intent.canPlay)
                XCTAssertEqual(intent.interruptionEnded(shouldResume: shouldResume), playing && shouldResume)
                XCTAssertEqual(intent.canPlay, playing && shouldResume)
            }
        }
    }

    func testExplicitPlayRecoversFromInterruptionThatNeverEnded() {
        var intent = PlaybackIntent()
        let generation = intent.beginEpisode()
        intent.completeLoad(generation: generation)
        intent.interruptionBegan()
        XCTAssertFalse(intent.canResumeLoadedEpisode)
        intent.play()
        XCTAssertTrue(intent.canResumeLoadedEpisode)

        intent.interruptionBegan()
        let next = intent.beginEpisode()
        intent.completeLoad(generation: next)
        XCTAssertTrue(intent.canResumeLoadedEpisode)
    }

    func testLoadThatNeverFinishesIsStalledAfterTimeout() {
        var intent = PlaybackIntent()
        XCTAssertFalse(intent.loadStalled(now: 1_000))
        let generation = intent.beginEpisode(now: 1_000)
        XCTAssertFalse(intent.loadStalled(now: 1_000 + PlaybackIntent.loadTimeout - 1))
        XCTAssertTrue(intent.loadStalled(now: 1_000 + PlaybackIntent.loadTimeout + 1))
        intent.completeLoad(generation: generation)
        XCTAssertFalse(intent.loadStalled(now: 5_000))
    }

    func testManualPauseDuringInterruptionCancelsAutomaticResume() {
        var intent = PlaybackIntent()
        intent.play()
        intent.interruptionBegan()
        intent.pause()
        XCTAssertFalse(intent.interruptionEnded(shouldResume: true))
    }

    func testLateLoadOrSeekCannotStartAfterPauseStopOrEpisodeChange() {
        var intent = PlaybackIntent()
        let first = intent.beginEpisode()
        intent.pause()
        XCTAssertFalse(intent.canStart(generation: first))
        intent.play()
        XCTAssertTrue(intent.canStart(generation: first))
        let second = intent.beginEpisode()
        XCTAssertFalse(intent.canStart(generation: first))
        XCTAssertTrue(intent.canStart(generation: second))
        intent.stop()
        intent.play()
        XCTAssertFalse(intent.canStart(generation: second))
    }

    func testTogglePausesWhileBufferingAfterPlayWasRequested() {
        var intent = PlaybackIntent()
        let generation = intent.beginEpisode()
        intent.completeLoad(generation: generation)
        // Stalled stream: AVPlayer is waiting, which still counts as active.
        XCTAssertTrue(intent.toggleShouldPause(transportActive: true, itemUsable: true))
        // Requested but AVPlayer momentarily reports paused (e.g. mid-seek).
        XCTAssertTrue(intent.toggleShouldPause(transportActive: false, itemUsable: true))
    }

    func testToggleResumesWhenPausedOrInterrupted() {
        var intent = PlaybackIntent()
        let generation = intent.beginEpisode()
        intent.completeLoad(generation: generation)
        intent.pause()
        XCTAssertFalse(intent.toggleShouldPause(transportActive: false, itemUsable: true))
        intent.play()
        intent.interruptionBegan()
        // iOS often never sends interruption-ended; the button must still resume.
        XCTAssertFalse(intent.toggleShouldPause(transportActive: false, itemUsable: true))
    }

    func testTogglePausesAudioThePlayerIsPlayingEvenWithoutARequest() {
        let intent = PlaybackIntent()
        XCTAssertTrue(intent.toggleShouldPause(transportActive: true, itemUsable: true))
    }

    func testToggleOnAnUnusableItemPlaysSoItReloads() {
        var intent = PlaybackIntent()
        _ = intent.beginEpisode()
        XCTAssertFalse(intent.toggleShouldPause(transportActive: true, itemUsable: false))
        XCTAssertFalse(intent.toggleShouldPause(transportActive: false, itemUsable: false))
    }

    func testInvalidRatesDoNotReplaceUserPreference() {
        var intent = PlaybackIntent()
        intent.setRate(2)
        for invalid: Float in [0, -1, .infinity, .nan] {
            intent.setRate(invalid)
            XCTAssertEqual(intent.rate, 2)
        }
    }
}
