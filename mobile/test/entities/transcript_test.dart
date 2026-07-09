import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/entities/transcript.dart';

void main() {
  group('TranscriptUrl', () {
    test('upgrades an http url to https on construction', () {
      final url = TranscriptUrl(url: 'http://example.com/t.json', type: TranscriptFormat.json);
      expect(url.url, 'https://example.com/t.json');
    });

    test('every format round-trips through its stored numeric code', () {
      for (final format in TranscriptFormat.values) {
        final original = TranscriptUrl(url: 'https://example.com/t', type: format);
        final restored = TranscriptUrl.fromMap(original.toMap());

        expect(restored.type, format, reason: 'failed for $format');
      }
    });

    test('round-trips url, language, and rel', () {
      final original = TranscriptUrl(
        url: 'https://example.com/t.srt',
        type: TranscriptFormat.subrip,
        language: 'en',
        rel: 'captions',
      );

      final restored = TranscriptUrl.fromMap(original.toMap());

      expect(restored.url, original.url);
      expect(restored.language, 'en');
      expect(restored.rel, 'captions');
    });

    test('equality is based on url, type, language, and rel', () {
      final a = TranscriptUrl(url: 'https://example.com/t', type: TranscriptFormat.json, language: 'en');
      final b = TranscriptUrl(url: 'https://example.com/t', type: TranscriptFormat.json, language: 'en');

      expect(a, equals(b));
      expect(a.hashCode, b.hashCode);
    });
  });

  group('Transcript', () {
    test('transcriptAvailable is false with no subtitles and not filtered', () {
      final transcript = Transcript();
      expect(transcript.transcriptAvailable, isFalse);
    });

    test('transcriptAvailable is true once there are subtitles', () {
      final transcript = Transcript(subtitles: [Subtitle(index: 0, start: Duration.zero)]);
      expect(transcript.transcriptAvailable, isTrue);
    });

    test('transcriptAvailable is true when explicitly marked filtered, even with no subtitles', () {
      final transcript = Transcript(filtered: true);
      expect(transcript.transcriptAvailable, isTrue);
    });

    test('toMap/fromMap round-trips guid and subtitles', () {
      final original = Transcript(
        guid: 'guid-1',
        subtitles: [
          Subtitle(index: 0, start: Duration.zero, end: const Duration(seconds: 5), speaker: 'Alice', data: 'Hello'),
        ],
      );

      final restored = Transcript.fromMap(3, original.toMap());

      expect(restored.id, 3);
      expect(restored.guid, 'guid-1');
      expect(restored.subtitles, hasLength(1));
      expect(restored.subtitles.first.speaker, 'Alice');
      expect(restored.subtitles.first.data, 'Hello');
    });
  });

  group('Subtitle', () {
    test('round-trips index, start/end, speaker, and data', () {
      final original = Subtitle(
        index: 2,
        start: const Duration(seconds: 10),
        end: const Duration(seconds: 15),
        speaker: 'Bob',
        data: 'Some line',
      );

      final restored = Subtitle.fromMap(original.toMap());

      expect(restored.index, 2);
      expect(restored.start, const Duration(seconds: 10));
      expect(restored.end, const Duration(seconds: 15));
      expect(restored.speaker, 'Bob');
      expect(restored.data, 'Some line');
    });

    test('equality is based on index, timing, speaker, and data', () {
      final a = Subtitle(index: 0, start: Duration.zero, end: const Duration(seconds: 1));
      final b = Subtitle(index: 0, start: Duration.zero, end: const Duration(seconds: 1));

      expect(a, equals(b));
      expect(a.hashCode, b.hashCode);
    });
  });
}
