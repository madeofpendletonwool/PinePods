import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/entities/chapter.dart';

void main() {
  group('construction', () {
    test('upgrades http image/url to https', () {
      final chapter = Chapter(
        title: 'Intro',
        imageUrl: 'http://example.com/chapter.png',
        url: 'http://example.com',
        startTime: 0,
      );

      expect(chapter.imageUrl, 'https://example.com/chapter.png');
      expect(chapter.url, 'https://example.com');
    });
  });

  group('toMap/fromMap', () {
    test('round-trips title, times, and the table-of-contents flag', () {
      final original = Chapter(
        title: 'Chapter One',
        imageUrl: 'https://example.com/art.png',
        url: 'https://example.com',
        startTime: 30.5,
        endTime: 120.0,
        toc: false,
      );

      final restored = Chapter.fromMap(original.toMap());

      expect(restored.title, original.title);
      expect(restored.imageUrl, original.imageUrl);
      expect(restored.url, original.url);
      expect(restored.startTime, original.startTime);
      expect(restored.endTime, original.endTime);
      expect(restored.toc, isFalse);
    });

    test('toc defaults to true unless the stored value is exactly "false"', () {
      final restored = Chapter.fromMap({'title': 'T', 'imageUrl': null, 'startTime': '0'});
      expect(restored.toc, isTrue);
    });
  });

  group('equality', () {
    test('chapters with the same title and startTime are equal', () {
      final a = Chapter(title: 'Intro', imageUrl: null, startTime: 0);
      final b = Chapter(title: 'Intro', imageUrl: null, startTime: 0);

      expect(a, equals(b));
      expect(a.hashCode, b.hashCode);
    });

    test('chapters with a different startTime are not equal', () {
      final a = Chapter(title: 'Intro', imageUrl: null, startTime: 0);
      final b = Chapter(title: 'Intro', imageUrl: null, startTime: 30);

      expect(a, isNot(equals(b)));
    });
  });
}
