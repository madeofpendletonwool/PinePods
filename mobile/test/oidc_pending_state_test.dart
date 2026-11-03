// A callback link only signs the app in for the login it started: one carrying another state,
// or arriving with no login in progress, would sign it into someone else's account.

import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/services/pinepods/oidc_service.dart';

void main() {
  setUp(OidcService.clearPendingState);

  test('accepts the state of the login in progress, once', () {
    OidcService.rememberPendingState('abc');
    expect(OidcService.takePendingState('abc'), isTrue);
    expect(OidcService.takePendingState('abc'), isFalse);
  });

  test('refuses a different state and ends the login', () {
    OidcService.rememberPendingState('abc');
    expect(OidcService.takePendingState('attacker'), isFalse);
    expect(OidcService.takePendingState('abc'), isFalse);
  });

  test('refuses callbacks when no login is in progress', () {
    expect(OidcService.takePendingState('abc'), isFalse);
    expect(OidcService.takePendingState(null), isFalse);
  });

  test('parses the state the server sends back with the key', () {
    final result = OidcService.parseCallback('pinepods://auth/callback?api_key=k&state=abc');
    expect(result.hasApiKey, isTrue);
    expect(result.state, 'abc');
  });
}
