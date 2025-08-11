// lib/ui/pinepods/feed.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/bloc/podcast/podcast_bloc.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_audio_service.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/services/audio/default_audio_player_service.dart';
import 'package:pinepods_mobile/services/download/download_service.dart';
import 'package:pinepods_mobile/services/logging/app_logger.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/downloadable.dart';
import 'package:pinepods_mobile/ui/widgets/episode_context_menu.dart';
import 'package:pinepods_mobile/ui/widgets/pinepods_episode_card.dart';
import 'package:pinepods_mobile/ui/pinepods/episode_details.dart';
import 'package:pinepods_mobile/ui/utils/player_utils.dart';
import 'package:pinepods_mobile/ui/utils/position_utils.dart';
import 'package:pinepods_mobile/services/global_services.dart';
import 'package:provider/provider.dart';

class PinepodsFeed extends StatefulWidget {
  // Constructor with optional key parameter
  const PinepodsFeed({Key? key}) : super(key: key);

  @override
  State<PinepodsFeed> createState() => _PinepodsFeedState();
}

class _PinepodsFeedState extends State<PinepodsFeed> {
  bool _isLoading = false;
  String _errorMessage = '';
  List<PinepodsEpisode> _episodes = [];
  final PinepodsService _pinepodsService = PinepodsService();
  // Use global audio service instead of creating local instance
  int? _contextMenuEpisodeIndex; // Index of episode showing context menu
  Map<String, bool> _localDownloadStatus = {}; // Cache for local download status

  @override
  void initState() {
    super.initState();
    _loadRecentEpisodes();
  }

  PinepodsAudioService? get _audioService {
    final service = GlobalServices.pinepodsAudioService;
    if (service == null) {
      final logger = AppLogger();
      logger.error('Feed', 'Global audio service is null - this should not happen');
    }
    return service;
  }

