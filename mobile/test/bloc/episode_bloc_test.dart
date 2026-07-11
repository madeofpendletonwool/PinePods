// Tests the rxdart-based EpisodeBloc by injecting mocked services, pushing its
// input sinks, and asserting on the emitted BlocState stream. This is the
// reusable pattern for the app's hand-rolled (non-flutter_bloc) blocs: no
// bloc_test, just expectLater(stream, emitsInOrder([...])) and mock verification.

import 'package:flutter_test/flutter_test.dart';
import 'package:mocktail/mocktail.dart';
import 'package:pinepods_mobile/bloc/podcast/episode_bloc.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/state/bloc_state.dart';
import 'package:pinepods_mobile/state/episode_state.dart';

import '../support/factories.dart';
import '../support/fakes.dart';

void main() {
  late MockPodcastService podcastService;
  late MockAudioPlayerService audioPlayerService;
  late EpisodeBloc bloc;

  setUpAll(registerCommonFallbacks);

  setUp(() {
    podcastService = MockPodcastService();
    audioPlayerService = MockAudioPlayerService();
    // The bloc subscribes to this at construction; an empty stream means no
    // external episode events fire during the test.
    when(() => podcastService.episodeListener)
        .thenAnswer((_) => Stream<EpisodeState>.empty());
    bloc = EpisodeBloc(
      podcastService: podcastService,
      audioPlayerService: audioPlayerService,
    );
  });

  tearDown(() => bloc.dispose());

  test('fetchEpisodes emits Loading then Populated with the loaded episodes', () async {
    final loaded = [buildEpisode(guid: 'e1'), buildEpisode(guid: 'e2')];
    when(() => podcastService.loadEpisodes()).thenAnswer((_) async => loaded);

    expectLater(
      bloc.episodes,
      emitsInOrder([
        isA<BlocLoadingState<List<Episode>>>(),
        isA<BlocPopulatedState<List<Episode>>>()
            .having((s) => s.results, 'results', hasLength(2)),
      ]),
    );

    bloc.fetchEpisodes(false);
  });

  test('a silent fetch skips the Loading state and emits only Populated', () async {
    when(() => podcastService.loadEpisodes()).thenAnswer((_) async => <Episode>[]);

    expectLater(
      bloc.episodes,
      emitsInOrder([
        isA<BlocPopulatedState<List<Episode>>>(),
      ]),
    );

    bloc.fetchEpisodes(true);
  });

  test('fetchDownloads emits Loading then Populated from loadDownloads', () async {
    when(() => podcastService.loadDownloads())
        .thenAnswer((_) async => [buildEpisode(guid: 'd1')]);

    expectLater(
      bloc.downloads,
      emitsInOrder([
        isA<BlocLoadingState<List<Episode>>>(),
        isA<BlocPopulatedState<List<Episode>>>()
            .having((s) => s.results, 'results', hasLength(1)),
      ]),
    );

    bloc.fetchDownloads(false);
  });

  test('togglePlayed delegates to the service and refreshes downloads', () async {
    final episode = buildEpisode(guid: 'e1');
    when(() => podcastService.toggleEpisodePlayed(any())).thenAnswer((_) async {});
    when(() => podcastService.loadDownloads()).thenAnswer((_) async => <Episode>[]);

    // The post-toggle refresh only runs when the downloads output is observed.
    final sub = bloc.downloads!.listen((_) {});
    addTearDown(sub.cancel);

    bloc.togglePlayed(episode);
    await pumpEventQueue();

    verify(() => podcastService.toggleEpisodePlayed(episode)).called(1);
    // Marking played refreshes the downloads list.
    verify(() => podcastService.loadDownloads()).called(1);
  });

  test('deleteDownload stops playback only when deleting the now-playing episode', () async {
    final playing = buildEpisode(guid: 'now-playing');
    when(() => audioPlayerService.nowPlaying).thenReturn(playing);
    when(() => audioPlayerService.stop()).thenAnswer((_) async {});
    when(() => podcastService.deleteDownload(any())).thenAnswer((_) async {});
    when(() => podcastService.loadDownloads()).thenAnswer((_) async => <Episode>[]);

    bloc.deleteDownload(playing);
    await pumpEventQueue();

    verify(() => audioPlayerService.stop()).called(1);
    verify(() => podcastService.deleteDownload(playing)).called(1);
  });

  test('deleteDownload does not stop playback for a different episode', () async {
    final other = buildEpisode(guid: 'other');
    when(() => audioPlayerService.nowPlaying).thenReturn(buildEpisode(guid: 'now-playing'));
    when(() => audioPlayerService.stop()).thenAnswer((_) async {});
    when(() => podcastService.deleteDownload(any())).thenAnswer((_) async {});
    when(() => podcastService.loadDownloads()).thenAnswer((_) async => <Episode>[]);

    bloc.deleteDownload(other);
    await pumpEventQueue();

    verifyNever(() => audioPlayerService.stop());
    verify(() => podcastService.deleteDownload(other)).called(1);
  });
}
