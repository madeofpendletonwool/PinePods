import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/core/download_cleanup.dart';

void main() {
  final now = DateTime(2026, 10, 23, 12);
  const off = CleanupRules(deleteWhenFinished: false, deleteAfterDays: 0);
  const finishedOnly = CleanupRules(deleteWhenFinished: true, deleteAfterDays: 0);
  const weekOnly = CleanupRules(deleteWhenFinished: false, deleteAfterDays: 7);

  CleanupReason? reason(CleanupRules rules,
          {Duration downloadedAgo = Duration.zero, Duration? finishedAgo, bool exempt = false}) =>
      cleanupReason(
        rules: rules,
        now: now,
        downloadedAt: now.subtract(downloadedAgo),
        finishedSince: finishedAgo == null ? null : now.subtract(finishedAgo),
        exemptFromFinished: exempt,
      );

  test('keeps everything when no rule is on', () {
    expect(reason(off, downloadedAgo: const Duration(days: 365), finishedAgo: const Duration(days: 30)), isNull);
  });

  test('finished downloads go after the grace period', () {
    expect(reason(finishedOnly, finishedAgo: const Duration(minutes: 59)), isNull);
    expect(reason(finishedOnly, finishedAgo: finishedGracePeriod), CleanupReason.finished);
    expect(reason(finishedOnly), isNull);
  });

  test('a re-listen download is not removed for being finished', () {
    expect(reason(finishedOnly, finishedAgo: const Duration(days: 1), exempt: true), isNull);
  });

  test('old downloads go after the configured days', () {
    expect(reason(weekOnly, downloadedAgo: const Duration(days: 6, hours: 23)), isNull);
    expect(reason(weekOnly, downloadedAgo: const Duration(days: 7)), CleanupReason.expired);
    // The age rule ignores the finished exemption.
    expect(reason(weekOnly, downloadedAgo: const Duration(days: 8), exempt: true), CleanupReason.expired);
  });

  test('per-podcast overrides replace only what they set', () {
    const defaults = CleanupRules(deleteWhenFinished: true, deleteAfterDays: 30);
    final rules = defaults.withOverrides(deleteWhenFinished: false);
    expect(rules.deleteWhenFinished, isFalse);
    expect(rules.deleteAfterDays, 30);
    expect(defaults.withOverrides(deleteAfterDays: 0).anyEnabled, isTrue);
    expect(defaults.withOverrides(deleteWhenFinished: false, deleteAfterDays: 0).anyEnabled, isFalse);
  });

  test('labels', () {
    expect(deleteAfterDaysLabel(0), 'Never');
    expect(deleteAfterDaysLabel(1), '1 day');
    expect(deleteAfterDaysLabel(14), '14 days');
  });
}
