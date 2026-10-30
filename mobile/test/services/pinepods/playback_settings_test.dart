import 'dart:convert';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';

void main() {
  for (final customized in [true, false, null]) {
    test('decodes explicit podcast speed override: $customized', () async {
      final payload = <String, dynamic>{
        'playback_speed': 1.75,
        'start_skip': 10,
        'end_skip': 20,
      };
      if (customized != null) payload['playback_speed_customized'] = customized;
      final client = MockClient(
        (_) async => http.Response(jsonEncode(payload), 200),
      );
      addTearDown(client.close);
      final service = PinepodsService(client: client);
      service.setCredentials('https://example.test', 'test-key');
      final settings = await service.getPlayEpisodeDetails(1, 2, false);
      expect(settings.playbackSpeed, 1.75);
      expect(settings.playbackSpeedCustomized, customized ?? false);
      expect(settings.startSkip, 10);
      expect(settings.endSkip, 20);
    });
  }
}
