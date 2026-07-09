import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/entities/funding.dart';

void main() {
  test('upgrades an http funding URL to https on construction', () {
    final funding = Funding(url: 'http://example.com/donate', value: 'Support the show');
    expect(funding.url, 'https://example.com/donate');
  });

  group('toMap/fromMap', () {
    test('round-trips url and value', () {
      final original = Funding(url: 'https://example.com/donate', value: 'Support the show');
      final restored = Funding.fromMap(original.toMap());

      expect(restored.url, original.url);
      expect(restored.value, original.value);
    });
  });
}
