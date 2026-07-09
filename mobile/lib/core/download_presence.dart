import 'package:pinepods_mobile/entities/downloadable.dart';
import 'package:pinepods_mobile/entities/episode.dart';

/// Pure helpers for repairing/checking download records against files that
/// may no longer be present on disk. Shared by the playback path
/// (NativeAudioPlayerService, which repairs a record the moment a missing
/// file is discovered while trying to play it) and the UI path
/// (LocalDownloadUtils, which proactively checks presence for the
/// "Downloaded" badge) so both call the same logic instead of duplicating it.
///
/// There can be more than one matching row per episode (legacy
/// 'pinepods_<id>_<timestamp>' guids mean the same download can have
/// duplicate rows - see LocalDownloadUtils.deleteLocalDownload, which already
/// handles this).

/// Every episode in [allEpisodes] that's marked downloaded and points at the
/// same (now-missing) [filepath]/[filename].
///
/// The episode actually asked to play is sometimes a transient playback
/// wrapper rather than the real repository row for its download (its guid is
/// the stream URL, not 'pinepods_<id>' - see the "transient playback record"
/// comment in PinepodsAudioService._convertToEpisode), so matching by
/// filepath/filename - which *is* copied through faithfully - finds the real
/// record(s) regardless of which object triggered the check.
List<Episode> findStaleDownloadRecords(
  List<Episode> allEpisodes, {
  required String? filepath,
  required String? filename,
}) {
  if (filepath == null || filename == null) return const [];

  return allEpisodes
      .where((e) =>
          e.downloadState == DownloadState.downloaded &&
          e.filepath == filepath &&
          e.filename == filename)
      .toList();
}

/// Decision logic for whether a set of repository rows matching a given
/// episode actually represents a usable local download.
class DownloadPresenceResult {
  /// True if at least one matching row is marked downloaded *and* its file
  /// was confirmed present on disk.
  final bool isDownloaded;

  /// Rows marked downloaded whose file was *not* found - these should be
  /// reset so the "Downloaded" badge stops lying about them.
  final List<Episode> staleRecords;

  const DownloadPresenceResult({required this.isDownloaded, required this.staleRecords});
}

/// [fileExists] carries the already-checked (I/O happens elsewhere) result
/// for each of [matchingEpisodes] that's marked downloaded - callers only
/// need to have checked the ones worth checking.
DownloadPresenceResult resolveDownloadPresence(
  List<Episode> matchingEpisodes,
  Map<Episode, bool> fileExists,
) {
  final downloadedRows =
      matchingEpisodes.where((e) => e.downloaded || e.downloadState == DownloadState.downloaded).toList();

  final present = downloadedRows.where((e) => fileExists[e] == true).toList();
  final stale = downloadedRows.where((e) => fileExists[e] != true).toList();

  return DownloadPresenceResult(isDownloaded: present.isNotEmpty, staleRecords: stale);
}

/// Resets a single episode to "not downloaded" in place.
void clearDownloadState(Episode episode) {
  episode.downloadState = DownloadState.none;
  episode.downloadPercentage = 0;
  episode.filepath = null;
  episode.filename = null;
}
