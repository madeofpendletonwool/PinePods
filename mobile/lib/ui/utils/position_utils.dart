// lib/ui/utils/position_utils.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/services/logging/app_logger.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/bloc/podcast/podcast_bloc.dart';
import 'package:pinepods_mobile/ui/utils/local_download_utils.dart';
import 'package:provider/provider.dart';

/// Utility class for managing episode position synchronization and display
class PositionUtils {
  static final AppLogger _logger = AppLogger();

  /// Generate consistent GUID for PinePods episodes
  static String generateEpisodeGuid(PinepodsEpisode episode) {
    return 'pinepods_${episode.episodeId}';
  }

  /// Furthest local position in seconds per server episode id, covering both
  /// the current and legacy timestamp-suffixed guid formats.
  static Future<Map<int, double>> _localPositions(BuildContext context) async {
    try {
      final podcastBloc = Provider.of<PodcastBloc>(context, listen: false);
      final local = await podcastBloc.podcastService.repository.findPinepodsEpisodes();
      final positions = <int, double>{};
      for (final ep in local) {
        final id = LocalDownloadUtils.episodeIdFromGuid(ep.guid);
        if (id == 0) continue;
        final seconds = ep.position / 1000.0;
        if (seconds > (positions[id] ?? -1)) positions[id] = seconds;
      }
      return positions;
    } catch (e) {
      _logger.error('PositionUtils', 'Error loading local positions', e.toString());
      return {};
    }
  }

  /// Enrich a list of episodes with the best available position (furthest of
  /// local vs server). The local store is read once for the whole list.
  static Future<List<PinepodsEpisode>> enrichEpisodesWithBestPositions(
    BuildContext context,
    PinepodsService pinepodsService,
    List<PinepodsEpisode> episodes,
    int userId,
  ) async {
    final local = await _localPositions(context);

    return episodes.map((episode) {
      final localPosition = local[episode.episodeId] ?? 0.0;
      final serverPosition = episode.listenDuration?.toDouble() ?? 0.0;
      if (localPosition <= serverPosition) return episode;
      return episode.withListenDuration(localPosition.round());
    }).toList();
  }
}
