import 'dart:convert';
import 'dart:io';
import 'dart:math';
import 'package:crypto/crypto.dart';
import 'package:http/http.dart' as http;
import 'package:url_launcher/url_launcher.dart';

class OidcService {
  static const String userAgent = 'PinePods Mobile/1.0';
  static const String callbackUrlScheme = 'pinepods';
  static const String callbackPath = '/auth/callback';
  
  /// Get available OIDC providers from server
  static Future<List<OidcProvider>> getPublicProviders(String serverUrl) async {
    try {
      final normalizedUrl = serverUrl.trim().replaceAll(RegExp(r'/$'), '');
      final url = Uri.parse('$normalizedUrl/api/data/public_oidc_providers');
      
      final response = await http.get(
        url,
        headers: {'User-Agent': userAgent},
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        final providers = (data['providers'] as List)
            .map((provider) => OidcProvider.fromJson(provider))
            .toList();
        return providers;
      }
      return [];
    } catch (e) {
      return [];
    }
  }
  
  /// Generate PKCE code verifier and challenge for secure OIDC flow
  static OidcPkce generatePkce() {
    final codeVerifier = _generateCodeVerifier();
    final codeChallenge = _generateCodeChallenge(codeVerifier);
    
    return OidcPkce(
      codeVerifier: codeVerifier,
      codeChallenge: codeChallenge,
    );
  }
  
  /// Generate random state parameter
  static String generateState() {
    final random = Random.secure();
    final bytes = List<int>.generate(32, (i) => random.nextInt(256));
    return base64UrlEncode(bytes).replaceAll('=', '');
  }
  
  /// Store OIDC state on server (matches web implementation)
  static Future<bool> storeOidcState({
    required String serverUrl,
    required String state,
    required String clientId,
    String? originUrl,
    String? codeVerifier,
  }) async {
    try {
      final normalizedUrl = serverUrl.trim().replaceAll(RegExp(r'/$'), '');
      final url = Uri.parse('$normalizedUrl/api/auth/store_state');
      
      final requestBody = jsonEncode({
        'state': state,
        'client_id': clientId,
        'origin_url': originUrl,
        'code_verifier': codeVerifier,
      });
      
      final response = await http.post(
        url,
        headers: {
          'Content-Type': 'application/json',
          'User-Agent': userAgent,
        },
        body: requestBody,
      );
      
      return response.statusCode == 200;
    } catch (e) {
      return false;
    }
  }
  
  /// Build authorization URL and return it for in-app browser use
  static Future<String?> buildOidcLoginUrl({
    required OidcProvider provider,
    required String serverUrl,
    required String state,
    OidcPkce? pkce,
  }) async {
    try {
      // Store state on server first - use web origin for in-app browser
      print('OIDC: Storing state for in-app browser flow');
      final stateStored = await storeOidcState(
        serverUrl: serverUrl,
        state: state,
        clientId: provider.clientId,
        originUrl: '$serverUrl/oauth/callback', // Use web callback for in-app browser
        codeVerifier: pkce?.codeVerifier, // Include PKCE code verifier
      );
      print('OIDC: State stored successfully: $stateStored');
      
      if (!stateStored) {
        return null;
      }
      
      // Build authorization URL
      final authUri = Uri.parse(provider.authorizationUrl);
      final queryParams = <String, String>{
        'client_id': provider.clientId,
        'response_type': 'code',
        'scope': provider.scope,
        'redirect_uri': '$serverUrl/api/auth/callback',
        'state': state,
      };
      
      // Add PKCE parameters if provided
      if (pkce != null) {
        queryParams['code_challenge'] = pkce.codeChallenge;
        queryParams['code_challenge_method'] = 'S256';
      }
      
      final authUrl = authUri.replace(queryParameters: queryParams);
      
      print('OIDC: Built authorization URL: $authUrl');
      return authUrl.toString();
      
    } catch (e) {
      print('OIDC: Failed to build authorization URL: $e');
      return null;
    }
  }

  /// Extract API key from callback URL (for in-app browser)
  static String? extractApiKeyFromUrl(String url) {
    try {
      final uri = Uri.parse(url);
      
      // Check if this is our callback URL with API key
      if (uri.path.contains('/oauth/callback')) {
        return uri.queryParameters['api_key'];
      }
      
      return null;
    } catch (e) {
      print('OIDC: Failed to extract API key from URL: $e');
      return null;
    }
  }
  
  /// Handle OIDC callback and extract authentication result
  static OidcCallbackResult parseCallback(String callbackUrl) {
    try {
      final uri = Uri.parse(callbackUrl);
      final queryParams = uri.queryParameters;
      
      // Check for error
      if (queryParams.containsKey('error')) {
        return OidcCallbackResult.error(
          error: queryParams['error'] ?? 'Unknown error',
          errorDescription: queryParams['error_description'],
        );
      }
      
      // Check if we have an API key directly (PinePods backend provides this)
      final apiKey = queryParams['api_key'];
      if (apiKey != null && apiKey.isNotEmpty) {
        return OidcCallbackResult.success(
          apiKey: apiKey,
          state: queryParams['state'],
        );
      }
      
      // Fallback: Extract traditional OAuth code and state
      final code = queryParams['code'];
      final state = queryParams['state'];
      
      if (code != null && state != null) {
        return OidcCallbackResult.success(
          code: code,
          state: state,
        );
      }
      
      return OidcCallbackResult.error(
        error: 'missing_parameters',
        errorDescription: 'Neither API key nor authorization code found in callback',
      );
    } catch (e) {
      return OidcCallbackResult.error(
        error: 'parse_error',
        errorDescription: e.toString(),
      );
    }
  }
  
