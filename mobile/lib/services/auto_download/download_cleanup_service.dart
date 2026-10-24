// lib/services/auto_download/download_cleanup_service.dart
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/podcast/podcast_bloc.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/core/download_cleanup.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/services/global_services.dart';
import 'package:pinepods_mobile/services/logging/app_logger.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/ui/utils/local_download_utils.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// Removes device downloads once they're finished and/or older than a number
/// of days — the device-side twin of the server's download cleanup. The user
/// sets defaults in Settings; each podcast's "Device downloads" sheet can
/// override them. Runs on launch and when the app returns to the foreground.
///
/// "Finished" comes from the server when it's reachable (so an episode
/// finished on the web counts), else from the local played flag.
class DownloadCleanupService {
  static const String _finishedOverridePrefix = 'device_delete_finished_podcast_';
  static const String _daysOverridePrefix = 'device_delete_days_podcast_';

  /// guid -> {d: downloaded-at, p: podcast id, y: is youtube, f: already
  /// finished when downloaded}.
  static const String _infoKey = 'device_download_info';

  /// guid -> when the episode was first seen finished.
  static const String _finishedSeenKey = 'device_download_finished_seen';

  /// Guids removed because they were finished, so the mirror doesn't fetch
  /// them straight back.
  static const String _cleanedKey = 'device_download_cleaned_guids';
  static const int _maxCleaned = 500;

  static const String _lastRunKey = 'device_download_cleanup_last_run';
  static const Duration _minInterval = Duration(minutes: 10);

  static bool _running = false;

  static Future<bool?> getFinishedOverride(int podcastId) async =>
      (await SharedPreferences.getInstance()).getBool('$_finishedOverridePrefix$podcastId');

  static Future<void> setFinishedOverride(int podcastId, bool? enabled) async {
    final prefs = await SharedPreferences.getInstance();
    enabled == null
        ? await prefs.remove('$_finishedOverridePrefix$podcastId')
        : await prefs.setBool('$_finishedOverridePrefix$podcastId', enabled);
  }

  static Future<int?> getDaysOverride(int podcastId) async =>
      (await SharedPreferences.getInstance()).getInt('$_daysOverridePrefix$podcastId');

  static Future<void> setDaysOverride(int podcastId, int? days) async {
    final prefs = await SharedPreferences.getInstance();
    days == null
        ? await prefs.remove('$_daysOverridePrefix$podcastId')
        : await prefs.setInt('$_daysOverridePrefix$podcastId', days);
  }

  /// Remember when a download started and which podcast it belongs to.
  static Future<void> recordDownload(PinepodsEpisode episode) async {
    final prefs = await SharedPreferences.getInstance();
    final info = _readMap(prefs, _infoKey);
    info[LocalDownloadUtils.generateEpisodeGuid(episode)] = {
      'd': DateTime.now().toIso8601String(),
      if (episode.podcastId != null) 'p': episode.podcastId,
      'y': episode.isYoutube,
      // Downloading a finished episode means a re-listen; don't sweep it.
      if (episode.completed) 'f': true,
    };
    await prefs.setString(_infoKey, jsonEncode(info));
  }

  /// Guids cleaned up for being finished; automatic downloaders skip them.
  static Future<Set<String>> cleanedGuids() async =>
      ((await SharedPreferences.getInstance()).getStringList(_cleanedKey) ?? const <String>[]).toSet();

  static Future<void> run({
    required BuildContext context,
    PinepodsService? pinepodsService,
    int? userId,
    bool force = false,
  }) async {
    if (_running || !context.mounted) return;
    _running = true;
    try {
      await _run(context, pinepodsService, userId, force);
    } catch (e) {
      AppLogger().warning('DownloadCleanup', 'Cleanup failed: $e');
    } finally {
      _running = false;
    }
  }

