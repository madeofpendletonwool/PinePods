// lib/ui/pinepods/playlist_episodes.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_audio_service.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/ui/widgets/platform_progress_indicator.dart';
import 'package:pinepods_mobile/ui/widgets/pinepods_episode_card.dart';
import 'package:pinepods_mobile/ui/widgets/episode_context_menu.dart';
import 'package:pinepods_mobile/ui/pinepods/episode_details.dart';
import 'package:provider/provider.dart';

class PlaylistEpisodesPage extends StatefulWidget {
  final PlaylistData playlist;

  const PlaylistEpisodesPage({
    Key? key,
    required this.playlist,
  }) : super(key: key);

  @override
  State<PlaylistEpisodesPage> createState() => _PlaylistEpisodesPageState();
}

class _PlaylistEpisodesPageState extends State<PlaylistEpisodesPage> {
  final PinepodsService _pinepodsService = PinepodsService();
  PlaylistEpisodesResponse? _playlistResponse;
  bool _isLoading = true;
  String? _errorMessage;
  
  // Audio service and context menu state
  PinepodsAudioService? _audioService;
  int? _contextMenuEpisodeIndex;

  @override
  void initState() {
    super.initState();
    _loadPlaylistEpisodes();
  }

  Future<void> _loadPlaylistEpisodes() async {
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;

    if (settings.pinepodsServer == null || 
        settings.pinepodsApiKey == null || 
        settings.pinepodsUserId == null) {
      setState(() {
        _errorMessage = 'Not connected to PinePods server. Please connect in Settings.';
        _isLoading = false;
      });
      return;
    }

    setState(() {
      _isLoading = true;
      _errorMessage = null;
    });

    try {
      _pinepodsService.setCredentials(
        settings.pinepodsServer!,
        settings.pinepodsApiKey!,
      );
      
      final response = await _pinepodsService.getPlaylistEpisodes(
        settings.pinepodsUserId!, 
        widget.playlist.playlistId,
      );
      
      setState(() {
        _playlistResponse = response;
        _isLoading = false;
      });
    } catch (e) {
      setState(() {
        _errorMessage = e.toString();
        _isLoading = false;
      });
    }
  }

  IconData _getPlaylistIcon(String? iconName) {
    if (iconName == null) return Icons.playlist_play;
    
    // Map common icon names to Material icons
    switch (iconName) {
      case 'ph-playlist':
        return Icons.playlist_play;
      case 'ph-music-notes':
        return Icons.music_note;
      case 'ph-play-circle':
        return Icons.play_circle;
      case 'ph-headphones':
        return Icons.headphones;
      case 'ph-star':
        return Icons.star;
      case 'ph-heart':
        return Icons.favorite;
      case 'ph-bookmark':
        return Icons.bookmark;
      case 'ph-clock':
        return Icons.access_time;
      case 'ph-calendar':
        return Icons.calendar_today;
      case 'ph-timer':
        return Icons.timer;
      case 'ph-shuffle':
        return Icons.shuffle;
      case 'ph-repeat':
        return Icons.repeat;
      case 'ph-microphone':
        return Icons.mic;
      case 'ph-queue':
        return Icons.queue_music;
      default:
        return Icons.playlist_play;
    }
  }

