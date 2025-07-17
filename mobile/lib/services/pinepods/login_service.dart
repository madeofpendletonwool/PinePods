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

  /// Get API key using Basic authentication
  static Future<String?> getApiKey(String serverUrl, String username, String password) async {
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
        return data['retrieved_key'];
      }
      return null;
    } catch (e) {
      return null;
    }
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

  /// Verify MFA code
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

  /// Complete login flow
  static Future<LoginResult> login(String serverUrl, String username, String password, {String? mfaCode}) async {
    try {
      // Step 1: Verify server
      final isPinepods = await verifyPinepodsInstance(serverUrl);
      if (!isPinepods) {
        return LoginResult.failure('Not a valid PinePods server');
      }

      // Step 2: Get API key
      final apiKey = await getApiKey(serverUrl, username, password);
      if (apiKey == null) {
        return LoginResult.failure('Login failed. Check your credentials.');
      }

      // Step 3: Verify API key
      final isValidKey = await verifyApiKey(serverUrl, apiKey);
      if (!isValidKey) {
        return LoginResult.failure('API key verification failed');
      }

      // Step 4: Get user ID
      final userId = await getUserId(serverUrl, apiKey);
      if (userId == null) {
        return LoginResult.failure('Failed to get user ID');
      }

      // Step 5: Check MFA
      final mfaEnabled = await checkMfaEnabled(serverUrl, apiKey, userId);
      if (mfaEnabled) {
        if (mfaCode == null || mfaCode.isEmpty) {
          return LoginResult.mfaRequired(serverUrl, apiKey, userId);
        }
        
        final mfaValid = await verifyMfa(serverUrl, apiKey, userId, mfaCode);
        if (!mfaValid) {
          return LoginResult.failure('Invalid MFA code');
        }
      }

      // Step 6: Get user details
      final userDetails = await getUserDetails(serverUrl, apiKey, userId);
      if (userDetails == null) {
        return LoginResult.failure('Failed to get user details');
      }

      // Step 7: Get API configuration
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
    } catch (e) {
      return LoginResult.failure('Error: ${e.toString()}');
    }
  }
}

class LoginResult {
  final bool isSuccess;
  final bool requiresMfa;
  final String? errorMessage;
  final String? serverUrl;
  final String? apiKey;
  final int? userId;
  final UserDetails? userDetails;
  final ApiConfig? apiConfig;

  LoginResult._({
    required this.isSuccess,
    required this.requiresMfa,
    this.errorMessage,
    this.serverUrl,
    this.apiKey,
    this.userId,
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

  factory LoginResult.mfaRequired(String serverUrl, String apiKey, int userId) {
    return LoginResult._(
      isSuccess: false,
      requiresMfa: true,
      serverUrl: serverUrl,
      apiKey: apiKey,
      userId: userId,
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