// Verifies checkServer maps transport failures to specific, non-generic user
// messages. Previously these drove a real network request at
// `does-not-exist.invalid`, which is slow and depends on the runner's DNS
// behavior; now the failure is injected via a fake http.Client so the mapping
// is exercised deterministically and offline.

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:pinepods_mobile/services/pinepods/login_service.dart';

void main() {
  test('maps a DNS/connection failure to a reachability message, not the generic one', () async {
    final client = MockClient(
      (_) => throw const SocketException('Failed host lookup: does-not-exist.invalid'),
    );

    final r = await PinepodsLoginService.checkServer(
      'https://does-not-exist.invalid',
      client: client,
    );

    expect(r.isPinepods, isFalse);
    expect(r.errorMessage, isNotNull);
    // Must NOT be the old misleading generic message.
    expect(r.errorMessage, isNot(equals('Not a valid PinePods server')));
    expect(
      r.errorMessage!.toLowerCase(),
      anyOf(contains('reach'), contains('dns'), contains('connection')),
    );
  });

  test('maps a reachable-but-not-PinePods host to a distinct message', () async {
    final client = MockClient((_) async => http.Response('{"something_else": true}', 200));

    final r = await PinepodsLoginService.checkServer(
      'https://example.com',
      client: client,
    );

    expect(r.isPinepods, isFalse);
    expect(r.errorMessage, isNotNull);
    expect(r.errorMessage!.toLowerCase(), contains('does not look like a pinepods'));
  });

  test('a genuine PinePods response checks out', () async {
    final client = MockClient((_) async => http.Response('{"pinepods_instance": true}', 200));

    final r = await PinepodsLoginService.checkServer(
      'https://pods.example.com',
      client: client,
    );

    expect(r.isPinepods, isTrue);
    expect(r.errorMessage, isNull);
  });

  test('a malformed address is handled gracefully without throwing', () async {
    // No injected client on purpose: a schemeless URL makes the real client
    // throw synchronously before any I/O, exercising the catch-all mapping.
    final r = await PinepodsLoginService.checkServer('not-a-url');

    expect(r.isPinepods, isFalse);
    expect(r.errorMessage, isNotNull);
  });
}
