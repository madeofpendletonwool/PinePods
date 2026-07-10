// lib/ui/utils/local_download_utils.dart
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:pinepods_mobile/core/download_presence.dart';
import 'package:pinepods_mobile/core/utils.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/downloadable.dart';
import 'package:pinepods_mobile/repository/repository.dart';
import 'package:pinepods_mobile/services/logging/app_logger.dart';
import 'package:pinepods_mobile/bloc/podcast/podcast_bloc.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/app_settings.dart';
import 'package:provider/provider.dart';

/// Utility class for managing local downloads of PinePods episodes
class LocalDownloadUtils {
  static final Map<String, bool> _localDownloadStatusCache = {};

  /// Generate consistent GUID for PinePods episodes for local downloads
  static String generateEpisodeGuid(PinepodsEpisode episode) {
    return 'pinepods_${episode.episodeId}';
  }

  /// Parse the server episode id out of a local-download guid. Handles both the
  /// canonical `pinepods_<id>` format and the legacy `pinepods_<id>_<ts>` one.
  static int episodeIdFromGuid(String guid) {
    if (!guid.startsWith('pinepods_')) return 0;
    final rest = guid.substring('pinepods_'.length);
    return int.tryParse(rest.split('_').first) ?? 0;
  }

  /// Convert a stored local-download [Episode] back into a [PinepodsEpisode] so
  /// it can be rendered and played through the same widgets/path as every other
  /// (server) episode. The reverse of [localDownloadEpisode]'s conversion.
  static PinepodsEpisode toPinepodsEpisode(Episode episode) {
    return PinepodsEpisode(
      podcastName: episode.podcast ?? 'Unknown Podcast',
      episodeTitle: episode.title ?? '',
      episodePubDate: episode.publicationDate?.toIso8601String() ?? '',
      episodeDescription: episode.description ?? '',
      episodeArtwork: episode.imageUrl ?? '',
      // episodeUrl doubles as the now-playing match key; keep it as the original
      // content URL so highlighting lines up with the unified play path.
      episodeUrl: episode.contentUrl ?? '',
      episodeDuration: episode.duration, // stored in seconds
      // Episode.position is milliseconds; listenDuration is seconds.
      listenDuration: episode.position > 0 ? episode.position ~/ 1000 : null,
      episodeId: episodeIdFromGuid(episode.guid),
      completed: episode.played,
      saved: false,
      queued: false,
      downloaded: true,
      isYoutube: episode.pguid?.contains('youtube') ?? false,
      podcastId: null,
    );
  }

  /// Clear the local download status cache (call on refresh)
  static void clearCache() {
    _localDownloadStatusCache.clear();
  }

  /// Check if episode is downloaded locally with caching
  static Future<bool> isEpisodeDownloadedLocally(
    BuildContext context, 
    PinepodsEpisode episode
  ) async {
    final guid = generateEpisodeGuid(episode);
    final logger = AppLogger();
    logger.debug('LocalDownload', 'Checking download status for episode: ${episode.episodeTitle}, GUID: $guid');
    
    // Check cache first
    if (_localDownloadStatusCache.containsKey(guid)) {
      logger.debug('LocalDownload', 'Found cached status for $guid: ${_localDownloadStatusCache[guid]}');
      return _localDownloadStatusCache[guid]!;
    }
    
    try {
      final podcastBloc = Provider.of<PodcastBloc>(context, listen: false);
      final repository = podcastBloc.podcastService.repository;

      // Get all episodes and find matches with both new and old GUID formats
      final allEpisodes = await repository.findAllEpisodes();
      final matchingEpisodes = allEpisodes.where((ep) =>
        ep.guid == guid || ep.guid.startsWith('${guid}_')
      ).toList();

      logger.debug('LocalDownload', 'Repository lookup for $guid: found ${matchingEpisodes.length} matching episodes');

      final isDownloaded = await _resolvePresenceAndHeal(repository, matchingEpisodes, logger);

      logger.debug('LocalDownload', 'Final download status for $guid: $isDownloaded');

      // Cache the result
      _localDownloadStatusCache[guid] = isDownloaded;
      return isDownloaded;
    } catch (e) {
      final logger = AppLogger();
      logger.error('LocalDownload', 'Error checking local download status for episode: ${episode.episodeTitle}', e.toString());
      return false;
    }
  }

