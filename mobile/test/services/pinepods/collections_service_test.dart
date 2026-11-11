// Request/response contract for the collection and multi-select calls.

import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';

void main() {
  const server = 'https://pods.example.com';

  PinepodsService serviceWith(Future<http.Response> Function(http.Request) handler) =>
      PinepodsService(client: MockClient(handler))..setCredentials(server, 'key');

  test('collections parse, including the default and auto-add categories', () async {
    final service = serviceWith((req) async {
      expect(req.url.path, '/api/data/collections/user/7');
      expect(req.headers['Api-Key'], 'key');
      return http.Response(
          jsonEncode({
            'collections': [
              {'collection_id': 1, 'name': 'Saved', 'is_default': true, 'icon': 'ph-bookmark', 'episode_count': 3},
              {
                'collection_id': 9,
                'name': 'Old run',
                'description': null,
                'is_default': false,
                'icon': 'ph-headphones',
                'episode_count': 15,
                'auto_add_categories': ['Comedy'],
              },
            ],
          }),
          200);
    });

    final cols = await service.getCollections(7);

    expect(cols.map((c) => c.id), [1, 9]);
    expect(cols.first.isDefault, isTrue);
    expect(cols.last.episodeCount, 15);
    expect(cols.last.autoAddCategories, ['Comedy']);
  });

  test('bulk add sends (episode, is_youtube) pairs', () async {
    late Map<String, dynamic> body;
    final service = serviceWith((req) async {
      expect(req.url.path, '/api/data/collections/bulk_add');
      body = jsonDecode(req.body) as Map<String, dynamic>;
      return http.Response('{"detail":"Episodes added to collection"}', 200);
    });

    await service.bulkAddToCollection(9, 7, [(101, false), (102, false)]);

    expect(body, {
      'user_id': 7,
      'collection_id': 9,
      'episodes': [
        [101, false],
        [102, false],
      ],
    });
  });

  test('collection episodes page reads saved_episodes and total', () async {
    final service = serviceWith((req) async {
      expect(req.url.path, '/api/data/collections/9/episodes');
      expect(req.url.queryParameters['offset'], '50');
      return http.Response(
          jsonEncode({
            'saved_episodes': [
              {
                'episodetitle': 'Ep 1',
                'podcastname': 'Show',
                'episodepubdate': '2025-01-01T00:00:00',
                'episodedescription': '',
                'episodeartwork': '',
                'episodeurl': 'https://x/1.mp3',
                'episodeduration': 60,
                'episodeid': 101,
                'completed': false,
                'saved': false,
                'queued': false,
                'downloaded': false,
                'is_youtube': false,
                'podcastid': 4,
              },
            ],
            'total': 51,
          }),
          200);
    });

    final page = await service.getCollectionEpisodesPaged(9, offset: 50);

    expect(page.total, 51);
    expect(page.episodes.single.episodeId, 101);
    expect(page.episodes.single.podcastId, 4);
  });

  test('bulk transcribe returns the server summary and surfaces its errors', () async {
    var status = 200;
    final service = serviceWith((req) async {
      expect(req.url.path, '/api/data/bulk_transcribe_episodes');
      expect(jsonDecode(req.body), {'episode_ids': [1, 2], 'user_id': 7, 'force': false});
      return status == 200
          ? http.Response('{"detail":"Queued 2 episodes for transcription."}', 200)
          : http.Response('{"message":"Service unavailable: AI service is not available."}', status);
    });

    expect(await service.bulkTranscribeEpisodes([1, 2], 7), 'Queued 2 episodes for transcription.');

    status = 503;
    expect(
      () => service.bulkTranscribeEpisodes([1, 2], 7),
      throwsA(predicate((e) => '$e'.contains('AI service is not available'))),
    );
  });

  test('bulk episode actions hit bulk_<action> with the feed type', () async {
    final service = serviceWith((req) async {
      expect(req.url.path, '/api/data/bulk_queue_episodes');
      expect(jsonDecode(req.body), {'episode_ids': [3], 'user_id': 7, 'is_youtube': true});
      return http.Response('{"message":"Successfully queued 1 episodes","processed_count":1}', 200);
    });

    expect(await service.bulkEpisodeAction('queue_episodes', [3], 7, true), 'Successfully queued 1 episodes');
  });
}
