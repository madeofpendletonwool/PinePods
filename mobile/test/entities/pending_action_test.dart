import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/entities/pending_action.dart';

void main() {
  group('toMap/fromMap', () {
    test('round-trips every field', () {
      // createdAt is stored as whole milliseconds, so use a value with no
      // sub-millisecond component - DateTime.now() has microsecond
      // resolution and would make an exact round-trip comparison flaky.
      final createdAt = DateTime(2024, 1, 1, 12, 0, 0);
      final original = PendingAction(
        type: PendingActionType.recordPosition,
        episodeId: 101,
        userId: 42,
        isYoutube: true,
        payload: const {'position': 123.0},
        createdAt: createdAt,
        retryCount: 2,
      );

      final restored = PendingAction.fromMap(9, original.toMap());

      expect(restored.id, 9);
      expect(restored.type, PendingActionType.recordPosition);
      expect(restored.episodeId, 101);
      expect(restored.userId, 42);
      expect(restored.isYoutube, isTrue);
      expect(restored.payload, {'position': 123.0});
      expect(restored.retryCount, 2);
      expect(restored.createdAt, createdAt);
    });

    test('every action type round-trips through its stored name', () {
      for (final type in PendingActionType.values) {
        final original = PendingAction(type: type, episodeId: 1, userId: 1);
        final restored = PendingAction.fromMap(1, original.toMap());

        expect(restored.type, type, reason: 'failed for $type');
      }
    });

    test('an unrecognized stored type name falls back to recordPosition', () {
      final restored = PendingAction.fromMap(1, {'type': 'somethingUnknown', 'episodeId': 1, 'userId': 1});
      expect(restored.type, PendingActionType.recordPosition);
    });

    test('missing episodeId/userId default to 0 rather than throwing', () {
      final restored = PendingAction.fromMap(1, const {'type': 'saveEpisode'});
      expect(restored.episodeId, 0);
      expect(restored.userId, 0);
      expect(restored.retryCount, 0);
    });
  });

  group('position', () {
    test('reads the numeric position out of the payload', () {
      final action = PendingAction(
        type: PendingActionType.recordPosition,
        episodeId: 1,
        userId: 1,
        payload: const {'position': 45.5},
      );

      expect(action.position, 45.5);
    });

    test('is null when the payload has no position entry', () {
      final action = PendingAction(type: PendingActionType.markCompleted, episodeId: 1, userId: 1);
      expect(action.position, isNull);
    });
  });

  group('description', () {
    test('describes a recordPosition action with its position when known', () {
      final action = PendingAction(
        type: PendingActionType.recordPosition,
        episodeId: 1,
        userId: 1,
        payload: const {'position': 30.0},
      );

      expect(action.description, 'Save progress (30s)');
    });

    test('describes a recordPosition action generically when no position is set', () {
      final action = PendingAction(type: PendingActionType.recordPosition, episodeId: 1, userId: 1);
      expect(action.description, 'Save progress');
    });

    test('has a human-readable description for every action type', () {
      final expected = {
        PendingActionType.markCompleted: 'Mark completed',
        PendingActionType.markUncompleted: 'Mark not completed',
        PendingActionType.saveEpisode: 'Save episode',
        PendingActionType.removeSaved: 'Remove saved episode',
        PendingActionType.queue: 'Add to queue',
        PendingActionType.addHistory: 'Add to history',
      };

      expected.forEach((type, label) {
        final action = PendingAction(type: type, episodeId: 1, userId: 1);
        expect(action.description, label, reason: 'failed for $type');
      });
    });
  });
}
