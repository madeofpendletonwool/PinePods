// Regression tests for PinepodsService's HTTP handling: before this fix every
// request used the top-level http.get/post/put functions (a fresh
// http.Client per call, no timeout), so a stalled request hung forever with
// no feedback. These tests inject a fake http.Client (package:http/testing.dart)
// to verify the shared client + timeout wrapper actually behaves as intended.

import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';

void main() {
  group('PinepodsService HTTP client', () {
    test('a normal response still resolves correctly through the wrapper', () async {
      Uri? capturedUrl;
      final client = MockClient((request) async {
        capturedUrl = request.url;
        return http.Response('{"pinepods_instance": true}', 200);
      });
      final service = PinepodsService(client: client);

      final result = await service.verifyPinepodsInstance('https://pods.example.com');

      expect(result, isTrue);
      expect(capturedUrl.toString(), 'https://pods.example.com/api/pinepods_check');
    });

    test('a request that never responds times out instead of hanging forever', () async {
      // Simulates a stalled connection: the handler's future never completes.
      final client = MockClient((request) => Completer<http.Response>().future);
      final service = PinepodsService(client: client, timeout: const Duration(milliseconds: 50));

      final stopwatch = Stopwatch()..start();
      final result = await service.verifyPinepodsInstance('https://pods.example.com');
      stopwatch.stop();

      // Before this fix there was no timeout at all, so this call would have
      // hung indefinitely rather than failing fast.
      expect(result, isFalse);
      expect(stopwatch.elapsed, lessThan(const Duration(seconds: 2)));
    });

    test('the client is reused across requests rather than recreated each call', () async {
      var callCount = 0;
      final client = MockClient((request) async {
        callCount++;
        return http.Response('{"pinepods_instance": true}', 200);
      });
      final service = PinepodsService(client: client);

      final first = await service.verifyPinepodsInstance('https://pods.example.com');
      final second = await service.verifyPinepodsInstance('https://pods.example.com');

      expect(first, isTrue);
      expect(second, isTrue);
      expect(callCount, 2);
    });
  });
}
