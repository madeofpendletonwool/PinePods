// lib/ui/utils/local_download_utils.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/downloadable.dart';
import 'package:pinepods_mobile/services/logging/app_logger.dart';
import 'package:pinepods_mobile/bloc/podcast/podcast_bloc.dart';
import 'package:provider/provider.dart';

/// Utility class for managing local downloads of PinePods episodes
class LocalDownloadUtils {
  static final Map<String, bool> _localDownloadStatusCache = {};

  /// Generate consistent GUID for PinePods episodes for local downloads
  static String generateEpisodeGuid(PinepodsEpisode episode) {
    return 'pinepods_${episode.episodeId}';
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
      
      // Get all episodes and find matches with both new and old GUID formats
      final allEpisodes = await podcastBloc.podcastService.repository.findAllEpisodes();
      final matchingEpisodes = allEpisodes.where((ep) => 
        ep.guid == guid || ep.guid.startsWith('${guid}_')
      ).toList();
      
      logger.debug('LocalDownload', 'Repository lookup for $guid: found ${matchingEpisodes.length} matching episodes');
      
      for (final match in matchingEpisodes) {
        logger.debug('LocalDownload', 'Match: ${match.guid} - downloaded: ${match.downloaded}, downloadState: ${match.downloadState}, downloadPercentage: ${match.downloadPercentage}');
      }
      
      // Consider downloaded if ANY matching episode is downloaded
      final isDownloaded = matchingEpisodes.any((ep) => 
        ep.downloaded || ep.downloadState == DownloadState.downloaded
      );
      
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
      
      // Get all downloaded episodes from repository
      final allEpisodes = await podcastBloc.podcastService.repository.findAllEpisodes();
      logger.debug('LocalDownload', 'Found ${allEpisodes.length} total episodes in repository');
      
      // Filter to PinePods episodes only and log them
      final pinepodsEpisodes = allEpisodes.where((ep) => ep.guid.startsWith('pinepods_')).toList();
      logger.debug('LocalDownload', 'Found ${pinepodsEpisodes.length} PinePods episodes in repository');
      
      for (final localEp in pinepodsEpisodes) {
        logger.debug('LocalDownload', 'Local episode: ${localEp.title} - GUID: ${localEp.guid} - Downloaded: ${localEp.downloaded} - State: ${localEp.downloadState}');
      }
      
      // Now check each episode against the repository
      for (final episode in episodes) {
        final guid = generateEpisodeGuid(episode);
        
        // Look for episodes with either new format (pinepods_123) or old format (pinepods_123_timestamp)
        final matchingEpisodes = allEpisodes.where((ep) => 
          ep.guid == guid || ep.guid.startsWith('${guid}_')
        ).toList();
        
        logger.debug('LocalDownload', 'Looking for matches for $guid, found ${matchingEpisodes.length} episodes');
        for (final match in matchingEpisodes) {
          logger.debug('LocalDownload', '  Match: ${match.guid} - Downloaded: ${match.downloaded} - State: ${match.downloadState}');
        }
        
        // Consider downloaded if ANY matching episode is downloaded
        final isDownloaded = matchingEpisodes.any((ep) => 
          ep.downloaded || ep.downloadState == DownloadState.downloaded
        );
        
        _localDownloadStatusCache[guid] = isDownloaded;
        logger.debug('LocalDownload', 'Episode ${episode.episodeTitle} ($guid): ${isDownloaded ? 'DOWNLOADED' : 'NOT DOWNLOADED'}');
      }
      
      logger.debug('LocalDownload', 'Cached ${_localDownloadStatusCache.length} download statuses');
      
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
        position: episode.listenDuration ?? 0,
        played: episode.completed,
        chapters: [],
        transcriptUrls: [],
      );
      
      logger.debug('LocalDownload', 'Created local episode with GUID: ${localEpisode.guid}');
      logger.debug('LocalDownload', 'Episode title: ${localEpisode.title}');
      logger.debug('LocalDownload', 'Episode URL: ${localEpisode.contentUrl}');
      
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