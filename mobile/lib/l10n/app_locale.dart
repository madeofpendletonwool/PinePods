import 'package:flutter/widgets.dart';

/// UI languages, keyed by the same codes the server and web app use
/// (`Users.Language`), with each language's own name for the picker.
const Map<String, String> appLanguages = {
  'en': 'English',
  'ar': 'العربية',
  'be': 'Беларуская',
  'bg': 'Български',
  'bn': 'বাংলা',
  'ca': 'Català',
  'cs': 'Čeština',
  'da': 'Dansk',
  'de': 'Deutsch',
  'es': 'Español',
  'et': 'Eesti',
  'eu': 'Euskera',
  'fa': 'فارسی',
  'fi': 'Suomi',
  'fr': 'Français',
  'gu': 'ગુજરાતી',
  'he': 'עברית',
  'hi': 'हिन्दी',
  'hr': 'Hrvatski',
  'hu': 'Magyar',
  'it': 'Italiano',
  'ja': '日本語',
  'ko': '한국어',
  'lt': 'Lietuvių',
  'nb': 'Norsk Bokmål',
  'nl': 'Nederlands',
  'pl': 'Polski',
  'pt': 'Português',
  'pt-BR': 'Português (Brasil)',
  'ro': 'Română',
  'ru': 'Русский',
  'sk': 'Slovenčina',
  'sl': 'Slovenščina',
  'sv': 'Svenska',
  'tr': 'Türkçe',
  'uk': 'Українська',
  'vi': 'Tiếng Việt',
  'zh-Hans': '简体中文',
  'zh-Hant': '繁體中文',
};

/// The Flutter [Locale] for a language code such as `pt-BR` or `zh-Hant`.
Locale localeForLanguageCode(String code) {
  final parts = code.split(RegExp('[-_]'));
  if (parts.length == 1) return Locale(parts[0]);
  final sub = parts[1];
  if (sub.length == 4) {
    return Locale.fromSubtags(languageCode: parts[0], scriptCode: sub);
  }
  return Locale(parts[0], sub);
}

/// The best supported language code for [locale], or null if none fits.
///
/// Chinese picks Simplified or Traditional from the script, falling back to
/// the region; Portuguese picks Brazilian only for Brazil.
String? languageCodeForLocale(Locale locale) {
  final lang = locale.languageCode;
  if (lang == 'zh') {
    final script = locale.scriptCode;
    if (script == 'Hant') return 'zh-Hant';
    if (script == 'Hans') return 'zh-Hans';
    const traditionalRegions = {'TW', 'HK', 'MO'};
    return traditionalRegions.contains(locale.countryCode) ? 'zh-Hant' : 'zh-Hans';
  }
  if (lang == 'pt') return locale.countryCode == 'BR' ? 'pt-BR' : 'pt';
  // Old Hebrew / Norwegian codes some devices still report.
  if (lang == 'iw') return 'he';
  if (lang == 'no' || lang == 'nn') return 'nb';
  if (appLanguages.containsKey(lang)) return lang;
  return null;
}

/// Normalises a stored or server-provided code to a supported one.
String? normalizeLanguageCode(String? code) {
  if (code == null || code.isEmpty) return null;
  if (appLanguages.containsKey(code)) return code;
  return languageCodeForLocale(localeForLanguageCode(code));
}

/// Picks the UI locale: the explicit [preferred] code if it is supported,
/// else the first supported device locale, else English.
Locale resolveAppLocale(String? preferred, List<Locale>? deviceLocales) {
  final explicit = normalizeLanguageCode(preferred);
  if (explicit != null) return localeForLanguageCode(explicit);
  for (final locale in deviceLocales ?? const <Locale>[]) {
    final code = languageCodeForLocale(locale);
    if (code != null) return localeForLanguageCode(code);
  }
  return const Locale('en');
}
