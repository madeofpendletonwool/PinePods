import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/entities/persistable.dart';

void main() {
  group('empty', () {
    test('produces a not-yet-played placeholder', () {
      final persistable = Persistable.empty();

      expect(persistable.pguid, '');
      expect(persistable.episodeId, 0);
      expect(persistable.position, 0);
      expect(persistable.state, LastState.none);
    });
  });

  group('toMap/fromMap', () {
    test('round-trips pguid, episodeId, and position', () {
      final original = Persistable(
        pguid: 'podcast-guid',
        episodeId: 42,
        position: 120,
        state: LastState.paused,
        lastUpdated: DateTime.fromMillisecondsSinceEpoch(1700000000000),
      );

      final restored = Persistable.fromMap(original.toMap());

      expect(restored.pguid, 'podcast-guid');
      expect(restored.episodeId, 42);
      expect(restored.position, 120);
      expect(restored.lastUpdated, original.lastUpdated);
    });

    test('every playback state round-trips through its stored string', () {
      for (final state in LastState.values) {
        final original = Persistable(pguid: 'g', episodeId: 1, position: 0, state: state);
        final restored = Persistable.fromMap(original.toMap());

        expect(restored.state, state, reason: 'failed for $state');
      }
    });
  });
}
