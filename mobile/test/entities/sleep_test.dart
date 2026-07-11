import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/entities/sleep.dart';

void main() {
  group('equality', () {
    test('two sleeps with the same type, duration, and episode count are equal', () {
      final a = Sleep(type: SleepType.time, duration: const Duration(minutes: 30));
      final b = Sleep(type: SleepType.time, duration: const Duration(minutes: 30));

      expect(a, equals(b));
      expect(a.hashCode, b.hashCode);
    });

    test('sleeps with a different duration are not equal', () {
      final a = Sleep(type: SleepType.time, duration: const Duration(minutes: 30));
      final b = Sleep(type: SleepType.time, duration: const Duration(minutes: 45));

      expect(a, isNot(equals(b)));
    });
  });

  group('timeRemaining', () {
    test('is approximately the configured duration right after creation', () {
      final sleep = Sleep(type: SleepType.time, duration: const Duration(minutes: 10));

      final remaining = sleep.timeRemaining;

      expect(remaining.inSeconds, greaterThan(Duration(minutes: 10).inSeconds - 2));
      expect(remaining.inSeconds, lessThanOrEqualTo(Duration(minutes: 10).inSeconds));
    });

    test('a zero-duration sleep has (approximately) no time remaining', () {
      final sleep = Sleep(type: SleepType.none);

      expect(sleep.timeRemaining.inSeconds, 0);
    });
  });
}
