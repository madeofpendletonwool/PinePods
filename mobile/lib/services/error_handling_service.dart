// lib/services/error_handling_service.dart
import 'dart:io';
import 'package:http/http.dart' as http;
import 'package:pinepods_mobile/l10n/L.dart';

/// Service for handling and categorizing errors, especially server connection issues
class ErrorHandlingService {
  /// Checks if an error indicates a server connection issue
  static bool isServerConnectionError(dynamic error) {
    if (error == null) return false;
    
    final errorString = error.toString().toLowerCase();
    
    // Network-related errors
    if (error is SocketException) return true;
    if (error is HttpException) return true;
    if (error is http.ClientException) return true;
    
    // Check for common connection error patterns
    final connectionErrorPatterns = [
      'connection refused',
      'connection timeout',
      'connection failed',
      'network is unreachable',
      'no route to host',
      'connection reset',
      'connection aborted',
      'host is unreachable',
      'server unavailable',
      'service unavailable',
      'bad gateway',
      'gateway timeout',
      'connection timed out',
      'failed host lookup',
      'no address associated with hostname',
      'network unreachable',
      'operation timed out',
      'handshake failure',
      'certificate verify failed',
      'ssl handshake failed',
      'unable to connect',
      'server closed the connection',
      'connection closed',
      'broken pipe',
      'no internet connection',
      'offline',
      'dns lookup failed',
      'name resolution failed',
    ];
    
    return connectionErrorPatterns.any((pattern) => errorString.contains(pattern));
  }

  /// Checks if an error indicates authentication/authorization issues
  static bool isAuthenticationError(dynamic error) {
    if (error == null) return false;
    
    final errorString = error.toString().toLowerCase();
    
    final authErrorPatterns = [
      'unauthorized',
      'authentication failed',
      'invalid credentials',
      'access denied',
      'forbidden',
      'token expired',
      'invalid token',
      'login required',
      '401',
      '403',
    ];
    
    return authErrorPatterns.any((pattern) => errorString.contains(pattern));
  }

  /// Checks if an error indicates server-side issues (5xx errors)
  static bool isServerError(dynamic error) {
    if (error == null) return false;
    
    final errorString = error.toString().toLowerCase();
    
    final serverErrorPatterns = [
      'internal server error',
      'server error',
      'service unavailable',
      'bad gateway',
      'gateway timeout',
      '500',
      '502',
      '503',
      '504',
      '505',
    ];
    
    return serverErrorPatterns.any((pattern) => errorString.contains(pattern));
  }

  /// Gets a user-friendly error message based on the error type
  static String getUserFriendlyErrorMessage(dynamic error) {
    if (error == null) return L.current.error_unknown;
    
    if (isServerConnectionError(error)) {
      return L.current.error_cannot_connect_long;
    }
    
    if (isAuthenticationError(error)) {
      return L.current.error_auth_long;
    }
    
    if (isServerError(error)) {
      return L.current.error_server_long;
    }
    
    // Return the original error message for other types of errors
    return error.toString();
  }

  /// Gets an appropriate title for the error
  static String getErrorTitle(dynamic error) {
    if (error == null) return L.current.error_title;
    
    if (isServerConnectionError(error)) {
      return L.current.server_unavailable;
    }
    
    if (isAuthenticationError(error)) {
      return L.current.authentication_error;
    }
    
    if (isServerError(error)) {
      return L.current.server_error;
    }
    
    return L.current.error_title;
  }

  /// Gets troubleshooting suggestions based on the error type
  static List<String> getTroubleshootingSteps(dynamic error) {
    if (error == null) return [L.current.tip_try_later];
    
    if (isServerConnectionError(error)) {
      return [
        L.current.tip_check_internet,
        L.current.tip_verify_url,
        L.current.tip_server_running,
        L.current.tip_port_accessible,
        L.current.tip_contact_admin_persists,
      ];
    }
    
    if (isAuthenticationError(error)) {
      return [
        L.current.tip_check_credentials,
        L.current.tip_account_active,
        L.current.tip_relogin,
        L.current.tip_contact_admin_help,
      ];
    }
    
    if (isServerError(error)) {
      return [
        L.current.tip_wait,
        L.current.tip_overloaded,
        L.current.tip_contact_admin,
        L.current.tip_server_logs,
      ];
    }
    
    return [
      L.current.tip_refresh,
      L.current.tip_restart_app,
      L.current.tip_contact_support,
    ];
  }

  /// Wraps an async function call with error handling
  static Future<T> handleApiCall<T>(
    Future<T> Function() apiCall, {
    String? context,
  }) async {
    try {
      return await apiCall();
    } catch (error) {
      // Log the error with context if provided
      if (context != null) {
        print('API Error in $context: $error');
      }
      
      // Re-throw the error to be handled by the UI layer
      rethrow;
    }
  }
}

/// Extension to make error checking easier
extension ErrorTypeExtension on dynamic {
  bool get isServerConnectionError => ErrorHandlingService.isServerConnectionError(this);
  bool get isAuthenticationError => ErrorHandlingService.isAuthenticationError(this);
  bool get isServerError => ErrorHandlingService.isServerError(this);
  String get userFriendlyMessage => ErrorHandlingService.getUserFriendlyErrorMessage(this);
  String get errorTitle => ErrorHandlingService.getErrorTitle(this);
  List<String> get troubleshootingSteps => ErrorHandlingService.getTroubleshootingSteps(this);
}