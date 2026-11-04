import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/ui/themes.dart';

void main() {
  group('ThemeRegistry.normalizeKey', () {
    test('keeps exact keys', () {
      expect(ThemeRegistry.normalizeKey('Nordic Light'), 'Nordic Light');
    });

    test('matches server names regardless of case', () {
      expect(ThemeRegistry.normalizeKey('light'), 'Light');
      expect(ThemeRegistry.normalizeKey('NORDIC'), 'Nordic');
    });

    test('falls back to Dark for unknown names', () {
      expect(ThemeRegistry.normalizeKey('My Custom Theme'), 'Dark');
    });

    test('always yields exactly one dropdown item', () {
      for (final stored in ['light', 'Dark', 'My Custom Theme']) {
        final key = ThemeRegistry.normalizeKey(stored);
        expect(ThemeRegistry.themeList.where((t) => t.key == key).length, 1);
      }
    });
  });
}
