// Regression test for playPinepodsEpisode() fetching podcast id and podcast
// 2.0 data in parallel instead of sequentially - before this fix, starting
// playback paid the cost of both round trips back-to-back.
//
// Hand-written ("manual") mocks rather than @GenerateMocks-based ones, since
// this project has no build_runner setup yet.

import 'package:flutter_test/flutter_test.dart';
import 'package:mockito/mockito.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/app_settings.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_audio_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';

class MockAudioPlayerService extends Mock implements AudioPlayerService {}

class MockPinepodsService extends Mock implements PinepodsService {}

class MockSettingsBloc extends Mock implements SettingsBloc {}

const _perCallDelay = Duration(milliseconds: 100);

void main() {
  test(
    'playPinepodsEpisode fetches podcast id and podcast 2.0 data in parallel, not sequentially',
    () async {
      final audioPlayerService = MockAudioPlayerService();
      final pinepodsService = MockPinepodsService();
      final settingsBloc = MockSettingsBloc();
      final service = PinepodsAudioService(audioPlayerService, pinepodsService, settingsBloc);
      addTearDown(service.dispose);

      when(settingsBloc.currentSettings)
          .thenReturn(AppSettings.sensibleDefaults().copyWith(pinepodsUserId: 42));
      when(audioPlayerService.findDownloadedEpisode(any)).thenAnswer((_) async => null);
      when(audioPlayerService.playEpisode(episode: anyNamed('episode'), resume: anyNamed('resume')))
          .thenAnswer((_) async {});
      when(audioPlayerService.setPlaybackSpeed(any)).thenAnswer((_) async {});
      when(pinepodsService.getPlayEpisodeDetails(any, any, any)).thenAnswer(
        (_) async => PlayEpisodeDetails(playbackSpeed: 1.0, startSkip: 0, endSkip: 0),
      );
      // Each of these takes _perCallDelay on its own - if they ran
      // sequentially the whole call would take roughly 2x that.
      when(pinepodsService.getPodcastIdFromEpisode(any, any, any)).thenAnswer((_) async {
        await Future.delayed(_perCallDelay);
        return 7;
      });
      when(pinepodsService.fetchPodcasting2Data(any, any)).thenAnswer((_) async {
        await Future.delayed(_perCallDelay);
        return <String, dynamic>{};
      });

      final episode = PinepodsEpisode(
        podcastName: 'Test Podcast',
        episodeTitle: 'Test Episode',
        episodePubDate: '2026-01-01T00:00:00',
        episodeDescription: 'Description',
        episodeArtwork: '',
        episodeUrl: 'https://example.com/episode.mp3',
        episodeDuration: 600,
        episodeId: 101,
        completed: false,
        saved: false,
        queued: false,
        downloaded: false,
        isYoutube: false,
      );

      final stopwatch = Stopwatch()..start();
      await service.playPinepodsEpisode(pinepodsEpisode: episode, resume: false);
      stopwatch.stop();

      // Comfortably above the ~100ms parallel case and well below the ~200ms
      // it would take if the two calls ran one after another.
      expect(stopwatch.elapsed, lessThan(const Duration(milliseconds: 180)));
    },
  );
}
