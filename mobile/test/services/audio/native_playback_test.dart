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
  for (final resume in [false, true]) {
    testWidgets('resume=$resume starts at the episode position only when resuming', (
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
      await service.playEpisode(episode: episode, resume: resume);
      final play = calls.singleWhere((c) => c.method == 'playEpisode');
      final expected = resume ? 120000 : 0;
      expect(play.arguments['startPosition'], expected);
      expect(episode.position, expected);
      await service.stop();
      await tester.pump(const Duration(milliseconds: 1));
    });
  }

  testWidgets('stop keeps the position from before native clears the item', (tester) async {
    final repository = MockRepository();
    final settings = MockSettings();
    final episode = buildEpisode(guid: 'pinepods_101', position: 120000);
    when(() => repository.loadQueue()).thenAnswer((_) async => []);
    when(() => repository.saveEpisode(episode)).thenAnswer((_) async => episode);
    when(() => settings.settingsListener).thenAnswer((_) => const Stream.empty());
    when(() => settings.playbackSpeed).thenReturn(1.0);
    when(() => settings.trimSilence).thenReturn(false);
    when(() => settings.volumeBoost).thenReturn(false);
    when(() => settings.fastForwardInterval).thenReturn(30);
    when(() => settings.rewindInterval).thenReturn(10);
    var stopped = false;
    final messenger = TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(NativeAudioPlayerService.platform, (call) async {
      if (call.method == 'stop') stopped = true;
      if (call.method == 'getPosition') return stopped ? 0 : 90000;
      return null;
    });
    const events = MethodChannel('com.pinepods/audio_events');
    messenger.setMockMethodCallHandler(events, (_) async => null);
    addTearDown(() {
      messenger.setMockMethodCallHandler(NativeAudioPlayerService.platform, null);
      messenger.setMockMethodCallHandler(events, null);
    });
    final service = NativeAudioPlayerService(repository: repository, settingsService: settings);
    await tester.pump(const Duration(milliseconds: 1));

    await service.playEpisode(episode: episode);
    await service.stop();

    expect(episode.position, 90000);
    expect(service.playPosition.value.position, const Duration(milliseconds: 90000));
    await tester.pump(const Duration(milliseconds: 1));
  });
}
