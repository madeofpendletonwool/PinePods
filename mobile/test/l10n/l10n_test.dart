import 'dart:convert';
import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/l10n/app_locale.dart';
import 'package:pinepods_mobile/l10n/app_localizations.dart';

Map<String, dynamic> _readArb(File f) => jsonDecode(f.readAsStringSync()) as Map<String, dynamic>;

/// Placeholder names used in a message, including inside ICU plurals.
Set<String> _placeholders(String message) =>
    RegExp(r'\{(\w+)[,}]').allMatches(message).map((m) => m.group(1)!).where((n) => int.tryParse(n) == null).toSet();

void main() {
  final dir = Directory('lib/l10n');
  final en = _readArb(File('lib/l10n/app_en.arb'));
  final enKeys = en.keys.where((k) => !k.startsWith('@')).toSet();
  final locales = dir
      .listSync()
      .whereType<File>()
      .where((f) => RegExp(r'app_.+\.arb$').hasMatch(f.path) && !f.path.endsWith('app_en.arb'))
      .toList();

  group('ARB files', () {
    test('every web language has a file', () {
      for (final code in appLanguages.keys.where((c) => c != 'en')) {
        expect(File('lib/l10n/app_${code.replaceAll('-', '_')}.arb').existsSync(), isTrue, reason: code);
      }
    });

    for (final file in locales) {
      final name = file.uri.pathSegments.last;
      test('$name is a clean translation of app_en.arb', () {
        final arb = _readArb(file);
        // gen-l10n reads the locale from the file name when @@locale is absent.
        if (arb.containsKey('@@locale')) {
          expect(arb['@@locale'], name.substring(4, name.length - 4), reason: '@@locale must match the file name');
        }
        for (final entry in arb.entries) {
          if (entry.key.startsWith('@')) continue;
          // Untranslated strings must be absent, not blank, or they render
          // empty instead of falling back to English.
          expect((entry.value as String).trim(), isNotEmpty, reason: '${entry.key} is blank');
          expect(enKeys, contains(entry.key), reason: '${entry.key} is not in app_en.arb');
          expect(_placeholders(entry.value as String).difference(_placeholders(en[entry.key] as String)), isEmpty,
              reason: '${entry.key} uses a placeholder the English string does not have');
        }
      });
    }
  });

  group('locale resolution', () {
    test('explicit choice wins over the device', () {
      expect(resolveAppLocale('de', const [Locale('fr')]), const Locale('de'));
    });

    test('device locale is used when nothing is chosen', () {
      expect(resolveAppLocale('', const [Locale('fr', 'CA')]), const Locale('fr'));
      expect(resolveAppLocale(null, const [Locale('xx'), Locale('es')]), const Locale('es'));
    });

    test('unsupported languages fall back to English', () {
      expect(resolveAppLocale(null, const [Locale('xx')]), const Locale('en'));
      expect(resolveAppLocale('xx', null), const Locale('en'));
    });

    test('Chinese picks the script', () {
      Locale zh(String? script, String? country) =>
          Locale.fromSubtags(languageCode: 'zh', scriptCode: script, countryCode: country);
      final hant = Locale.fromSubtags(languageCode: 'zh', scriptCode: 'Hant');
      final hans = Locale.fromSubtags(languageCode: 'zh', scriptCode: 'Hans');
      expect(resolveAppLocale(null, [zh('Hant', 'TW')]), hant);
      expect(resolveAppLocale(null, [zh(null, 'HK')]), hant);
      expect(resolveAppLocale(null, [zh('Hans', 'CN')]), hans);
      expect(resolveAppLocale(null, [zh(null, null)]), hans);
      expect(resolveAppLocale('zh-Hant', null), hant);
    });

    test('Portuguese picks Brazil only for Brazil', () {
      expect(resolveAppLocale(null, const [Locale('pt', 'BR')]), const Locale('pt', 'BR'));
      expect(resolveAppLocale(null, const [Locale('pt', 'PT')]), const Locale('pt'));
      expect(resolveAppLocale('pt-BR', null), const Locale('pt', 'BR'));
    });

    test('legacy device codes map to current ones', () {
      expect(resolveAppLocale(null, const [Locale('iw')]), const Locale('he'));
      expect(resolveAppLocale(null, const [Locale('no')]), const Locale('nb'));
    });

    test('every language code is supported by the generated localizations', () {
      for (final code in appLanguages.keys) {
        expect(AppLocalizations.delegate.isSupported(localeForLanguageCode(code)), isTrue, reason: code);
      }
    });
  });

  group('generated strings', () {
    test('keys missing from a locale fall back to English', () {
      // The stub `zh` file exists only as gen-l10n's required base for
      // zh_Hans/zh_Hant, so it never has translations.
      expect(_readArb(File('lib/l10n/app_zh.arb')).keys, ['@@locale']);
      final zh = lookupAppLocalizations(const Locale('zh'));
      final english = lookupAppLocalizations(const Locale('en'));
      expect(zh.no_episodes_match_filters, english.no_episodes_match_filters);
      expect(zh.days_count(2), english.days_count(2));
    });

    test('plurals and placeholders format in every locale', () {
      for (final locale in AppLocalizations.supportedLocales) {
        final l = lookupAppLocalizations(locale);
        expect(l.episodes_count(1), isNotEmpty, reason: '$locale');
        expect(l.episodes_count(5), isNotEmpty, reason: '$locale');
        expect(l.days_ago(3), isNotEmpty, reason: '$locale');
        expect(l.error_with_detail('x'), contains('x'), reason: '$locale');
      }
    });

    test('English plurals', () {
      final l = lookupAppLocalizations(const Locale('en'));
      expect(l.episodes_count(1), '1 episode');
      expect(l.episodes_count(3), '3 episodes');
      expect(l.days_count(1), '1 day');
      expect(l.days_count(14), '14 days');
    });
  });
}