  Future<void> _loadRecentEpisodes() async {
    setState(() {
      _isLoading = true;
      _errorMessage = '';
    });

    try {
      final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
      final settings = settingsBloc.currentSettings;

      if (settings.pinepodsServer == null || 
          settings.pinepodsApiKey == null || 
          settings.pinepodsUserId == null) {
        setState(() {
          _errorMessage = 'Not connected to PinePods server. Please login first.';
          _isLoading = false;
        });
        return;
      }

      // Set credentials in both local and global services
      _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
      GlobalServices.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);

      // Use the stored user ID from login
      final userId = settings.pinepodsUserId!;

      final episodes = await _pinepodsService.getRecentEpisodes(userId);
      
      // Enrich episodes with best available positions (local vs server)
      final enrichedEpisodes = await PositionUtils.enrichEpisodesWithBestPositions(
        context,
        _pinepodsService,
        episodes,
        userId,
      );
      
      setState(() {
        _episodes = enrichedEpisodes;
        _isLoading = false;
      });
      
      // After loading episodes, check their local download status
      await _loadLocalDownloadStatuses();
    } catch (e) {
      setState(() {
        _errorMessage = 'Failed to load recent episodes: ${e.toString()}';
        _isLoading = false;
      });
    }
  }

  // Proactively load local download status for all episodes
  Future<void> _loadLocalDownloadStatuses() async {
    final logger = AppLogger();
    logger.debug('Feed', 'Loading local download statuses for ${_episodes.length} episodes');
    
    try {
      final podcastBloc = Provider.of<PodcastBloc>(context, listen: false);
      
      // Get all downloaded episodes from repository
      final allEpisodes = await podcastBloc.podcastService.repository.findAllEpisodes();
      logger.debug('Feed', 'Found ${allEpisodes.length} total episodes in repository');
      
      // Filter to PinePods episodes only and log them
      final pinepodsEpisodes = allEpisodes.where((ep) => ep.guid.startsWith('pinepods_')).toList();
      logger.debug('Feed', 'Found ${pinepodsEpisodes.length} PinePods episodes in repository');
      
      for (final localEp in pinepodsEpisodes) {
        logger.debug('Feed', 'Local episode: ${localEp.title} - GUID: ${localEp.guid} - Downloaded: ${localEp.downloaded} - State: ${localEp.downloadState}');
      }
      
      // Now check each feed episode against the repository
      for (final episode in _episodes) {
        final guid = _generateEpisodeGuid(episode);
        
        // Look for episodes with either new format (pinepods_123) or old format (pinepods_123_timestamp)
        final matchingEpisodes = allEpisodes.where((ep) => 
          ep.guid == guid || ep.guid.startsWith('${guid}_')
        ).toList();
        
        logger.debug('Feed', 'Looking for matches for $guid, found ${matchingEpisodes.length} episodes');
        for (final match in matchingEpisodes) {
          logger.debug('Feed', '  Match: ${match.guid} - Downloaded: ${match.downloaded} - State: ${match.downloadState}');
        }
        
        // Consider downloaded if ANY matching episode is downloaded
        final isDownloaded = matchingEpisodes.any((ep) => 
          ep.downloaded || ep.downloadState == DownloadState.downloaded
        );
        
        _localDownloadStatus[guid] = isDownloaded;
        logger.debug('Feed', 'Episode ${episode.episodeTitle} ($guid): ${isDownloaded ? 'DOWNLOADED' : 'NOT DOWNLOADED'}');
      }
      
      logger.debug('Feed', 'Cached ${_localDownloadStatus.length} download statuses');
      
    } catch (e) {
      logger.error('Feed', 'Error loading local download statuses', e.toString());
    }
  }

  Future<void> _refresh() async {
    // Clear local download status cache on refresh
    _localDownloadStatus.clear();
    await _loadRecentEpisodes();
  }

  Future<void> _playEpisode(PinepodsEpisode episode) async {
    final logger = AppLogger();
    logger.info('Feed', 'Attempting to play episode: ${episode.episodeTitle}');
    
    if (_audioService == null) {
      logger.error('Feed', 'Audio service not available for episode: ${episode.episodeTitle}');
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text('Audio service not available'),
          backgroundColor: Colors.red,
        ),
      );
      return;
    }

    try {
      // Show loading indicator
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Row(
            children: [
              const SizedBox(
                width: 16,
                height: 16,
                child: CircularProgressIndicator(strokeWidth: 2),
              ),
              const SizedBox(width: 12),
              Text('Starting ${episode.episodeTitle}...'),
            ],
          ),
          duration: const Duration(seconds: 2),
        ),
      );

      // Start playing the episode with full PinePods integration
      await playPinepodsEpisodeWithOptionalFullScreen(
        context,
        _audioService!,
        episode,
        resume: episode.isStarted, // Resume if episode was previously started
      );

      logger.info('Feed', 'Successfully started playing episode: ${episode.episodeTitle}');
      
      // Show success message
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text('Now playing: ${episode.episodeTitle}'),
          backgroundColor: Colors.green,
          duration: const Duration(seconds: 2),
        ),
      );
    } catch (e) {
      logger.error('Feed', 'Failed to play episode: ${episode.episodeTitle}', e.toString());
      
      // Show error message
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text('Failed to play episode: ${e.toString()}'),
          backgroundColor: Colors.red,
          duration: const Duration(seconds: 3),
        ),
      );
    }
  }

  Future<void> _showContextMenu(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    final isDownloadedLocally = await _isEpisodeDownloadedLocally(episode);
    
    if (!mounted) return;
    
    showDialog(
      context: context,
      barrierColor: Colors.black.withOpacity(0.3),
      builder: (context) => EpisodeContextMenu(
        episode: episode,
        isDownloadedLocally: isDownloadedLocally,
        onSave: () {
          Navigator.of(context).pop();
          _saveEpisode(episodeIndex);
        },
        onRemoveSaved: () {
          Navigator.of(context).pop();
          _removeSavedEpisode(episodeIndex);
        },
        onDownload: episode.downloaded 
          ? () {
              Navigator.of(context).pop();
              _deleteEpisode(episodeIndex);
            }
          : () {
              Navigator.of(context).pop();
              _downloadEpisode(episodeIndex);
            },
        onLocalDownload: () {
          Navigator.of(context).pop();
          _localDownloadEpisode(episodeIndex);
        },
        onDeleteLocalDownload: () {
          Navigator.of(context).pop();
          _deleteLocalDownload(episodeIndex);
        },
        onQueue: () {
          Navigator.of(context).pop();
          _toggleQueueEpisode(episodeIndex);
        },
        onMarkComplete: () {
          Navigator.of(context).pop();
          _toggleMarkComplete(episodeIndex);
        },
        onDismiss: () {
          Navigator.of(context).pop();
        },
      ),
    );
  }

  void _hideContextMenu() {
    setState(() {
      _contextMenuEpisodeIndex = null;
    });
  }

  Future<void> _saveEpisode(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    // Set credentials if not already set
    _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);

    try {
      final success = await _pinepodsService.saveEpisode(
        episode.episodeId,
        userId,
        episode.isYoutube,
      );

      if (success) {
        // Update local state
        setState(() {
          _episodes[episodeIndex] = PinepodsEpisode(
            podcastName: episode.podcastName,
            episodeTitle: episode.episodeTitle,
            episodePubDate: episode.episodePubDate,
            episodeDescription: episode.episodeDescription,
            episodeArtwork: episode.episodeArtwork,
            episodeUrl: episode.episodeUrl,
            episodeDuration: episode.episodeDuration,
            listenDuration: episode.listenDuration,
            episodeId: episode.episodeId,
            completed: episode.completed,
            saved: true, // Mark as saved
            queued: episode.queued,
            downloaded: episode.downloaded,
            isYoutube: episode.isYoutube,
          );
        });
        _showSnackBar('Episode saved!', Colors.green);
      } else {
        _showSnackBar('Failed to save episode', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error saving episode: $e', Colors.red);
    }

    _hideContextMenu();
  }

  Future<void> _removeSavedEpisode(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    // Set credentials if not already set
    _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);

    try {
      final success = await _pinepodsService.removeSavedEpisode(
        episode.episodeId,
        userId,
        episode.isYoutube,
      );

      if (success) {
        // Update local state
        setState(() {
          _episodes[episodeIndex] = PinepodsEpisode(
            podcastName: episode.podcastName,
            episodeTitle: episode.episodeTitle,
            episodePubDate: episode.episodePubDate,
            episodeDescription: episode.episodeDescription,
            episodeArtwork: episode.episodeArtwork,
            episodeUrl: episode.episodeUrl,
            episodeDuration: episode.episodeDuration,
            listenDuration: episode.listenDuration,
            episodeId: episode.episodeId,
            completed: episode.completed,
            saved: false, // Mark as not saved
            queued: episode.queued,
            downloaded: episode.downloaded,
            isYoutube: episode.isYoutube,
          );
        });
        _showSnackBar('Removed from saved episodes', Colors.orange);
      } else {
        _showSnackBar('Failed to remove saved episode', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error removing saved episode: $e', Colors.red);
    }

    _hideContextMenu();
  }

  Future<void> _downloadEpisode(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);

    try {
      final success = await _pinepodsService.downloadEpisode(
        episode.episodeId,
        userId,
        episode.isYoutube,
      );

      if (success) {
        setState(() {
          _episodes[episodeIndex] = _updateEpisodeProperty(episode, downloaded: true);
        });
        _showSnackBar('Episode download queued!', Colors.green);
      } else {
        _showSnackBar('Failed to queue download', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error downloading episode: $e', Colors.red);
    }

    _hideContextMenu();
  }

  Future<void> _deleteEpisode(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);

    try {
      final success = await _pinepodsService.deleteEpisode(
        episode.episodeId,
        userId,
        episode.isYoutube,
      );

      if (success) {
        setState(() {
          _episodes[episodeIndex] = _updateEpisodeProperty(episode, downloaded: false);
        });
        _showSnackBar('Episode deleted from server', Colors.orange);
      } else {
        _showSnackBar('Failed to delete episode', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error deleting episode: $e', Colors.red);
    }

    _hideContextMenu();
  }

  Future<void> _toggleQueueEpisode(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);

    try {
      bool success;
      if (episode.queued) {
        success = await _pinepodsService.removeQueuedEpisode(
          episode.episodeId,
          userId,
          episode.isYoutube,
        );
        if (success) {
          setState(() {
            _episodes[episodeIndex] = _updateEpisodeProperty(episode, queued: false);
          });
          _showSnackBar('Removed from queue', Colors.orange);
        }
      } else {
        success = await _pinepodsService.queueEpisode(
          episode.episodeId,
          userId,
          episode.isYoutube,
        );
        if (success) {
          setState(() {
            _episodes[episodeIndex] = _updateEpisodeProperty(episode, queued: true);
          });
          _showSnackBar('Added to queue!', Colors.green);
        }
      }

      if (!success) {
        _showSnackBar('Failed to update queue', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error updating queue: $e', Colors.red);
    }

    _hideContextMenu();
  }

  Future<void> _toggleMarkComplete(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);

    try {
      bool success;
      if (episode.completed) {
        success = await _pinepodsService.markEpisodeUncompleted(
          episode.episodeId,
          userId,
          episode.isYoutube,
        );
        if (success) {
          setState(() {
            _episodes[episodeIndex] = _updateEpisodeProperty(episode, completed: false);
          });
          _showSnackBar('Marked as incomplete', Colors.orange);
        }
      } else {
        success = await _pinepodsService.markEpisodeCompleted(
          episode.episodeId,
          userId,
          episode.isYoutube,
        );
        if (success) {
          setState(() {
            _episodes[episodeIndex] = _updateEpisodeProperty(episode, completed: true);
          });
          _showSnackBar('Marked as complete!', Colors.green);
        }
      }

      if (!success) {
        _showSnackBar('Failed to update completion status', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error updating completion: $e', Colors.red);
    }

    _hideContextMenu();
  }

  Future<void> _localDownloadEpisode(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    
    try {
      // Convert PinepodsEpisode to Episode for local download
      final localEpisode = Episode(
        guid: _generateEpisodeGuid(episode),
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
      final logger = AppLogger();
      logger.debug('Feed', 'Created local episode with GUID: ${localEpisode.guid}');
      logger.debug('Feed', 'Episode title: ${localEpisode.title}');
      logger.debug('Feed', 'Episode URL: ${localEpisode.contentUrl}');
      
      final podcastBloc = Provider.of<PodcastBloc>(context, listen: false);
      
      // First save the episode to the repository so it can be tracked
      await podcastBloc.podcastService.saveEpisode(localEpisode);
      logger.debug('Feed', 'Episode saved to repository');
      
      // Use the download service from podcast bloc
      final success = await podcastBloc.downloadService.downloadEpisode(localEpisode);
      logger.debug('Feed', 'Download service result: $success');
      
      if (success) {
        _updateLocalDownloadStatus(episode, true);
        _showSnackBar('Episode download started', Colors.green);
      } else {
        _showSnackBar('Failed to start download', Colors.red);
      }
    } catch (e) {
      final logger = AppLogger();
      logger.error('Feed', 'Error in local download for episode: ${episode.episodeTitle}', e.toString());
      _showSnackBar('Error starting local download: $e', Colors.red);
    }

    _hideContextMenu();
  }

  Future<void> _deleteLocalDownload(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    final logger = AppLogger();
    
    try {
      final podcastBloc = Provider.of<PodcastBloc>(context, listen: false);
      final guid = _generateEpisodeGuid(episode);
      
      // Get all episodes and find matches with both new and old GUID formats
      final allEpisodes = await podcastBloc.podcastService.repository.findAllEpisodes();
      final matchingEpisodes = allEpisodes.where((ep) => 
        ep.guid == guid || ep.guid.startsWith('${guid}_')
      ).toList();
      
      logger.debug('Feed', 'Found ${matchingEpisodes.length} episodes to delete for $guid');
      
      if (matchingEpisodes.isNotEmpty) {
        // Delete ALL matching episodes (handles duplicates from old timestamp GUIDs)
        for (final localEpisode in matchingEpisodes) {
          logger.debug('Feed', 'Deleting episode: ${localEpisode.guid}');
          await podcastBloc.podcastService.repository.deleteEpisode(localEpisode);
        }
        
        // Update cache
        _updateLocalDownloadStatus(episode, false);
        
        final deletedCount = matchingEpisodes.length;
        _showSnackBar('Deleted $deletedCount local download${deletedCount > 1 ? 's' : ''}', Colors.orange);
      } else {
        _showSnackBar('Local download not found', Colors.red);
      }
    } catch (e) {
      logger.error('Feed', 'Error deleting local download for episode: ${episode.episodeTitle}', e.toString());
      _showSnackBar('Error deleting local download: $e', Colors.red);
    }

    _hideContextMenu();
  }

  // Generate consistent GUID for PinePods episodes for local downloads
  String _generateEpisodeGuid(PinepodsEpisode episode) {
    return 'pinepods_${episode.episodeId}';
  }

  // Check if episode is downloaded locally
  Future<bool> _isEpisodeDownloadedLocally(PinepodsEpisode episode) async {
    final guid = _generateEpisodeGuid(episode);
    final logger = AppLogger();
    logger.debug('Feed', 'Checking download status for episode: ${episode.episodeTitle}, GUID: $guid');
    
    // Check cache first
    if (_localDownloadStatus.containsKey(guid)) {
      logger.debug('Feed', 'Found cached status for $guid: ${_localDownloadStatus[guid]}');
      return _localDownloadStatus[guid]!;
    }
    
    try {
      final podcastBloc = Provider.of<PodcastBloc>(context, listen: false);
      
      // Get all episodes and find matches with both new and old GUID formats
      final allEpisodes = await podcastBloc.podcastService.repository.findAllEpisodes();
      final matchingEpisodes = allEpisodes.where((ep) => 
        ep.guid == guid || ep.guid.startsWith('${guid}_')
      ).toList();
      
      logger.debug('Feed', 'Repository lookup for $guid: found ${matchingEpisodes.length} matching episodes');
      
      for (final match in matchingEpisodes) {
        logger.debug('Feed', 'Match: ${match.guid} - downloaded: ${match.downloaded}, downloadState: ${match.downloadState}, downloadPercentage: ${match.downloadPercentage}');
      }
      
      // Consider downloaded if ANY matching episode is downloaded
      final isDownloaded = matchingEpisodes.any((ep) => 
        ep.downloaded || ep.downloadState == DownloadState.downloaded
      );
      
      logger.debug('Feed', 'Final download status for $guid: $isDownloaded');
      
      // Cache the result
      _localDownloadStatus[guid] = isDownloaded;
      return isDownloaded;
    } catch (e) {
      final logger = AppLogger();
      logger.error('Feed', 'Error checking local download status for episode: ${episode.episodeTitle}', e.toString());
      return false;
    }
  }

  // Update local download status cache
  void _updateLocalDownloadStatus(PinepodsEpisode episode, bool isDownloaded) {
    final guid = _generateEpisodeGuid(episode);
    _localDownloadStatus[guid] = isDownloaded;
  }

  // Helper method to update episode properties efficiently
  PinepodsEpisode _updateEpisodeProperty(
    PinepodsEpisode episode, {
    bool? saved,
    bool? downloaded,
    bool? queued,
    bool? completed,
  }) {
    return PinepodsEpisode(
      podcastName: episode.podcastName,
      episodeTitle: episode.episodeTitle,
      episodePubDate: episode.episodePubDate,
      episodeDescription: episode.episodeDescription,
      episodeArtwork: episode.episodeArtwork,
      episodeUrl: episode.episodeUrl,
      episodeDuration: episode.episodeDuration,
      listenDuration: episode.listenDuration,
      episodeId: episode.episodeId,
      completed: completed ?? episode.completed,
      saved: saved ?? episode.saved,
      queued: queued ?? episode.queued,
      downloaded: downloaded ?? episode.downloaded,
      isYoutube: episode.isYoutube,
    );
  }

  void _showSnackBar(String message, Color backgroundColor) {
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(message),
        backgroundColor: backgroundColor,
        duration: const Duration(seconds: 2),
      ),
    );
  }

  @override
  void dispose() {
    // Don't dispose global audio service - it should persist across pages
    super.dispose();
  }


  @override
  Widget build(BuildContext context) {
    if (_isLoading) {
      return const SliverFillRemaining(
        child: Center(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              CircularProgressIndicator(),
              SizedBox(height: 16),
              Text('Loading recent episodes...'),
            ],
          ),
        ),
      );
    }

    if (_errorMessage.isNotEmpty) {
      return SliverFillRemaining(
        child: Center(
          child: Padding(
            padding: const EdgeInsets.all(16.0),
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Icon(
                  Icons.error_outline,
                  color: Theme.of(context).colorScheme.error,
                  size: 48,
                ),
                const SizedBox(height: 16),
                Text(
                  _errorMessage,
                  style: TextStyle(
                    color: Theme.of(context).colorScheme.error,
                  ),
                  textAlign: TextAlign.center,
                ),
                const SizedBox(height: 16),
                ElevatedButton(
                  onPressed: _refresh,
                  child: const Text('Retry'),
                ),
              ],
            ),
          ),
        ),
      );
    }

    if (_episodes.isEmpty) {
      return const SliverFillRemaining(
        child: Center(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Icon(
                Icons.inbox_outlined,
                size: 64,
                color: Colors.grey,
              ),
              SizedBox(height: 16),
              Text(
                'No recent episodes found',
                style: TextStyle(
                  fontSize: 18,
                  color: Colors.grey,
                ),
              ),
              SizedBox(height: 8),
              Text(
                'Episodes from the last 30 days will appear here',
                style: TextStyle(
                  color: Colors.grey,
                ),
                textAlign: TextAlign.center,
              ),
            ],
          ),
        ),
      );
    }

    return _buildEpisodesList();
  }

  Widget _buildEpisodesList() {
    return SliverList(
      delegate: SliverChildBuilderDelegate(
        (context, index) {
          if (index == 0) {
            // Header
            return Padding(
              padding: const EdgeInsets.all(16.0),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  const Text(
                    'Recent Episodes',
                    style: TextStyle(
                      fontSize: 24,
                      fontWeight: FontWeight.bold,
                    ),
                  ),
                  IconButton(
                    icon: const Icon(Icons.refresh),
                    onPressed: _refresh,
                  ),
                ],
              ),
            );
          }
          // Episodes (index - 1 because of header)
          final episodeIndex = index - 1;
          return PinepodsEpisodeCard(
            episode: _episodes[episodeIndex],
            onTap: () {
              Navigator.push(
                context,
                MaterialPageRoute(
                  builder: (context) => PinepodsEpisodeDetails(
                    initialEpisode: _episodes[episodeIndex],
                  ),
                ),
              );
            },
            onLongPress: () => _showContextMenu(episodeIndex),
            onPlayPressed: () => _playEpisode(_episodes[episodeIndex]),
          );
        },
        childCount: _episodes.length + 1, // +1 for header
      ),
    );
  }

  Widget _buildEpisodeCard(PinepodsEpisode episode, int episodeIndex) {
    return Card(
      margin: const EdgeInsets.symmetric(horizontal: 12.0, vertical: 4.0),
      elevation: 1,
      child: InkWell(
        onTap: () {
          // TODO: Navigate to episode details or start playing
        },
        onLongPress: () => _showContextMenu(episodeIndex),
        borderRadius: BorderRadius.circular(8),
        child: Padding(
          padding: const EdgeInsets.all(12.0),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              // Episode artwork (smaller)
              ClipRRect(
                borderRadius: BorderRadius.circular(6),
                child: episode.episodeArtwork.isNotEmpty
                    ? Image.network(
                        episode.episodeArtwork,
                        width: 50,
                        height: 50,
                        fit: BoxFit.cover,
                        cacheWidth: 100, // Optimize memory usage
                        cacheHeight: 100,
                        errorBuilder: (context, error, stackTrace) {
                          return Container(
                            width: 50,
                            height: 50,
                            decoration: BoxDecoration(
                              color: Colors.grey[300],
                              borderRadius: BorderRadius.circular(6),
                            ),
                            child: const Icon(
                              Icons.music_note,
                              color: Colors.grey,
                              size: 24,
                            ),
                          );
                        },
                        loadingBuilder: (context, child, loadingProgress) {
                          if (loadingProgress == null) return child;
                          return Container(
                            width: 50,
                            height: 50,
                            decoration: BoxDecoration(
                              color: Colors.grey[200],
                              borderRadius: BorderRadius.circular(6),
                            ),
                            child: const Center(
                              child: SizedBox(
                                width: 20,
                                height: 20,
                                child: CircularProgressIndicator(strokeWidth: 2),
                              ),
                            ),
                          );
                        },
                      )
                    : Container(
                        width: 50,
                        height: 50,
                        decoration: BoxDecoration(
                          color: Colors.grey[300],
                          borderRadius: BorderRadius.circular(6),
                        ),
                        child: const Icon(
                          Icons.music_note,
                          color: Colors.grey,
                          size: 24,
                        ),
                      ),
              ),
              const SizedBox(width: 12),
              
              // Episode info
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      episode.episodeTitle,
                      style: const TextStyle(
                        fontSize: 14,
                        fontWeight: FontWeight.w600,
                      ),
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                    ),
                    const SizedBox(height: 2),
                    Text(
                      episode.podcastName,
                      style: TextStyle(
                        fontSize: 12,
                        color: Theme.of(context).primaryColor,
                        fontWeight: FontWeight.w500,
                      ),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                    const SizedBox(height: 4),
                    Row(
                      children: [
                        Text(
                          episode.formattedPubDate,
                          style: TextStyle(
                            fontSize: 11,
                            color: Colors.grey[600],
                          ),
                        ),
                        const SizedBox(width: 8),
                        Text(
                          episode.formattedDuration,
                          style: TextStyle(
                            fontSize: 11,
                            color: Colors.grey[600],
                          ),
                        ),
                      ],
                    ),
                    
                    // Progress bar if episode has been started
                    if (episode.isStarted) ...[
                      const SizedBox(height: 6),
                      LinearProgressIndicator(
                        value: episode.progressPercentage / 100,
                        backgroundColor: Colors.grey[300],
                        valueColor: AlwaysStoppedAnimation<Color>(
                          Theme.of(context).primaryColor,
                        ),
                        minHeight: 2,
                      ),
                    ],
                  ],
                ),
              ),
              
              // Action button (just play)
              IconButton(
                icon: Icon(
                  episode.completed ? Icons.replay : Icons.play_arrow,
                  color: Theme.of(context).primaryColor,
                ),
                onPressed: () => _playEpisode(episode),
                iconSize: 24,
                padding: const EdgeInsets.all(8),
                constraints: const BoxConstraints(
                  minWidth: 40,
                  minHeight: 40,
                ),
              ),
              
              // Status indicators (compact)
              if (episode.saved || episode.downloaded || episode.queued)
                SizedBox(
                  width: 20,
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      if (episode.saved)
                        Icon(
                          Icons.bookmark,
                          color: Colors.orange[600],
                          size: 14,
                        ),
                      if (episode.downloaded)
                        Icon(
                          Icons.download_done,
                          color: Colors.blue[600],
                          size: 14,
                        ),
                      if (episode.queued)
                        Icon(
                          Icons.queue_music,
                          color: Colors.purple[600],
                          size: 14,
                        ),
                    ],
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }

}