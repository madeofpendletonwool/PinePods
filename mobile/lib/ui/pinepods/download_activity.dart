// lib/ui/pinepods/download_activity.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/podcast/podcast_bloc.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/downloadable.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/ui/utils/local_download_utils.dart';
import 'package:provider/provider.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';
import 'package:pinepods_mobile/l10n/L.dart';

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

    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;

    // Local download activity from the on-device repository. Always available,
    // even offline, and includes auto-downloads, in-progress, and failures.
    List<DownloadTask> localTasks = [];
    try {
      final podcastBloc = Provider.of<PodcastBloc>(context, listen: false);
      final episodes = await podcastBloc.podcastService.repository.findAllEpisodes();
      localTasks = _localTasksFromEpisodes(episodes);
    } catch (e) {
      // Non-fatal; just show whatever else we have.
    }

    // Server-side download activity (best effort — may be offline / logged out).
    List<DownloadTask> serverTasks = [];
    String? serverError;
    if (settings.pinepodsServer != null &&
        settings.pinepodsApiKey != null &&
        settings.pinepodsUserId != null) {
      try {
        _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
        serverTasks = await _pinepodsService.getDownloadActivity(settings.pinepodsUserId!);
      } catch (e) {
        serverError = L.current.failed_to_load_server_download_activity;
      }
    }

    final all = [...serverTasks, ...localTasks]
      ..sort((a, b) => b.updatedAt.compareTo(a.updatedAt));

    if (!mounted) return;
    setState(() {
      _tasks = all;
      // Only surface an error if we genuinely have nothing to show.
      _errorMessage = all.isEmpty ? serverError : null;
      _isLoading = false;
    });
  }

  /// Build activity entries for local downloads from the on-device episode
  /// store. Local downloads persist their state on the [Episode] record
  /// (downloadState / downloadPercentage / lastUpdated), so recent activity —
  /// including failures and auto-downloads — can be derived directly.
  List<DownloadTask> _localTasksFromEpisodes(List<Episode> episodes) {
    final cutoff = DateTime.now().subtract(const Duration(days: 7));
    final tasks = <DownloadTask>[];

    for (final e in episodes) {
      // Local downloads are always keyed by a 'pinepods_<id>' guid; anything
      // else (streaming/playback records) is not a local download.
      if (!e.guid.startsWith('pinepods_')) continue;
      if (e.downloadState == DownloadState.none) continue;

      final updated = e.lastUpdated ?? DateTime.now();
      if (updated.isBefore(cutoff)) continue;

      tasks.add(DownloadTask(
        id: e.guid,
        taskType: 'local',
        status: _statusForState(e.downloadState),
        progress: (e.downloadPercentage ?? 0).toDouble(),
        message: e.downloadState == DownloadState.failed
            ? L.of(context)!.local_download_failed
            : e.downloadState == DownloadState.cancelled
                ? L.of(context)!.download_cancelled
                : null,
        createdAt: updated,
        updatedAt: updated,
        result: {'episode_id': LocalDownloadUtils.episodeIdFromGuid(e.guid)},
        episodeTitle: e.title,
        podcastName: e.podcast,
      ));
    }

    return tasks;
  }

  String _statusForState(DownloadState state) {
    switch (state) {
      case DownloadState.downloaded:
        return 'SUCCESS';
      case DownloadState.failed:
      case DownloadState.cancelled:
        return 'FAILED';
      case DownloadState.downloading:
      case DownloadState.paused:
        return 'DOWNLOADING';
      case DownloadState.queued:
      case DownloadState.none:
        return 'PENDING';
    }
  }

  Future<void> _retryDownload(DownloadTask task) async {
    // Local downloads retry through the on-device download service rather than
    // the server.
    if (task.taskType == 'local') {
      final podcastBloc = Provider.of<PodcastBloc>(context, listen: false);
      final episode = await podcastBloc.podcastService.repository.findEpisodeByGuid(task.id);
      if (episode == null) {
        if (!mounted) return;
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(L.of(context)!.episode_no_longer_available), duration: const Duration(seconds: 2)),
        );
        return;
      }
      final success = await podcastBloc.downloadService.downloadEpisode(episode);
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(success ? L.of(context)!.local_download_restarted : L.of(context)!.failed_to_restart_download),
          backgroundColor: success ? Colors.green : Colors.red,
          duration: const Duration(seconds: 2),
        ),
      );
      if (success) _loadActivity();
      return;
    }

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
        content: Text(success ? 'Download queued' : L.of(context)!.failed_to_queue_download),
        backgroundColor: success ? Colors.green : Colors.red,
        duration: const Duration(seconds: 2),
      ),
    );

    if (success) _loadActivity();
  }

  String _relativeTime(DateTime dt) {
    final diff = DateTime.now().difference(dt);
    if (diff.inMinutes < 1) return 'just now';
    if (diff.inMinutes < 60) return L.of(context)!.minutes_ago_short(diff.inMinutes);
    if (diff.inHours < 24) return L.of(context)!.hours_ago_short(diff.inHours);
    return L.of(context)!.days_ago_short(diff.inDays);
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return Scaffold(
      appBar: AppBar(
        title: Text(L.of(context)!.download_activity_title),
        actions: [
          IconButton(
            icon: const Icon(PhosphorIconsRegular.arrowClockwise),
            onPressed: _loadActivity,
            tooltip: L.of(context)!.refresh,
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
            Icon(PhosphorIconsRegular.warningCircle, size: 64, color: Colors.red[300]),
            const SizedBox(height: 16),
            Text(_errorMessage!, textAlign: TextAlign.center),
            const SizedBox(height: 16),
            ElevatedButton(onPressed: _loadActivity, child: Text(L.of(context)!.retry)),
          ],
        ),
      );
    }

    if (_tasks.isEmpty) {
      return Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(PhosphorIconsRegular.downloadSimple, size: 64, color: Colors.grey[400]),
            const SizedBox(height: 16),
            Text(L.of(context)!.no_recent_download_activity, style: theme.textTheme.titleMedium),
            const SizedBox(height: 8),
            Text(
              L.of(context)!.download_activity_empty_body,
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
        ? Icon(PhosphorIconsFill.checkCircle, color: Colors.green[600], size: 22)
        : isFailed
            ? Icon(PhosphorIconsFill.xCircle, color: Colors.red[600], size: 22)
            : SizedBox(
                width: 22,
                height: 22,
                child: CircularProgressIndicator(
                  value: task.progress > 0 ? task.progress / 100.0 : null,
                  strokeWidth: 2.5,
                ),
              );

    final title = task.episodeTitle ?? _titleFromMessage(task.message) ?? L.of(context)!.unknown_episode;
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
                    _sourceChip(task, theme),
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
                      icon: const Icon(PhosphorIconsRegular.arrowCounterClockwise, size: 14),
                      label: Text(L.of(context)!.retry, style: const TextStyle(fontSize: 12)),
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

  /// Small chip distinguishing on-device (local) downloads from server-side ones.
  Widget _sourceChip(DownloadTask task, ThemeData theme) {
    final isLocal = task.taskType == 'local';
    final color = isLocal ? Colors.green[600]! : Colors.blue[600]!;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
      decoration: BoxDecoration(
        color: color.withOpacity(0.12),
        borderRadius: BorderRadius.circular(6),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(isLocal ? PhosphorIconsRegular.deviceMobile : PhosphorIconsRegular.cloud, size: 11, color: color),
          const SizedBox(width: 3),
          Text(
            isLocal ? 'Local' : L.of(context)!.server,
            style: TextStyle(fontSize: 10, color: color, fontWeight: FontWeight.w600),
          ),
        ],
      ),
    );
  }

  // Extract a title from a status message like "Downloaded Episode Name" or "Preparing Episode Name"
  String? _titleFromMessage(String? message) {
    if (message == null) return null;
    for (final prefix in ['Downloaded ', 'Downloading ', 'Preparing ', 'Finalizing ', 'Processing ', 'Connecting to ', 'Starting download ']) { // i18n-ignore
      if (message.startsWith(prefix)) {
        return message.substring(prefix.length);
      }
    }
    return null;
  }
}
