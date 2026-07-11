// Regression tests for two Episode bugs:
//  1. A null lastUpdated used to serialize as '' and then throw a
//     FormatException on the way back in (int.parse('')).
//  2. hashCode included id/lastUpdated (and hashed chapters/persons by list
//     identity) while == did not, violating the equals/hashCode contract.

import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/entities/chapter.dart';
import 'package:pinepods_mobile/entities/episode.dart';

Episode _episode({
  int? id,
  DateTime? lastUpdated,
  List<Chapter> chapters = const <Chapter>[],
}) {
  return Episode(
    id: id,
    guid: 'guid-1',
    pguid: 'podcast-guid',
    podcast: 'Some Podcast',
    title: 'Some Episode',
    publicationDate: DateTime.fromMillisecondsSinceEpoch(1700000000000),
    lastUpdated: lastUpdated,
    chapters: chapters,
  );
}

void main() {
  group('lastUpdated round-trip (bug 1)', () {
    test('an episode with a null lastUpdated survives toMap -> fromMap', () {
      final map = _episode(lastUpdated: null).toMap();

      // Previously int.parse('') threw here.
      final restored = Episode.fromMap(1, map);

      expect(restored.lastUpdated, isNotNull);
    });

    test('a legacy empty-string lastUpdated is treated as absent, not parsed', () {
      final map = _episode(lastUpdated: DateTime(2024)).toMap();
      map['lastUpdated'] = ''; // simulate data written by the old toMap

      expect(() => Episode.fromMap(1, map), returnsNormally);
    });

    test('a concrete lastUpdated still round-trips exactly', () {
      final when = DateTime.fromMillisecondsSinceEpoch(1700000005000);
      final restored = Episode.fromMap(1, _episode(lastUpdated: when).toMap());
      expect(restored.lastUpdated, when);
    });

    test('an empty-string publicationDate is also handled defensively', () {
      final map = _episode().toMap();
      map['publicationDate'] = '';
      expect(() => Episode.fromMap(1, map), returnsNormally);
    });
  });

  group('equals/hashCode contract (bug 2)', () {
    test('episodes equal by == have equal hashCodes despite differing id', () {
      final a = _episode(id: 1, lastUpdated: DateTime(2024, 1, 1));
      final b = _episode(id: 2, lastUpdated: DateTime(2024, 1, 1));

      expect(a, equals(b));
      expect(a.hashCode, b.hashCode);
    });

    test('episodes equal by == have equal hashCodes despite differing lastUpdated', () {
      final a = _episode(id: 1, lastUpdated: DateTime(2024, 1, 1));
      final b = _episode(id: 1, lastUpdated: DateTime(2025, 6, 30));

      expect(a, equals(b));
      expect(a.hashCode, b.hashCode);
    });

    test('an equal episode is found in a HashSet keyed on Episode', () {
      final stored = _episode(id: 1, lastUpdated: DateTime(2024, 1, 1));
      final lookup = _episode(id: 99, lastUpdated: DateTime(2030, 1, 1));

      final set = {stored};
      expect(set.contains(lookup), isTrue);
    });

    test('equal chapter *content* in distinct list instances still hashes equally', () {
      final a = _episode(chapters: [Chapter(title: 'Intro', imageUrl: null, startTime: 0)]);
      final b = _episode(chapters: [Chapter(title: 'Intro', imageUrl: null, startTime: 0)]);

      expect(a, equals(b));
      expect(a.hashCode, b.hashCode);
    });
  });
}
