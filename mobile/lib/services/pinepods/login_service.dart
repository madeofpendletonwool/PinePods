import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'package:http/http.dart' as http;
import 'package:pinepods_mobile/l10n/L.dart';

/// Outcome of checking whether a URL is a reachable PinePods server.
///
/// Distinguishes the failure modes that the old generic
/// "Not a valid PinePods server" message used to collapse together (TLS/cert
/// trust, DNS/connection, timeout, non-PinePods response) so the UI can tell
/// the user what actually went wrong.
class ServerCheckResult {
  const ServerCheckResult._(this.isPinepods, this.errorMessage);

  final bool isPinepods;
  final String? errorMessage;

  static const ServerCheckResult ok = ServerCheckResult._(true, null);
  factory ServerCheckResult.error(String message) =>
      ServerCheckResult._(false, message);
}

class PinepodsLoginService {
  static const String userAgent = 'PinePods Mobile/1.0'; // i18n-ignore
  static const Duration _requestTimeout = Duration(seconds: 20);

  /// Verify if the server is a valid PinePods instance.
  ///
  /// Kept for backwards compatibility; prefer [checkServer] which reports *why*
  /// a check failed.
  static Future<bool> verifyPinepodsInstance(String serverUrl) async {
    return (await checkServer(serverUrl)).isPinepods;
  }

  /// Check whether [serverUrl] is a reachable PinePods instance, surfacing the
  /// specific failure reason (TLS/certificate, DNS/connection, timeout, or a
  /// reachable-but-not-PinePods host).
  ///
  /// [client] is injectable so this can be unit-tested against canned responses
  /// and failures without touching the network; when omitted a short-lived
  /// [http.Client] is created and closed for the single request.
  static Future<ServerCheckResult> checkServer(String serverUrl,
      {http.Client? client}) async {
    final normalizedUrl = serverUrl.trim().replaceAll(RegExp(r'/$'), '');
    final url = Uri.parse('$normalizedUrl/api/pinepods_check');
    final httpClient = client ?? http.Client();

    try {
      final response = await httpClient
          .get(url, headers: {'User-Agent': userAgent})
          .timeout(_requestTimeout);

      if (response.statusCode == 200) {
        try {
          final data = jsonDecode(response.body);
          if (data['pinepods_instance'] == true) {
            return ServerCheckResult.ok;
          }
        } on FormatException {
          // fall through to the not-PinePods message below
        }
        return ServerCheckResult.error(
            L.current.not_a_pinepods_server);
      }
      return ServerCheckResult.error(
          L.current.server_http_not_pinepods(response.statusCode));
    } on HandshakeException catch (e) {
      return ServerCheckResult.error(_tlsMessage(e));
    } on TlsException catch (e) {
      return ServerCheckResult.error(_tlsMessage(e));
    } on SocketException catch (e) {
      final detail = e.osError?.message ?? e.message;
      return ServerCheckResult.error(
          L.current.couldnt_reach_server(detail));
    } on TimeoutException {
      return ServerCheckResult.error(
          L.current.server_too_slow);
    } on http.ClientException catch (e) {
      return ServerCheckResult.error(L.current.connection_failed(e.message));
    } on FormatException {
      return ServerCheckResult.error(
          L.current.invalid_server_address);
    } catch (e) {
      return ServerCheckResult.error(L.current.could_not_connect(e.toString()));
    } finally {
      if (client == null) httpClient.close();
    }
  }

  static String _tlsMessage(Exception e) {
    return L.current.tls_not_trusted;
  }

  /// Initial login - returns either API key or MFA session info
  static Future<InitialLoginResponse> initialLogin(String serverUrl, String username, String password) async {
    try {
      final normalizedUrl = serverUrl.trim().replaceAll(RegExp(r'/$'), '');
      final credentials = base64Encode(utf8.encode('$username:$password'));
      final authHeader = 'Basic $credentials'; // i18n-ignore
      final url = Uri.parse('$normalizedUrl/api/data/get_key');

      final response = await http.get(
        url,
        headers: {
          'Authorization': authHeader,
          'User-Agent': userAgent,
        },
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        
        // Check if MFA is required
        if (data['status'] == 'mfa_required' && data['mfa_required'] == true) {
          return InitialLoginResponse.mfaRequired(
            serverUrl: normalizedUrl,
            username: username,
            userId: data['user_id'],
            mfaSessionToken: data['mfa_session_token'],
          );
        }
        
        // Normal flow - no MFA required
        final apiKey = data['retrieved_key'];
        if (apiKey != null) {
          return InitialLoginResponse.success(apiKey: apiKey);
        }
      }
      
      return InitialLoginResponse.failure(L.current.authentication_failed_plain);
    } catch (e) {
      return InitialLoginResponse.failure(L.current.error_with_detail(e.toString()));
    }
  }

