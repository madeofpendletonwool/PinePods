// lib/core/download_cleanup.dart
//
// Decision logic for automatically removing device downloads, kept free of
// Flutter/IO so it can be unit tested. DownloadCleanupService gathers the
// inputs and acts on the result.

/// How long an episode has to stay finished before its download is removed,
/// so an accidental "mark complete" can be undone. Matches the server.
const Duration finishedGracePeriod = Duration(hours: 1);

/// "Delete after" choices shared by the device and server download settings.
const List<int> deleteAfterDayChoices = [0, 1, 3, 7, 14, 30, 60, 90];

String deleteAfterDaysLabel(int d) => d == 0 ? 'Never' : '$d day${d == 1 ? '' : 's'}';

enum CleanupReason { finished, expired }

/// The rules that apply to one podcast: the user's defaults, with any
/// per-podcast override layered on top.
class CleanupRules {
  final bool deleteWhenFinished;

  /// 0 = never.
  final int deleteAfterDays;

  const CleanupRules({required this.deleteWhenFinished, required this.deleteAfterDays});

  bool get anyEnabled => deleteWhenFinished || deleteAfterDays > 0;

  CleanupRules withOverrides({bool? deleteWhenFinished, int? deleteAfterDays}) => CleanupRules(
        deleteWhenFinished: deleteWhenFinished ?? this.deleteWhenFinished,
        deleteAfterDays: deleteAfterDays ?? this.deleteAfterDays,
      );
}

/// Why a download should be removed now, or null to keep it.
///
/// [finishedSince] is when the episode was first seen finished (null when it
/// isn't). [exemptFromFinished] covers an episode that was already finished
/// when it was downloaded — a deliberate re-listen.
CleanupReason? cleanupReason({
  required CleanupRules rules,
  required DateTime now,
  required DateTime downloadedAt,
  DateTime? finishedSince,
  bool exemptFromFinished = false,
}) {
  if (rules.deleteWhenFinished &&
      !exemptFromFinished &&
      finishedSince != null &&
      now.difference(finishedSince) >= finishedGracePeriod) {
    return CleanupReason.finished;
  }
  if (rules.deleteAfterDays > 0 && now.difference(downloadedAt) >= Duration(days: rules.deleteAfterDays)) {
    return CleanupReason.expired;
  }
  return null;
}