  /// Complete OIDC authentication by verifying with server
  static Future<OidcAuthResult> completeAuthentication({
    required String serverUrl,
    required String code,
    required String state,
    OidcPkce? pkce,
  }) async {
    try {
      final normalizedUrl = serverUrl.trim().replaceAll(RegExp(r'/$'), '');
      final url = Uri.parse('$normalizedUrl/api/auth/oidc_complete');
      
      final requestBody = <String, dynamic>{
        'code': code,
        'state': state,
      };
      
      // Add PKCE verifier if provided
      if (pkce != null) {
        requestBody['code_verifier'] = pkce.codeVerifier;
      }
      
      final response = await http.post(
        url,
        headers: {
          'Content-Type': 'application/json',
          'User-Agent': userAgent,
        },
        body: jsonEncode(requestBody),
      );
      
      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return OidcAuthResult.success(
          apiKey: data['api_key'],
          userId: data['user_id'],
          serverUrl: normalizedUrl,
        );
      } else {
        final errorData = jsonDecode(response.body);
        return OidcAuthResult.failure(
          errorData['error'] ?? 'Authentication failed',
        );
      }
    } catch (e) {
      return OidcAuthResult.failure('Network error: ${e.toString()}');
    }
  }
  
  /// Generate secure random code verifier
  static String _generateCodeVerifier() {
    final random = Random.secure();
    // Generate 32 random bytes (256 bits) which will create a ~43 character base64url string
    final bytes = List<int>.generate(32, (i) => random.nextInt(256));
    // Use base64url encoding (- and _ instead of + and /) and remove padding
    return base64UrlEncode(bytes).replaceAll('=', '');
  }
  
  /// Generate code challenge from verifier using SHA256
  static String _generateCodeChallenge(String codeVerifier) {
    final bytes = utf8.encode(codeVerifier);
    final digest = sha256.convert(bytes);
    return base64UrlEncode(digest.bytes)
        .replaceAll('=', '')
        .replaceAll('+', '-')
        .replaceAll('/', '_');
  }
}

/// OIDC Provider model
class OidcProvider {
  final int providerId;
  final String providerName;
  final String clientId;
  final String authorizationUrl;
  final String scope;
  final String? buttonColor;
  final String? buttonText;
  final String? buttonTextColor;
  final String? iconSvg;

  OidcProvider({
    required this.providerId,
    required this.providerName,
    required this.clientId,
    required this.authorizationUrl,
    required this.scope,
    this.buttonColor,
    this.buttonText,
    this.buttonTextColor,
    this.iconSvg,
  });

  factory OidcProvider.fromJson(Map<String, dynamic> json) {
    return OidcProvider(
      providerId: json['provider_id'],
      providerName: json['provider_name'],
      clientId: json['client_id'],
      authorizationUrl: json['authorization_url'],
      scope: json['scope'],
      buttonColor: json['button_color'],
      buttonText: json['button_text'],
      buttonTextColor: json['button_text_color'],
      iconSvg: json['icon_svg'],
    );
  }

  /// Get display text for the provider button
  String get displayText => buttonText ?? 'Login with $providerName';
  
  /// Get button color or default
  String get buttonColorHex => buttonColor ?? '#007bff';
  
  /// Get button text color or default
  String get buttonTextColorHex => buttonTextColor ?? '#ffffff';
}

/// PKCE (Proof Key for Code Exchange) parameters
class OidcPkce {
  final String codeVerifier;
  final String codeChallenge;

  OidcPkce({
    required this.codeVerifier,
    required this.codeChallenge,
  });
}

/// OIDC callback parsing result
class OidcCallbackResult {
  final bool isSuccess;
  final String? code;
  final String? state;
  final String? apiKey;
  final String? error;
  final String? errorDescription;

  OidcCallbackResult._({
    required this.isSuccess,
    this.code,
    this.state,
    this.apiKey,
    this.error,
    this.errorDescription,
  });

  factory OidcCallbackResult.success({
    String? code,
    String? state,
    String? apiKey,
  }) {
    return OidcCallbackResult._(
      isSuccess: true,
      code: code,
      state: state,
      apiKey: apiKey,
    );
  }

  factory OidcCallbackResult.error({
    required String error,
    String? errorDescription,
  }) {
    return OidcCallbackResult._(
      isSuccess: false,
      error: error,
      errorDescription: errorDescription,
    );
  }

  bool get hasApiKey => apiKey != null && apiKey!.isNotEmpty;
  bool get hasCode => code != null && code!.isNotEmpty;
}

/// OIDC authentication completion result
class OidcAuthResult {
  final bool isSuccess;
  final String? apiKey;
  final int? userId;
  final String? serverUrl;
  final String? errorMessage;

  OidcAuthResult._({
    required this.isSuccess,
    this.apiKey,
    this.userId,
    this.serverUrl,
    this.errorMessage,
  });

  factory OidcAuthResult.success({
    required String apiKey,
    required int userId,
    required String serverUrl,
  }) {
    return OidcAuthResult._(
      isSuccess: true,
      apiKey: apiKey,
      userId: userId,
      serverUrl: serverUrl,
    );
  }

  factory OidcAuthResult.failure(String errorMessage) {
    return OidcAuthResult._(
      isSuccess: false,
      errorMessage: errorMessage,
    );
  }
}