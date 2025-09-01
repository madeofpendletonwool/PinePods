// lib/ui/debug/debug_logs_page.dart
import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:pinepods_mobile/services/logging/app_logger.dart';

class DebugLogsPage extends StatefulWidget {
  const DebugLogsPage({Key? key}) : super(key: key);

  @override
  State<DebugLogsPage> createState() => _DebugLogsPageState();
}

class _DebugLogsPageState extends State<DebugLogsPage> {
  final AppLogger _logger = AppLogger();
  final ScrollController _scrollController = ScrollController();
  List<LogEntry> _logs = [];
  LogLevel? _selectedLevel;
  bool _showDeviceInfo = true;
  List<File> _sessionFiles = [];
  bool _hasPreviousCrash = false;

  @override
  void initState() {
    super.initState();
    _loadLogs();
    _loadSessionFiles();
  }

  void _loadLogs() {
    setState(() {
      if (_selectedLevel == null) {
        _logs = _logger.logs;
      } else {
        _logs = _logger.getLogsByLevel(_selectedLevel!);
      }
    });
  }

  Future<void> _copyLogsToClipboard() async {
    try {
      final formattedLogs = _logger.getFormattedLogs();
      await Clipboard.setData(ClipboardData(text: formattedLogs));
      
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text('Logs copied to clipboard!'),
            backgroundColor: Colors.green,
            duration: Duration(seconds: 2),
          ),
        );
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Failed to copy logs: $e'),
            backgroundColor: Colors.red,
            duration: const Duration(seconds: 3),
          ),
        );
      }
    }
  }

  Future<void> _loadSessionFiles() async {
    try {
      final files = await _logger.getSessionFiles();
      final hasCrash = await _logger.hasPreviousCrash();
      setState(() {
        _sessionFiles = files;
        _hasPreviousCrash = hasCrash;
      });
    } catch (e) {
      print('Failed to load session files: $e');
    }
  }

  Future<void> _copyCurrentSessionToClipboard() async {
    try {
      final formattedLogs = _logger.getFormattedLogsWithSessionInfo();
      await Clipboard.setData(ClipboardData(text: formattedLogs));
      
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text('Current session logs copied to clipboard!'),
            backgroundColor: Colors.green,
            duration: Duration(seconds: 2),
          ),
        );
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Failed to copy logs: $e'),
            backgroundColor: Colors.red,
            duration: const Duration(seconds: 3),
          ),
        );
      }
    }
  }
  
  Future<void> _copySessionFileToClipboard(File sessionFile) async {
    try {
      final content = await sessionFile.readAsString();
      final deviceInfo = _logger.deviceInfo?.formattedInfo ?? 'Device info not available';
      final formattedContent = '$deviceInfo\n\n${'=' * 50}\nSession File: ${sessionFile.path.split('/').last}\n${'=' * 50}\n\n$content';
      
      await Clipboard.setData(ClipboardData(text: formattedContent));
      
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Session ${sessionFile.path.split('/').last} copied to clipboard!'),
            backgroundColor: Colors.green,
            duration: const Duration(seconds: 2),
          ),
        );
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Failed to copy session file: $e'),
            backgroundColor: Colors.red,
            duration: const Duration(seconds: 3),
          ),
        );
      }
    }
  }
  
  Future<void> _copyCrashLogToClipboard() async {
    try {
      final crashPath = _logger.crashLogPath;
      if (crashPath == null) {
        throw Exception('Crash log path not available');
      }
      
      final crashFile = File(crashPath);
      final content = await crashFile.readAsString();
      
      await Clipboard.setData(ClipboardData(text: content));
      
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text('Crash log copied to clipboard!'),
            backgroundColor: Colors.orange,
            duration: Duration(seconds: 2),
          ),
        );
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Failed to copy crash log: $e'),
            backgroundColor: Colors.red,
            duration: const Duration(seconds: 3),
          ),
        );
      }
    }
  }

  Future<void> _openBugTracker() async {
    const url = 'https://github.com/madeofpendletonwool/pinepods/issues';
    try {
      final uri = Uri.parse(url);
      await launchUrl(uri, mode: LaunchMode.externalApplication);
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Could not open bug tracker: $e'),
            backgroundColor: Colors.red,
          ),
        );
      }
    }
  }

  void _clearLogs() {
    showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Clear Logs'),
        content: const Text('Are you sure you want to clear all logs? This action cannot be undone.'),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () {
              _logger.clearLogs();
              _loadLogs();
              Navigator.of(context).pop();
              ScaffoldMessenger.of(context).showSnackBar(
                const SnackBar(
                  content: Text('Logs cleared'),
                  backgroundColor: Colors.orange,
                ),
              );
            },
            child: const Text('Clear'),
          ),
        ],
      ),
    );
  }

  void _scrollToBottom() {
    if (_scrollController.hasClients) {
      _scrollController.animateTo(
        _scrollController.position.maxScrollExtent,
        duration: const Duration(milliseconds: 300),
        curve: Curves.easeOut,
      );
    }
  }

  Color _getLevelColor(LogLevel level) {
    switch (level) {
      case LogLevel.debug:
        return Colors.grey;
      case LogLevel.info:
        return Colors.blue;
      case LogLevel.warning:
        return Colors.orange;
      case LogLevel.error:
        return Colors.red;
      case LogLevel.critical:
        return Colors.purple;
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Debug Logs'),
        elevation: 0,
        actions: [
          PopupMenuButton<String>(
            onSelected: (value) {
              switch (value) {
                case 'filter':
                  _showFilterDialog();
                  break;
                case 'clear':
                  _clearLogs();
                  break;
                case 'refresh':
                  _loadLogs();
                  break;
                case 'scroll_bottom':
                  _scrollToBottom();
                  break;
              }
            },
            itemBuilder: (context) => [
              const PopupMenuItem(
                value: 'filter',
                child: Row(
                  children: [
                    Icon(Icons.filter_list),
                    SizedBox(width: 8),
                    Text('Filter'),
                  ],
                ),
              ),
              const PopupMenuItem(
                value: 'refresh',
                child: Row(
                  children: [
                    Icon(Icons.refresh),
                    SizedBox(width: 8),
                    Text('Refresh'),
                  ],
                ),
              ),
              const PopupMenuItem(
                value: 'scroll_bottom',
                child: Row(
                  children: [
                    Icon(Icons.vertical_align_bottom),
                    SizedBox(width: 8),
                    Text('Scroll to Bottom'),
                  ],
                ),
              ),
              const PopupMenuItem(
                value: 'clear',
                child: Row(
                  children: [
                    Icon(Icons.clear_all),
                    SizedBox(width: 8),
                    Text('Clear Logs'),
                  ],
                ),
              ),
            ],
          ),
        ],
      ),
      body: Column(
        children: [
          // Header with device info toggle and stats
          Container(
            padding: const EdgeInsets.all(16.0),
            color: Theme.of(context).cardColor,
            child: Column(
              children: [
                Row(
                  children: [
                    Expanded(
                      child: Text(
                        'Total Entries: ${_logs.length}',
                        style: Theme.of(context).textTheme.titleMedium,
                      ),
                    ),
                    if (_selectedLevel != null)
                      Chip(
                        label: Text(_selectedLevel!.name.toUpperCase()),
                        backgroundColor: _getLevelColor(_selectedLevel!).withOpacity(0.2),
                        deleteIcon: const Icon(Icons.close, size: 16),
                        onDeleted: () {
                          setState(() {
                            _selectedLevel = null;
                          });
                          _loadLogs();
                        },
                      ),
                  ],
                ),
                const SizedBox(height: 8),
                Row(
                  children: [
                    Expanded(
                      child: ElevatedButton.icon(
                        onPressed: _copyCurrentSessionToClipboard,
                        icon: const Icon(Icons.copy),
                        label: const Text('Copy Current'),
                        style: ElevatedButton.styleFrom(
                          backgroundColor: Colors.green,
                          foregroundColor: Colors.white,
                        ),
                      ),
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: ElevatedButton.icon(
                        onPressed: _openBugTracker,
                        icon: const Icon(Icons.bug_report),
                        label: const Text('Report Bug'),
                        style: ElevatedButton.styleFrom(
                          backgroundColor: Colors.orange,
                          foregroundColor: Colors.white,
                        ),
                      ),
                    ),
                  ],
                ),
              ],
            ),
          ),
          const Divider(height: 1),
          
          // Session Files Section
          if (_sessionFiles.isNotEmpty || _hasPreviousCrash)
            ExpansionTile(
              title: const Text('Session Files & Crash Logs'),
              leading: const Icon(Icons.folder),
              initiallyExpanded: false,
              children: [
                if (_hasPreviousCrash)
                  ListTile(
                    leading: const Icon(Icons.warning, color: Colors.red),
                    title: const Text('Previous Crash Log'),
                    subtitle: const Text('Tap to copy crash log to clipboard'),
                    trailing: IconButton(
                      icon: const Icon(Icons.copy),
                      onPressed: _copyCrashLogToClipboard,
                    ),
                    onTap: _copyCrashLogToClipboard,
                  ),
                ..._sessionFiles.map((file) {
                  final fileName = file.path.split('/').last;
                  final isCurrentSession = fileName.contains(_logger.currentSessionPath?.split('/').last?.replaceFirst('session_', '').replaceFirst('.log', '') ?? '');
                  
                  return ListTile(
                    leading: Icon(
                      isCurrentSession ? Icons.play_circle : Icons.history,
                      color: isCurrentSession ? Colors.green : Colors.grey,
                    ),
                    title: Text(fileName),
                    subtitle: Text(
                      'Modified: ${file.lastModifiedSync().toString().substring(0, 16)}${isCurrentSession ? ' (Current)' : ''}',
                      style: TextStyle(
                        fontSize: 12,
                        color: isCurrentSession ? Colors.green : Colors.grey[600],
                      ),
                    ),
                    trailing: IconButton(
                      icon: const Icon(Icons.copy),
                      onPressed: () => _copySessionFileToClipboard(file),
                    ),
                    onTap: () => _copySessionFileToClipboard(file),
                  );
                }).toList(),
                if (_sessionFiles.isEmpty && !_hasPreviousCrash)
                  const Padding(
                    padding: EdgeInsets.all(16.0),
                    child: Text(
                      'No session files available yet',
                      style: TextStyle(color: Colors.grey),
                    ),
                  ),
              ],
            ),
          
          // Device info section (collapsible)
          if (_showDeviceInfo && _logger.deviceInfo != null)
            ExpansionTile(
              title: const Text('Device Information'),
              leading: const Icon(Icons.phone_android),
              initiallyExpanded: false,
              children: [
                Padding(
                  padding: const EdgeInsets.all(16.0),
                  child: Container(
                    width: double.infinity,
                    padding: const EdgeInsets.all(12.0),
                    decoration: BoxDecoration(
                      color: Theme.of(context).cardColor,
                      borderRadius: BorderRadius.circular(8),
                      border: Border.all(color: Colors.grey.withOpacity(0.3)),
                    ),
                    child: Text(
                      _logger.deviceInfo!.formattedInfo,
                      style: const TextStyle(fontFamily: 'monospace', fontSize: 12),
                    ),
                  ),
                ),
              ],
            ),
          
          // Logs list
          Expanded(
            child: _logs.isEmpty
                ? const Center(
                    child: Column(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        Icon(Icons.inbox_outlined, size: 64, color: Colors.grey),
                        SizedBox(height: 16),
                        Text(
                          'No logs found',
                          style: TextStyle(fontSize: 18, color: Colors.grey),
                        ),
                        SizedBox(height: 8),
                        Text(
                          'Use the app to generate logs',
                          style: TextStyle(color: Colors.grey),
                        ),
                      ],
                    ),
                  )
                : ListView.builder(
                    controller: _scrollController,
                    itemCount: _logs.length,
                    itemBuilder: (context, index) {
                      final log = _logs[index];
                      return _buildLogEntry(log);
                    },
                  ),
          ),
        ],
      ),
      floatingActionButton: _logs.isNotEmpty
          ? FloatingActionButton(
              onPressed: _scrollToBottom,
              tooltip: 'Scroll to bottom',
              child: const Icon(Icons.vertical_align_bottom),
            )
          : null,
    );
  }

  Widget _buildLogEntry(LogEntry log) {
    final levelColor = _getLevelColor(log.level);
    
    return Container(
      margin: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
      child: Card(
        elevation: 1,
        child: ExpansionTile(
          leading: Container(
            width: 8,
            height: 8,
            decoration: BoxDecoration(
              color: levelColor,
              shape: BoxShape.circle,
            ),
          ),
          title: Text(
            log.message,
            style: const TextStyle(fontSize: 14),
            maxLines: 2,
            overflow: TextOverflow.ellipsis,
          ),
          subtitle: Text(
            '${log.timestamp.toString().substring(0, 19)} • ${log.levelString} • ${log.tag}',
            style: TextStyle(
              fontSize: 12,
              color: Colors.grey[600],
            ),
          ),
          children: [
            Padding(
              padding: const EdgeInsets.all(16.0),
              child: Container(
                width: double.infinity,
                padding: const EdgeInsets.all(12.0),
                decoration: BoxDecoration(
                  color: Theme.of(context).colorScheme.surfaceVariant.withOpacity(0.3),
                  borderRadius: BorderRadius.circular(8),
                ),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    SelectableText(
                      log.formattedMessage,
                      style: const TextStyle(
                        fontFamily: 'monospace',
                        fontSize: 12,
                      ),
                    ),
                    if (log.stackTrace != null && log.stackTrace!.isNotEmpty) ...[
                      const SizedBox(height: 8),
                      const Divider(),
                      const SizedBox(height: 8),
                      const Text(
                        'Stack Trace:',
                        style: TextStyle(fontWeight: FontWeight.bold),
                      ),
                      const SizedBox(height: 4),
                      SelectableText(
                        log.stackTrace!,
                        style: const TextStyle(
                          fontFamily: 'monospace',
                          fontSize: 10,
                        ),
                      ),
                    ],
                    const SizedBox(height: 8),
                    Row(
                      mainAxisAlignment: MainAxisAlignment.end,
                      children: [
                        TextButton.icon(
                          onPressed: () {
                            Clipboard.setData(ClipboardData(text: log.formattedMessage));
                            ScaffoldMessenger.of(context).showSnackBar(
                              const SnackBar(content: Text('Log entry copied to clipboard')),
                            );
                          },
                          icon: const Icon(Icons.copy, size: 16),
                          label: const Text('Copy'),
                        ),
                      ],
                    ),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  void _showFilterDialog() {
    showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Filter Logs'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Text('Show only logs of level:'),
            const SizedBox(height: 16),
            ...LogLevel.values.map((level) => RadioListTile<LogLevel?>(
              title: Row(
                children: [
                  Container(
                    width: 12,
                    height: 12,
                    decoration: BoxDecoration(
                      color: _getLevelColor(level),
                      shape: BoxShape.circle,
                    ),
                  ),
                  const SizedBox(width: 8),
                  Text(level.name.toUpperCase()),
                ],
              ),
              value: level,
              groupValue: _selectedLevel,
              onChanged: (value) {
                setState(() {
                  _selectedLevel = value;
                });
              },
            )),
            RadioListTile<LogLevel?>(
              title: const Text('All Levels'),
              value: null,
              groupValue: _selectedLevel,
              onChanged: (value) {
                setState(() {
                  _selectedLevel = null;
                });
              },
            ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: const Text('Cancel'),
          ),
          ElevatedButton(
            onPressed: () {
              _loadLogs();
              Navigator.of(context).pop();
            },
            child: const Text('Apply'),
          ),
        ],
      ),
    );
  }

  @override
  void dispose() {
    _scrollController.dispose();
    super.dispose();
  }
}