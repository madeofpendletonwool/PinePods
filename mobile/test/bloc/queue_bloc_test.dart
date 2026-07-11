// Tests QueueBloc's event routing: queue events pushed into its sink should
// drive the matching AudioPlayerService calls, and its `queue` output should
// forward the service's queueState stream.

import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:mocktail/mocktail.dart';
import 'package:pinepods_mobile/bloc/podcast/queue_bloc.dart';
import 'package:pinepods_mobile/state/queue_event_state.dart';

import '../support/factories.dart';
import '../support/fakes.dart';

void main() {
  late MockPodcastService podcastService;
  late MockAudioPlayerService audioPlayerService;
  late StreamController<QueueListState> queueStateController;
  late QueueBloc bloc;

  setUpAll(registerCommonFallbacks);

  setUp(() {
    podcastService = MockPodcastService();
    audioPlayerService = MockAudioPlayerService();
    queueStateController = StreamController<QueueListState>.broadcast();
    // QueueBloc subscribes to queueState at construction and, after a debounce,
    // persists it via saveQueue (flushed when the stream closes in tearDown).
    when(() => audioPlayerService.queueState).thenAnswer((_) => queueStateController.stream);
    when(() => podcastService.saveQueue(any())).thenAnswer((_) async {});
    bloc = QueueBloc(
      audioPlayerService: audioPlayerService,
      podcastService: podcastService,
    );
  });

  tearDown(() async {
    bloc.dispose();
    await queueStateController.close();
  });

  test('a QueueAddEvent adds the episode to Up Next', () async {
    final episode = buildEpisode(guid: 'e1');
    when(() => audioPlayerService.addUpNextEpisode(any())).thenAnswer((_) async {});

    bloc.queueEvent(QueueAddEvent(episode: episode));
    await pumpEventQueue();

    verify(() => audioPlayerService.addUpNextEpisode(episode)).called(1);
  });

  test('a QueueRemoveEvent removes the episode from Up Next', () async {
    final episode = buildEpisode(guid: 'e1');
    when(() => audioPlayerService.removeUpNextEpisode(any())).thenAnswer((_) async => true);

    bloc.queueEvent(QueueRemoveEvent(episode: episode));
    await pumpEventQueue();

    verify(() => audioPlayerService.removeUpNextEpisode(episode)).called(1);
  });

  test('a QueueClearEvent clears Up Next', () async {
    when(() => audioPlayerService.clearUpNext()).thenAnswer((_) async {});

    bloc.queueEvent(QueueClearEvent());
    await pumpEventQueue();

    verify(() => audioPlayerService.clearUpNext()).called(1);
  });

  test('the queue output forwards the service queueState stream', () async {
    final state = QueueListState(
      playing: buildEpisode(guid: 'playing'),
      queue: [buildEpisode(guid: 'q1')],
    );

    expectLater(bloc.queue, emits(state));

    queueStateController.add(state);
  });
}
