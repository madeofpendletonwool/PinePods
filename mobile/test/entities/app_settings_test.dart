import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/entities/app_settings.dart';

void main() {
  group('sensibleDefaults', () {
    test('produces a usable dark-theme, iTunes-search, no-account default state', () {
      final settings = AppSettings.sensibleDefaults();

      expect(settings.theme, 'Dark');
      expect(settings.searchProvider, 'itunes');
      expect(settings.playbackSpeed, 1.0);
      expect(settings.autoUpdateEpisodePeriod, -1);
      expect(settings.showFunding, isTrue);
      expect(settings.pinepodsServer, isNull);
      expect(settings.pinepodsUserId, isNull);
      expect(settings.bottomBarOrder, isNotEmpty);
    });
  });

  group('copyWith', () {
    test('overrides only the specified fields, keeping the rest', () {
      final base = AppSettings.sensibleDefaults();

      final updated = base.copyWith(theme: 'Light', pinepodsUserId: 42);

      expect(updated.theme, 'Light');
      expect(updated.pinepodsUserId, 42);
      // Untouched fields carry over from the original.
      expect(updated.searchProvider, base.searchProvider);
      expect(updated.playbackSpeed, base.playbackSpeed);
      expect(updated.bottomBarOrder, base.bottomBarOrder);
    });

    test('calling copyWith with no arguments returns an equivalent settings object', () {
      final base = AppSettings.sensibleDefaults();
      final copy = base.copyWith();

      expect(copy.theme, base.theme);
      expect(copy.pinepodsServer, base.pinepodsServer);
      expect(copy.bottomBarOrder, base.bottomBarOrder);
    });
  });
}