  /// Build the URL to download an episode's bytes from. When the user prefers
  /// server copies and the server already has this episode downloaded, this
  /// returns the server's stream endpoint so we mirror the server copy instead
  /// of re-fetching from the original feed. Returns null to fall back to the
  /// episode's original content URL.
  static String? resolveServerDownloadUrl(AppSettings settings, PinepodsEpisode episode) {
    if (!settings.preferServerDownloadSource) return null;
    if (!episode.downloaded) return null;
    if (episode.episodeId <= 0) return null;

    final server = settings.pinepodsServer;
    final apiKey = settings.pinepodsApiKey;
    final userId = settings.pinepodsUserId;
    if (server == null || server.isEmpty || apiKey == null || apiKey.isEmpty || userId == null) {
      return null;
    }

    final type = episode.isYoutube ? 'youtube' : 'episode';
    return '$server/api/data/stream/${episode.episodeId}'
        '?api_key=$apiKey&user_id=$userId&type=$type';
  }

  /// Checks each row in [matchingEpisodes] that's marked downloaded against
  /// the filesystem (not just the DB flag), heals - resets - any whose file
  /// is missing so the "Downloaded" badge stops lying about them, and
  /// returns whether at least one is genuinely present. See
  /// core/download_presence.dart for why matching can involve more than one
  /// row (legacy duplicate-guid downloads) and why the decision logic lives
  /// there as a plain, testable function rather than inline here.
  static Future<bool> _resolvePresenceAndHeal(
    Repository repository,
    List<Episode> matchingEpisodes,
    AppLogger logger,
  ) async {
    final downloadedRows = matchingEpisodes
        .where((ep) => ep.downloaded || ep.downloadState == DownloadState.downloaded)
        .toList();

    if (downloadedRows.isEmpty) return false;

    final fileExists = <Episode, bool>{};
    for (final row in downloadedRows) {
      fileExists[row] = await _fileActuallyExists(row);
    }

    final result = resolveDownloadPresence(matchingEpisodes, fileExists);

    for (final stale in result.staleRecords) {
      logger.warning('LocalDownload', 'Healing stale download record for missing file: ${stale.guid}');
      clearDownloadState(stale);
      await repository.saveEpisode(stale);
    }

    return result.isDownloaded;
  }

  static Future<bool> _fileActuallyExists(Episode episode) async {
    if (episode.filepath == null || episode.filename == null) return false;

    try {
      if (!await hasStoragePermission()) return false;

      final path = await resolvePath(episode);
      final file = File(path);
      return await file.exists() && (await file.length()) > 0;
    } catch (e) {
      return false;
    }
  }

  /// Update local download status cache
  static void updateLocalDownloadStatus(PinepodsEpisode episode, bool isDownloaded) {
    final guid = generateEpisodeGuid(episode);
    _localDownloadStatusCache[guid] = isDownloaded;
  }

  /// Proactively load local download status for a list of episodes
  static Future<void> loadLocalDownloadStatuses(
    BuildContext context, 
    List<PinepodsEpisode> episodes
  ) async {
    final logger = AppLogger();
    logger.debug('LocalDownload', 'Loading local download statuses for ${episodes.length} episodes');
    
    try {
      final podcastBloc = Provider.of<PodcastBloc>(context, listen: false);
      final repository = podcastBloc.podcastService.repository;

      // Get all downloaded episodes from repository
      final allEpisodes = await repository.findAllEpisodes();
      logger.debug('LocalDownload', 'Found ${allEpisodes.length} total episodes in repository');

      // Filter to PinePods episodes only and log them
      final pinepodsEpisodes = allEpisodes.where((ep) => ep.guid.startsWith('pinepods_')).toList();
      logger.debug('LocalDownload', 'Found ${pinepodsEpisodes.length} PinePods episodes in repository');

      // Found pinepods episodes in repository

      // Now check each episode against the repository
      for (final episode in episodes) {
        final guid = generateEpisodeGuid(episode);

        // Look for episodes with either new format (pinepods_123) or old format (pinepods_123_timestamp)
        final matchingEpisodes = allEpisodes.where((ep) =>
          ep.guid == guid || ep.guid.startsWith('${guid}_')
        ).toList();

        _localDownloadStatusCache[guid] = await _resolvePresenceAndHeal(repository, matchingEpisodes, logger);
        // Episode status checked
      }

      // Download statuses cached

    } catch (e) {
      logger.error('LocalDownload', 'Error loading local download statuses', e.toString());
    }
  }

