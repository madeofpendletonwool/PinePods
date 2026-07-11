// Regression test for playPinepodsEpisode() fetching podcast id and podcast
// 2.0 data in parallel instead of sequentially - before this fix, starting
// playback paid the cost of both round trips back-to-back.
//
// Proves concurrency deterministically (rather than via a wall-clock stopwatch,
// which is flaky on a loaded CI runner): each mocked call announces it has
// started, then blocks until the *other* call has also started. If playback ran
// them sequentially the second call would never start, the first would block
// forever, and the guard timeout would fail the test fast.
//
// Hand-written ("manual") mocks rather than @GenerateMocks-based ones, since
// this project has no build_runner setup yet.

import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:mockito/mockito.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/app_settings.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_audio_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';

// Mockito's `any`/`argThat` matchers are statically typed `Null`, which the
// analyzer rejects for non-nullable parameters (see mockito's
// NULL_SAFETY_README, "Solution 2: manual mock implementation"). Since this
// project doesn't have build_runner-based @GenerateMocks codegen, the methods
// exercised below are manually overridden with nullable parameter types and
// delegate to Mock's noSuchMethod.
class MockAudioPlayerService extends Mock implements AudioPlayerService {
  @override
  Future<void> playEpisode({Episode? episode, bool? resume}) => super.noSuchMethod(
        Invocation.method(#playEpisode, [], {#episode: episode, #resume: resume}),
        returnValue: Future<void>.value(),
      );

  @override
  Future<Episode?> findDownloadedEpisode(int? episodeId) => super.noSuchMethod(
        Invocation.method(#findDownloadedEpisode, [episodeId]),
        returnValue: Future<Episode?>.value(),
      );

  @override
  Future<void> setPlaybackSpeed(double? speed) => super.noSuchMethod(
        Invocation.method(#setPlaybackSpeed, [speed]),
        returnValue: Future<void>.value(),
      );
}

class MockPinepodsService extends Mock implements PinepodsService {
  @override
  Future<PlayEpisodeDetails> getPlayEpisodeDetails(int? userId, int? podcastId, bool? isYoutube) =>
      super.noSuchMethod(
        Invocation.method(#getPlayEpisodeDetails, [userId, podcastId, isYoutube]),
        returnValue: Future.value(PlayEpisodeDetails(playbackSpeed: 1.0, startSkip: 0, endSkip: 0)),
      );

  @override
  Future<int> getPodcastIdFromEpisode(int? episodeId, int? userId, bool? isYoutube) => super.noSuchMethod(
        Invocation.method(#getPodcastIdFromEpisode, [episodeId, userId, isYoutube]),
        returnValue: Future.value(0),
      );

  @override
  Future<Map<String, dynamic>?> fetchPodcasting2Data(int? episodeId, int? userId) => super.noSuchMethod(
        Invocation.method(#fetchPodcasting2Data, [episodeId, userId]),
        returnValue: Future<Map<String, dynamic>?>.value(),
      );
}

class MockSettingsBloc extends Mock implements SettingsBloc {
  @override
  AppSettings get currentSettings => super.noSuchMethod(
        Invocation.getter(#currentSettings),
        returnValue: AppSettings.sensibleDefaults(),
      );
}

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

      // Each call signals that it has started, then waits for the other to
      // start before returning. This only resolves if both are in flight at
      // once; if they were awaited sequentially the second would never begin
      // and the 5s guard would throw, failing the test rather than hanging.
      final getPodcastIdStarted = Completer<void>();
      final fetchData2Started = Completer<void>();

      Future<T> awaitConcurrent<T>(Completer<void> other, String label, T value) async {
        await other.future.timeout(
          const Duration(seconds: 5),
          onTimeout: () => throw StateError('$label did not run concurrently'),
        );
        return value;
      }

      when(pinepodsService.getPodcastIdFromEpisode(any, any, any)).thenAnswer((_) async {
        if (!getPodcastIdStarted.isCompleted) getPodcastIdStarted.complete();
        return awaitConcurrent(fetchData2Started, 'fetchPodcasting2Data', 7);
      });
      when(pinepodsService.fetchPodcasting2Data(any, any)).thenAnswer((_) async {
        if (!fetchData2Started.isCompleted) fetchData2Started.complete();
        return awaitConcurrent(getPodcastIdStarted, 'getPodcastIdFromEpisode', <String, dynamic>{});
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

      // Completes only if both fetches overlapped; a sequential regression makes
      // the guard above throw well within this bound.
      await service
          .playPinepodsEpisode(pinepodsEpisode: episode, resume: false)
          .timeout(const Duration(seconds: 10));

      expect(getPodcastIdStarted.isCompleted, isTrue);
      expect(fetchData2Started.isCompleted, isTrue);
    },
  );
}
