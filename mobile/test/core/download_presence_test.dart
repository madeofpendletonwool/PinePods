// Unit tests for resolveDownloadPresence/resetDownloadState, extracted from
// local_download_utils.dart's fix for the "Downloaded" badge lying about
// episodes whose local file no longer exists on disk. These are pure
// functions over plain Episode objects and a pre-computed presence map, so
// no file-system/repository scaffolding is needed to test them.

import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/core/download_presence.dart';
import 'package:pinepods_mobile/entities/downloadable.dart';
import 'package:pinepods_mobile/entities/episode.dart';

Episode _episode({
  required String guid,
  DownloadState downloadState = DownloadState.downloaded,
}) {
  return Episode(
    guid: guid,
    podcast: 'Some Podcast',
    downloadState: downloadState,
    filepath: '/storage/PinePods/some-podcast',
    filename: '$guid.mp3',
    downloadPercentage: downloadState == DownloadState.downloaded ? 100 : 0,
  );
}

void main() {
  group('resolveDownloadPresence', () {
    test('is downloaded when the single matching row is present on disk', () {
      final row = _episode(guid: 'pinepods_101');

      final result = resolveDownloadPresence([row], {row: true});

      expect(result.isDownloaded, isTrue);
      expect(result.staleRecords, isEmpty);
    });

    test('is not downloaded and flags the row as stale when its file is missing', () {
      final row = _episode(guid: 'pinepods_101');

      final result = resolveDownloadPresence([row], {row: false});

      expect(result.isDownloaded, isFalse);
      expect(result.staleRecords, [row]);
    });

    test('is downloaded if any duplicate/legacy-guid row is present, even if another is stale', () {
      final missing = _episode(guid: 'pinepods_101');
      final present = _episode(guid: 'pinepods_101_1699999999');

      final result = resolveDownloadPresence(
        [missing, present],
        {missing: false, present: true},
      );

      expect(result.isDownloaded, isTrue);
      // Still worth healing the dead duplicate even though the episode
      // overall counts as downloaded via the other row.
      expect(result.staleRecords, [missing]);
    });

    test('ignores rows that are not marked downloaded', () {
      final notDownloaded = _episode(guid: 'pinepods_101', downloadState: DownloadState.none);

      final result = resolveDownloadPresence([notDownloaded], {});

      expect(result.isDownloaded, isFalse);
      expect(result.staleRecords, isEmpty);
    });

    test('treats a downloaded row missing from the presence map as stale', () {
      // Defensive case: a caller that forgot to check a downloaded row
      // should not silently count it as present.
      final row = _episode(guid: 'pinepods_101');

      final result = resolveDownloadPresence([row], {});

      expect(result.isDownloaded, isFalse);
      expect(result.staleRecords, [row]);
    });
  });

  group('resetDownloadState', () {
    test('resets downloadState, percentage, filepath, and filename', () {
      final episode = _episode(guid: 'pinepods_101');

      resetDownloadState(episode);

      expect(episode.downloadState, DownloadState.none);
      expect(episode.downloadPercentage, 0);
      expect(episode.filepath, isNull);
      expect(episode.filename, isNull);
    });
  });
}
