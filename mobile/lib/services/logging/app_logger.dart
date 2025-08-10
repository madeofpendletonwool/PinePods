// lib/services/logging/app_logger.dart
import 'dart:io';
import 'dart:collection';
import 'package:flutter/foundation.dart';
import 'package:device_info_plus/device_info_plus.dart';
import 'package:package_info_plus/package_info_plus.dart';

enum LogLevel {
  debug,
  info,
  warning,
  error,
  critical,
}

class LogEntry {
  final DateTime timestamp;
  final LogLevel level;
  final String tag;
  final String message;
  final String? stackTrace;

  LogEntry({
    required this.timestamp,
    required this.level,
    required this.tag,
    required this.message,
    this.stackTrace,
  });

  String get levelString {
    switch (level) {
      case LogLevel.debug:
        return 'DEBUG';
      case LogLevel.info:
        return 'INFO';
      case LogLevel.warning:
        return 'WARN';
      case LogLevel.error:
        return 'ERROR';
      case LogLevel.critical:
        return 'CRITICAL';
    }
  }

  String get formattedMessage {
    final timeStr = timestamp.toString().substring(0, 19); // Remove milliseconds for readability
    var result = '[$timeStr] [$levelString] [$tag] $message';
    if (stackTrace != null && stackTrace!.isNotEmpty) {
      result += '\nStackTrace: $stackTrace';
    }
    return result;
  }
}

class DeviceInfo {
  final String platform;
  final String osVersion;
  final String model;
  final String manufacturer;
  final String appVersion;
  final String buildNumber;

  DeviceInfo({
    required this.platform,
    required this.osVersion,
    required this.model,
    required this.manufacturer,
    required this.appVersion,
    required this.buildNumber,
  });

  String get formattedInfo {
    return '''
Device Information:
- Platform: $platform
- OS Version: $osVersion
- Model: $model
- Manufacturer: $manufacturer
- App Version: $appVersion
- Build Number: $buildNumber
''';
  }
}

class AppLogger {
  static final AppLogger _instance = AppLogger._internal();
  factory AppLogger() => _instance;
  AppLogger._internal();

  static const int maxLogEntries = 1000; // Keep last 1000 log entries
  final Queue<LogEntry> _logs = Queue<LogEntry>();
  DeviceInfo? _deviceInfo;

  // Initialize the logger and collect device info
  Future<void> initialize() async {
    await _collectDeviceInfo();
    
    // Log initialization
    log(LogLevel.info, 'AppLogger', 'Logger initialized successfully');
  }

  Future<void> _collectDeviceInfo() async {
    try {
      final deviceInfoPlugin = DeviceInfoPlugin();
      final packageInfo = await PackageInfo.fromPlatform();

      if (Platform.isAndroid) {
        final androidInfo = await deviceInfoPlugin.androidInfo;
        _deviceInfo = DeviceInfo(
          platform: 'Android',
          osVersion: 'Android ${androidInfo.version.release} (API ${androidInfo.version.sdkInt})',
          model: '${androidInfo.manufacturer} ${androidInfo.model}',
          manufacturer: androidInfo.manufacturer,
          appVersion: packageInfo.version,
          buildNumber: packageInfo.buildNumber,
        );
      } else if (Platform.isIOS) {
        final iosInfo = await deviceInfoPlugin.iosInfo;
        _deviceInfo = DeviceInfo(
          platform: 'iOS',
          osVersion: '${iosInfo.systemName} ${iosInfo.systemVersion}',
          model: iosInfo.model,
          manufacturer: 'Apple',
          appVersion: packageInfo.version,
          buildNumber: packageInfo.buildNumber,
        );
      } else {
        _deviceInfo = DeviceInfo(
          platform: Platform.operatingSystem,
          osVersion: Platform.operatingSystemVersion,
          model: 'Unknown',
          manufacturer: 'Unknown',
          appVersion: packageInfo.version,
          buildNumber: packageInfo.buildNumber,
        );
      }
    } catch (e) {
      // If device info collection fails, create a basic info object
      try {
        final packageInfo = await PackageInfo.fromPlatform();
        _deviceInfo = DeviceInfo(
          platform: Platform.operatingSystem,
          osVersion: Platform.operatingSystemVersion,
          model: 'Unknown',
          manufacturer: 'Unknown',
          appVersion: packageInfo.version,
          buildNumber: packageInfo.buildNumber,
        );
      } catch (e2) {
        _deviceInfo = DeviceInfo(
          platform: 'Unknown',
          osVersion: 'Unknown',
          model: 'Unknown',
          manufacturer: 'Unknown',
          appVersion: 'Unknown',
          buildNumber: 'Unknown',
        );
      }
    }
  }

  void log(LogLevel level, String tag, String message, [String? stackTrace]) {
    final entry = LogEntry(
      timestamp: DateTime.now(),
      level: level,
      tag: tag,
      message: message,
      stackTrace: stackTrace,
    );

    _logs.add(entry);

    // Keep only the last maxLogEntries
    while (_logs.length > maxLogEntries) {
      _logs.removeFirst();
    }

    // Also print to console in debug mode
    if (kDebugMode) {
      print(entry.formattedMessage);
    }
  }

  // Convenience methods for different log levels
  void debug(String tag, String message) => log(LogLevel.debug, tag, message);
  void info(String tag, String message) => log(LogLevel.info, tag, message);
  void warning(String tag, String message) => log(LogLevel.warning, tag, message);
  void error(String tag, String message, [String? stackTrace]) => log(LogLevel.error, tag, message, stackTrace);
  void critical(String tag, String message, [String? stackTrace]) => log(LogLevel.critical, tag, message, stackTrace);

  // Log an exception with automatic stack trace
  void logException(String tag, String message, dynamic exception, [StackTrace? stackTrace]) {
    final stackTraceStr = stackTrace?.toString() ?? exception.toString();
    error(tag, '$message: $exception', stackTraceStr);
  }

  // Get all logs
  List<LogEntry> get logs => _logs.toList();

  // Get logs filtered by level
  List<LogEntry> getLogsByLevel(LogLevel level) {
    return _logs.where((log) => log.level == level).toList();
  }

  // Get logs from a specific time period
  List<LogEntry> getLogsInTimeRange(DateTime start, DateTime end) {
    return _logs.where((log) => 
      log.timestamp.isAfter(start) && log.timestamp.isBefore(end)
    ).toList();
  }

  // Get formatted log string for copying
  String getFormattedLogs() {
    final buffer = StringBuffer();
    
    // Add device info
    if (_deviceInfo != null) {
      buffer.writeln(_deviceInfo!.formattedInfo);
    }
    
    // Add separator
    buffer.writeln('=' * 50);
    buffer.writeln('Application Logs:');
    buffer.writeln('=' * 50);
    
    // Add all logs
    for (final log in _logs) {
      buffer.writeln(log.formattedMessage);
    }
    
    // Add footer
    buffer.writeln();
    buffer.writeln('=' * 50);
    buffer.writeln('End of logs - Total entries: ${_logs.length}');
    buffer.writeln('Bug reports: https://github.com/madeofpendletonwool/pinepods/issues');
    
    return buffer.toString();
  }

  // Clear all logs
  void clearLogs() {
    _logs.clear();
    log(LogLevel.info, 'AppLogger', 'Logs cleared by user');
  }

  // Get device info
  DeviceInfo? get deviceInfo => _deviceInfo;
}