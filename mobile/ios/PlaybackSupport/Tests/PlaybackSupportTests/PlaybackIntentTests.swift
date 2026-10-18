import XCTest
@testable import PlaybackSupport

final class PlaybackIntentTests: XCTestCase {
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

    func testInvalidRatesDoNotReplaceUserPreference() {
        var intent = PlaybackIntent()
        intent.setRate(2)
        for invalid: Float in [0, -1, .infinity, .nan] {
            intent.setRate(invalid)
            XCTAssertEqual(intent.rate, 2)
        }
    }
}