  String _getEmptyStateMessage() {
    switch (widget.playlist.name) {
      case 'Fresh Releases':
        return 'No new episodes have been released in the last 24 hours. Check back later for fresh content!';
      case 'Currently Listening':
        return 'Start listening to some episodes and they\'ll appear here for easy access.';
      case 'Almost Done':
        return 'You don\'t have any episodes that are near completion. Keep listening!';
      default:
        return 'No episodes match the current playlist criteria. Try adjusting the filters or add more podcasts.';
    }
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
    }
  }

  Future<void> _playEpisode(PinepodsEpisode episode) async {
    // Try to initialize audio service if not already done
    _initializeAudioService();
    
    if (_audioService == null) {
      _showSnackBar('Audio service not available', Colors.red);
      return;
    }

    try {
      await _audioService!.playPinepodsEpisode(pinepodsEpisode: episode);
    } catch (e) {
      if (mounted) {
        _showSnackBar('Failed to play episode: $e', Colors.red);
      }
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
    final episode = _playlistResponse!.episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    try {
      final success = await _pinepodsService.saveEpisode(
        episode.episodeId,
        userId,
        episode.isYoutube,
      );

      if (success && mounted) {
        _showSnackBar('Episode saved', Colors.green);
      } else if (mounted) {
        _showSnackBar('Failed to save episode', Colors.red);
      }
    } catch (e) {
      if (mounted) {
        _showSnackBar('Error saving episode: $e', Colors.red);
      }
    }
  }

  Future<void> _removeSavedEpisode(int episodeIndex) async {
    final episode = _playlistResponse!.episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    try {
      final success = await _pinepodsService.removeSavedEpisode(
        episode.episodeId,
        userId,
        episode.isYoutube,
      );

      if (success && mounted) {
        _showSnackBar('Episode removed from saved', Colors.orange);
      } else if (mounted) {
        _showSnackBar('Failed to remove saved episode', Colors.red);
      }
    } catch (e) {
      if (mounted) {
        _showSnackBar('Error removing saved episode: $e', Colors.red);
      }
    }
  }

  Future<void> _downloadEpisode(int episodeIndex) async {
    final episode = _playlistResponse!.episodes[episodeIndex];
    _showSnackBar('Download started for ${episode.episodeTitle}', Colors.blue);
    // Note: Actual download implementation would depend on download service integration
  }

  Future<void> _deleteEpisode(int episodeIndex) async {
    final episode = _playlistResponse!.episodes[episodeIndex];
    _showSnackBar('Delete requested for ${episode.episodeTitle}', Colors.orange);
    // Note: Actual delete implementation would depend on download service integration
  }

  Future<void> _localDownloadEpisode(int episodeIndex) async {
    final episode = _playlistResponse!.episodes[episodeIndex];
    _showSnackBar('Local download started for ${episode.episodeTitle}', Colors.blue);
    // Note: Actual local download implementation would depend on download service integration
  }

  Future<void> _toggleQueueEpisode(int episodeIndex) async {
    final episode = _playlistResponse!.episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    try {
      if (episode.queued) {
        final success = await _pinepodsService.removeQueuedEpisode(
          episode.episodeId,
          userId,
          episode.isYoutube,
        );
        
        if (success && mounted) {
          _showSnackBar('Episode removed from queue', Colors.orange);
        }
      } else {
        final success = await _pinepodsService.queueEpisode(
          episode.episodeId,
          userId,
          episode.isYoutube,
        );
        
        if (success && mounted) {
          _showSnackBar('Episode added to queue', Colors.green);
        }
      }
    } catch (e) {
      if (mounted) {
        _showSnackBar('Error updating queue: $e', Colors.red);
      }
    }
  }

  Future<void> _toggleMarkComplete(int episodeIndex) async {
    final episode = _playlistResponse!.episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    try {
      if (episode.completed) {
        final success = await _pinepodsService.markEpisodeUncompleted(
          episode.episodeId,
          userId,
          episode.isYoutube,
        );
        
        if (success && mounted) {
          _showSnackBar('Episode marked as incomplete', Colors.orange);
        }
      } else {
        final success = await _pinepodsService.markEpisodeCompleted(
          episode.episodeId,
          userId,
          episode.isYoutube,
        );
        
        if (success && mounted) {
          _showSnackBar('Episode marked as complete', Colors.green);
        }
      }
    } catch (e) {
      if (mounted) {
        _showSnackBar('Error updating completion status: $e', Colors.red);
      }
    }
  }

  void _showSnackBar(String message, Color backgroundColor) {
    if (mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(message),
          backgroundColor: backgroundColor,
          duration: const Duration(seconds: 2),
        ),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    // Show context menu as a modal overlay if needed
    if (_contextMenuEpisodeIndex != null) {
      final episodeIndex = _contextMenuEpisodeIndex!;
      final episode = _playlistResponse!.episodes[episodeIndex];
      WidgetsBinding.instance.addPostFrameCallback((_) {
        showDialog(
          context: context,
          barrierColor: Colors.black.withOpacity(0.3),
          builder: (context) => EpisodeContextMenu(
            episode: episode,
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
              _hideContextMenu();
            },
          ),
        );
      });
      // Reset the context menu index after storing it locally
      _contextMenuEpisodeIndex = null;
    }

    return Scaffold(
      appBar: AppBar(
        title: Text(widget.playlist.name),
        backgroundColor: Theme.of(context).scaffoldBackgroundColor,
        elevation: 0,
      ),
      body: _buildBody(),
    );
  }

  Widget _buildBody() {
    if (_isLoading) {
      return const Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            PlatformProgressIndicator(),
            SizedBox(height: 16),
            Text('Loading playlist episodes...'),
          ],
        ),
      );
    }

    if (_errorMessage != null) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(32.0),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Icon(
                Icons.error_outline,
                size: 75,
                color: Theme.of(context).colorScheme.error,
              ),
              const SizedBox(height: 16),
              Text(
                'Error loading playlist',
                style: Theme.of(context).textTheme.titleLarge,
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 8),
              Text(
                _errorMessage!,
                style: Theme.of(context).textTheme.bodyMedium,
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 16),
              ElevatedButton(
                onPressed: _loadPlaylistEpisodes,
                child: const Text('Retry'),
              ),
            ],
          ),
        ),
      );
    }

    if (_playlistResponse == null) {
      return const Center(
        child: Text('No data available'),
      );
    }

    return CustomScrollView(
      slivers: [
        // Playlist header
        SliverToBoxAdapter(
          child: Container(
            padding: const EdgeInsets.all(20.0),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Row(
                  children: [
                    Icon(
                      _getPlaylistIcon(_playlistResponse!.playlistInfo.iconName),
                      size: 48,
                      color: Theme.of(context).primaryColor,
                    ),
                    const SizedBox(width: 16),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            _playlistResponse!.playlistInfo.name,
                            style: const TextStyle(
                              fontSize: 24,
                              fontWeight: FontWeight.bold,
                            ),
                          ),
                          if (_playlistResponse!.playlistInfo.description != null &&
                              _playlistResponse!.playlistInfo.description!.isNotEmpty)
                            Padding(
                              padding: const EdgeInsets.only(top: 4),
                              child: Text(
                                _playlistResponse!.playlistInfo.description!,
                                style: TextStyle(
                                  fontSize: 14,
                                  color: Theme.of(context).textTheme.bodyMedium?.color,
                                ),
                              ),
                            ),
                          Padding(
                            padding: const EdgeInsets.only(top: 4),
                            child: Text(
                              '${_playlistResponse!.playlistInfo.episodeCount ?? _playlistResponse!.episodes.length} episodes',
                              style: TextStyle(
                                fontSize: 14,
                                color: Theme.of(context).textTheme.bodyMedium?.color,
                              ),
                            ),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ],
            ),
          ),
        ),
        
        // Episodes list
        if (_playlistResponse!.episodes.isEmpty)
          SliverFillRemaining(
            hasScrollBody: false,
            child: Padding(
              padding: const EdgeInsets.all(32.0),
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Icon(
                    Icons.playlist_remove,
                    size: 75,
                    color: Theme.of(context).primaryColor.withOpacity(0.5),
                  ),
                  const SizedBox(height: 16),
                  Text(
                    'No Episodes Found',
                    style: Theme.of(context).textTheme.titleLarge,
                    textAlign: TextAlign.center,
                  ),
                  const SizedBox(height: 8),
                  Text(
                    _getEmptyStateMessage(),
                    style: Theme.of(context).textTheme.bodyMedium,
                    textAlign: TextAlign.center,
                  ),
                ],
              ),
            ),
          )
        else
          SliverList(
            delegate: SliverChildBuilderDelegate(
              (context, index) {
                final episode = _playlistResponse!.episodes[index];
                return Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 8.0, vertical: 2.0),
                  child: PinepodsEpisodeCard(
                    episode: episode,
                    onTap: () {
                      Navigator.push(
                        context,
                        MaterialPageRoute(
                          builder: (context) => PinepodsEpisodeDetails(
                            initialEpisode: episode,
                          ),
                        ),
                      );
                    },
                    onLongPress: () => _showContextMenu(index),
                    onPlayPressed: () => _playEpisode(episode),
                  ),
                );
              },
              childCount: _playlistResponse!.episodes.length,
            ),
          ),
      ],
    );
  }
}