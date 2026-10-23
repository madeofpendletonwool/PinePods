import 'dart:async';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mocktail/mocktail.dart';
import 'package:pinepods_mobile/repository/repository.dart';
import 'package:pinepods_mobile/services/audio/native_audio_player_service.dart';
import 'package:pinepods_mobile/services/settings/settings_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_audio_service.dart';
import '../../support/factories.dart';

class MockRepository extends Mock implements Repository {}

class MockSettings extends Mock implements SettingsService {}

class MockPinepodsAudio extends Mock implements PinepodsAudioService {}

void main() {
  for (final (resume, advancesDuringLookup) in [
    (false, false),
    (true, false),
    (true, true),
  ]) {
    testWidgets('resume=$resume preserves newer progress=$advancesDuringLookup', (
      tester,
    ) async {
      final repository = MockRepository();
      final settings = MockSettings();
      final episode = buildEpisode(guid: 'pinepods_101', position: 120000);
      when(() => repository.loadQueue()).thenAnswer((_) async => []);
      when(
        () => repository.saveEpisode(episode),
      ).thenAnswer((_) async => episode);
      when(
        () => settings.settingsListener,
      ).thenAnswer((_) => const Stream.empty());
      when(() => settings.playbackSpeed).thenReturn(1.75);
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
      if (advancesDuringLookup) {
        final pinepods = MockPinepodsAudio();
        when(
          () => pinepods.getServerPositionForEpisode(101, 0, false),
        ).thenAnswer((_) async {
          // A local position save can finish while the server lookup is pending.
          episode.position = 150000;
          return 100;
        });
        service.setPinepodsAudioService(pinepods);
      }
      await service.playEpisode(episode: episode, resume: resume);
      final play = calls.singleWhere((c) => c.method == 'playEpisode');
      final expected = advancesDuringLookup ? 150000 : (resume ? 120000 : 0);
      expect(play.arguments['startPosition'], expected);
      expect(episode.position, expected);
      service.setPinepodsAudioService(null);
      await service.stop();
      await tester.pump(const Duration(milliseconds: 1));
    });
  }
}
