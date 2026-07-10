// lib/services/auto_download/mirror_download_service.dart
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:pinepods_mobile/bloc/podcast/podcast_bloc.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/services/network/network_status.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/ui/utils/local_download_utils.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// Keeps the device's local downloads mirrored to the episodes downloaded on the
/// server. Two-way: episodes the server has but the device doesn't are pulled
/// down (from the server's copy), and mirror-managed local copies the server no
/// longer has are pruned. Manual, user-initiated downloads are never pruned.
class MirrorDownloadService {
  /// SharedPreferences key holding the guids of downloads this service manages.
  static const String _managedGuidsKey = 'mirror_managed_download_guids';

  static Future<void> syncMirror({
    required BuildContext context,
    required PinepodsService pinepodsService,
    required int userId,
  }) async {
    if (!context.mounted) return;

    final settings = Provider.of<SettingsBloc>(context, listen: false).currentSettings;

    final prefs = await SharedPreferences.getInstance();
    final managed = (prefs.getStringList(_managedGuidsKey) ?? <String>[]).toSet();

    // Disabled: prune everything we manage, then clear our tracking.
    if (!settings.mirrorServerDownloads) {
      for (final guid in managed) {
        if (!context.mounted) break;
        await LocalDownloadUtils.deleteLocalDownloadByGuid(context, guid);
      }
      if (managed.isNotEmpty) await prefs.remove(_managedGuidsKey);
      return;
    }

    final allowed = await NetworkStatus.canAutoDownload(wifiOnly: settings.autoDownloadWifiOnly);
    if (!allowed) return;
    if (!context.mounted) return;

    List<PinepodsEpisode> serverEpisodes;
    try {
      serverEpisodes = await pinepodsService.getServerDownloads(userId);
    } catch (_) {
      return;
    }
    if (!context.mounted) return;

    // Map canonical guid -> server episode.
    final serverByGuid = <String, PinepodsEpisode>{
      for (final e in serverEpisodes)
        if (e.episodeId > 0) LocalDownloadUtils.generateEpisodeGuid(e): e,
    };
    final serverGuids = serverByGuid.keys.toSet();

    // Build the set of canonical guids that already have a local record (any
    // download state), so we neither re-enqueue in-progress downloads nor treat
    // them as missing.
    final localGuids = <String>{};
    try {
      final podcastBloc = Provider.of<PodcastBloc>(context, listen: false);
      final allLocal = await podcastBloc.podcastService.repository.findAllEpisodes();
      for (final ep in allLocal) {
        if (ep.guid.startsWith('pinepods_')) {
          localGuids.add('pinepods_${LocalDownloadUtils.episodeIdFromGuid(ep.guid)}');
        }
      }
    } catch (_) {
      return;
    }
    if (!context.mounted) return;

    // Prune: managed downloads the server no longer has.
    for (final guid in managed.difference(serverGuids)) {
      if (!context.mounted) break;
      await LocalDownloadUtils.deleteLocalDownloadByGuid(context, guid);
    }

    // Add: server downloads missing on the device.
    final newManaged = managed.intersection(serverGuids);
    for (final entry in serverByGuid.entries) {
      if (!context.mounted) break;
      final guid = entry.key;
      if (localGuids.contains(guid)) {
        // Already present locally. Keep tracking it only if we owned it.
        continue;
      }
      final success = await LocalDownloadUtils.localDownloadEpisode(context, entry.value);
      if (success) {
        newManaged.add(guid);
      }
    }

    await prefs.setStringList(_managedGuidsKey, newManaged.toList());
  }
}
