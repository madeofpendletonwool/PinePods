// The per-user auto-queue default for new subscriptions (#648), read and saved from Settings.

import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';

void main() {
  PinepodsService serviceWith(Future<http.Response> Function(http.Request) handler) {
    return PinepodsService(client: MockClient(handler))..setCredentials('https://pods.example.com', 'key-1');
  }

  test('reads the default for the user', () async {
    late http.Request sent;
    final service = serviceWith((request) async {
      sent = request;
      return http.Response('{"user_id": 3, "enabled": true}', 200);
    });

    expect(await service.getAutoQueueNewPodcasts(3), isTrue);
    expect(sent.method, 'GET');
    expect(sent.url.toString(), 'https://pods.example.com/api/data/get_auto_queue_new_podcasts/3');
    expect(sent.headers['Api-Key'], 'key-1');
  });

  test('a missing field reads as off, and an error is thrown so the switch keeps loading', () async {
    expect(await serviceWith((_) async => http.Response('{"user_id": 3}', 200)).getAutoQueueNewPodcasts(3), isFalse);
    expect(serviceWith((_) async => http.Response('nope', 403)).getAutoQueueNewPodcasts(3), throwsException);
  });

  test('saves the new value with a PUT', () async {
    late http.Request sent;
    final service = serviceWith((request) async {
      sent = request;
      return http.Response('{"user_id": 3, "enabled": false}', 200);
    });

    await service.setAutoQueueNewPodcasts(3, false);
    expect(sent.method, 'PUT');
    expect(sent.url.toString(), 'https://pods.example.com/api/data/update_auto_queue_new_podcasts');
    expect(jsonDecode(sent.body), {'user_id': 3, 'enabled': false});
  });

  test('a failed save throws so the switch goes back', () async {
    expect(serviceWith((_) async => http.Response('forbidden', 403)).setAutoQueueNewPodcasts(3, true), throwsException);
  });
}
