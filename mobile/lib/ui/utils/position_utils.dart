// lib/ui/utils/position_utils.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/downloadable.dart';
import 'package:pinepods_mobile/services/logging/app_logger.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/bloc/podcast/podcast_bloc.dart';
import 'package:provider/provider.dart';

/// Utility class for managing episode position synchronization and display
class PositionUtils {
  static final AppLogger _logger = AppLogger();

  /// Generate consistent GUID for PinePods episodes
  static String generateEpisodeGuid(PinepodsEpisode episode) {
    return 'pinepods_${episode.episodeId}';
  }

  /// Get local position for episode from repository
  static Future<double?> getLocalPosition(BuildContext context, PinepodsEpisode episode) async {
    try {
      final podcastBloc = Provider.of<PodcastBloc>(context, listen: false);
      final guid = generateEpisodeGuid(episode);
      
      // Get all episodes and find matches with both new and old GUID formats
      final allEpisodes = await podcastBloc.podcastService.repository.findAllEpisodes();
      final matchingEpisodes = allEpisodes.where((ep) => 
        ep.guid == guid || ep.guid.startsWith('${guid}_')
      ).toList();
      
      if (matchingEpisodes.isNotEmpty) {
        // Return the highest position from any matching episode (in case of duplicates)
        final positions = matchingEpisodes.map((ep) => ep.position / 1000.0).toList();
        return positions.reduce((a, b) => a > b ? a : b);
      }
      
      return null;
    } catch (e) {
      _logger.error('PositionUtils', 'Error getting local position for episode: ${episode.episodeTitle}', e.toString());
      return null;
    }
  }

  /// Get server position for episode (use existing data from feed)
  static Future<double?> getServerPosition(PinepodsService pinepodsService, PinepodsEpisode episode, int userId) async {
    return episode.listenDuration?.toDouble();
  }

  /// Get the best available position (furthest of local vs server)
  static Future<PositionInfo> getBestPosition(
    BuildContext context,
    PinepodsService pinepodsService,
    PinepodsEpisode episode,
    int userId,
  ) async {
    // Get both positions in parallel
    final futures = await Future.wait([
      getLocalPosition(context, episode),
      getServerPosition(pinepodsService, episode, userId),
    ]);
    
    final localPosition = futures[0] ?? 0.0;
    final serverPosition = futures[1] ?? episode.listenDuration?.toDouble() ?? 0.0;
    
    final bestPosition = localPosition > serverPosition ? localPosition : serverPosition;
    final isLocal = localPosition >= serverPosition;
    
    
    return PositionInfo(
      position: bestPosition,
      isLocal: isLocal,
      localPosition: localPosition,
      serverPosition: serverPosition,
    );
  }

  /// Enrich a single episode with the best available position
  static Future<PinepodsEpisode> enrichEpisodeWithBestPosition(
    BuildContext context,
    PinepodsService pinepodsService,
    PinepodsEpisode episode,
    int userId,
  ) async {
    final positionInfo = await getBestPosition(context, pinepodsService, episode, userId);
    
    // Create a new episode with updated position
    return PinepodsEpisode(
      podcastName: episode.podcastName,
      episodeTitle: episode.episodeTitle,
      episodePubDate: episode.episodePubDate,
      episodeDescription: episode.episodeDescription,
      episodeArtwork: episode.episodeArtwork,
      episodeUrl: episode.episodeUrl,
      episodeDuration: episode.episodeDuration,
      listenDuration: positionInfo.position.round(),
      episodeId: episode.episodeId,
      completed: episode.completed,
      saved: episode.saved,
      queued: episode.queued,
      downloaded: episode.downloaded,
      isYoutube: episode.isYoutube,
      podcastId: episode.podcastId,
    );
  }

  /// Enrich a list of episodes with the best available positions
  static Future<List<PinepodsEpisode>> enrichEpisodesWithBestPositions(
    BuildContext context,
    PinepodsService pinepodsService,
    List<PinepodsEpisode> episodes,
    int userId,
  ) async {
    _logger.info('PositionUtils', 'Enriching ${episodes.length} episodes with best positions');
    
    final enrichedEpisodes = <PinepodsEpisode>[];
    
    for (final episode in episodes) {
      try {
        final enrichedEpisode = await enrichEpisodeWithBestPosition(
          context,
          pinepodsService,
          episode,
          userId,
        );
        enrichedEpisodes.add(enrichedEpisode);
      } catch (e) {
        _logger.warning('PositionUtils', 'Failed to enrich episode ${episode.episodeTitle}, using original: ${e.toString()}');
        enrichedEpisodes.add(episode);
      }
    }
    
    _logger.info('PositionUtils', 'Successfully enriched ${enrichedEpisodes.length} episodes');
    return enrichedEpisodes;
  }
}

/// Information about episode position
class PositionInfo {
  final double position;
  final bool isLocal;
  final double localPosition;
  final double serverPosition;

  PositionInfo({
    required this.position,
    required this.isLocal,
    required this.localPosition,
    required this.serverPosition,
  });
}