  /// Download episode locally
  static Future<bool> localDownloadEpisode(
    BuildContext context, 
    PinepodsEpisode episode
  ) async {
    final logger = AppLogger();
    
    try {
      // Convert PinepodsEpisode to Episode for local download
      final localEpisode = Episode(
        guid: generateEpisodeGuid(episode),
        pguid: 'pinepods_${episode.podcastName.replaceAll(' ', '_').toLowerCase()}',
        podcast: episode.podcastName,
        title: episode.episodeTitle,
        description: episode.episodeDescription,
        imageUrl: episode.episodeArtwork,
        contentUrl: episode.episodeUrl,
        duration: episode.episodeDuration,
        publicationDate: DateTime.tryParse(episode.episodePubDate),
        author: episode.podcastName,
        season: 0,
        episode: 0,
        // Episode.position is stored in milliseconds; listenDuration is seconds.
        position: (episode.listenDuration ?? 0) * 1000,
        played: episode.completed,
        chapters: [],
        transcriptUrls: [],
      );
      
      logger.debug('LocalDownload', 'Created local episode with GUID: ${localEpisode.guid}');
      logger.debug('LocalDownload', 'Episode title: ${localEpisode.title}');
      logger.debug('LocalDownload', 'Episode URL: ${localEpisode.contentUrl}');

      // Prefer the server's downloaded copy as the byte source when enabled and
      // available. contentUrl stays the original feed URL (used for filename
      // derivation, playback and now-playing matching); downloadUrl is a
      // transient override consumed only by the download manager.
      try {
        final settings = Provider.of<SettingsBloc>(context, listen: false).currentSettings;
        localEpisode.downloadUrl = resolveServerDownloadUrl(settings, episode);
        if (localEpisode.downloadUrl != null) {
          logger.debug('LocalDownload', 'Using server download source for ${localEpisode.guid}');
        }
      } catch (e) {
        logger.debug('LocalDownload', 'Could not resolve server download URL: $e');
      }

      final podcastBloc = Provider.of<PodcastBloc>(context, listen: false);

      // First save the episode to the repository so it can be tracked
      await podcastBloc.podcastService.saveEpisode(localEpisode);
      logger.debug('LocalDownload', 'Episode saved to repository');
      
      // Use the download service from podcast bloc
      final success = await podcastBloc.downloadService.downloadEpisode(localEpisode);
      logger.debug('LocalDownload', 'Download service result: $success');
      
      if (success) {
        updateLocalDownloadStatus(episode, true);
      }
      
      return success;
    } catch (e) {
      logger.error('LocalDownload', 'Error in local download for episode: ${episode.episodeTitle}', e.toString());
      return false;
    }
  }

  /// Delete local download(s) for episode
  static Future<int> deleteLocalDownload(
    BuildContext context, 
    PinepodsEpisode episode
  ) async {
    final logger = AppLogger();
    
    try {
      final podcastBloc = Provider.of<PodcastBloc>(context, listen: false);
      final guid = generateEpisodeGuid(episode);
      
      // Get all episodes and find matches with both new and old GUID formats
      final allEpisodes = await podcastBloc.podcastService.repository.findAllEpisodes();
      final matchingEpisodes = allEpisodes.where((ep) => 
        ep.guid == guid || ep.guid.startsWith('${guid}_')
      ).toList();
      
      logger.debug('LocalDownload', 'Found ${matchingEpisodes.length} episodes to delete for $guid');
      
      if (matchingEpisodes.isNotEmpty) {
        // Delete ALL matching episodes (handles duplicates from old timestamp GUIDs)
        for (final localEpisode in matchingEpisodes) {
          logger.debug('LocalDownload', 'Deleting episode: ${localEpisode.guid}');
          await podcastBloc.podcastService.repository.deleteEpisode(localEpisode);
        }
        
        // Update cache
        updateLocalDownloadStatus(episode, false);
        
        return matchingEpisodes.length;
      } else {
        return 0;
      }
    } catch (e) {
      logger.error('LocalDownload', 'Error deleting local download for episode: ${episode.episodeTitle}', e.toString());
      return 0;
    }
  }

  /// Delete local download(s) for a raw local-download [guid] (`pinepods_<id>`),
  /// including legacy `pinepods_<id>_<ts>` duplicates. Used by automatic
  /// download managers (queue/mirror) that only have the guid, not a full
  /// [PinepodsEpisode]. Returns the number of episode records deleted.
  static Future<int> deleteLocalDownloadByGuid(
    BuildContext context,
    String guid,
  ) async {
    final logger = AppLogger();

    try {
      final podcastBloc = Provider.of<PodcastBloc>(context, listen: false);

      final allEpisodes = await podcastBloc.podcastService.repository.findAllEpisodes();
      final matchingEpisodes = allEpisodes.where((ep) =>
        ep.guid == guid || ep.guid.startsWith('${guid}_')
      ).toList();

      if (matchingEpisodes.isEmpty) {
        _localDownloadStatusCache[guid] = false;
        return 0;
      }

      for (final localEpisode in matchingEpisodes) {
        await podcastBloc.podcastService.repository.deleteEpisode(localEpisode);
      }
      _localDownloadStatusCache[guid] = false;
      return matchingEpisodes.length;
    } catch (e) {
      logger.error('LocalDownload', 'Error deleting local download for guid: $guid', e.toString());
      return 0;
    }
  }

  /// Show snackbar with message
  static void showSnackBar(BuildContext context, String message, Color backgroundColor) {
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(message),
        backgroundColor: backgroundColor,
        duration: const Duration(seconds: 2),
      ),
    );
  }
}