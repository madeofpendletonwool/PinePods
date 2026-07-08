// Unit tests for ActionGuard, extracted from episode_details.dart's button
// handlers so the re-entrant-tap guard logic can be tested without any
// widget/Provider scaffolding.

import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/ui/utils/action_guard.dart';

void main() {
  group('ActionGuard', () {
    test('starts out not in progress', () {
      final guard = ActionGuard();
      expect(guard.inProgress, isFalse);
    });

    test('flips to in progress as soon as run() starts, before the action completes', () async {
      final guard = ActionGuard();
      final completer = Completer<void>();
      var onChangeCalls = 0;

      final future = guard.run(() => completer.future, onChange: () => onChangeCalls++);

      expect(guard.inProgress, isTrue);
      expect(onChangeCalls, 1);

      completer.complete();
      await future;

      expect(guard.inProgress, isFalse);
      expect(onChangeCalls, 2);
    });

    test('a second call while one is in flight is dropped - the action never runs', () async {
      final guard = ActionGuard();
      final firstCompleter = Completer<void>();
      var secondActionRan = false;

      final firstRun = guard.run(() => firstCompleter.future, onChange: () {});
      // This is the "impatient repeat tap" scenario: fired while the first
      // action is still awaiting.
      await guard.run(() async {
        secondActionRan = true;
      }, onChange: () {});

      expect(secondActionRan, isFalse);

      firstCompleter.complete();
      await firstRun;
    });

    test('resets inProgress and rethrows when the action throws', () async {
      final guard = ActionGuard();

      await expectLater(
        guard.run(() async => throw Exception('network error'), onChange: () {}),
        throwsA(isException),
      );

      expect(guard.inProgress, isFalse);
    });

    test('a new action can run once the previous one has finished', () async {
      final guard = ActionGuard();
      var runCount = 0;

      await guard.run(() async => runCount++, onChange: () {});
      await guard.run(() async => runCount++, onChange: () {});

      expect(runCount, 2);
    });
  });
}
