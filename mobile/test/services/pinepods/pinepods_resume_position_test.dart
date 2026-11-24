// Where playback resumes. The episode data a screen hands to
// playPinepodsEpisode can be stale (loaded before the last listen), so the
// server is asked; progress made on this device that the server doesn't have
// yet comes before both. Starting an episode never reports a position.

import 'package:flutter_test/flutter_test.dart';
import 'package:mocktail/mocktail.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/app_settings.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/repository/sembast/sembast_repository.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/services/offline/offline_action_queue.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_audio_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/pinepods/progress_tracker.dart';
import 'package:pinepods_mobile/services/settings/settings_service.dart';

import '../../support/factories.dart';
import '../../support/fakes.dart';
import '../../support/in_memory_repository.dart';

class MockPinepodsService extends Mock implements PinepodsService {}

class MockSettingsBloc extends Mock implements SettingsBloc {}

// pinepodsServer stays null, so the outbox never tries to send: everything
// enqueued remains pending and observable.
class MockSettingsService extends Mock implements SettingsService {}

void main() {
  late MockAudioPlayerService audio;
  late MockPinepodsService api;
  late SembastRepository repository;
  late OfflineActionQueue queue;
  late PinepodsAudioService service;

  setUpAll(registerCommonFallbacks);

  setUp(() {
    audio = MockAudioPlayerService();
    api = MockPinepodsService();
    final settingsBloc = MockSettingsBloc();
    repository = newInMemoryRepository();
    queue = OfflineActionQueue(
      repository: repository,
      pinepodsService: api,
      settingsService: MockSettingsService(),
    );
    service = PinepodsAudioService(audio, api, settingsBloc)..setActionQueue(queue);
    addTearDown(service.dispose);

    when(() => settingsBloc.currentSettings)
        .thenReturn(AppSettings.sensibleDefaults().copyWith(pinepodsUserId: 42));
    when(() => audio.findDownloadedEpisode(any())).thenAnswer((_) async => null);
    when(() => audio.playEpisode(
          episode: any(named: 'episode'),
          resume: any(named: 'resume'),
          playbackSpeed: any(named: 'playbackSpeed'),
        ))
        .thenAnswer((_) async {});
    when(() => audio.setPlaybackSpeed(any())).thenAnswer((_) async {});
    when(() => api.getPodcastIdFromEpisode(any(), any(), any())).thenAnswer((_) async => 7);
    when(() => api.fetchPodcasting2Data(any(), any())).thenAnswer((_) async => null);
    when(() => api.getPlayEpisodeDetails(any(), any(), any())).thenAnswer(
      (_) async => PlayEpisodeDetails(playbackSpeed: 1.0, startSkip: 0, endSkip: 0),
    );
  });

  void serverPosition(int? seconds) {
    when(() => api.getEpisodeMetadata(any(), any(), isYoutube: any(named: 'isYoutube')))
        .thenAnswer((_) async => buildPinepodsEpisode(listenDuration: seconds));
  }

  Episode playedEpisode() => verify(
        () => audio.playEpisode(
          episode: captureAny(named: 'episode'),
          resume: true,
          playbackSpeed: any(named: 'playbackSpeed'),
        ),
      ).captured.single as Episode;

  test('resumes from the server when the list data is stale', () async {
    serverPosition(300);

    await service.playPinepodsEpisode(pinepodsEpisode: buildPinepodsEpisode(listenDuration: 100));

    expect(playedEpisode().position, 300000);
    expect(await queue.pendingProgress(101, false), isNull, reason: 'starting is not progress');
  });

  test('resumes behind the list data when the user went back on another device', () async {
    serverPosition(40);

    await service.playPinepodsEpisode(pinepodsEpisode: buildPinepodsEpisode(listenDuration: 500));

    expect(playedEpisode().position, 40000);
  });

  test('resumes from unsynced progress, even when it is behind the server', () async {
    serverPosition(500);
    await queue.enqueuePosition(101, 42, 90, false);

    await service.playPinepodsEpisode(pinepodsEpisode: buildPinepodsEpisode(listenDuration: 100));

    expect(playedEpisode().position, 90000);
    expect(await queue.pendingPosition(101, false), 90, reason: 'still waiting to be sent');
  });

  test('local download resumes from unsynced progress without asking the server', () async {
    when(() => audio.findDownloadedEpisode(any())).thenAnswer(
      (_) async => buildEpisode()..filepath = '/tmp',
    );
    await queue.enqueuePosition(101, 42, 400, false);

    await service.playPinepodsEpisode(pinepodsEpisode: buildPinepodsEpisode(listenDuration: 100));

    expect(playedEpisode().position, 400000);
    verifyNever(() => api.getEpisodeMetadata(any(), any(), isYoutube: any(named: 'isYoutube')));
  });

  test('falls back to the list data when the server cannot be asked', () async {
    when(() => api.getEpisodeMetadata(any(), any(), isYoutube: any(named: 'isYoutube')))
        .thenThrow(Exception('offline'));

    await service.playPinepodsEpisode(pinepodsEpisode: buildPinepodsEpisode(listenDuration: 100));

    expect(playedEpisode().position, 100000);
  });

  test('an episode left at its end starts over', () async {
    serverPosition(600); // the whole 600s: finished

    await service.playPinepodsEpisode(
      pinepodsEpisode: buildPinepodsEpisode(listenDuration: 600, completed: true),
    );

    expect(playedEpisode().position, 0);
  });

  test('a finished episode being replayed resumes inside the replay', () async {
    serverPosition(250);

    await service.playPinepodsEpisode(
      pinepodsEpisode: buildPinepodsEpisode(listenDuration: 600, completed: true),
    );

    expect(playedEpisode().position, 250000);
  });

  test('an episode finished offline and not yet sent starts over', () async {
    serverPosition(100);
    await queue.enqueueProgress(ProgressUpdate(
      episodeId: 101,
      userId: 42,
      isYoutube: false,
      position: 600,
      at: DateTime.now(),
      finished: true,
    ));

    await service.playPinepodsEpisode(pinepodsEpisode: buildPinepodsEpisode(listenDuration: 100));

    expect(playedEpisode().position, 0);
  });

  test('pending progress is per episode and per kind', () async {
    await queue.enqueuePosition(101, 42, 50, true);
    await queue.enqueuePosition(202, 42, 70, false);

    expect(await queue.pendingPosition(101, false), isNull);
    expect(await queue.pendingPosition(101, true), 50);
  });
}
