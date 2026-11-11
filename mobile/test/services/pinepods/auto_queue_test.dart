// Per-podcast auto-queue (#648): the server adds a podcast's new episodes to the queue when it
// refreshes the feed. These cover the two requests the podcast page makes.

import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';

void main() {
  PinepodsService serviceWith(Future<http.Response> Function(http.Request) handler) {
    return PinepodsService(client: MockClient(handler))
      ..setCredentials('https://pods.example.com/', 'key-1');
  }

  group('getAutoQueueStatus', () {
    test('asks the server for this podcast and user and reads auto_queue', () async {
      late http.Request sent;
      final service = serviceWith((request) async {
        sent = request;
        return http.Response('{"auto_queue": true}', 200);
      });

      expect(await service.getAutoQueueStatus(12, 3), isTrue);
      expect(sent.method, 'POST');
      expect(sent.url.toString(), 'https://pods.example.com/api/data/get_auto_queue_status');
      expect(sent.headers['Api-Key'], 'key-1');
      expect(jsonDecode(sent.body), {'podcast_id': 12, 'user_id': 3});
    });

    test('is off when the server says so, errors, or omits the field', () async {
      expect(await serviceWith((_) async => http.Response('{"auto_queue": false}', 200)).getAutoQueueStatus(1, 1), isFalse);
      expect(await serviceWith((_) async => http.Response('{}', 200)).getAutoQueueStatus(1, 1), isFalse);
      expect(await serviceWith((_) async => http.Response('nope', 500)).getAutoQueueStatus(1, 1), isFalse);
      expect(await serviceWith((_) async => throw http.ClientException('offline')).getAutoQueueStatus(1, 1), isFalse);
    });
  });

  group('setAutoQueue', () {
    test('sends the new value for this podcast and user', () async {
      late http.Request sent;
      final service = serviceWith((request) async {
        sent = request;
        return http.Response('{"detail": "Auto-queue status updated."}', 200);
      });

      expect(await service.setAutoQueue(12, 3, true), isTrue);
      expect(sent.url.toString(), 'https://pods.example.com/api/data/enable_auto_queue');
      expect(sent.headers['Api-Key'], 'key-1');
      expect(jsonDecode(sent.body), {'podcast_id': 12, 'user_id': 3, 'auto_queue': true});
    });

    test('reports failure so the toggle is not shown as changed', () async {
      expect(await serviceWith((_) async => http.Response('{"detail": "forbidden"}', 403)).setAutoQueue(1, 1, false), isFalse);
      expect(await serviceWith((_) async => throw http.ClientException('offline')).setAutoQueue(1, 1, false), isFalse);
    });
  });

  test('both calls need credentials', () async {
    final service = PinepodsService(client: MockClient((_) async => http.Response('{}', 200)));
    expect(() => service.getAutoQueueStatus(1, 1), throwsException);
    expect(() => service.setAutoQueue(1, 1, true), throwsException);
  });
}
