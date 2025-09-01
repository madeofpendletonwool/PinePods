// lib/services/logging/app_logger.dart
import 'dart:io';
import 'dart:collection';
import 'dart:convert';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:device_info_plus/device_info_plus.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:path_provider/path_provider.dart';
import 'package:path/path.dart' as path_helper;
import 'package:intl/intl.dart';

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

  static const int maxLogEntries = 1000; // Keep last 1000 log entries in memory
  static const int maxSessionFiles = 5; // Keep last 5 session log files
  static const String crashLogFileName = 'pinepods_last_crash.txt';
  
  final Queue<LogEntry> _logs = Queue<LogEntry>();
  DeviceInfo? _deviceInfo;
  File? _currentSessionFile;
  File? _crashLogFile;
  Directory? _logsDirectory;
  String? _sessionId;
  bool _isInitialized = false;

  // Initialize the logger and collect device info
  Future<void> initialize() async {
    if (_isInitialized) return;
    
    await _collectDeviceInfo();
    await _initializeLogFiles();
    await _setupCrashHandler();
    await _loadPreviousCrash();
    
    _isInitialized = true;
    
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

    // Keep only the last maxLogEntries in memory
    while (_logs.length > maxLogEntries) {
      _logs.removeFirst();
    }

    // Write to current session file asynchronously (don't await to avoid blocking)
    _writeToSessionFile(entry);

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

  // Initialize log files and directory structure
  Future<void> _initializeLogFiles() async {
    try {
      final appDocDir = await getApplicationDocumentsDirectory();
      _logsDirectory = Directory(path_helper.join(appDocDir.path, 'logs'));
      
      // Create logs directory if it doesn't exist
      if (!await _logsDirectory!.exists()) {
        await _logsDirectory!.create(recursive: true);
      }
      
      // Clean up old session files (keep only last 5)
      await _cleanupOldSessionFiles();
      
      // Create new session file
      _sessionId = DateFormat('yyyyMMdd_HHmmss').format(DateTime.now());
      _currentSessionFile = File(path_helper.join(_logsDirectory!.path, 'session_$_sessionId.log'));
      await _currentSessionFile!.create();
      
      // Initialize crash log file
      _crashLogFile = File(path_helper.join(_logsDirectory!.path, crashLogFileName));
      if (!await _crashLogFile!.exists()) {
        await _crashLogFile!.create();
      }
      
      log(LogLevel.info, 'AppLogger', 'Session log files initialized at ${_logsDirectory!.path}');
    } catch (e) {
      if (kDebugMode) {
        print('Failed to initialize log files: $e');
      }
    }
  }
  
  // Clean up old session files, keeping only the most recent ones
  Future<void> _cleanupOldSessionFiles() async {
    try {
      final files = await _logsDirectory!.list().toList();
      final sessionFiles = files
          .whereType<File>()
          .where((f) => path_helper.basename(f.path).startsWith('session_'))
          .toList();
      
      // Sort by last modified date (newest first)
      sessionFiles.sort((a, b) => b.lastModifiedSync().compareTo(a.lastModifiedSync()));
      
      // Delete files beyond the limit
      if (sessionFiles.length > maxSessionFiles) {
        for (int i = maxSessionFiles; i < sessionFiles.length; i++) {
          await sessionFiles[i].delete();
        }
      }
    } catch (e) {
      if (kDebugMode) {
        print('Failed to cleanup old session files: $e');
      }
    }
  }
  
  // Write log entry to current session file
  Future<void> _writeToSessionFile(LogEntry entry) async {
    if (_currentSessionFile == null) return;
    
    try {
      await _currentSessionFile!.writeAsString(
        '${entry.formattedMessage}\n',
        mode: FileMode.append,
      );
    } catch (e) {
      // Silently fail to avoid logging loops
      if (kDebugMode) {
        print('Failed to write log to session file: $e');
      }
    }
  }
  
  // Setup crash handler
  Future<void> _setupCrashHandler() async {
    FlutterError.onError = (FlutterErrorDetails details) {
      _logCrash('Flutter Error', details.exception.toString(), details.stack);
      // Still call the default error handler
      FlutterError.presentError(details);
    };
    
    PlatformDispatcher.instance.onError = (error, stack) {
      _logCrash('Platform Error', error.toString(), stack);
      return true; // Mark as handled
    };
  }
  
  // Log crash to persistent storage
  Future<void> _logCrash(String type, String error, StackTrace? stackTrace) async {
    try {
      final crashInfo = {
        'timestamp': DateTime.now().toIso8601String(),
        'sessionId': _sessionId,
        'type': type,
        'error': error,
        'stackTrace': stackTrace?.toString(),
        'deviceInfo': _deviceInfo?.formattedInfo,
        'recentLogs': _logs.length > 20 ? _logs.skip(_logs.length - 20).map((e) => e.formattedMessage).toList() : _logs.map((e) => e.formattedMessage).toList(), // Only last 20 entries
      };
      
      if (_crashLogFile != null) {
        await _crashLogFile!.writeAsString(jsonEncode(crashInfo));
      }
      
      // Also log through normal logging
      critical('CrashHandler', '$type: $error', stackTrace?.toString());
    } catch (e) {
      if (kDebugMode) {
        print('Failed to log crash: $e');
      }
    }
  }
  
  // Load and log previous crash if exists
  Future<void> _loadPreviousCrash() async {
    if (_crashLogFile == null || !await _crashLogFile!.exists()) return;
    
    try {
      final crashData = await _crashLogFile!.readAsString();
      if (crashData.isNotEmpty) {
        final crash = jsonDecode(crashData);
        warning('PreviousCrash', 'Previous crash detected: ${crash['type']} at ${crash['timestamp']}');
        warning('PreviousCrash', 'Session: ${crash['sessionId'] ?? 'unknown'}');
        warning('PreviousCrash', 'Error: ${crash['error']}');
        if (crash['stackTrace'] != null) {
          warning('PreviousCrash', 'Stack trace available in crash log file');
        }
      }
    } catch (e) {
      warning('AppLogger', 'Failed to load previous crash info: $e');
    }
  }
  
  // Get list of available session files
  Future<List<File>> getSessionFiles() async {
    if (_logsDirectory == null) return [];
    
    try {
      final files = await _logsDirectory!.list().toList();
      final sessionFiles = files
          .whereType<File>()
          .where((f) => path_helper.basename(f.path).startsWith('session_'))
          .toList();
      
      // Sort by last modified date (newest first)
      sessionFiles.sort((a, b) => b.lastModifiedSync().compareTo(a.lastModifiedSync()));
      return sessionFiles;
    } catch (e) {
      return [];
    }
  }
  
  // Get current session file path
  String? get currentSessionPath => _currentSessionFile?.path;
  
  // Get crash log file path
  String? get crashLogPath => _crashLogFile?.path;
  
  // Get logs directory path
  String? get logsDirectoryPath => _logsDirectory?.path;
  
  // Check if previous crash exists
  Future<bool> hasPreviousCrash() async {
    if (_crashLogFile == null) return false;
    try {
      final exists = await _crashLogFile!.exists();
      if (!exists) return false;
      final content = await _crashLogFile!.readAsString();
      return content.isNotEmpty;
    } catch (e) {
      return false;
    }
  }
  
  // Clear crash log
  Future<void> clearCrashLog() async {
    if (_crashLogFile != null && await _crashLogFile!.exists()) {
      await _crashLogFile!.writeAsString('');
    }
  }
  
  // Get formatted logs with session info
  String getFormattedLogsWithSessionInfo() {
    final buffer = StringBuffer();
    
    // Add session info
    buffer.writeln('Session ID: $_sessionId');
    buffer.writeln('Session started: ${DateTime.now().toString()}');
    
    // Add device info
    if (_deviceInfo != null) {
      buffer.writeln(_deviceInfo!.formattedInfo);
    }
    
    // Add separator
    buffer.writeln('=' * 50);
    buffer.writeln('Application Logs (Current Session):');
    buffer.writeln('=' * 50);
    
    // Add all logs
    for (final log in _logs) {
      buffer.writeln(log.formattedMessage);
    }
    
    // Add footer
    buffer.writeln();
    buffer.writeln('=' * 50);
    buffer.writeln('End of logs - Total entries: ${_logs.length}');
    buffer.writeln('Session file: ${_currentSessionFile?.path}');
    buffer.writeln('Bug reports: https://github.com/madeofpendletonwool/pinepods/issues');
    
    return buffer.toString();
  }

  // Get device info
  DeviceInfo? get deviceInfo => _deviceInfo;
}