// Regression tests for PinepodsAudioService.playNextFromServerQueue() and
// its peekAndDequeueNextServerEpisode() helper - the fix for auto-advance
// never pulling from the PinePods server queue when an episode finishes
// (see native_audio_player_service.dart's _handleCompletedEvent).
//
// These are hand-written ("manual") mocks rather than @GenerateMocks-based
// ones, since this project has no build_runner setup yet.

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
  Future<bool> queueEpisode(int? episodeId, int? userId, bool? isYoutube) => super.noSuchMethod(
        Invocation.method(#queueEpisode, [episodeId, userId, isYoutube]),
        returnValue: Future.value(false),
      );

  @override
  Future<bool> removeQueuedEpisode(int? episodeId, int? userId, bool? isYoutube) => super.noSuchMethod(
        Invocation.method(#removeQueuedEpisode, [episodeId, userId, isYoutube]),
        returnValue: Future.value(false),
      );

  @override
  Future<List<PinepodsEpisode>> getQueuedEpisodes(int? userId) => super.noSuchMethod(
        Invocation.method(#getQueuedEpisodes, [userId]),
        returnValue: Future.value(<PinepodsEpisode>[]),
      );

  @override
  Future<PlayEpisodeDetails> getPlayEpisodeDetails(int? userId, int? podcastId, bool? isYoutube) =>
      super.noSuchMethod(
        Invocation.method(#getPlayEpisodeDetails, [userId, podcastId, isYoutube]),
        returnValue: Future.value(PlayEpisodeDetails(playbackSpeed: 1.0, startSkip: 0, endSkip: 0)),
      );
}

class MockSettingsBloc extends Mock implements SettingsBloc {
  @override
  AppSettings get currentSettings => super.noSuchMethod(
        Invocation.getter(#currentSettings),
        returnValue: AppSettings.sensibleDefaults(),
      );
}

PinepodsEpisode _episode({
  int episodeId = 101,
  bool isYoutube = false,
  int? listenDuration,
}) {
  return PinepodsEpisode(
    podcastName: 'Test Podcast',
    episodeTitle: 'Test Episode',
    episodePubDate: '2026-01-01T00:00:00',
    episodeDescription: 'Description',
    episodeArtwork: '',
    episodeUrl: 'https://example.com/episode.mp3',
    episodeDuration: 600,
    listenDuration: listenDuration,
    episodeId: episodeId,
    completed: false,
    saved: false,
    queued: true,
    downloaded: false,
    isYoutube: isYoutube,
  );
}

void main() {
  late MockAudioPlayerService audioPlayerService;
  late MockPinepodsService pinepodsService;
  late MockSettingsBloc settingsBloc;
  late PinepodsAudioService service;

  setUp(() {
    audioPlayerService = MockAudioPlayerService();
    pinepodsService = MockPinepodsService();
    settingsBloc = MockSettingsBloc();
    service = PinepodsAudioService(audioPlayerService, pinepodsService, settingsBloc);
  });

  tearDown(() {
    service.dispose();
  });

  group('peekAndDequeueNextServerEpisode', () {
    test('returns null and does not call removeQueuedEpisode when the server queue is empty', () async {
      when(pinepodsService.getQueuedEpisodes(42)).thenAnswer((_) async => <PinepodsEpisode>[]);

      final result = await service.peekAndDequeueNextServerEpisode(42);

      expect(result, isNull);
      verifyNever(pinepodsService.removeQueuedEpisode(any, any, any));
    });

    test('returns the first queued episode and removes it from the server queue', () async {
      final first = _episode(episodeId: 101, isYoutube: true);
      final second = _episode(episodeId: 202);
      when(pinepodsService.getQueuedEpisodes(42)).thenAnswer((_) async => [first, second]);
      when(pinepodsService.removeQueuedEpisode(101, 42, true)).thenAnswer((_) async => true);

      final result = await service.peekAndDequeueNextServerEpisode(42);

      expect(result?.episodeId, 101);
      verify(pinepodsService.removeQueuedEpisode(101, 42, true)).called(1);
      // The second episode should be left alone.
      verifyNever(pinepodsService.removeQueuedEpisode(argThat(equals(202)), any, any));
    });

    test('still returns the episode when removing it from the server queue fails', () async {
      final next = _episode(episodeId: 101);
      when(pinepodsService.getQueuedEpisodes(42)).thenAnswer((_) async => [next]);
      when(pinepodsService.removeQueuedEpisode(101, 42, false)).thenThrow(Exception('offline'));

      final result = await service.peekAndDequeueNextServerEpisode(42);

      expect(result?.episodeId, 101);
    });
  });

  group('playNextFromServerQueue', () {
    test('returns false without hitting the network when there is no logged-in user', () async {
      when(settingsBloc.currentSettings).thenReturn(AppSettings.sensibleDefaults());

      final played = await service.playNextFromServerQueue();

      expect(played, isFalse);
      verifyNever(pinepodsService.getQueuedEpisodes(any));
    });

    test('returns false when the server queue is empty', () async {
      when(settingsBloc.currentSettings)
          .thenReturn(AppSettings.sensibleDefaults().copyWith(pinepodsUserId: 42));
      when(pinepodsService.getQueuedEpisodes(42)).thenAnswer((_) async => <PinepodsEpisode>[]);

      final played = await service.playNextFromServerQueue();

      expect(played, isFalse);
    });

    test('plays and dequeues the next episode without re-adding it to the queue', () async {
      final next = _episode(episodeId: 101, listenDuration: 30);
      when(settingsBloc.currentSettings)
          .thenReturn(AppSettings.sensibleDefaults().copyWith(pinepodsUserId: 42));
      when(pinepodsService.getQueuedEpisodes(42)).thenAnswer((_) async => [next]);
      when(pinepodsService.removeQueuedEpisode(101, 42, false)).thenAnswer((_) async => true);
      when(pinepodsService.getPlayEpisodeDetails(any, any, any)).thenAnswer(
        (_) async => PlayEpisodeDetails(playbackSpeed: 1.0, startSkip: 0, endSkip: 0),
      );
      when(audioPlayerService.findDownloadedEpisode(any)).thenAnswer((_) async => null);
      when(audioPlayerService.playEpisode(episode: anyNamed('episode'), resume: anyNamed('resume')))
          .thenAnswer((_) async {});
      when(audioPlayerService.setPlaybackSpeed(any)).thenAnswer((_) async {});

      final played = await service.playNextFromServerQueue();

      expect(played, isTrue);
      // Started at a non-zero listen position, so playback should resume.
      verify(audioPlayerService.playEpisode(
        episode: anyNamed('episode'),
        resume: argThat(isTrue, named: 'resume'),
      )).called(1);
      // skipQueue: true is the whole point of the fix - without it the episode
      // we just pulled off the queue would immediately be re-added to it.
      verifyNever(pinepodsService.queueEpisode(any, any, any));
    });
  });
}