  /// Legacy method for backwards compatibility
  @deprecated
  static Future<String?> getApiKey(String serverUrl, String username, String password) async {
    final result = await initialLogin(serverUrl, username, password);
    return result.isSuccess ? result.apiKey : null;
  }

  /// Verify API key is valid
  static Future<bool> verifyApiKey(String serverUrl, String apiKey) async {
    try {
      final normalizedUrl = serverUrl.trim().replaceAll(RegExp(r'/$'), '');
      final url = Uri.parse('$normalizedUrl/api/data/verify_key');

      final response = await http.get(
        url,
        headers: {
          'Api-Key': apiKey,
          'User-Agent': userAgent,
        },
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data['status'] == 'success';
      }
      return false;
    } catch (e) {
      return false;
    }
  }

  /// Get user ID
  static Future<int?> getUserId(String serverUrl, String apiKey) async {
    try {
      final normalizedUrl = serverUrl.trim().replaceAll(RegExp(r'/$'), '');
      final url = Uri.parse('$normalizedUrl/api/data/get_user');

      final response = await http.get(
        url,
        headers: {
          'Api-Key': apiKey,
          'User-Agent': userAgent,
        },
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        if (data['status'] == 'success' && data['retrieved_id'] != null) {
          return data['retrieved_id'] as int;
        }
      }
      return null;
    } catch (e) {
      return null;
    }
  }

  /// Get user details
  static Future<UserDetails?> getUserDetails(String serverUrl, String apiKey, int userId) async {
    try {
      final normalizedUrl = serverUrl.trim().replaceAll(RegExp(r'/$'), '');
      final url = Uri.parse('$normalizedUrl/api/data/user_details_id/$userId');

      final response = await http.get(
        url,
        headers: {
          'Api-Key': apiKey,
          'User-Agent': userAgent,
        },
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return UserDetails.fromJson(data);
      }
      return null;
    } catch (e) {
      return null;
    }
  }

  /// Get API configuration
  static Future<ApiConfig?> getApiConfig(String serverUrl, String apiKey) async {
    try {
      final normalizedUrl = serverUrl.trim().replaceAll(RegExp(r'/$'), '');
      final url = Uri.parse('$normalizedUrl/api/data/config');

      final response = await http.get(
        url,
        headers: {
          'Api-Key': apiKey,
          'User-Agent': userAgent,
        },
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return ApiConfig.fromJson(data);
      }
      return null;
    } catch (e) {
      return null;
    }
  }

  /// Check if MFA is enabled for user
  static Future<bool> checkMfaEnabled(String serverUrl, String apiKey, int userId) async {
    try {
      final normalizedUrl = serverUrl.trim().replaceAll(RegExp(r'/$'), '');
      final url = Uri.parse('$normalizedUrl/api/data/check_mfa_enabled/$userId');

      final response = await http.get(
        url,
        headers: {
          'Api-Key': apiKey,
          'User-Agent': userAgent,
        },
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data['mfa_enabled'] == true;
      }
      return false;
    } catch (e) {
      return false;
    }
  }

  /// Verify MFA code and get API key during login (secure flow)
  static Future<String?> verifyMfaAndGetKey(String serverUrl, String mfaSessionToken, String mfaCode) async {
    try {
      final normalizedUrl = serverUrl.trim().replaceAll(RegExp(r'/$'), '');
      final url = Uri.parse('$normalizedUrl/api/data/verify_mfa_and_get_key');

      final requestBody = jsonEncode({
        'mfa_session_token': mfaSessionToken,
        'mfa_code': mfaCode,
      });

      final response = await http.post(
        url,
        headers: {
          'Content-Type': 'application/json',
          'User-Agent': userAgent,
        },
        body: requestBody,
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        if (data['verified'] == true && data['status'] == 'success') {
          return data['retrieved_key'];
        }
      }
      return null;
    } catch (e) {
      return null;
    }
  }

