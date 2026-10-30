// End-of-episode advance. Near the end the next episode is worked out ahead of
// time, so completion starts it without waiting on the server.

import 'package:flutter_test/flutter_test.dart';
import 'package:mocktail/mocktail.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/app_settings.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_audio_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:rxdart/rxdart.dart';

import '../../support/factories.dart';
import '../../support/fakes.dart';

class MockPinepodsService extends Mock implements PinepodsService {}

class MockSettingsBloc extends Mock implements SettingsBloc {}

void main() {
  late MockAudioPlayerService audio;
  late MockPinepodsService api;
  late PinepodsAudioService service;
  late BehaviorSubject<PositionState> positions;

  setUpAll(registerCommonFallbacks);

  setUp(() async {
    audio = MockAudioPlayerService();
    api = MockPinepodsService();
    final settingsBloc = MockSettingsBloc();
    service = PinepodsAudioService(audio, api, settingsBloc);
    positions = BehaviorSubject<PositionState>();
    addTearDown(service.dispose);
    addTearDown(positions.close);

    when(() => settingsBloc.currentSettings).thenReturn(AppSettings.sensibleDefaults()
        .copyWith(pinepodsUserId: 42, pinepodsServer: 'https://pods.example', pinepodsApiKey: 'key'));
    when(() => audio.playPosition).thenAnswer((_) => positions.stream);
    when(() => audio.findDownloadedEpisode(any())).thenAnswer((_) async => null);
    when(() => audio.playEpisode(
          episode: any(named: 'episode'),
          resume: any(named: 'resume'),
          playbackSpeed: any(named: 'playbackSpeed'),
        )).thenAnswer((_) async {});
    when(() => api.setCredentials(any(), any())).thenReturn(null);
    when(() => api.getPodcastIdFromEpisode(any(), any(), any())).thenAnswer((_) async => 7);
    when(() => api.fetchPodcasting2Data(any(), any())).thenAnswer((_) async => null);
    when(() => api.getPlayEpisodeDetails(any(), any(), any())).thenAnswer(
      (_) async => PlayEpisodeDetails(playbackSpeed: 1.0, startSkip: 0, endSkip: 0),
    );
    when(() => api.getAutoPlayNextStatus(any(), any())).thenAnswer((_) async => false);
    when(() => api.markEpisodeCompleted(any(), any(), any())).thenAnswer((_) async => true);

    await service.playPinepodsEpisode(pinepodsEpisode: buildPinepodsEpisode(episodeId: 101), resume: false);
    clearInteractions(audio);
  });

  Episode started() => verify(() => audio.playEpisode(
        episode: captureAny(named: 'episode'),
        resume: any(named: 'resume'),
        playbackSpeed: any(named: 'playbackSpeed'),
      )).captured.single as Episode;

  void queue(List<int> ids) => when(() => api.getQueuedEpisodes(any())).thenAnswer(
        (_) async => [for (final id in ids) buildPinepodsEpisode(episodeId: id, episodeTitle: 'Episode $id')],
      );

  test('the next episode is resolved near the end and started without asking again', () async {
    queue([101, 202]);
    positions.add(PositionState(
      position: const Duration(seconds: 550),
      length: const Duration(seconds: 600),
      percentage: 91,
    ));

    service.prefetchNextIfNearEnd();
    await pumpEventQueue();
    verify(() => api.getQueuedEpisodes(42)).called(1);

    expect(await service.handleEpisodeCompleted(), isTrue);
    expect(started().title, 'Episode 202');
    verifyNever(() => api.getQueuedEpisodes(any()));
  });

  test('nothing is resolved early while the episode is far from its end', () async {
    queue([101, 202]);
    positions.add(PositionState(
      position: const Duration(seconds: 60),
      length: const Duration(seconds: 600),
      percentage: 10,
    ));

    service.prefetchNextIfNearEnd();
    await pumpEventQueue();
    verifyNever(() => api.getQueuedEpisodes(any()));

    expect(await service.handleEpisodeCompleted(), isTrue);
    expect(started().title, 'Episode 202');
  });

  test('an empty queue stops playback', () async {
    queue([101]);
    expect(await service.handleEpisodeCompleted(), isFalse);
  });
}
