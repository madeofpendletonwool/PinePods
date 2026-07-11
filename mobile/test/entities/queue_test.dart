import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/entities/queue.dart';

void main() {
  group('toMap/fromMap', () {
    test('round-trips the list of guids', () {
      final original = Queue(guids: ['guid-1', 'guid-2', 'guid-3']);

      final restored = Queue.fromMap(1, original.toMap());

      expect(restored.guids, original.guids);
    });

    test('an empty queue round-trips to an empty list', () {
      final original = Queue(guids: []);

      final restored = Queue.fromMap(1, original.toMap());

      expect(restored.guids, isEmpty);
    });

    test('a missing "q" key defaults to an empty list rather than throwing', () {
      final restored = Queue.fromMap(1, const {});
      expect(restored.guids, isEmpty);
    });
  });
}
