import 'dart:convert';

import 'package:http/http.dart' as http;
import 'package:logging/logging.dart';
import 'package:pinepods_mobile/l10n/app_locale.dart';

/// Reads and writes the account's UI language on the PinePods server — the
/// same preference the web app uses.
class AccountLanguage {
  static final _log = Logger('AccountLanguage');

  /// The language the user chose for their account, or null when it is just
  /// the server default (i.e. they never picked one) or can't be fetched.
  static Future<String?> fetchExplicit({
    required String server,
    required String apiKey,
    required int userId,
  }) async {
    try {
      final userRes = await http.get(
        Uri.parse('$server/api/data/get_user_language?user_id=$userId'),
        headers: {'Api-Key': apiKey},
      );
      if (userRes.statusCode != 200) return null;
      final user = normalizeLanguageCode(jsonDecode(userRes.body)['language'] as String?);
      if (user == null) return null;

      final defaultRes = await http.get(Uri.parse('$server/api/data/get_server_default_language'));
      final serverDefault = defaultRes.statusCode == 200
          ? normalizeLanguageCode(jsonDecode(defaultRes.body)['default_language'] as String?)
          : null;
      return user == (serverDefault ?? 'en') ? null : user;
    } catch (e) {
      _log.fine('Could not fetch account language: $e');
      return null;
    }
  }

  /// Saves [language] to the account. With null, resets it to the server
  /// default so the device language is used again.
  static Future<bool> save({
    required String server,
    required String apiKey,
    required int userId,
    String? language,
  }) async {
    try {
      var code = language;
      if (code == null) {
        final res = await http.get(Uri.parse('$server/api/data/get_server_default_language'));
        code = res.statusCode == 200 ? jsonDecode(res.body)['default_language'] as String? : null;
        code ??= 'en';
      }
      final res = await http.put(
        Uri.parse('$server/api/data/update_user_language'),
        headers: {'Api-Key': apiKey, 'Content-Type': 'application/json'},
        body: jsonEncode({'user_id': userId, 'language': code}),
      );
      return res.statusCode == 200;
    } catch (e) {
      _log.warning('Could not save account language: $e');
      return false;
    }
  }
}
