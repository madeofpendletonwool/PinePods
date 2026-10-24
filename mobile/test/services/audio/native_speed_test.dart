import 'dart:async';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mocktail/mocktail.dart';
import 'package:pinepods_mobile/repository/repository.dart';
import 'package:pinepods_mobile/services/audio/native_audio_player_service.dart';
import 'package:pinepods_mobile/services/settings/settings_service.dart';
import '../../support/factories.dart';

class MockRepository extends Mock implements Repository {}

class MockSettings extends Mock implements SettingsService {}

void main() {
  for (final persist in [false, true]) {
    testWidgets('speed persists only when requested: $persist', (tester) async {
      final repository = MockRepository();
      final settings = MockSettings();
      final episode = buildEpisode(guid: 'pinepods_101', position: 120000);
      when(
        () => repository.saveEpisode(episode),
      ).thenAnswer((_) async => episode);
      when(
        () => settings.settingsListener,
      ).thenAnswer((_) => const Stream.empty());
      var preferredSpeed = 1.75;
      when(() => settings.playbackSpeed).thenAnswer((_) => preferredSpeed);
      when(() => settings.playbackSpeed = any()).thenAnswer((invocation) {
        preferredSpeed = invocation.positionalArguments.single as double;
        return preferredSpeed;
      });
      when(() => settings.trimSilence).thenReturn(false);
      when(() => settings.volumeBoost).thenReturn(false);
      when(() => settings.fastForwardInterval).thenReturn(30);
      when(() => settings.rewindInterval).thenReturn(10);
      final calls = <MethodCall>[];
      final messenger =
          TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
      messenger.setMockMethodCallHandler(NativeAudioPlayerService.platform, (
        call,
      ) async {
        calls.add(call);
        if (call.method == 'getPosition') return 0;
        return null;
      });
      const events = MethodChannel('com.pinepods/audio_events');
      messenger.setMockMethodCallHandler(events, (_) async => null);
      addTearDown(() {
        messenger.setMockMethodCallHandler(
          NativeAudioPlayerService.platform,
          null,
        );
        messenger.setMockMethodCallHandler(events, null);
      });
      final service = NativeAudioPlayerService(
        repository: repository,
        settingsService: settings,
      );
      await tester.pump(const Duration(milliseconds: 1));
      await service.setPlaybackSpeed(2.25, persist: persist);
      expect(preferredSpeed, persist ? 2.25 : 1.75);
      expect(
        calls
            .lastWhere((c) => c.method == 'setPlaybackSpeed')
            .arguments['speed'],
        2.25,
      );
      // A new episode starts at the saved default, carried on the play call.
      await service.playEpisode(episode: episode);
      expect(
        calls.lastWhere((c) => c.method == 'playEpisode').arguments['speed'],
        persist ? 2.25 : 1.75,
      );
      // A session override applies to that episode without being saved.
      await service.playEpisode(episode: episode, playbackSpeed: 3.0);
      expect(
        calls.lastWhere((c) => c.method == 'playEpisode').arguments['speed'],
        3.0,
      );
      expect(preferredSpeed, persist ? 2.25 : 1.75);
      await service.stop();
      await tester.pump(const Duration(milliseconds: 1));
    });
  }
}
