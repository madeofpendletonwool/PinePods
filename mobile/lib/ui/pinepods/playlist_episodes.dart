// lib/ui/pinepods/playlist_episodes.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_audio_service.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/services/global_services.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/ui/widgets/platform_progress_indicator.dart';
import 'package:pinepods_mobile/ui/widgets/pinepods_episode_card.dart';
import 'package:pinepods_mobile/ui/widgets/episode_context_menu.dart';
import 'package:pinepods_mobile/ui/pinepods/podcast_nav.dart';
import 'package:pinepods_mobile/ui/pinepods/episode_details.dart';
import 'package:provider/provider.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';
import 'package:pinepods_mobile/l10n/L.dart';

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

  // Pagination state
  int _offset = 0;
  bool _isLoadingMore = false;

  // Use global audio service instead of creating local instance
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
        _errorMessage = L.of(context)!.not_connected_connect_in_settings;
        _isLoading = false;
      });
      return;
    }

    setState(() {
      _isLoading = true;
      _errorMessage = null;
      _offset = 0;
    });

    try {
      _pinepodsService.setCredentials(
        settings.pinepodsServer!,
        settings.pinepodsApiKey!,
      );
      GlobalServices.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);

      final response = await _pinepodsService.getPlaylistEpisodes(
        settings.pinepodsUserId!,
        widget.playlist.playlistId,
        limit: 50,
        offset: 0,
      );

      setState(() {
        _playlistResponse = response;
        _offset = response.episodes.length;
        _isLoading = false;
      });
    } catch (e) {
      setState(() {
        _errorMessage = e.toString();
        _isLoading = false;
      });
    }
  }

  Future<void> _loadMorePlaylistEpisodes() async {
    if (_isLoadingMore || _playlistResponse == null) return;
    if (_offset >= _playlistResponse!.total) return;

    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;

    if (settings.pinepodsServer == null ||
        settings.pinepodsApiKey == null ||
        settings.pinepodsUserId == null) {
      return;
    }

    setState(() => _isLoadingMore = true);

    try {
      final response = await _pinepodsService.getPlaylistEpisodes(
        settings.pinepodsUserId!,
        widget.playlist.playlistId,
        limit: 50,
        offset: _offset,
      );

      setState(() {
        final merged = PlaylistEpisodesResponse(
          episodes: [..._playlistResponse!.episodes, ...response.episodes],
          playlistInfo: _playlistResponse!.playlistInfo,
          total: response.total,
        );
        _playlistResponse = merged;
        _offset += response.episodes.length;
        _isLoadingMore = false;
      });
    } catch (e) {
      setState(() => _isLoadingMore = false);
    }
  }

  IconData _getPlaylistIcon(String? iconName) {
    if (iconName == null) return PhosphorIconsRegular.playlist;
    
    // Map common icon names to Material icons
    switch (iconName) {
      case 'ph-playlist':
        return PhosphorIconsRegular.playlist;
      case 'ph-music-notes':
        return PhosphorIconsRegular.musicNote;
      case 'ph-play-circle':
        return PhosphorIconsFill.playCircle;
      case 'ph-headphones':
        return PhosphorIconsRegular.headphones;
      case 'ph-star':
        return PhosphorIconsFill.star;
      case 'ph-heart':
        return PhosphorIconsFill.heart;
      case 'ph-bookmark':
        return PhosphorIconsFill.bookmarkSimple;
      case 'ph-clock':
        return PhosphorIconsRegular.clock;
      case 'ph-calendar':
        return PhosphorIconsRegular.calendarBlank;
      case 'ph-timer':
        return PhosphorIconsRegular.timer;
      case 'ph-shuffle':
        return PhosphorIconsRegular.shuffle;
      case 'ph-repeat':
        return PhosphorIconsRegular.repeat;
      case 'ph-microphone':
        return PhosphorIconsRegular.microphone;
      case 'ph-queue':
        return PhosphorIconsRegular.queue;
      default:
        return PhosphorIconsRegular.playlist;
    }
  }

  String _getEmptyStateMessage() {
    switch (widget.playlist.name) {
      case 'Fresh Releases':
        return L.of(context)!.playlist_empty_fresh;
      case 'Currently Listening':
        return L.of(context)!.playlist_empty_continue;
      case 'Almost Done':
        return L.of(context)!.playlist_empty_almost_done;
      default:
        return L.of(context)!.playlist_empty_default;
    }
  }

  PinepodsAudioService? get _audioService => GlobalServices.pinepodsAudioService;

  Future<void> _playEpisode(PinepodsEpisode episode) async {
    if (_audioService == null) {
      _showSnackBar(L.of(context)!.audio_service_not_available, Colors.red);
      return;
    }

    try {
      await _audioService!.playPinepodsEpisode(
        pinepodsEpisode: episode,
        playlistId: widget.playlist.playlistId,
      );
    } catch (e) {
      if (mounted) {
        _showSnackBar(L.of(context)!.failed_to_play_episode(e.toString()), Colors.red);
      }
    }
  }

  Future<void> _playFromTop() async {
    if (_audioService == null || _playlistResponse == null) return;
    if (_playlistResponse!.episodes.isEmpty) return;

    try {
      await _audioService!.playPinepodsEpisode(
        pinepodsEpisode: _playlistResponse!.episodes.first,
        playlistId: widget.playlist.playlistId,
      );
    } catch (e) {
      if (mounted) {
        _showSnackBar(L.of(context)!.failed_to_start_playlist(e.toString()), Colors.red);
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
      _showSnackBar(L.of(context)!.not_logged_in, Colors.red);
      return;
    }

    try {
      final success = await _pinepodsService.saveEpisode(
        episode.episodeId,
        userId,
        episode.isYoutube,
      );

      if (success && mounted) {
        _showSnackBar(L.of(context)!.episode_saved_plain, Colors.green);
      } else if (mounted) {
        _showSnackBar(L.of(context)!.failed_to_save_episode, Colors.red);
      }
    } catch (e) {
      if (mounted) {
        _showSnackBar(L.of(context)!.error_saving_episode(e.toString()), Colors.red);
      }
    }
  }

  Future<void> _removeSavedEpisode(int episodeIndex) async {
    final episode = _playlistResponse!.episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar(L.of(context)!.not_logged_in, Colors.red);
      return;
    }

    try {
      final success = await _pinepodsService.removeSavedEpisode(
        episode.episodeId,
        userId,
        episode.isYoutube,
      );

      if (success && mounted) {
        _showSnackBar(L.of(context)!.episode_removed_from_saved, Colors.orange);
      } else if (mounted) {
        _showSnackBar(L.of(context)!.failed_to_remove_saved_episode, Colors.red);
      }
    } catch (e) {
      if (mounted) {
        _showSnackBar(L.of(context)!.error_removing_saved_episode(e.toString()), Colors.red);
      }
    }
  }

  Future<void> _downloadEpisode(int episodeIndex) async {
    final episode = _playlistResponse!.episodes[episodeIndex];
    _showSnackBar(L.of(context)!.download_started_for(episode.episodeTitle), Colors.blue);
    // Note: Actual download implementation would depend on download service integration
  }

  Future<void> _deleteEpisode(int episodeIndex) async {
    final episode = _playlistResponse!.episodes[episodeIndex];
    _showSnackBar(L.of(context)!.delete_requested_for(episode.episodeTitle), Colors.orange);
    // Note: Actual delete implementation would depend on download service integration
  }

  Future<void> _localDownloadEpisode(int episodeIndex) async {
    final episode = _playlistResponse!.episodes[episodeIndex];
    _showSnackBar(L.of(context)!.local_download_started_for(episode.episodeTitle), Colors.blue);
    // Note: Actual local download implementation would depend on download service integration
  }

  Future<void> _toggleQueueEpisode(int episodeIndex, {String? position}) async {
    final episode = _playlistResponse!.episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar(L.of(context)!.not_logged_in, Colors.red);
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
          _showSnackBar(L.of(context)!.episode_removed_from_queue, Colors.orange);
        }
      } else {
        final success = await _pinepodsService.queueEpisode(
          episode.episodeId,
          userId,
          episode.isYoutube,
          position: position,
        );
        
        if (success && mounted) {
          _showSnackBar(L.of(context)!.episode_added_to_queue, Colors.green);
        }
      }
    } catch (e) {
      if (mounted) {
        _showSnackBar(L.of(context)!.error_updating_queue(e.toString()), Colors.red);
      }
    }
  }

  Future<void> _toggleMarkComplete(int episodeIndex) async {
    final episode = _playlistResponse!.episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar(L.of(context)!.not_logged_in, Colors.red);
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
          _showSnackBar(L.of(context)!.episode_marked_incomplete, Colors.orange);
        }
      } else {
        final success = await _pinepodsService.markEpisodeCompleted(
          episode.episodeId,
          userId,
          episode.isYoutube,
        );
        
        if (success && mounted) {
          _showSnackBar(L.of(context)!.episode_marked_complete, Colors.green);
        }
      }
    } catch (e) {
      if (mounted) {
        _showSnackBar(L.of(context)!.error_updating_completion_status(e.toString()), Colors.red);
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
      final pageContext = context;
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
            onQueueAt: (position) {
              Navigator.of(context).pop();
              _toggleQueueEpisode(episodeIndex, position: position);
            },
            onMarkComplete: () {
              Navigator.of(context).pop();
              _toggleMarkComplete(episodeIndex);
            },
            onDismiss: () {
              Navigator.of(context).pop();
              _hideContextMenu();
            },
            onPodcastTap: () {
              Navigator.of(context).pop();
              _hideContextMenu();
              navigateToPodcastById(
                pageContext,
                episode.podcastId,
                fallbackTitle: episode.podcastName,
                fallbackArtwork: episode.episodeArtwork,
              );
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
        actions: [
          if (_playlistResponse != null && _playlistResponse!.episodes.isNotEmpty)
            IconButton(
              icon: const Icon(PhosphorIconsRegular.playlist),
              tooltip: L.of(context)!.play_from_top,
              onPressed: _playFromTop,
            ),
        ],
      ),
      body: _buildBody(),
    );
  }

  Widget _buildBody() {
    if (_isLoading) {
      return Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            const PlatformProgressIndicator(),
            const SizedBox(height: 16),
            Text(L.of(context)!.loading_playlist_episodes),
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
                PhosphorIconsRegular.warningCircle,
                size: 75,
                color: Theme.of(context).colorScheme.error,
              ),
              const SizedBox(height: 16),
              Text(
                L.of(context)!.error_loading_playlist,
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
                child: Text(L.of(context)!.retry),
              ),
            ],
          ),
        ),
      );
    }

    if (_playlistResponse == null) {
      return Center(
        child: Text(L.of(context)!.no_data_available),
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
                              L.of(context)!.episodes_count(_playlistResponse!.playlistInfo.episodeCount ?? _playlistResponse!.episodes.length),
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
                    PhosphorIconsRegular.listDashes,
                    size: 75,
                    color: Theme.of(context).primaryColor.withOpacity(0.5),
                  ),
                  const SizedBox(height: 16),
                  Text(
                    L.of(context)!.episode_filter_no_episodes_title_label,
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
                final episodes = _playlistResponse!.episodes;
                final hasMore = _offset < _playlistResponse!.total;
                final showFooter = hasMore || _isLoadingMore;

                // Footer item
                if (showFooter && index == episodes.length) {
                  if (!_isLoadingMore) {
                    WidgetsBinding.instance.addPostFrameCallback(
                      (_) => _loadMorePlaylistEpisodes(),
                    );
                  }
                  return const Padding(
                    padding: EdgeInsets.symmetric(vertical: 16.0),
                    child: Center(child: CircularProgressIndicator()),
                  );
                }

                final episode = episodes[index];
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
              childCount: _playlistResponse!.episodes.length +
                  ((_offset < _playlistResponse!.total || _isLoadingMore) ? 1 : 0),
            ),
          ),
      ],
    );
  }
}