  /// Legacy MFA verification (for post-login MFA checks)
  @deprecated
  static Future<bool> verifyMfa(String serverUrl, String apiKey, int userId, String mfaCode) async {
    try {
      final normalizedUrl = serverUrl.trim().replaceAll(RegExp(r'/$'), '');
      final url = Uri.parse('$normalizedUrl/api/data/verify_mfa');

      final requestBody = jsonEncode({
        'user_id': userId,
        'mfa_code': mfaCode,
      });

      final response = await http.post(
        url,
        headers: {
          'Api-Key': apiKey,
          'Content-Type': 'application/json',
          'User-Agent': userAgent,
        },
        body: requestBody,
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data['verified'] == true;
      }
      return false;
    } catch (e) {
      return false;
    }
  }

  /// Complete login flow (new secure MFA implementation)
  static Future<LoginResult> login(String serverUrl, String username, String password) async {
    try {
      // Step 1: Verify server
      final serverCheck = await checkServer(serverUrl);
      if (!serverCheck.isPinepods) {
        return LoginResult.failure(
            serverCheck.errorMessage ?? L.current.not_a_valid_pinepods_server);
      }

      // Step 2: Initial login - get API key or MFA session
      final initialResult = await initialLogin(serverUrl, username, password);
      
      if (!initialResult.isSuccess) {
        return LoginResult.failure(initialResult.errorMessage ?? L.current.login_failed);
      }
      
      if (initialResult.requiresMfa) {
        // MFA required - return MFA prompt state
        return LoginResult.mfaRequired(
          serverUrl: initialResult.serverUrl!,
          username: username,
          userId: initialResult.userId!,
          mfaSessionToken: initialResult.mfaSessionToken!,
        );
      }

      // No MFA required - complete login with API key
      return await _completeLoginWithApiKey(
        serverUrl, 
        username, 
        initialResult.apiKey!,
      );
    } catch (e) {
      return LoginResult.failure(L.current.error_with_detail(e.toString()));
    }
  }

  /// Complete MFA login flow
  static Future<LoginResult> completeMfaLogin({
    required String serverUrl,
    required String username,
    required String mfaSessionToken,
    required String mfaCode,
  }) async {
    try {
      // Verify MFA and get API key
      final apiKey = await verifyMfaAndGetKey(serverUrl, mfaSessionToken, mfaCode);
      if (apiKey == null) {
        return LoginResult.failure(L.current.invalid_mfa_code);
      }

      // Complete login with verified API key
      return await _completeLoginWithApiKey(serverUrl, username, apiKey);
    } catch (e) {
      return LoginResult.failure(L.current.error_with_detail(e.toString()));
    }
  }

  /// Complete login flow with API key (common logic)
  static Future<LoginResult> _completeLoginWithApiKey(String serverUrl, String username, String apiKey) async {
    // Step 1: Verify API key
    final isValidKey = await verifyApiKey(serverUrl, apiKey);
    if (!isValidKey) {
      return LoginResult.failure(L.current.api_key_verification_failed);
    }

    // Step 2: Get user ID
    final userId = await getUserId(serverUrl, apiKey);
    if (userId == null) {
      return LoginResult.failure(L.current.failed_to_get_user_id);
    }

    // Step 3: Get user details
    final userDetails = await getUserDetails(serverUrl, apiKey, userId);
    if (userDetails == null) {
      return LoginResult.failure(L.current.failed_to_get_user_details);
    }

    // Step 4: Get API configuration
    final apiConfig = await getApiConfig(serverUrl, apiKey);
    if (apiConfig == null) {
      return LoginResult.failure(L.current.failed_to_get_server_config);
    }

    return LoginResult.success(
      serverUrl: serverUrl,
      apiKey: apiKey,
      userId: userId,
      userDetails: userDetails,
      apiConfig: apiConfig,
    );
  }
}

