// The outbox: progress is kept as one entry per episode, sent in one request,
// kept until the server has it, and sent the old way to a server that has no
// progress endpoint.

import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:mocktail/mocktail.dart';
import 'package:pinepods_mobile/entities/pending_action.dart';
import 'package:pinepods_mobile/repository/sembast/sembast_repository.dart';
import 'package:pinepods_mobile/services/offline/offline_action_queue.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/pinepods/progress_tracker.dart';
import 'package:pinepods_mobile/services/settings/settings_service.dart';

import '../../support/in_memory_repository.dart';

class MockSettingsService extends Mock implements SettingsService {}

final t0 = DateTime(2026, 11, 12, 9);

ProgressUpdate progress(
  double position, {
  int episodeId = 101,
  bool isYoutube = false,
  Duration after = Duration.zero,
  double listened = 0,
  bool finished = false,
  double? duration,
}) =>
    ProgressUpdate(
      episodeId: episodeId,
      userId: 42,
      isYoutube: isYoutube,
      position: position,
      at: t0.add(after),
      listened: listened,
      finished: finished,
      duration: duration,
    );

void main() {
  late SembastRepository repository;
  late MockSettingsService settings;
  late List<http.Request> requests;

  /// What the fake server answers; replaced per test.
  late Future<http.Response> Function(http.Request) server;

  late OfflineActionQueue queue;

  Future<List<PendingAction>> pending() => repository.getPendingActions();

  List<Map<String, dynamic>> updatesOf(http.Request request) =>
      ((jsonDecode(request.body) as Map<String, dynamic>)['updates'] as List).cast<Map<String, dynamic>>();

  Iterable<http.Request> requestsTo(String path) => requests.where((r) => r.url.path.endsWith(path));

  void online() {
    when(() => settings.pinepodsServer).thenReturn('https://pods.example.com');
    when(() => settings.pinepodsApiKey).thenReturn('key');
  }

  setUp(() {
    repository = newInMemoryRepository();
    settings = MockSettingsService();
    requests = [];
    server = (_) async => http.Response('{"results": []}', 200);
    final service = PinepodsService(client: MockClient((request) {
      requests.add(request);
      return server(request);
    }));
    queue = OfflineActionQueue(repository: repository, pinepodsService: service, settingsService: settings);
    // Logged out: nothing is sent, everything queued stays visible.
    when(() => settings.pinepodsServer).thenReturn(null);
    when(() => settings.pinepodsApiKey).thenReturn(null);
  });

  group('one entry per episode', () {
    test('keeps the newest position and adds up the time played', () async {
      await queue.enqueueProgress(progress(100, listened: 30, duration: 600));
      await queue.enqueueProgress(progress(40, after: const Duration(seconds: 30), listened: 12)); // went back

      final entry = (await pending()).single;
      expect(entry.position, 40);
      expect(entry.observedAt, t0.add(const Duration(seconds: 30)));
      expect(entry.listened, 42);
      expect(entry.duration, 600);
      expect(entry.finished, isFalse);
    });

    test('something older arriving late adds its time but not its position', () async {
      await queue.enqueueProgress(progress(400, after: const Duration(minutes: 5), listened: 10));
      await queue.enqueueProgress(progress(90, listened: 5));

      final entry = (await pending()).single;
      expect(entry.position, 400);
      expect(entry.listened, 15);
    });

    test('reports made at the same moment do not lose each other', () async {
      await Future.wait([
        for (var i = 1; i <= 8; i++) queue.enqueueProgress(progress(i * 10, after: Duration(seconds: i), listened: 1)),
      ]);

      final entry = (await pending()).single;
      expect(entry.position, 80);
      expect(entry.listened, 8);
    });

    test('episodes and videos with the same id are separate', () async {
      await queue.enqueueProgress(progress(10));
      await queue.enqueueProgress(progress(20, isYoutube: true));
      await queue.enqueueProgress(progress(30, episodeId: 202));

      expect(await pending(), hasLength(3));
      expect(await queue.pendingPosition(101, false), 10);
      expect(await queue.pendingPosition(101, true), 20);
    });

    test('finishing is part of the entry', () async {
      await queue.enqueueProgress(progress(590, listened: 20));
      await queue.enqueueProgress(progress(600, after: const Duration(seconds: 10), listened: 10, finished: true));

      final entry = (await queue.pendingProgress(101, false))!;
      expect(entry.finished, isTrue);
      expect(entry.listened, 30);
    });

    test('an entry saved by an older version of the app is read as observed when it was saved', () async {
      await repository.savePendingAction(PendingAction(
        type: PendingActionType.recordPosition,
        episodeId: 101,
        userId: 42,
        payload: {'position': 55.0},
        createdAt: t0,
      ));

      final entry = (await queue.pendingProgress(101, false))!;
      expect(entry.observedAt, t0);
      expect(entry.listened, 0);
      expect(entry.finished, isFalse);
    });
  });

  group('sending', () {
    test('everything pending goes out in one request with the time each was observed', () async {
      await queue.enqueueProgress(progress(100, listened: 30, duration: 600));
      await queue.enqueueProgress(progress(600, episodeId: 202, after: const Duration(minutes: 1), listened: 45, finished: true));
      online();

      await queue.flush();

      final request = requestsTo('/api/data/progress').single;
      expect(request.headers['Api-Key'], 'key');
      final body = jsonDecode(request.body) as Map<String, dynamic>;
      expect(body['sent_at'], isA<int>());
      expect(updatesOf(request), [
        {
          'episode_id': 101,
          'is_youtube': false,
          'position': 100.0,
          'at': t0.millisecondsSinceEpoch,
          'listened': 30.0,
          'finished': false,
          'duration': 600.0,
        },
        {
          'episode_id': 202,
          'is_youtube': false,
          'position': 600.0,
          'at': t0.add(const Duration(minutes: 1)).millisecondsSinceEpoch,
          'listened': 45.0,
          'finished': true,
        },
      ]);
      expect(await pending(), isEmpty);
    });

    test('progress is kept when the request fails and sent again later', () async {
      await queue.enqueueProgress(progress(100, listened: 30));
      online();
      server = (_) async => throw http.ClientException('no route to host');

      await queue.flush();
      expect((await pending()).single.position, 100);

      server = (_) async => http.Response('nope', 500);
      await queue.flush();
      expect((await pending()).single.listened, 30);

      server = (_) async => http.Response('{"results": []}', 200);
      await queue.flush();
      expect(await pending(), isEmpty);
      expect(requestsTo('/api/data/progress'), hasLength(3));
    });

    test('progress made while a request is in flight is sent afterwards, without the time already delivered', () async {
      await queue.enqueueProgress(progress(100, listened: 30));
      online();
      var first = true;
      server = (_) async {
        if (first) {
          first = false;
          // The user keeps listening while the first report is on its way.
          await queue.enqueueProgress(progress(130, after: const Duration(seconds: 30), listened: 30));
        }
        return http.Response('{"results": []}', 200);
      };

      await queue.flush();

      final sent = requestsTo('/api/data/progress').map((r) => updatesOf(r).single).toList();
      expect(sent.map((u) => u['position']), [100.0, 130.0]);
      expect(sent.map((u) => u['listened']), [30.0, 30.0], reason: 'the first 30s are not counted twice');
      expect(await pending(), isEmpty);
    });

    test('other actions wait for the server too and go out in order', () async {
      await queue.enqueueSimple(PendingActionType.saveEpisode, 101, 42, false);
      await queue.enqueueSimple(PendingActionType.markUncompleted, 101, 42, false);
      online();
      server = (_) async => http.Response('no', 500);

      await queue.flush();
      expect(await pending(), hasLength(2), reason: 'a refused action is not taken as delivered');

      server = (_) async => http.Response('{"detail": "ok"}', 200);
      await queue.flush();
      expect(requests.map((r) => r.url.path).where((p) => !p.endsWith('/progress')).toList().sublist(1), [
        '/api/data/save_episode',
        '/api/data/mark_episode_uncompleted',
      ]);
      expect(await pending(), isEmpty);
    });
  });

  group('a server without the progress endpoint', () {
    setUp(() {
      online();
      server = (request) async => request.url.path.endsWith('/progress')
          ? http.Response('Not Found', 404)
          : http.Response('{"detail": "ok"}', 200);
    });

    test('gets the position, the completion and the minutes through the calls it has', () async {
      when(() => settings.pinepodsServer).thenReturn(null);
      await queue.enqueueProgress(progress(600, listened: 130, finished: true));
      await queue.enqueueProgress(progress(50, episodeId: 202, listened: 20));
      online();

      await queue.flush();

      expect(requestsTo('/progress'), hasLength(1));
      final positions = requestsTo('/record_listen_duration').map((r) => jsonDecode(r.body) as Map).toList();
      expect(positions.map((p) => [p['episode_id'], p['listen_duration'], p['user_id']]), [
        [101, 600.0, 42],
        [202, 50.0, 42],
      ]);
      expect((jsonDecode(requestsTo('/mark_episode_completed').single.body) as Map)['episode_id'], 101);
      expect(requestsTo('/increment_listen_time/42'), hasLength(2), reason: '150s played is two whole minutes');
      expect(await pending(), isEmpty);
    });

    test('is not asked for the progress endpoint again', () async {
      // Online, so each report is sent as it is queued.
      await queue.enqueueProgress(progress(10));
      await pumpEventQueue();
      await queue.enqueueProgress(progress(40, after: const Duration(seconds: 30)));
      await pumpEventQueue();

      expect(requestsTo('/progress'), hasLength(1));
      expect(requestsTo('/record_listen_duration'), hasLength(2));
    });

    test('keeps progress it could not deliver', () async {
      server = (request) async =>
          request.url.path.endsWith('/progress') ? http.Response('Not Found', 404) : http.Response('down', 502);

      await queue.enqueueProgress(progress(10));
      await pumpEventQueue();

      expect((await pending()).single.position, 10);
    });
  });
}
