// When listening progress is reported: every 30s while playing, at once on a
// pause or a seek, never while paused, and how long was actually played.

import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/services/pinepods/progress_tracker.dart';

final t0 = DateTime.utc(2026, 11, 12, 9);

PlaybackSample at(double position, {bool playing = true, double rate = 1.0, int episodeId = 7, bool isYoutube = false}) =>
    PlaybackSample(
      episodeId: episodeId,
      userId: 42,
      isYoutube: isYoutube,
      position: position,
      duration: 1800,
      playing: playing,
      rate: rate,
    );

/// Play from [from] for [seconds], looking once a second.
List<ProgressUpdate> play(ProgressTracker tracker, DateTime start, double from, int seconds, {double rate = 1.0}) => [
      for (var s = 1; s <= seconds; s++)
        ...tracker.observe(at(from + s * rate, rate: rate), start.add(Duration(seconds: s))),
    ];

void main() {
  late ProgressTracker tracker;

  setUp(() => tracker = ProgressTracker());

  test('a playing episode is reported every half minute, with the time played', () {
    tracker.begin(at(100), t0);

    final reports = play(tracker, t0, 100, 65);

    expect(reports.map((u) => u.position), [130, 160]);
    expect(reports.map((u) => u.listened), [30, 30]);
    expect(reports.every((u) => u.duration == 1800 && !u.finished && u.userId == 42), isTrue);
    expect(reports.first.at, t0.add(const Duration(seconds: 30)));
  });

  test('a pause is reported at once and nothing is reported while paused', () {
    tracker.begin(at(100), t0);

    final onPause = tracker.observe(at(105, playing: false), t0.add(const Duration(seconds: 5)));

    expect(onPause.single.position, 105);
    expect(onPause.single.listened, 5);
    for (var minute = 1; minute <= 180; minute++) {
      expect(tracker.observe(at(105, playing: false), t0.add(Duration(seconds: 5, minutes: minute))), isEmpty);
    }
  });

  test('time spent paused is not time played', () {
    tracker.begin(at(100), t0);
    tracker.observe(at(105, playing: false), t0.add(const Duration(seconds: 5)));
    final resumed = t0.add(const Duration(hours: 3));
    tracker.observe(at(105), resumed);

    final later = tracker.observe(at(106, playing: false), resumed.add(const Duration(seconds: 1)));

    expect(later.single.listened, 1);
  });

  test('a seek is reported in either direction, playing or paused', () {
    tracker.begin(at(600), t0);

    final back = tracker.observe(at(30), t0.add(const Duration(seconds: 1)));
    final ahead = tracker.observe(at(900), t0.add(const Duration(seconds: 2)));
    tracker.observe(at(901, playing: false), t0.add(const Duration(seconds: 3)));
    final scrubbed = tracker.observe(at(120, playing: false), t0.add(const Duration(seconds: 4)));

    expect(back.single.position, 30);
    expect(ahead.single.position, 900);
    expect(scrubbed.single.position, 120);
    expect(scrubbed.single.listened, 0);
  });

  test('faster playback is not a seek and counts real time', () {
    tracker.begin(at(0, rate: 2), t0);

    final reports = play(tracker, t0, 0, 30, rate: 2);

    expect(reports.single.position, 60);
    expect(reports.single.listened, 30);
  });

  test('time the app spent asleep is not listening', () {
    tracker.begin(at(100), t0);

    final woke = tracker.observe(at(101), t0.add(const Duration(hours: 1)));

    expect(woke.every((u) => u.listened == 0), isTrue);
  });

  test('another episode reports where the old one was left', () {
    tracker.begin(at(100), t0);
    play(tracker, t0, 100, 10);

    final reports = tracker.observe(at(0, episodeId: 8), t0.add(const Duration(seconds: 11)));

    expect(reports.single.episodeId, 7);
    expect(reports.single.position, 110);
    expect(reports.single.listened, 10);
    // The new episode starts with a clean count.
    final next = tracker.observe(at(4, episodeId: 8, playing: false), t0.add(const Duration(seconds: 15)));
    expect(next.single.episodeId, 8);
    expect(next.single.listened, 4);
  });

  test('a video with the same id is a different episode', () {
    tracker.begin(at(100), t0);
    play(tracker, t0, 100, 3);

    final reports = tracker.observe(at(0, isYoutube: true), t0.add(const Duration(seconds: 4)));

    expect(reports.single.isYoutube, isFalse);
  });

  test('finishing reports the end, once', () {
    expect(tracker.finish(t0), isNull, reason: 'nothing is being followed');
    tracker.begin(at(1790), t0);
    play(tracker, t0, 1790, 8);

    final done = tracker.finish(t0.add(const Duration(seconds: 10)))!;

    expect(done.position, 1800);
    expect(done.finished, isTrue);
    expect(done.listened, 8);
    expect(tracker.observe(at(1800, playing: false), t0.add(const Duration(seconds: 11))), isEmpty);
  });

  test('leaving reports only what has not been reported', () {
    expect(tracker.leave(t0), isNull);
    tracker.begin(at(100), t0);
    expect(tracker.leave(t0), isNull, reason: 'starting is not progress');
    play(tracker, t0, 100, 8);

    final left = tracker.leave(t0.add(const Duration(seconds: 8)))!;

    expect(left.position, 108);
    expect(left.listened, 8);
    expect(tracker.leave(t0.add(const Duration(seconds: 9))), isNull);
  });

  test('reset stops following', () {
    tracker.begin(at(100), t0);
    play(tracker, t0, 100, 5);
    tracker.reset();

    expect(tracker.isTracking, isFalse);
    expect(tracker.leave(t0.add(const Duration(seconds: 6))), isNull);
  });

  test('an episode resumes where it was left unless that is the end', () {
    expect(resumeFrom(640, 1800), 640);
    expect(resumeFrom(1800, 1800), 0);
    expect(resumeFrom(1799, 1800), 0);
    expect(resumeFrom(1700, 1800), 1700);
    expect(resumeFrom(640, 0), 640, reason: 'length unknown');
    expect(resumeFrom(-3, 1800), 0);
    expect(resumeFrom(double.nan, 1800), 0);
  });
}
