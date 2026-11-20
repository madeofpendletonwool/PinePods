// What the audio service records while an episode plays: nothing for the
// start, a report when playback pauses from anywhere, a report for a seek,
// the end through the outbox, and no positions that aren't the user's while a
// new episode is still loading.

import 'package:flutter_test/flutter_test.dart';
import 'package:mocktail/mocktail.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/app_settings.dart';
import 'package:pinepods_mobile/entities/pending_action.dart';
import 'package:pinepods_mobile/repository/sembast/sembast_repository.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/services/offline/offline_action_queue.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_audio_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/settings/settings_service.dart';
import 'package:rxdart/rxdart.dart';

import '../../support/factories.dart';
import '../../support/fakes.dart';
import '../../support/in_memory_repository.dart';

class MockPinepodsService extends Mock implements PinepodsService {}

class MockSettingsBloc extends Mock implements SettingsBloc {}

// pinepodsServer stays null, so the outbox never sends: what was recorded
// stays pending and can be read back.
class MockSettingsService extends Mock implements SettingsService {}

void main() {
  late MockAudioPlayerService audio;
  late MockPinepodsService api;
  late SembastRepository repository;
  late OfflineActionQueue queue;
  late PinepodsAudioService service;
  late BehaviorSubject<PositionState> positions;
  late BehaviorSubject<AudioState> states;

  setUpAll(registerCommonFallbacks);

  setUp(() {
    audio = MockAudioPlayerService();
    api = MockPinepodsService();
    final settingsBloc = MockSettingsBloc();
    positions = BehaviorSubject<PositionState>();
    states = BehaviorSubject<AudioState>.seeded(AudioState.none);
    when(() => audio.playPosition).thenAnswer((_) => positions.stream);
    when(() => audio.playingState).thenAnswer((_) => states.stream);
    when(() => audio.playbackSpeed).thenReturn(null);

    repository = newInMemoryRepository();
    queue = OfflineActionQueue(repository: repository, pinepodsService: api, settingsService: MockSettingsService());
    service = PinepodsAudioService(audio, api, settingsBloc)..setActionQueue(queue);
    addTearDown(() {
      service.dispose();
      positions.close();
      states.close();
    });

    when(() => settingsBloc.currentSettings).thenReturn(AppSettings.sensibleDefaults().copyWith(
          pinepodsUserId: 42,
          pinepodsServer: 'https://pods.example.com',
          pinepodsApiKey: 'key',
        ));
    when(() => api.setCredentials(any(), any())).thenReturn(null);
    when(() => audio.findDownloadedEpisode(any())).thenAnswer((_) async => null);
    when(() => audio.playEpisode(
          episode: any(named: 'episode'),
          resume: any(named: 'resume'),
          playbackSpeed: any(named: 'playbackSpeed'),
        )).thenAnswer((_) async {});
    when(() => audio.setPlaybackSpeed(any())).thenAnswer((_) async {});
    when(() => api.getPodcastIdFromEpisode(any(), any(), any())).thenAnswer((_) async => 7);
    when(() => api.fetchPodcasting2Data(any(), any())).thenAnswer((_) async => null);
    when(() => api.getPlayEpisodeDetails(any(), any(), any())).thenAnswer(
      (_) async => PlayEpisodeDetails(playbackSpeed: 1.0, startSkip: 0, endSkip: 0),
    );
  });

  /// The player reports [seconds] into a 600s episode.
  Future<void> position(double seconds) async {
    positions.add(PositionState(
      position: Duration(milliseconds: (seconds * 1000).round()),
      length: const Duration(seconds: 600),
      percentage: 0,
      episode: null,
      buffering: false,
      buffered: Duration.zero,
    ));
    await pumpEventQueue();
  }

  Future<void> state(AudioState s) async {
    states.add(s);
    await pumpEventQueue();
  }

  /// Start episode 101 resuming at [at] seconds and let the player get there.
  Future<void> start(int at, {int episodeId = 101}) async {
    when(() => api.getEpisodeMetadata(any(), any(), isYoutube: any(named: 'isYoutube')))
        .thenAnswer((_) async => buildPinepodsEpisode(episodeId: episodeId, listenDuration: at));
    await service.playPinepodsEpisode(pinepodsEpisode: buildPinepodsEpisode(episodeId: episodeId, listenDuration: at));
    await state(AudioState.playing);
    await position(at.toDouble());
  }

  Future<PendingAction?> recorded([int episodeId = 101]) => queue.pendingProgress(episodeId, false);

  test('starting an episode records nothing', () async {
    await start(300);
    await position(301);

    expect(await recorded(), isNull);
  });

  test('a pause from outside the app is recorded the moment it happens', () async {
    await start(300);
    await position(304);

    // Headset, lock screen, a call: the app only sees the state change.
    await state(AudioState.pausing);

    expect((await recorded())!.position, 304);
  });

  test('nothing more is recorded while paused', () async {
    await start(300);
    await position(304);
    await state(AudioState.pausing);
    final at = (await recorded())!.observedAt;

    await position(304);
    await position(304);

    expect((await recorded())!.observedAt, at);
  });

  test('going back is recorded', () async {
    await start(300);
    await position(301);

    await position(20);

    expect((await recorded())!.position, 20);
  });

  test('buffering is not a pause and not listening', () async {
    await start(300);
    await position(301);

    await state(AudioState.buffering);
    await state(AudioState.playing);

    // The stall is reported as where playback stopped, with no time invented for it.
    final entry = (await recorded())!;
    expect(entry.position, 301);
    expect(entry.listened, lessThan(2));
  });

  test('positions seen before the new episode reaches its start are not recorded', () async {
    await start(300);
    await position(310);
    await state(AudioState.pausing);

    // Episode 202 is asked to resume at 50. The player still shows 310 from the
    // old episode, then 0 while the new one loads.
    when(() => api.getEpisodeMetadata(any(), any(), isYoutube: any(named: 'isYoutube')))
        .thenAnswer((_) async => buildPinepodsEpisode(episodeId: 202, listenDuration: 50));
    await service.playPinepodsEpisode(pinepodsEpisode: buildPinepodsEpisode(episodeId: 202, listenDuration: 50));
    await state(AudioState.buffering);
    await position(310);
    await position(0);
    await state(AudioState.playing);
    await position(0);
    await state(AudioState.pausing);

    expect(await recorded(202), isNull, reason: 'neither 310 nor 0 is where the user is in episode 202');
    expect((await recorded())!.position, 310, reason: 'the old episode keeps where it was left');

    // Once it gets to its position it is followed like any other.
    await state(AudioState.playing);
    await position(50);
    await position(53);
    await state(AudioState.pausing);
    expect((await recorded(202))!.position, 53);
  });

  test('switching episodes records where the first was left', () async {
    await start(300);
    await position(308);

    when(() => api.getEpisodeMetadata(any(), any(), isYoutube: any(named: 'isYoutube')))
        .thenAnswer((_) async => buildPinepodsEpisode(episodeId: 202, listenDuration: 0));
    await service.playPinepodsEpisode(pinepodsEpisode: buildPinepodsEpisode(episodeId: 202));

    expect((await recorded())!.position, 308);
  });

  test('the end of an episode goes through the outbox with its position', () async {
    await start(590);
    await position(598);

    expect(service.markCurrentEpisodeCompleted(), isTrue);
    await pumpEventQueue();

    final entry = (await recorded())!;
    expect(entry.finished, isTrue);
    expect(entry.position, 600);
    verifyNever(() => api.markEpisodeCompleted(any(), any(), any()));
  });

  test('saving on demand records the latest position once', () async {
    await start(300);
    await position(306);

    await service.syncCurrentPositionToServer();
    final first = (await recorded())!;
    await service.syncCurrentPositionToServer();

    expect(first.position, 306);
    expect((await recorded())!.observedAt, first.observedAt, reason: 'nothing new to say');
  });
}
