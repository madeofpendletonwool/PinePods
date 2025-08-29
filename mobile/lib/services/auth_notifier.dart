import 'package:flutter/material.dart';

// Global authentication notifier for cross-context communication
class AuthNotifier {
  static VoidCallback? _globalLoginSuccessCallback;
  
  static void setGlobalLoginSuccessCallback(VoidCallback? callback) {
    _globalLoginSuccessCallback = callback;
  }
  
  static void notifyLoginSuccess() {
    _globalLoginSuccessCallback?.call();
  }
  
  static void clearGlobalLoginSuccessCallback() {
    _globalLoginSuccessCallback = null;
  }
}