import 'package:pinepods_mobile/entities/downloadable.dart';
import 'package:pinepods_mobile/entities/episode.dart';

/// Pure decision logic for whether a set of repository rows matching a given
/// episode actually represents a usable local download, extracted so it can
/// be unit tested without any file-system I/O.
///
/// There can be more than one matching row (legacy 'pinepods_<id>_<timestamp>'
/// guids mean the same download can have duplicate rows - see
/// LocalDownloadUtils.deleteLocalDownload, which already handles this).
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
void resetDownloadState(Episode episode) {
  episode.downloadState = DownloadState.none;
  episode.downloadPercentage = 0;
  episode.filepath = null;
  episode.filename = null;
}
