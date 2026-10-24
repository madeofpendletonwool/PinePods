// Regression tests for where playback resumes. The episode data a screen hands
// to playPinepodsEpisode can be stale (loaded before the last listen), and the
// server can be behind when progress made offline is still in the outbox.
// Resume must take the furthest known position, and the play-start record must
// not replace newer unsynced progress in the outbox.

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
    expect(await queue.pendingPosition(101, false), 300);
  });

  test('resumes from unsynced offline progress and keeps it in the outbox', () async {
    serverPosition(100);
    await queue.enqueuePosition(101, 42, 900, false);

    await service.playPinepodsEpisode(pinepodsEpisode: buildPinepodsEpisode(listenDuration: 100));

    expect(playedEpisode().position, 900000);
    expect(await queue.pendingPosition(101, false), 900);
  });

  test('local download resumes from unsynced offline progress', () async {
    when(() => audio.findDownloadedEpisode(any())).thenAnswer(
      (_) async => buildEpisode()..filepath = '/tmp',
    );
    await queue.enqueuePosition(101, 42, 900, false);

    await service.playPinepodsEpisode(pinepodsEpisode: buildPinepodsEpisode(listenDuration: 100));

    expect(playedEpisode().position, 900000);
    expect(await queue.pendingPosition(101, false), 900);
    verifyNever(() => api.getEpisodeMetadata(any(), any(), isYoutube: any(named: 'isYoutube')));
  });

  test('completed episodes still start from the beginning', () async {
    serverPosition(500);
    await queue.enqueuePosition(101, 42, 900, false);

    await service.playPinepodsEpisode(
      pinepodsEpisode: buildPinepodsEpisode(listenDuration: 600, completed: true),
    );

    expect(playedEpisode().position, 0);
  });

  test('pendingPosition ignores other episodes and YouTube ids', () async {
    await queue.enqueuePosition(101, 42, 50, true);
    await queue.enqueuePosition(202, 42, 70, false);

    expect(await queue.pendingPosition(101, false), isNull);
    expect(await queue.pendingPosition(101, true), 50);
  });
}
