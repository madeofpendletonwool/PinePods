// lib/services/error_handling_service.dart
import 'dart:io';
import 'package:http/http.dart' as http;

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
    if (error == null) return 'An unknown error occurred';
    
    if (isServerConnectionError(error)) {
      return 'Unable to connect to the PinePods server. Please check your internet connection and server settings.';
    }
    
    if (isAuthenticationError(error)) {
      return 'Authentication failed. Please check your login credentials.';
    }
    
    if (isServerError(error)) {
      return 'The PinePods server is experiencing issues. Please try again later.';
    }
    
    // Return the original error message for other types of errors
    return error.toString();
  }

  /// Gets an appropriate title for the error
  static String getErrorTitle(dynamic error) {
    if (error == null) return 'Error';
    
    if (isServerConnectionError(error)) {
      return 'Server Unavailable';
    }
    
    if (isAuthenticationError(error)) {
      return 'Authentication Error';
    }
    
    if (isServerError(error)) {
      return 'Server Error';
    }
    
    return 'Error';
  }

  /// Gets troubleshooting suggestions based on the error type
  static List<String> getTroubleshootingSteps(dynamic error) {
    if (error == null) return ['Please try again later'];
    
    if (isServerConnectionError(error)) {
      return [
        'Check your internet connection',
        'Verify server URL in settings',
        'Ensure the PinePods server is running',
        'Check if the server port is accessible',
        'Contact your administrator if the issue persists',
      ];
    }
    
    if (isAuthenticationError(error)) {
      return [
        'Check your username and password',
        'Ensure your account is still active',
        'Try logging out and logging back in',
        'Contact your administrator for help',
      ];
    }
    
    if (isServerError(error)) {
      return [
        'Wait a few minutes and try again',
        'Check if the server is overloaded',
        'Contact your administrator',
        'Check server logs for more details',
      ];
    }
    
    return [
      'Try refreshing the page',
      'Restart the app if the issue persists',
      'Contact support for assistance',
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