// Recovery behaviour of NativeAudioPlayerService once the native player has
// failed: the UI must land in (and stay in) the error state, and play() must
// reload the episode rather than resume an item native can no longer play.

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mocktail/mocktail.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/services/audio/native_audio_player_service.dart';
import 'package:pinepods_mobile/services/settings/settings_service.dart';

import '../../support/factories.dart';
import '../../support/in_memory_repository.dart';

class _MockSettings extends Mock implements SettingsService {}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  final messenger = TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;

  late List<String> calls;
  late StreamController<Object?> nativeEvents;
  late NativeAudioPlayerService service;

  setUp(() async {
    calls = [];
    nativeEvents = StreamController<Object?>.broadcast();

    messenger.setMockMethodCallHandler(NativeAudioPlayerService.platform, (call) async {
      calls.add(call.method);
      if (call.method == 'getPosition' || call.method == 'getDuration') return 0;
      return null;
    });
    messenger.setMockStreamHandler(
      NativeAudioPlayerService.eventChannel,
      MockStreamHandler.inline(onListen: (args, sink) {
        nativeEvents.stream.listen((e) {
          if (e is PlatformException) {
            sink.error(code: e.code, message: e.message);
          } else {
            sink.success(e);
          }
        });
      }),
    );

    final settings = _MockSettings();
    when(() => settings.settingsListener).thenAnswer((_) => const Stream.empty());
    when(() => settings.playbackSpeed).thenReturn(1.0);
    when(() => settings.trimSilence).thenReturn(false);
    when(() => settings.volumeBoost).thenReturn(false);
    when(() => settings.fastForwardInterval).thenReturn(30);
    when(() => settings.rewindInterval).thenReturn(10);
    when(() => settings.pinepodsUserId).thenReturn(null);

    service = NativeAudioPlayerService(repository: newInMemoryRepository(), settingsService: settings);
    await pumpEventQueue();
  });

  tearDown(() async {
    messenger.setMockMethodCallHandler(NativeAudioPlayerService.platform, null);
    messenger.setMockStreamHandler(NativeAudioPlayerService.eventChannel, null);
    await nativeEvents.close();
  });

  Future<void> startAndFail() async {
    await service.playEpisode(episode: buildEpisode(), resume: false);
    nativeEvents.add({'type': 'error', 'code': -1100, 'message': 'boom'});
    await pumpEventQueue();
  }

  test('an error event moves playback to the error state', () async {
    await startAndFail();
    expect(service.playingState.value, AudioState.error);
  });

  test('a paused report after the error does not mask it', () async {
    await startAndFail();
    nativeEvents.add({'type': 'playbackState', 'state': 'paused', 'position': 0, 'duration': 0});
    await pumpEventQueue();
    expect(service.playingState.value, AudioState.error);
  });

  test('play() after an error reloads the episode instead of resuming', () async {
    await startAndFail();
    calls.clear();

    await service.play();

    expect(calls, contains('playEpisode'));
    expect(calls, isNot(contains('play')));
  });

  test('play() while paused just resumes', () async {
    await service.playEpisode(episode: buildEpisode(), resume: false);
    nativeEvents.add({'type': 'playbackState', 'state': 'paused', 'position': 1000, 'duration': 60000});
    await pumpEventQueue();
    calls.clear();

    await service.play();

    expect(calls, contains('play'));
    expect(calls, isNot(contains('playEpisode')));
  });

  test('a stream-level error still reaches the error state', () async {
    await service.playEpisode(episode: buildEpisode(), resume: false);
    nativeEvents.add(PlatformException(code: '-3', message: 'Failed to load audio'));
    await pumpEventQueue();
    expect(service.playingState.value, AudioState.error);
  });
}