  static Future<void> _run(BuildContext context, PinepodsService? service, int? userId, bool force) async {
    final prefs = await SharedPreferences.getInstance();
    final now = DateTime.now();
    final lastRun = DateTime.tryParse(prefs.getString(_lastRunKey) ?? '');
    if (!force && lastRun != null && now.difference(lastRun) < _minInterval) return;
    if (!context.mounted) return;

    final settings = Provider.of<SettingsBloc>(context, listen: false).currentSettings;
    final defaults = CleanupRules(
      deleteWhenFinished: settings.deleteDownloadedPlayedEpisodes,
      deleteAfterDays: settings.deleteDownloadsAfterDays,
    );

    final finishedOverrides = <int, bool>{};
    final daysOverrides = <int, int>{};
    for (final key in prefs.getKeys()) {
      if (key.startsWith(_finishedOverridePrefix)) {
        final id = int.tryParse(key.substring(_finishedOverridePrefix.length));
        final v = prefs.getBool(key);
        if (id != null && v != null) finishedOverrides[id] = v;
      } else if (key.startsWith(_daysOverridePrefix)) {
        final id = int.tryParse(key.substring(_daysOverridePrefix.length));
        final v = prefs.getInt(key);
        if (id != null && v != null) daysOverrides[id] = v;
      }
    }
    final anyFinishedRule = defaults.deleteWhenFinished || finishedOverrides.values.any((v) => v);
    final anyRule = anyFinishedRule || defaults.deleteAfterDays > 0 || daysOverrides.values.any((d) => d > 0);

    final podcastBloc = Provider.of<PodcastBloc>(context, listen: false);
    final downloads = await podcastBloc.podcastService.repository.findDownloads();
    if (!context.mounted) return;

    // One entry per episode; legacy pinepods_<id>_<ts> duplicates fold into
    // their canonical guid.
    final byGuid = <String, Episode>{};
    for (final ep in downloads) {
      byGuid.putIfAbsent(_canonicalGuid(ep.guid), () => ep);
    }

    final info = _readMap(prefs, _infoKey);
    final finishedSeen = _readMap(prefs, _finishedSeenKey);
    final cleaned = (prefs.getStringList(_cleanedKey) ?? <String>[]).toList();

    // Forget downloads that are gone (deleted by hand, or by another path).
    info.removeWhere((guid, _) => !byGuid.containsKey(guid));
    finishedSeen.removeWhere((guid, _) => !byGuid.containsKey(guid));

    final playingId = GlobalServices.pinepodsAudioService?.currentEpisodeId;
    var serverReachable = anyFinishedRule;
    var serverAnswered = false;

    for (final entry in byGuid.entries) {
      if (!context.mounted) break;
      final guid = entry.key;
      final ep = entry.value;
      final episodeId = LocalDownloadUtils.episodeIdFromGuid(guid);
      final record = (info[guid] as Map?)?.cast<String, dynamic>() ?? <String, dynamic>{};
      // A download from before this feature: start its clock now.
      record['d'] ??= now.toIso8601String();
      info[guid] = record;
      if (!anyRule) continue;
      if (episodeId > 0 && episodeId == playingId) continue;

      var finished = ep.played;
      if (serverReachable && service != null && userId != null && episodeId > 0) {
        PinepodsEpisode? meta;
        try {
          meta = await service.getEpisodeMetadata(episodeId, userId,
              isYoutube: record['y'] == true || (ep.pguid?.contains('youtube') ?? false));
        } catch (_) {}
        if (meta != null) {
          serverAnswered = true;
          finished = meta.completed;
          if (meta.podcastId != null) record['p'] = meta.podcastId;
        } else if (!serverAnswered) {
          // First lookup failed: treat the server as offline for this run.
          serverReachable = false;
        }
      }

      final podcastId = record['p'] as int?;
      final rules = defaults.withOverrides(
        deleteWhenFinished: podcastId == null ? null : finishedOverrides[podcastId],
        deleteAfterDays: podcastId == null ? null : daysOverrides[podcastId],
      );

      if (finished) {
        finishedSeen[guid] ??= now.toIso8601String();
      } else {
        finishedSeen.remove(guid);
        cleaned.remove(guid);
        record.remove('f');
      }

      final reason = cleanupReason(
        rules: rules,
        now: now,
        downloadedAt: DateTime.tryParse(record['d'] as String) ?? now,
        finishedSince: DateTime.tryParse(finishedSeen[guid] as String? ?? ''),
        exemptFromFinished: record['f'] == true,
      );
      if (reason == null || !context.mounted) continue;

      AppLogger().info('DownloadCleanup', 'Removing ${ep.title} (${reason.name})');
      await LocalDownloadUtils.deleteLocalDownloadByGuid(context, guid);
      info.remove(guid);
      finishedSeen.remove(guid);
      if (reason == CleanupReason.finished && !cleaned.contains(guid)) cleaned.add(guid);
    }

    if (cleaned.length > _maxCleaned) cleaned.removeRange(0, cleaned.length - _maxCleaned);
    await prefs.setString(_infoKey, jsonEncode(info));
    await prefs.setString(_finishedSeenKey, jsonEncode(finishedSeen));
    await prefs.setStringList(_cleanedKey, cleaned);
    await prefs.setString(_lastRunKey, now.toIso8601String());
  }

  static String _canonicalGuid(String guid) {
    final id = LocalDownloadUtils.episodeIdFromGuid(guid);
    return id > 0 ? 'pinepods_$id' : guid;
  }

  static Map<String, dynamic> _readMap(SharedPreferences prefs, String key) {
    try {
      final raw = prefs.getString(key);
      if (raw != null) return (jsonDecode(raw) as Map).cast<String, dynamic>();
    } catch (_) {}
    return <String, dynamic>{};
  }
}
