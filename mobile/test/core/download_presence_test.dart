// Unit tests for the shared download-presence helpers used by both the
// playback path (NativeAudioPlayerService, which repairs a record the
// moment a missing file is discovered while trying to play it) and the UI
// path (LocalDownloadUtils, which proactively checks presence for the
// "Downloaded" badge). These are pure functions over plain Episode objects,
// so no file-system/repository/platform-channel scaffolding is needed to
// test them.

import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/core/download_presence.dart';
import 'package:pinepods_mobile/entities/downloadable.dart';
import 'package:pinepods_mobile/entities/episode.dart';

Episode _episode({
  required String guid,
  DownloadState downloadState = DownloadState.downloaded,
  String? filepath = '/storage/PinePods/some-podcast',
  String? filename = 'episode.mp3',
}) {
  return Episode(
    guid: guid,
    podcast: 'Some Podcast',
    downloadState: downloadState,
    filepath: filepath,
    filename: filename,
    downloadPercentage: downloadState == DownloadState.downloaded ? 100 : 0,
  );
}

void main() {
  group('findStaleDownloadRecords', () {
    test('matches a downloaded episode pointing at the given filepath/filename', () {
      final target = _episode(guid: 'pinepods_101');
      final other = _episode(guid: 'pinepods_202', filepath: '/other', filename: 'other.mp3');

      final result = findStaleDownloadRecords(
        [target, other],
        filepath: '/storage/PinePods/some-podcast',
        filename: 'episode.mp3',
      );

      expect(result, [target]);
    });

    test('matches every duplicate/legacy-guid record for the same dead file', () {
      final first = _episode(guid: 'pinepods_101');
      final legacyDuplicate = _episode(guid: 'pinepods_101_1699999999');

      final result = findStaleDownloadRecords(
        [first, legacyDuplicate],
        filepath: '/storage/PinePods/some-podcast',
        filename: 'episode.mp3',
      );

      expect(result, containsAll([first, legacyDuplicate]));
      expect(result, hasLength(2));
    });

    test('ignores episodes that are not marked downloaded even with a matching path', () {
      final notDownloaded = _episode(guid: 'pinepods_101', downloadState: DownloadState.none);

      final result = findStaleDownloadRecords(
        [notDownloaded],
        filepath: '/storage/PinePods/some-podcast',
        filename: 'episode.mp3',
      );

      expect(result, isEmpty);
    });

    test('returns nothing when filepath or filename is null rather than mass-matching', () {
      final withNullPath = _episode(guid: 'pinepods_101', filepath: null, filename: null);

      final result = findStaleDownloadRecords(
        [withNullPath],
        filepath: null,
        filename: null,
      );

      expect(result, isEmpty);
    });
  });

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

  group('clearDownloadState', () {
    test('resets downloadState, percentage, filepath, and filename', () {
      final episode = _episode(guid: 'pinepods_101');

      clearDownloadState(episode);

      expect(episode.downloadState, DownloadState.none);
      expect(episode.downloadPercentage, 0);
      expect(episode.filepath, isNull);
      expect(episode.filename, isNull);
    });
  });
}
