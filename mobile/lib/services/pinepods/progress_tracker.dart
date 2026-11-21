// lib/services/pinepods/progress_tracker.dart

/// One episode's unreported progress.
class ProgressUpdate {
  final int episodeId;
  final int userId;
  final bool isYoutube;

  /// Seconds into the episode.
  final double position;

  /// When the position was observed, on this device's clock.
  final DateTime at;

  /// Seconds played since the last report.
  final double listened;

  /// The player's length for the episode, when it knows it.
  final double? duration;

  /// The episode played to its end.
  final bool finished;

  /// Of [listened]: episode seconds gained by playing faster than 1x
  /// (negative when slower).
  final double speedSaved;

  /// Seconds of silence and of ads the player skipped.
  final double silenceSkipped;
  final double adsSkipped;

  /// Played from a file on the device.
  final bool downloaded;

  const ProgressUpdate({
    required this.episodeId,
    required this.userId,
    required this.isYoutube,
    required this.position,
    required this.at,
    this.listened = 0,
    this.duration,
    this.finished = false,
    this.speedSaved = 0,
    this.silenceSkipped = 0,
    this.adsSkipped = 0,
    this.downloaded = false,
  });
}

/// A range the player skips on its own: silence, or an ad.
typedef SkipRange = ({double start, double end, bool ad});

/// What the player is doing at one moment.
class PlaybackSample {
  final int episodeId;
  final int userId;
  final bool isYoutube;
  final double position;
  final double duration;

  /// Audio is coming out (not paused, not buffering).
  final bool playing;
  final double rate;

  /// Playing a file on the device.
  final bool downloaded;

  const PlaybackSample({
    required this.episodeId,
    required this.userId,
    required this.isYoutube,
    required this.position,
    required this.duration,
    required this.playing,
    this.rate = 1.0,
    this.downloaded = false,
  });

  PlaybackSample _with({double? position, bool? playing}) => PlaybackSample(
        episodeId: episodeId,
        userId: userId,
        isYoutube: isYoutube,
        position: position ?? this.position,
        duration: duration,
        playing: playing ?? this.playing,
        rate: rate,
        downloaded: downloaded,
      );
}

/// Turns looks at the player into progress reports: every [reportEvery] while
/// playing, and whenever the position stops following the clock (a pause, a
/// seek, the end, another episode). Nothing is reported while paused.
class ProgressTracker {
  static const reportEvery = Duration(seconds: 30);

  /// The position moving this far from where the clock puts it is a seek.
  static const seekSeconds = 3.0;

  /// A longer gap between two looks means the app was asleep, not playing.
  static const maxTick = Duration(minutes: 2);

  PlaybackSample? _last;
  DateTime _lastTick = DateTime.fromMillisecondsSinceEpoch(0);
  DateTime _lastReport = DateTime.fromMillisecondsSinceEpoch(0);
  double _reportedPosition = 0;
  double _listened = 0;
  double _speedSaved = 0;
  double _silenceSkipped = 0;
  double _adsSkipped = 0;

  /// Ranges the player skips in the tracked episode. A forward jump that
  /// lands at the end of one is counted as skipped silence or ads.
  List<SkipRange> skipRanges = const [];

  /// How close to a range's end a jump has to land to be its skip.
  static const skipLanding = 1.5;

  /// The latest position seen, for callers that need one between ticks.
  double? get position => _last?.position;

  bool get isTracking => _last != null;

  ProgressUpdate _report(PlaybackSample sample, DateTime now, {bool finished = false}) {
    _lastReport = now;
    _reportedPosition = sample.position;
    final listened = _listened;
    final speedSaved = _speedSaved, silenceSkipped = _silenceSkipped, adsSkipped = _adsSkipped;
    _clearCounts();
    return ProgressUpdate(
      episodeId: sample.episodeId,
      userId: sample.userId,
      isYoutube: sample.isYoutube,
      position: sample.position,
      at: now,
      listened: listened,
      duration: sample.duration.isFinite && sample.duration >= 1 ? sample.duration : null,
      finished: finished,
      speedSaved: speedSaved,
      silenceSkipped: silenceSkipped,
      adsSkipped: adsSkipped,
      downloaded: sample.downloaded,
    );
  }

  void _clearCounts() {
    _listened = 0;
    _speedSaved = 0;
    _silenceSkipped = 0;
    _adsSkipped = 0;
  }

  /// A jump from [from] to [to] that is the player skipping a range.
  void _countSkip(double from, double to) {
    if (to <= from) return;
    for (final r in skipRanges) {
      if ((to - r.end).abs() <= skipLanding && from >= r.start - skipLanding && from < r.end) {
        if (r.ad) {
          _adsSkipped += r.end - from;
        } else {
          _silenceSkipped += r.end - from;
        }
        return;
      }
    }
  }

  bool _unreported(PlaybackSample sample) =>
      _listened >= 1 || _silenceSkipped + _adsSkipped >= 1 || (sample.position - _reportedPosition).abs() >= 1;

  /// Start following an episode from [sample]. Nothing is reported for the
  /// start itself: the starting position is what was last saved somewhere, not
  /// something observed here.
  void begin(PlaybackSample sample, DateTime now) {
    _last = sample;
    _lastTick = now;
    _lastReport = now;
    _reportedPosition = sample.position;
    _clearCounts();
  }

  /// Stop following (the episode was closed, or playback is untracked).
  void reset() {
    _last = null;
    _clearCounts();
    skipRanges = const [];
  }

  /// Look at the player. Returns what to report, usually nothing.
  List<ProgressUpdate> observe(PlaybackSample sample, DateTime now) {
    final previous = _last;
    if (previous == null || previous.episodeId != sample.episodeId || previous.isYoutube != sample.isYoutube) {
      final out = <ProgressUpdate>[if (previous != null && _unreported(previous)) _report(previous, now)];
      begin(sample, now);
      return out;
    }
    final elapsed = now.difference(_lastTick);
    final awake = !elapsed.isNegative && elapsed <= maxTick;
    var expected = previous.position;
    if (previous.playing && awake) {
      final seconds = elapsed.inMilliseconds / 1000.0;
      final rate = previous.rate > 0 ? previous.rate : 1.0;
      _listened += seconds;
      _speedSaved += seconds * (rate - 1);
      expected += seconds * rate;
    }
    final seeked = awake && (sample.position - expected).abs() > seekSeconds;
    // Skips of short silences move less than a seek does.
    if (awake && sample.position - expected > 0.3) _countSkip(expected, sample.position);
    final paused = previous.playing && !sample.playing;
    final due = sample.playing && now.difference(_lastReport) >= reportEvery;
    _last = sample;
    _lastTick = now;
    return [if (seeked || paused || due) _report(sample, now)];
  }

  /// The episode played to its end.
  ProgressUpdate? finish(DateTime now) {
    final last = _last;
    if (last == null) return null;
    final sample = last._with(
      position: last.duration.isFinite && last.duration > 0 ? last.duration : null,
      playing: false,
    );
    _last = sample;
    _lastTick = now;
    return _report(sample, now, finished: true);
  }

  /// Whatever hasn't been reported, for when the app may not get another
  /// chance (a pause, a stop, going to the background).
  ProgressUpdate? leave(DateTime now) {
    final last = _last;
    if (last == null || !_unreported(last)) return null;
    return _report(last, now);
  }
}

/// Where to start an episode saved at [position]: there, or from the top if
/// that is the end.
double resumeFrom(double position, double duration) {
  if (!position.isFinite || position <= 0) return 0;
  if (duration.isFinite && duration > 0 && position >= duration - 2) return 0;
  return position;
}
