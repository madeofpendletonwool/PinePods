import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/services/pinepods/login_service.dart';

void main() {
  test('checkServer maps an unreachable host to a connection/DNS message, not the generic one', () async {
    final r = await PinepodsLoginService.checkServer('https://does-not-exist.invalid');
    expect(r.isPinepods, isFalse);
    expect(r.errorMessage, isNotNull);
    // Must NOT be the old misleading generic message.
    expect(r.errorMessage, isNot(equals('Not a valid PinePods server')));
    expect(r.errorMessage!.toLowerCase(), anyOf(contains('reach'), contains('dns'), contains('connection')));
  });

  test('checkServer maps a malformed address to a helpful message', () async {
    final r = await PinepodsLoginService.checkServer('not-a-url');
    expect(r.isPinepods, isFalse);
    expect(r.errorMessage, isNotNull);
  });
}
