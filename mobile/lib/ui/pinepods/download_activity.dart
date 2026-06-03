// lib/ui/pinepods/download_activity.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:provider/provider.dart';

class DownloadActivity extends StatefulWidget {
  const DownloadActivity({super.key});

  @override
  State<DownloadActivity> createState() => _DownloadActivityState();
}

class _DownloadActivityState extends State<DownloadActivity> {
  final PinepodsService _pinepodsService = PinepodsService();
  List<DownloadTask> _tasks = [];
  bool _isLoading = true;
  String? _errorMessage;

  @override
  void initState() {
    super.initState();
    _loadActivity();
  }

  Future<void> _loadActivity() async {
    setState(() {
      _isLoading = true;
      _errorMessage = null;
    });

    try {
      final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
      final settings = settingsBloc.currentSettings;

      if (settings.pinepodsServer == null ||
          settings.pinepodsApiKey == null ||
          settings.pinepodsUserId == null) {
        setState(() {
          _errorMessage = 'Not connected to PinePods server.';
          _isLoading = false;
        });
        return;
      }

      _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
      final userId = settings.pinepodsUserId!;
      final tasks = await _pinepodsService.getDownloadActivity(userId);

      setState(() {
        _tasks = tasks;
        _isLoading = false;
      });
    } catch (e) {
      setState(() {
        _errorMessage = 'Failed to load download activity.';
        _isLoading = false;
      });
    }
  }

  Future<void> _retryDownload(DownloadTask task) async {
    final episodeId = task.episodeId;
    if (episodeId == null) return;

    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    if (settings.pinepodsServer == null ||
        settings.pinepodsApiKey == null ||
        settings.pinepodsUserId == null) return;

    _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
    final userId = settings.pinepodsUserId!;

    final success = await _pinepodsService.downloadEpisode(episodeId, userId, false);

    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(success ? 'Download queued' : 'Failed to queue download'),
        backgroundColor: success ? Colors.green : Colors.red,
        duration: const Duration(seconds: 2),
      ),
    );

    if (success) _loadActivity();
  }

  String _relativeTime(DateTime dt) {
    final diff = DateTime.now().difference(dt);
    if (diff.inMinutes < 1) return 'just now';
    if (diff.inMinutes < 60) return '${diff.inMinutes}m ago';
    if (diff.inHours < 24) return '${diff.inHours}h ago';
    return '${diff.inDays}d ago';
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return Scaffold(
      appBar: AppBar(
        title: const Text('Download Activity'),
        actions: [
          IconButton(
            icon: const Icon(Icons.refresh),
            onPressed: _loadActivity,
            tooltip: 'Refresh',
          ),
        ],
      ),
      body: _buildBody(theme),
    );
  }

  Widget _buildBody(ThemeData theme) {
    if (_isLoading) {
      return const Center(child: CircularProgressIndicator());
    }

    if (_errorMessage != null) {
      return Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(Icons.error_outline, size: 64, color: Colors.red[300]),
            const SizedBox(height: 16),
            Text(_errorMessage!, textAlign: TextAlign.center),
            const SizedBox(height: 16),
            ElevatedButton(onPressed: _loadActivity, child: const Text('Retry')),
          ],
        ),
      );
    }

    if (_tasks.isEmpty) {
      return Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(Icons.download_outlined, size: 64, color: Colors.grey[400]),
            const SizedBox(height: 16),
            Text('No recent download activity', style: theme.textTheme.titleMedium),
            const SizedBox(height: 8),
            Text(
              'Downloads triggered in the last 7 days will appear here.',
              style: theme.textTheme.bodyMedium?.copyWith(color: theme.hintColor),
              textAlign: TextAlign.center,
            ),
          ],
        ),
      );
    }

    return RefreshIndicator(
      onRefresh: _loadActivity,
      child: ListView.separated(
        padding: const EdgeInsets.symmetric(vertical: 8),
        itemCount: _tasks.length,
        separatorBuilder: (_, __) => const Divider(height: 1),
        itemBuilder: (context, index) => _buildTaskTile(_tasks[index], theme),
      ),
    );
  }

  Widget _buildTaskTile(DownloadTask task, ThemeData theme) {
    final isCompleted = task.isCompleted;
    final isFailed = task.isFailed;
    final isActive = task.isActive;

    final statusIcon = isCompleted
        ? Icon(Icons.check_circle, color: Colors.green[600], size: 22)
        : isFailed
            ? Icon(Icons.cancel, color: Colors.red[600], size: 22)
            : SizedBox(
                width: 22,
                height: 22,
                child: CircularProgressIndicator(
                  value: task.progress > 0 ? task.progress / 100.0 : null,
                  strokeWidth: 2.5,
                ),
              );

    final title = task.episodeTitle ?? _titleFromMessage(task.message) ?? 'Unknown episode';
    final subtitle = task.podcastName;
    final timeLabel = _relativeTime(task.updatedAt);

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: const EdgeInsets.only(top: 2),
            child: statusIcon,
          ),
          const SizedBox(width: 14),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Expanded(
                      child: Text(
                        title,
                        style: theme.textTheme.bodyMedium?.copyWith(fontWeight: FontWeight.w500),
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                    const SizedBox(width: 8),
                    Text(
                      timeLabel,
                      style: theme.textTheme.bodySmall?.copyWith(color: theme.hintColor),
                    ),
                  ],
                ),
                if (subtitle != null) ...[
                  const SizedBox(height: 2),
                  Text(
                    subtitle,
                    style: theme.textTheme.bodySmall?.copyWith(color: theme.hintColor),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ],
                if (isActive && task.progress > 0) ...[
                  const SizedBox(height: 6),
                  LinearProgressIndicator(
                    value: task.progress / 100.0,
                    minHeight: 3,
                    borderRadius: BorderRadius.circular(2),
                  ),
                ],
                if (isFailed && task.message != null) ...[
                  const SizedBox(height: 4),
                  Text(
                    task.message!,
                    style: theme.textTheme.bodySmall?.copyWith(color: Colors.red[600]),
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                  ),
                  const SizedBox(height: 6),
                  SizedBox(
                    height: 30,
                    child: OutlinedButton.icon(
                      onPressed: task.episodeId != null ? () => _retryDownload(task) : null,
                      icon: const Icon(Icons.replay, size: 14),
                      label: const Text('Retry', style: TextStyle(fontSize: 12)),
                      style: OutlinedButton.styleFrom(
                        padding: const EdgeInsets.symmetric(horizontal: 10),
                        side: BorderSide(color: theme.primaryColor),
                      ),
                    ),
                  ),
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }

  // Extract a title from a status message like "Downloaded Episode Name" or "Preparing Episode Name"
  String? _titleFromMessage(String? message) {
    if (message == null) return null;
    for (final prefix in ['Downloaded ', 'Downloading ', 'Preparing ', 'Finalizing ', 'Processing ', 'Connecting to ', 'Starting download ']) {
      if (message.startsWith(prefix)) {
        return message.substring(prefix.length);
      }
    }
    return null;
  }
}