class InitialLoginResponse {
  final bool isSuccess;
  final bool requiresMfa;
  final String? errorMessage;
  final String? apiKey;
  final String? serverUrl;
  final String? username;
  final int? userId;
  final String? mfaSessionToken;

  InitialLoginResponse._({
    required this.isSuccess,
    required this.requiresMfa,
    this.errorMessage,
    this.apiKey,
    this.serverUrl,
    this.username,
    this.userId,
    this.mfaSessionToken,
  });

  factory InitialLoginResponse.success({required String apiKey}) {
    return InitialLoginResponse._(
      isSuccess: true,
      requiresMfa: false,
      apiKey: apiKey,
    );
  }

  factory InitialLoginResponse.mfaRequired({
    required String serverUrl,
    required String username,
    required int userId,
    required String mfaSessionToken,
  }) {
    return InitialLoginResponse._(
      isSuccess: true,
      requiresMfa: true,
      serverUrl: serverUrl,
      username: username,
      userId: userId,
      mfaSessionToken: mfaSessionToken,
    );
  }

  factory InitialLoginResponse.failure(String errorMessage) {
    return InitialLoginResponse._(
      isSuccess: false,
      requiresMfa: false,
      errorMessage: errorMessage,
    );
  }
}

class LoginResult {
  final bool isSuccess;
  final bool requiresMfa;
  final String? errorMessage;
  final String? serverUrl;
  final String? apiKey;
  final String? username;
  final int? userId;
  final String? mfaSessionToken;
  final UserDetails? userDetails;
  final ApiConfig? apiConfig;

  LoginResult._({
    required this.isSuccess,
    required this.requiresMfa,
    this.errorMessage,
    this.serverUrl,
    this.apiKey,
    this.username,
    this.userId,
    this.mfaSessionToken,
    this.userDetails,
    this.apiConfig,
  });

  factory LoginResult.success({
    required String serverUrl,
    required String apiKey,
    required int userId,
    required UserDetails userDetails,
    required ApiConfig apiConfig,
  }) {
    return LoginResult._(
      isSuccess: true,
      requiresMfa: false,
      serverUrl: serverUrl,
      apiKey: apiKey,
      userId: userId,
      userDetails: userDetails,
      apiConfig: apiConfig,
    );
  }

  factory LoginResult.failure(String errorMessage) {
    return LoginResult._(
      isSuccess: false,
      requiresMfa: false,
      errorMessage: errorMessage,
    );
  }

  factory LoginResult.mfaRequired({
    required String serverUrl,
    required String username,
    required int userId,
    required String mfaSessionToken,
  }) {
    return LoginResult._(
      isSuccess: false,
      requiresMfa: true,
      serverUrl: serverUrl,
      username: username,
      userId: userId,
      mfaSessionToken: mfaSessionToken,
    );
  }
}

class UserDetails {
  final int userId;
  final String? fullname;
  final String? username;
  final String? email;
  final bool useGravatar;

  UserDetails({
    required this.userId,
    this.fullname,
    this.username,
    this.email,
    this.useGravatar = false,
  });

  factory UserDetails.fromJson(Map<String, dynamic> json) {
    return UserDetails(
      userId: json['UserID'],
      fullname: json['Fullname'],
      username: json['Username'],
      email: json['Email'],
      useGravatar: json['UseGravatar'] == true,
    );
  }
}

class ApiConfig {
  final String? apiUrl;
  final String? proxyUrl;
  final String? proxyHost;
  final String? proxyPort;
  final String? proxyProtocol;
  final String? reverseProxy;
  final String? peopleUrl;

  ApiConfig({
    this.apiUrl,
    this.proxyUrl,
    this.proxyHost,
    this.proxyPort,
    this.proxyProtocol,
    this.reverseProxy,
    this.peopleUrl,
  });

  factory ApiConfig.fromJson(Map<String, dynamic> json) {
    return ApiConfig(
      apiUrl: json['api_url'],
      proxyUrl: json['proxy_url'],
      proxyHost: json['proxy_host'],
      proxyPort: json['proxy_port'],
      proxyProtocol: json['proxy_protocol'],
      reverseProxy: json['reverse_proxy'],
      peopleUrl: json['people_url'],
    );
  }
}