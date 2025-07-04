// lib/ui/pinepods/feed.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/pinepods/login_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_audio_service.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/ui/widgets/episode_context_menu.dart';
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
  PinepodsAudioService? _audioService;
  int? _contextMenuEpisodeIndex; // Index of episode showing context menu

  @override
  void initState() {
    super.initState();
    _loadRecentEpisodes();
  }

  void _initializeAudioService() {
    if (_audioService != null) return; // Already initialized
    
    try {
      final audioPlayerService = Provider.of<AudioPlayerService>(context, listen: false);
      final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
      
      _audioService = PinepodsAudioService(
        audioPlayerService,
        _pinepodsService,
        settingsBloc,
      );
    } catch (e) {
      // Provider not available - audio service will remain null
      // This is fine, we'll handle it in the play method
    }
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

      // Set credentials in the service
      _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);

      // Use the stored user ID from login
      final userId = settings.pinepodsUserId!;

      final episodes = await _pinepodsService.getRecentEpisodes(userId);
      
      setState(() {
        _episodes = episodes;
        _isLoading = false;
      });
    } catch (e) {
      setState(() {
        _errorMessage = 'Failed to load recent episodes: ${e.toString()}';
        _isLoading = false;
      });
    }
  }

  Future<void> _refresh() async {
    await _loadRecentEpisodes();
  }

  Future<void> _playEpisode(PinepodsEpisode episode) async {
    // Try to initialize audio service if not already done
    _initializeAudioService();
    
    if (_audioService == null) {
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
      await _audioService!.playPinepodsEpisode(
        pinepodsEpisode: episode,
        resume: episode.isStarted, // Resume if episode was previously started
      );

      // Show success message
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text('Now playing: ${episode.episodeTitle}'),
          backgroundColor: Colors.green,
          duration: const Duration(seconds: 2),
        ),
      );
    } catch (e) {
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

  void _showContextMenu(int episodeIndex) {
    setState(() {
      _contextMenuEpisodeIndex = episodeIndex;
    });
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
    _audioService?.dispose();
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
          return _buildEpisodeCard(_episodes[index - 1]);
        },
        childCount: _episodes.length + 1, // +1 for header
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    Widget content;
    
    if (_isLoading) {
      content = const SliverFillRemaining(
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
    } else if (_errorMessage.isNotEmpty) {
      content = SliverFillRemaining(
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
    } else if (_episodes.isEmpty) {
      content = const SliverFillRemaining(
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
    } else {
      content = _buildEpisodesList();
    }

    // Wrap in Stack to show context menu overlay
    return Stack(
      children: [
        content,
        if (_contextMenuEpisodeIndex != null)
          EpisodeContextMenu(
            episode: _episodes[_contextMenuEpisodeIndex!],
            onSave: () => _saveEpisode(_contextMenuEpisodeIndex!),
            onRemoveSaved: () => _removeSavedEpisode(_contextMenuEpisodeIndex!),
            onDownload: () {
              // TODO: Implement download functionality
              _hideContextMenu();
            },
            onQueue: () {
              // TODO: Implement queue functionality
              _hideContextMenu();
            },
            onMarkComplete: () {
              // TODO: Implement mark complete functionality
              _hideContextMenu();
            },
            onDismiss: _hideContextMenu,
          ),
      ],
    );
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
          return _buildEpisodeCard(_episodes[episodeIndex], episodeIndex);
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