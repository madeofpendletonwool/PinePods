/// Resolves what an episode's progress display should show: live playback
/// position when it's the episode actually playing, otherwise a static
/// snapshot value (e.g. from a REST response fetched once on page load).
///
/// Extracted as a pure, framework-independent class (no widget/Provider/audio
/// stream dependencies) so the resolution logic can be unit tested directly.
class LiveProgressResolver {
  /// Progress percentage (0-100) to display.
  static double percentage({
    required bool isCurrentEpisode,
    required double staticPercentage,
    required int? livePercentage,
  }) {
    if (isCurrentEpisode && livePercentage != null) {
      return livePercentage.toDouble();
    }
    return staticPercentage;
  }

  /// Elapsed-time text to display, or null if there's nothing to show.
  static String? elapsedText({
    required bool isCurrentEpisode,
    required String? staticText,
    required Duration? livePosition,
  }) {
    if (isCurrentEpisode && livePosition != null) {
      return formatDuration(livePosition);
    }
    return staticText;
  }

  /// Whether a progress bar should be shown at all: either there's a static
  /// snapshot showing the episode as started, or it's actively playing right
  /// now even if the snapshot hadn't recorded that yet (e.g. just started
  /// from 0 via auto-advance or a play button on the same card).
  static bool shouldShowProgress({
    required bool isCurrentEpisode,
    required bool hasStaticProgress,
  }) {
    return isCurrentEpisode || hasStaticProgress;
  }
}

/// Formats [duration] as MM:SS, or HH:MM:SS once it reaches an hour.
String formatDuration(Duration duration) {
  final totalSeconds = duration.inSeconds;
  final hours = totalSeconds ~/ 3600;
  final minutes = (totalSeconds % 3600) ~/ 60;
  final seconds = totalSeconds % 60;
  if (hours > 0) {
    return '${hours.toString().padLeft(2, '0')}:${minutes.toString().padLeft(2, '0')}:${seconds.toString().padLeft(2, '0')}';
  }
  return '${minutes.toString().padLeft(2, '0')}:${seconds.toString().padLeft(2, '0')}';
}
