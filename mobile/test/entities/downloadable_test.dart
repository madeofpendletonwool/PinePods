import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/entities/downloadable.dart';

void main() {
  group('toMap/fromMap', () {
    test('round-trips every field', () {
      final original = Downloadable(
        guid: 'guid-1',
        url: 'https://example.com/episode.mp3',
        directory: '/storage/podcast',
        filename: 'episode.mp3',
        taskId: 'task-1',
        state: DownloadState.downloading,
        percentage: 42,
      );

      final restored = Downloadable.fromMap(original.toMap());

      expect(restored.guid, original.guid);
      expect(restored.url, original.url);
      expect(restored.directory, original.directory);
      expect(restored.filename, original.filename);
      expect(restored.taskId, original.taskId);
      expect(restored.state, original.state);
      expect(restored.percentage, original.percentage);
    });

    test('maps every DownloadState index round-trip', () {
      for (final state in DownloadState.values) {
        final original = Downloadable(
          guid: 'g',
          url: 'u',
          directory: 'd',
          filename: 'f',
          taskId: 't',
          state: state,
          percentage: 0,
        );

        final restored = Downloadable.fromMap(original.toMap());

        expect(restored.state, state, reason: 'failed for $state');
      }
    });
  });

  group('equality', () {
    test('two downloadables with the same guid are equal regardless of other fields', () {
      final a = Downloadable(guid: 'g', url: 'u1', directory: 'd1', filename: 'f1', taskId: 't1', state: DownloadState.none);
      final b = Downloadable(guid: 'g', url: 'u2', directory: 'd2', filename: 'f2', taskId: 't2', state: DownloadState.downloaded);

      expect(a, equals(b));
      expect(a.hashCode, b.hashCode);
    });

    test('downloadables with different guids are not equal', () {
      final a = Downloadable(guid: 'g1', url: 'u', directory: 'd', filename: 'f', taskId: 't', state: DownloadState.none);
      final b = Downloadable(guid: 'g2', url: 'u', directory: 'd', filename: 'f', taskId: 't', state: DownloadState.none);

      expect(a, isNot(equals(b)));
    });
  });
}
