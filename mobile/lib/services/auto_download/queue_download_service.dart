// lib/services/auto_download/queue_download_service.dart
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/services/network/network_status.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/ui/utils/local_download_utils.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// Keeps the first N episodes of the server queue downloaded locally. When an
/// episode leaves the top-N (played, removed or reordered out), the local copy
/// we created for it is pruned and the next episode is pulled down.
///
/// Only downloads WE created are pruned; a manual download of a queued episode
/// is left untouched.
class QueueDownloadService {
  /// SharedPreferences key holding the guids of downloads this service manages.
  static const String _managedGuidsKey = 'queue_managed_download_guids';

  /// Sync the local downloads to the current top-N of the queue.
  ///
  /// [count] is the number of leading queue episodes to keep downloaded; when 0
  /// the feature is disabled and any previously-managed downloads are pruned.
  static Future<void> syncQueueDownloads({
    required BuildContext context,
    required PinepodsService pinepodsService,
    required int userId,
  }) async {
    if (!context.mounted) return;

    final settings = Provider.of<SettingsBloc>(context, listen: false).currentSettings;
    final count = settings.autoDownloadQueueCount;

    final allowed = await NetworkStatus.canAutoDownload(wifiOnly: settings.autoDownloadWifiOnly);
    if (!allowed) return;
    if (!context.mounted) return;

    final prefs = await SharedPreferences.getInstance();
    final managed = (prefs.getStringList(_managedGuidsKey) ?? <String>[]).toSet();

    // Feature disabled: prune everything we manage, then clear.
    if (count <= 0) {
      for (final guid in managed) {
        if (!context.mounted) break;
        await LocalDownloadUtils.deleteLocalDownloadByGuid(context, guid);
      }
      await prefs.remove(_managedGuidsKey);
      return;
    }

    List<PinepodsEpisode> queue;
    try {
      queue = await pinepodsService.getQueuedEpisodes(userId);
    } catch (_) {
      return;
    }
    if (!context.mounted) return;

    final topN = queue.take(count).toList();
    final topNGuids = topN.map(LocalDownloadUtils.generateEpisodeGuid).toSet();

    // Prune managed downloads that have fallen out of the top-N.
    for (final guid in managed.difference(topNGuids)) {
      if (!context.mounted) break;
      await LocalDownloadUtils.deleteLocalDownloadByGuid(context, guid);
    }

    // Download any top-N episode not already present locally, and remember the
    // ones we create so we can prune them later.
    final newManaged = managed.intersection(topNGuids);
    for (final episode in topN) {
      if (!context.mounted) break;
      final guid = LocalDownloadUtils.generateEpisodeGuid(episode);
      final alreadyDownloaded = await LocalDownloadUtils.isEpisodeDownloadedLocally(context, episode);
      if (alreadyDownloaded) {
        // Leave user/other downloads alone; only keep tracking ones we owned.
        continue;
      }
      if (!context.mounted) break;
      final success = await LocalDownloadUtils.localDownloadEpisode(context, episode);
      if (success) {
        newManaged.add(guid);
      }
    }

    await prefs.setStringList(_managedGuidsKey, newManaged.toList());
  }
}
