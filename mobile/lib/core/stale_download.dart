import 'package:pinepods_mobile/entities/downloadable.dart';
import 'package:pinepods_mobile/entities/episode.dart';

/// Pure helpers for repairing download records that point at a file no
/// longer present on disk. Extracted from NativeAudioPlayerService so the
/// record-matching logic can be unit tested without any
/// file-system/platform-channel/repository scaffolding.

/// Every episode in [allEpisodes] that's marked downloaded and points at the
/// same (now-missing) [filepath]/[filename]. There can be more than one
/// matching record: legacy 'pinepods_<id>_<timestamp>' guids mean the same
/// download can have duplicate rows (see LocalDownloadUtils.deleteLocalDownload,
/// which handles the same situation).
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

/// Resets a single episode to "not downloaded" in place.
void clearDownloadState(Episode episode) {
  episode.downloadState = DownloadState.none;
  episode.downloadPercentage = 0;
  episode.filepath = null;
  episode.filename = null;
}
