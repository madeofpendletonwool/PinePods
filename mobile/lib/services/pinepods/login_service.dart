import 'dart:convert';
import 'package:http/http.dart' as http;

class PinepodsLoginService {
  static const String userAgent = 'PinePods Mobile/1.0';

  /// Verify if the server is a valid PinePods instance
  static Future<bool> verifyPinepodsInstance(String serverUrl) async {
    try {
      final normalizedUrl = serverUrl.trim().replaceAll(RegExp(r'/$'), '');
      final url = Uri.parse('$normalizedUrl/api/pinepods_check');
      
      final response = await http.get(
        url,
        headers: {'User-Agent': userAgent},
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data['pinepods_instance'] == true;
      }
      return false;
    } catch (e) {
      return false;
    }
  }

  /// Initial login - returns either API key or MFA session info
  static Future<InitialLoginResponse> initialLogin(String serverUrl, String username, String password) async {
    try {
      final normalizedUrl = serverUrl.trim().replaceAll(RegExp(r'/$'), '');
      final credentials = base64Encode(utf8.encode('$username:$password'));
      final authHeader = 'Basic $credentials';
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
      
      return InitialLoginResponse.failure('Authentication failed');
    } catch (e) {
      return InitialLoginResponse.failure('Error: ${e.toString()}');
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
      final isPinepods = await verifyPinepodsInstance(serverUrl);
      if (!isPinepods) {
        return LoginResult.failure('Not a valid PinePods server');
      }

      // Step 2: Initial login - get API key or MFA session
      final initialResult = await initialLogin(serverUrl, username, password);
      
      if (!initialResult.isSuccess) {
        return LoginResult.failure(initialResult.errorMessage ?? 'Login failed');
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
      return LoginResult.failure('Error: ${e.toString()}');
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
        return LoginResult.failure('Invalid MFA code');
      }

      // Complete login with verified API key
      return await _completeLoginWithApiKey(serverUrl, username, apiKey);
    } catch (e) {
      return LoginResult.failure('Error: ${e.toString()}');
    }
  }

  /// Complete login flow with API key (common logic)
  static Future<LoginResult> _completeLoginWithApiKey(String serverUrl, String username, String apiKey) async {
    // Step 1: Verify API key
    final isValidKey = await verifyApiKey(serverUrl, apiKey);
    if (!isValidKey) {
      return LoginResult.failure('API key verification failed');
    }

    // Step 2: Get user ID
    final userId = await getUserId(serverUrl, apiKey);
    if (userId == null) {
      return LoginResult.failure('Failed to get user ID');
    }

    // Step 3: Get user details
    final userDetails = await getUserDetails(serverUrl, apiKey, userId);
    if (userDetails == null) {
      return LoginResult.failure('Failed to get user details');
    }

    // Step 4: Get API configuration
    final apiConfig = await getApiConfig(serverUrl, apiKey);
    if (apiConfig == null) {
      return LoginResult.failure('Failed to get server configuration');
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

  UserDetails({
    required this.userId,
    this.fullname,
    this.username,
    this.email,
  });

  factory UserDetails.fromJson(Map<String, dynamic> json) {
    return UserDetails(
      userId: json['UserID'],
      fullname: json['Fullname'],
      username: json['Username'],
      email: json['Email'],
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