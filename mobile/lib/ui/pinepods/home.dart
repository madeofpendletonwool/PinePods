// lib/ui/pinepods/home.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/bloc/podcast/podcast_bloc.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_audio_service.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/services/global_services.dart';
import 'package:pinepods_mobile/entities/home_data.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/ui/pinepods/feed.dart';
import 'package:pinepods_mobile/ui/pinepods/saved.dart';
import 'package:pinepods_mobile/ui/pinepods/downloads.dart';
import 'package:pinepods_mobile/ui/pinepods/queue.dart';
import 'package:pinepods_mobile/ui/pinepods/history.dart';
import 'package:pinepods_mobile/ui/pinepods/playlists.dart';
import 'package:pinepods_mobile/ui/pinepods/playlist_episodes.dart';
import 'package:pinepods_mobile/ui/pinepods/episode_details.dart';
import 'package:pinepods_mobile/ui/pinepods/podcast_details.dart';
import 'package:pinepods_mobile/entities/pinepods_search.dart';
import 'package:pinepods_mobile/ui/widgets/episode_context_menu.dart';
import 'package:pinepods_mobile/ui/utils/player_utils.dart';
import 'package:pinepods_mobile/ui/widgets/server_error_page.dart';
import 'package:pinepods_mobile/services/error_handling_service.dart';
import 'package:provider/provider.dart';
import 'package:intl/intl.dart';

class PinepodsHome extends StatefulWidget {
  const PinepodsHome({Key? key}) : super(key: key);

  @override
  State<PinepodsHome> createState() => _PinepodsHomeState();
}

class _PinepodsHomeState extends State<PinepodsHome> {
  bool _isLoading = true;
  String _errorMessage = '';
  HomeOverview? _homeData;
  PlaylistResponse? _playlistData;
  final PinepodsService _pinepodsService = PinepodsService();

  // Use global audio service instead of creating local instance
  int? _contextMenuEpisodeIndex;
  bool _isContextMenuForContinueListening = false;

  @override
  void initState() {
    super.initState();
    _loadHomeContent();
  }

  Future<void> _loadHomeContent() async {
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
      _errorMessage = '';
    });

    try {
      _pinepodsService.setCredentials(
        settings.pinepodsServer!,
        settings.pinepodsApiKey!,
      );
      GlobalServices.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);

      // Load home data and playlists in parallel
      final futures = await Future.wait([
        _pinepodsService.getHomeOverview(settings.pinepodsUserId!),
        _pinepodsService.getPlaylists(settings.pinepodsUserId!),
      ]);

      setState(() {
        _homeData = futures[0] as HomeOverview;
        _playlistData = futures[1] as PlaylistResponse;
        _isLoading = false;
      });
    } catch (e) {
      setState(() {
        _errorMessage = 'Error loading home content: $e';
        _isLoading = false;
      });
    }
  }

  PinepodsAudioService? get _audioService => GlobalServices.pinepodsAudioService;

  Future<void> _playEpisode(HomeEpisode homeEpisode) async {
    if (_audioService == null) {
      _showSnackBar('Audio service not available', Colors.red);
      return;
    }

    // Convert HomeEpisode to PinepodsEpisode
    final episode = PinepodsEpisode(
      podcastName: homeEpisode.podcastName,
      episodeTitle: homeEpisode.episodeTitle,
      episodePubDate: homeEpisode.episodePubDate,
      episodeDescription: homeEpisode.episodeDescription ?? '',
      episodeArtwork: homeEpisode.episodeArtwork,
      episodeUrl: homeEpisode.episodeUrl,
      episodeDuration: homeEpisode.episodeDuration,
      listenDuration: homeEpisode.listenDuration,
      episodeId: homeEpisode.episodeId,
      completed: homeEpisode.completed,
      saved: homeEpisode.saved,
      queued: homeEpisode.queued,
      downloaded: homeEpisode.downloaded,
      isYoutube: homeEpisode.isYoutube,
    );

    try {
      await playPinepodsEpisodeWithOptionalFullScreen(
        context,
        _audioService!,
        episode,
      );
    } catch (e) {
      if (mounted) {
        _showSnackBar('Failed to play episode: $e', Colors.red);
      }
    }
  }

  void _showContextMenu(int episodeIndex, bool isContinueListening) {
    setState(() {
      _contextMenuEpisodeIndex = episodeIndex;
      _isContextMenuForContinueListening = isContinueListening;
    });
  }

  void _hideContextMenu() {
    setState(() {
      _contextMenuEpisodeIndex = null;
      _isContextMenuForContinueListening = false;
    });
  }

  Future<void> _saveEpisode(int episodeIndex, bool isContinueListening) async {
    final episodes = isContinueListening
        ? _homeData!.inProgressEpisodes
        : _homeData!.recentEpisodes;
    final homeEpisode = episodes[episodeIndex];
    
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
    GlobalServices.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);

    try {
      final success = await _pinepodsService.saveEpisode(
        homeEpisode.episodeId,
        userId,
        homeEpisode.isYoutube,
      );

      if (success) {
        // Update the local state
        setState(() {
          if (isContinueListening) {
            _homeData!.inProgressEpisodes[episodeIndex] = _updateHomeEpisodeProperty(homeEpisode, saved: true);
          } else {
            _homeData!.recentEpisodes[episodeIndex] = _updateHomeEpisodeProperty(homeEpisode, saved: true);
          }
        });
        _showSnackBar('Episode saved!', Colors.green);
      } else {
        _showSnackBar('Failed to save episode', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error saving episode: $e', Colors.red);
    }
  }

  Future<void> _removeSavedEpisode(int episodeIndex, bool isContinueListening) async {
    final episodes = isContinueListening
        ? _homeData!.inProgressEpisodes
        : _homeData!.recentEpisodes;
    final homeEpisode = episodes[episodeIndex];
    
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
    GlobalServices.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);

    try {
      final success = await _pinepodsService.removeSavedEpisode(
        homeEpisode.episodeId,
        userId,
        homeEpisode.isYoutube,
      );

      if (success) {
        setState(() {
          if (isContinueListening) {
            _homeData!.inProgressEpisodes[episodeIndex] = _updateHomeEpisodeProperty(homeEpisode, saved: false);
          } else {
            _homeData!.recentEpisodes[episodeIndex] = _updateHomeEpisodeProperty(homeEpisode, saved: false);
          }
        });
        _showSnackBar('Removed from saved episodes', Colors.orange);
      } else {
        _showSnackBar('Failed to remove saved episode', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error removing saved episode: $e', Colors.red);
    }
  }

  Future<void> _downloadEpisode(int episodeIndex, bool isContinueListening) async {
    final episodes = isContinueListening
        ? _homeData!.inProgressEpisodes
        : _homeData!.recentEpisodes;
    final homeEpisode = episodes[episodeIndex];
    
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
    GlobalServices.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);

    try {
      final success = await _pinepodsService.downloadEpisode(
        homeEpisode.episodeId,
        userId,
        homeEpisode.isYoutube,
      );

      if (success) {
        setState(() {
          if (isContinueListening) {
            _homeData!.inProgressEpisodes[episodeIndex] = _updateHomeEpisodeProperty(homeEpisode, downloaded: true);
          } else {
            _homeData!.recentEpisodes[episodeIndex] = _updateHomeEpisodeProperty(homeEpisode, downloaded: true);
          }
        });
        _showSnackBar('Episode download queued!', Colors.green);
      } else {
        _showSnackBar('Failed to queue download', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error downloading episode: $e', Colors.red);
    }
  }

  Future<void> _localDownloadEpisode(int episodeIndex, bool isContinueListening) async {
    final episodes = isContinueListening
        ? _homeData!.inProgressEpisodes
        : _homeData!.recentEpisodes;
    final homeEpisode = episodes[episodeIndex];
    
    try {
      // Convert HomeEpisode to Episode for local download
      final localEpisode = Episode(
        guid: 'pinepods_${homeEpisode.episodeId}_${DateTime.now().millisecondsSinceEpoch}',
        pguid: 'pinepods_${homeEpisode.podcastName.replaceAll(' ', '_').toLowerCase()}',
        podcast: homeEpisode.podcastName,
        title: homeEpisode.episodeTitle,
        description: homeEpisode.episodeDescription,
        imageUrl: homeEpisode.episodeArtwork,
        contentUrl: homeEpisode.episodeUrl,
        duration: homeEpisode.episodeDuration,
        publicationDate: DateTime.tryParse(homeEpisode.episodePubDate),
        author: homeEpisode.podcastName,
        season: 0,
        episode: 0,
        position: homeEpisode.listenDuration ?? 0,
        played: homeEpisode.completed,
        chapters: [],
        transcriptUrls: [],
      );
      
      final podcastBloc = Provider.of<PodcastBloc>(context, listen: false);
      
      // First save the episode to the repository so it can be tracked
      await podcastBloc.podcastService.saveEpisode(localEpisode);
      
      // Use the download service from podcast bloc
      final success = await podcastBloc.downloadService.downloadEpisode(localEpisode);
      
      if (success) {
        _showSnackBar('Episode download started', Colors.green);
      } else {
        _showSnackBar('Failed to start download', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error starting local download: $e', Colors.red);
    }

    _hideContextMenu();
  }

  Future<void> _deleteEpisode(int episodeIndex, bool isContinueListening) async {
    final episodes = isContinueListening
        ? _homeData!.inProgressEpisodes
        : _homeData!.recentEpisodes;
    final homeEpisode = episodes[episodeIndex];
    
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
    GlobalServices.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);

    try {
      final success = await _pinepodsService.deleteEpisode(
        homeEpisode.episodeId,
        userId,
        homeEpisode.isYoutube,
      );

      if (success) {
        setState(() {
          if (isContinueListening) {
            _homeData!.inProgressEpisodes[episodeIndex] = _updateHomeEpisodeProperty(homeEpisode, downloaded: false);
          } else {
            _homeData!.recentEpisodes[episodeIndex] = _updateHomeEpisodeProperty(homeEpisode, downloaded: false);
          }
        });
        _showSnackBar('Episode deleted from server', Colors.orange);
      } else {
        _showSnackBar('Failed to delete episode', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error deleting episode: $e', Colors.red);
    }
  }

  Future<void> _toggleQueueEpisode(int episodeIndex, bool isContinueListening) async {
    final episodes = isContinueListening
        ? _homeData!.inProgressEpisodes
        : _homeData!.recentEpisodes;
    final homeEpisode = episodes[episodeIndex];
    
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
    GlobalServices.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);

    try {
      bool success;
      if (homeEpisode.queued) {
        success = await _pinepodsService.removeQueuedEpisode(
          homeEpisode.episodeId,
          userId,
          homeEpisode.isYoutube,
        );
        if (success) {
          setState(() {
            if (isContinueListening) {
              _homeData!.inProgressEpisodes[episodeIndex] = _updateHomeEpisodeProperty(homeEpisode, queued: false);
            } else {
              _homeData!.recentEpisodes[episodeIndex] = _updateHomeEpisodeProperty(homeEpisode, queued: false);
            }
          });
          _showSnackBar('Removed from queue', Colors.orange);
        }
      } else {
        success = await _pinepodsService.queueEpisode(
          homeEpisode.episodeId,
          userId,
          homeEpisode.isYoutube,
        );
        if (success) {
          setState(() {
            if (isContinueListening) {
              _homeData!.inProgressEpisodes[episodeIndex] = _updateHomeEpisodeProperty(homeEpisode, queued: true);
            } else {
              _homeData!.recentEpisodes[episodeIndex] = _updateHomeEpisodeProperty(homeEpisode, queued: true);
            }
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
  }

  Future<void> _toggleMarkComplete(int episodeIndex, bool isContinueListening) async {
    final episodes = isContinueListening
        ? _homeData!.inProgressEpisodes
        : _homeData!.recentEpisodes;
    final homeEpisode = episodes[episodeIndex];
    
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
    GlobalServices.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);

    try {
      bool success;
      if (homeEpisode.completed) {
        success = await _pinepodsService.markEpisodeUncompleted(
          homeEpisode.episodeId,
          userId,
          homeEpisode.isYoutube,
        );
        if (success) {
          setState(() {
            if (isContinueListening) {
              _homeData!.inProgressEpisodes[episodeIndex] = _updateHomeEpisodeProperty(homeEpisode, completed: false);
            } else {
              _homeData!.recentEpisodes[episodeIndex] = _updateHomeEpisodeProperty(homeEpisode, completed: false);
            }
          });
          _showSnackBar('Marked as incomplete', Colors.orange);
        }
      } else {
        success = await _pinepodsService.markEpisodeCompleted(
          homeEpisode.episodeId,
          userId,
          homeEpisode.isYoutube,
        );
        if (success) {
          setState(() {
            if (isContinueListening) {
              _homeData!.inProgressEpisodes[episodeIndex] = _updateHomeEpisodeProperty(homeEpisode, completed: true);
            } else {
              _homeData!.recentEpisodes[episodeIndex] = _updateHomeEpisodeProperty(homeEpisode, completed: true);
            }
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
  }

  HomeEpisode _updateHomeEpisodeProperty(
    HomeEpisode episode, {
    bool? saved,
    bool? downloaded,
    bool? queued,
    bool? completed,
  }) {
    return HomeEpisode(
      episodeId: episode.episodeId,
      podcastId: episode.podcastId,
      episodeTitle: episode.episodeTitle,
      episodeDescription: episode.episodeDescription,
      episodeUrl: episode.episodeUrl,
      episodeArtwork: episode.episodeArtwork,
      episodePubDate: episode.episodePubDate,
      episodeDuration: episode.episodeDuration,
      completed: completed ?? episode.completed,
      podcastName: episode.podcastName,
      isYoutube: episode.isYoutube,
      listenDuration: episode.listenDuration,
      saved: saved ?? episode.saved,
      queued: queued ?? episode.queued,
      downloaded: downloaded ?? episode.downloaded,
    );
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
      final episodes = _isContextMenuForContinueListening
        ? (_homeData?.inProgressEpisodes ?? [])
        : (_homeData?.recentEpisodes ?? []);

      if (episodeIndex < episodes.length) {
        final homeEpisode = episodes[episodeIndex];
        final episode = PinepodsEpisode(
          podcastName: homeEpisode.podcastName,
          episodeTitle: homeEpisode.episodeTitle,
          episodePubDate: homeEpisode.episodePubDate,
          episodeDescription: homeEpisode.episodeDescription ?? '',
          episodeArtwork: homeEpisode.episodeArtwork,
          episodeUrl: homeEpisode.episodeUrl,
          episodeDuration: homeEpisode.episodeDuration,
          listenDuration: homeEpisode.listenDuration,
          episodeId: homeEpisode.episodeId,
          completed: homeEpisode.completed,
          saved: homeEpisode.saved,
          queued: homeEpisode.queued,
          downloaded: homeEpisode.downloaded,
          isYoutube: homeEpisode.isYoutube,
        );

        WidgetsBinding.instance.addPostFrameCallback((_) {
          showDialog(
            context: context,
            barrierColor: Colors.black.withOpacity(0.3),
            builder: (context) => EpisodeContextMenu(
              episode: episode,
              onSave: episode.saved ? null : () {
                Navigator.of(context).pop();
                _saveEpisode(episodeIndex, _isContextMenuForContinueListening);
              },
              onRemoveSaved: episode.saved ? () {
                Navigator.of(context).pop();
                _removeSavedEpisode(episodeIndex, _isContextMenuForContinueListening);
              } : null,
              onDownload: episode.downloaded ? () {
                Navigator.of(context).pop();
                _deleteEpisode(episodeIndex, _isContextMenuForContinueListening);
              } : () {
                Navigator.of(context).pop();
                _downloadEpisode(episodeIndex, _isContextMenuForContinueListening);
              },
              onLocalDownload: () {
                Navigator.of(context).pop();
                _localDownloadEpisode(episodeIndex, _isContextMenuForContinueListening);
              },
              onQueue: () {
                Navigator.of(context).pop();
                _toggleQueueEpisode(episodeIndex, _isContextMenuForContinueListening);
              },
              onMarkComplete: () {
                Navigator.of(context).pop();
                _toggleMarkComplete(episodeIndex, _isContextMenuForContinueListening);
              },
              onDismiss: () {
                Navigator.of(context).pop();
                _hideContextMenu();
              },
            ),
          );
        });
      }
      // Reset the context menu index after storing it locally
      _contextMenuEpisodeIndex = null;
    }

    return SliverList(
      delegate: SliverChildListDelegate([
        if (_isLoading)
          const Padding(
            padding: EdgeInsets.all(32.0),
            child: Center(
              child: Column(
                children: [
                  CircularProgressIndicator(),
                  SizedBox(height: 16),
                  Text('Loading your podcasts...'),
                ],
              ),
            ),
          )
        else if (_errorMessage.isNotEmpty)
          ServerErrorPage(
            errorMessage: _errorMessage.isServerConnectionError 
              ? null 
              : _errorMessage,
            onRetry: _loadHomeContent,
            title: 'Home Unavailable',
            subtitle: _errorMessage.isServerConnectionError
              ? 'Unable to connect to the PinePods server'
              : 'Failed to load home content',
          )
        else if (_homeData != null)
          Padding(
            padding: const EdgeInsets.all(16.0),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                // Stats Overview Section
                _buildStatsSection(),
                const SizedBox(height: 24),

                // Continue Listening Section
                if (_homeData!.inProgressEpisodes.isNotEmpty) ...[
                  _buildContinueListeningSection(),
                  const SizedBox(height: 24),
                ],

                // Top Podcasts Section
                if (_homeData!.topPodcasts.isNotEmpty) ...[
                  _buildTopPodcastsSection(),
                  const SizedBox(height: 24),
                ],

                // Smart Playlists Section
                if (_playlistData?.playlists.isNotEmpty == true) ...[
                  _buildPlaylistsSection(),
                  const SizedBox(height: 24),
                ],

                // Recent Episodes Section
                if (_homeData!.recentEpisodes.isNotEmpty) ...[
                  _buildRecentEpisodesSection(),
                  const SizedBox(height: 24),
                ],

                // Empty state if no content
                if (_homeData!.recentEpisodes.isEmpty &&
                    _homeData!.inProgressEpisodes.isEmpty &&
                    _homeData!.topPodcasts.isEmpty)
                  _buildEmptyState(),
              ],
            ),
          ),
      ]),
    );
  }

  Widget _buildStatsSection() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          'Your Library',
          style: Theme.of(context).textTheme.headlineSmall?.copyWith(
            fontWeight: FontWeight.bold,
          ),
        ),
        const SizedBox(height: 16),
        Row(
          children: [
            Expanded(
              child: _StatCard(
                title: 'Saved',
                count: _homeData!.savedCount,
                icon: Icons.bookmark,
                color: Colors.orange,
              ),
            ),
            const SizedBox(width: 12),
            Expanded(
              child: _StatCard(
                title: 'Downloaded',
                count: _homeData!.downloadedCount,
                icon: Icons.download,
                color: Colors.green,
              ),
            ),
            const SizedBox(width: 12),
            Expanded(
              child: _StatCard(
                title: 'Queue',
                count: _homeData!.queueCount,
                icon: Icons.queue_music,
                color: Colors.blue,
              ),
            ),
          ],
        ),
      ],
    );
  }

  Widget _buildContinueListeningSection() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          'Continue Listening',
          style: Theme.of(context).textTheme.headlineSmall?.copyWith(
            fontWeight: FontWeight.bold,
          ),
        ),
        const SizedBox(height: 16),
        ...(_homeData!.inProgressEpisodes.take(3).map((episode) =>
          Padding(
            padding: const EdgeInsets.only(bottom: 12),
            child: _EpisodeCard(
              episode: episode,
              onTap: () {
                // Convert HomeEpisode to PinepodsEpisode for navigation
                final pinepodsEpisode = PinepodsEpisode(
                  podcastName: episode.podcastName,
                  episodeTitle: episode.episodeTitle,
                  episodePubDate: episode.episodePubDate,
                  episodeDescription: episode.episodeDescription ?? '',
                  episodeArtwork: episode.episodeArtwork,
                  episodeUrl: episode.episodeUrl,
                  episodeDuration: episode.episodeDuration,
                  listenDuration: episode.listenDuration,
                  episodeId: episode.episodeId,
                  completed: episode.completed,
                  saved: episode.saved,
                  queued: episode.queued,
                  downloaded: episode.downloaded,
                  isYoutube: episode.isYoutube,
                );
                Navigator.push(
                  context,
                  MaterialPageRoute(
                    builder: (context) => PinepodsEpisodeDetails(
                      initialEpisode: pinepodsEpisode,
                    ),
                  ),
                );
              },
              onLongPress: () => _showContextMenu(_homeData!.inProgressEpisodes.indexOf(episode), true),
              onPlayPressed: () => _playEpisode(episode),
            ),
          ),
        )),
      ],
    );
  }

  Widget _buildTopPodcastsSection() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          'Top Podcasts',
          style: Theme.of(context).textTheme.headlineSmall?.copyWith(
            fontWeight: FontWeight.bold,
          ),
        ),
        const SizedBox(height: 16),
        SizedBox(
          height: 180,
          child: ListView.builder(
            scrollDirection: Axis.horizontal,
            itemCount: _homeData!.topPodcasts.length,
            itemBuilder: (context, index) {
              final podcast = _homeData!.topPodcasts[index];
              return Padding(
                padding: EdgeInsets.only(
                  right: index < _homeData!.topPodcasts.length - 1 ? 16 : 0,
                ),
                child: _PodcastCard(podcast: podcast),
              );
            },
          ),
        ),
      ],
    );
  }

  Widget _buildPlaylistsSection() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          'Smart Playlists',
          style: Theme.of(context).textTheme.headlineSmall?.copyWith(
            fontWeight: FontWeight.bold,
          ),
        ),
        const SizedBox(height: 16),
        SizedBox(
          height: 120,
          child: ListView.builder(
            scrollDirection: Axis.horizontal,
            itemCount: _playlistData!.playlists.length,
            itemBuilder: (context, index) {
              final playlist = _playlistData!.playlists[index];
              return Padding(
                padding: EdgeInsets.only(
                  right: index < _playlistData!.playlists.length - 1 ? 16 : 0,
                ),
                child: _PlaylistCard(playlist: playlist),
              );
            },
          ),
        ),
      ],
    );
  }

  Widget _buildRecentEpisodesSection() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          'Recent Episodes',
          style: Theme.of(context).textTheme.headlineSmall?.copyWith(
            fontWeight: FontWeight.bold,
          ),
        ),
        const SizedBox(height: 16),
        ...(_homeData!.recentEpisodes.take(5).map((episode) =>
          Padding(
            padding: const EdgeInsets.only(bottom: 12),
            child: _EpisodeCard(
              episode: episode,
              onTap: () {
                // Convert HomeEpisode to PinepodsEpisode for navigation
                final pinepodsEpisode = PinepodsEpisode(
                  podcastName: episode.podcastName,
                  episodeTitle: episode.episodeTitle,
                  episodePubDate: episode.episodePubDate,
                  episodeDescription: episode.episodeDescription ?? '',
                  episodeArtwork: episode.episodeArtwork,
                  episodeUrl: episode.episodeUrl,
                  episodeDuration: episode.episodeDuration,
                  listenDuration: episode.listenDuration,
                  episodeId: episode.episodeId,
                  completed: episode.completed,
                  saved: episode.saved,
                  queued: episode.queued,
                  downloaded: episode.downloaded,
                  isYoutube: episode.isYoutube,
                );
                Navigator.push(
                  context,
                  MaterialPageRoute(
                    builder: (context) => PinepodsEpisodeDetails(
                      initialEpisode: pinepodsEpisode,
                    ),
                  ),
                );
              },
              onLongPress: () => _showContextMenu(_homeData!.recentEpisodes.indexOf(episode), false),
              onPlayPressed: () => _playEpisode(episode),
            ),
          ),
        )),
      ],
    );
  }

  Widget _buildEmptyState() {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(32.0),
        child: Column(
          children: [
            Icon(
              Icons.podcasts_outlined,
              size: 64,
              color: Theme.of(context).colorScheme.primary.withOpacity(0.5),
            ),
            const SizedBox(height: 16),
            Text(
              'Welcome to PinePods!',
              style: Theme.of(context).textTheme.headlineSmall,
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 8),
            Text(
              'Start by searching for podcasts to subscribe to.',
              style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                color: Theme.of(context).colorScheme.onSurface.withOpacity(0.7),
              ),
              textAlign: TextAlign.center,
            ),
          ],
        ),
      ),
    );
  }

  void _navigateToPage(String pageName) {
    // This would be implemented to navigate to the appropriate page
    // For now, we'll show a placeholder snackbar
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text('Navigate to $pageName')),
    );
  }
}

class _QuickLinkCard extends StatelessWidget {
  final String title;
  final IconData icon;
  final Color color;
  final VoidCallback onTap;

  const _QuickLinkCard({
    required this.title,
    required this.icon,
    required this.color,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return Card(
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(12),
        child: Padding(
          padding: const EdgeInsets.all(16.0),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Icon(icon, color: color, size: 32),
              const SizedBox(height: 8),
              Text(
                title,
                style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  fontWeight: FontWeight.w600,
                ),
                textAlign: TextAlign.center,
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _StatCard extends StatelessWidget {
  final String title;
  final int count;
  final IconData icon;
  final Color color;

  const _StatCard({
    required this.title,
    required this.count,
    required this.icon,
    required this.color,
  });

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          children: [
            Icon(icon, color: color, size: 24),
            const SizedBox(height: 8),
            Text(
              count.toString(),
              style: Theme.of(context).textTheme.headlineSmall?.copyWith(
                fontWeight: FontWeight.bold,
                color: color,
              ),
            ),
            Text(
              title,
              style: Theme.of(context).textTheme.bodySmall,
              textAlign: TextAlign.center,
            ),
          ],
        ),
      ),
    );
  }
}

class _EpisodeCard extends StatelessWidget {
  final HomeEpisode episode;
  final VoidCallback? onTap;
  final VoidCallback? onLongPress;
  final VoidCallback? onPlayPressed;

  const _EpisodeCard({
    required this.episode,
    this.onTap,
    this.onLongPress,
    this.onPlayPressed,
  });

  @override
  Widget build(BuildContext context) {
    return Card(
      child: InkWell(
        onTap: onTap,
        onLongPress: onLongPress,
        borderRadius: BorderRadius.circular(8),
        child: Padding(
          padding: const EdgeInsets.all(12.0),
          child: Row(
            children: [
            // Episode artwork
            ClipRRect(
              borderRadius: BorderRadius.circular(8),
              child: Image.network(
                episode.episodeArtwork,
                width: 60,
                height: 60,
                fit: BoxFit.cover,
                errorBuilder: (context, error, stackTrace) {
                  return Container(
                    width: 60,
                    height: 60,
                    color: Theme.of(context).colorScheme.surfaceVariant,
                    child: Icon(
                      Icons.podcasts,
                      color: Theme.of(context).colorScheme.onSurfaceVariant,
                    ),
                  );
                },
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
                    style: Theme.of(context).textTheme.titleSmall?.copyWith(
                      fontWeight: FontWeight.w600,
                    ),
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                  ),
                  const SizedBox(height: 4),
                  Text(
                    episode.podcastName,
                    style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: Theme.of(context).colorScheme.onSurface.withOpacity(0.7),
                    ),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                  const SizedBox(height: 8),
                  // Progress bar for in-progress episodes
                  if (episode.listenDuration != null && episode.listenDuration! > 0) ...[
                    LinearProgressIndicator(
                      value: episode.progressPercentage / 100,
                      backgroundColor: Theme.of(context).colorScheme.surfaceVariant,
                      valueColor: AlwaysStoppedAnimation<Color>(
                        Theme.of(context).colorScheme.primary,
                      ),
                    ),
                    const SizedBox(height: 4),
                    Row(
                      mainAxisAlignment: MainAxisAlignment.spaceBetween,
                      children: [
                        Text(
                          episode.formattedListenDuration ?? '',
                          style: Theme.of(context).textTheme.bodySmall,
                        ),
                        Text(
                          episode.formattedDuration,
                          style: Theme.of(context).textTheme.bodySmall,
                        ),
                      ],
                    ),
                  ] else ...[
                    Text(
                      episode.formattedDuration,
                      style: Theme.of(context).textTheme.bodySmall,
                    ),
                  ],
                ],
              ),
            ),
            // Status indicators and play button
            Column(
              children: [
                if (onPlayPressed != null)
                  IconButton(
                    onPressed: onPlayPressed,
                    icon: Icon(
                      episode.completed 
                        ? Icons.check_circle 
                        : ((episode.listenDuration != null && episode.listenDuration! > 0) 
                            ? Icons.play_circle_filled 
                            : Icons.play_circle_outline),
                      color: episode.completed 
                        ? Colors.green 
                        : Theme.of(context).primaryColor,
                      size: 28,
                    ),
                    padding: EdgeInsets.zero,
                    constraints: const BoxConstraints(
                      minWidth: 32,
                      minHeight: 32,
                    ),
                  ),
                const SizedBox(height: 4),
                Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    if (episode.saved)
                      Icon(
                        Icons.bookmark,
                        size: 16,
                        color: Colors.orange[600],
                      ),
                    if (episode.downloaded)
                      Padding(
                        padding: const EdgeInsets.only(left: 4),
                        child: Icon(
                          Icons.download_done,
                          size: 16,
                          color: Colors.green[600],
                        ),
                      ),
                    if (episode.queued)
                      Padding(
                        padding: const EdgeInsets.only(left: 4),
                        child: Icon(
                          Icons.queue_music,
                          size: 16,
                          color: Colors.blue[600],
                        ),
                      ),
                  ],
                ),
              ],
            ),
          ],
        ),
      ),
    ),
      );
  }
}

class _PodcastCard extends StatelessWidget {
  final HomePodcast podcast;

  const _PodcastCard({required this.podcast});

  UnifiedPinepodsPodcast _convertToUnifiedPodcast() {
    return UnifiedPinepodsPodcast(
      id: podcast.podcastId,
      indexId: podcast.podcastIndexId ?? 0,
      title: podcast.podcastName,
      url: podcast.feedUrl ?? '',
      originalUrl: podcast.feedUrl ?? '',
      link: podcast.websiteUrl ?? '',
      description: podcast.description ?? '',
      author: podcast.author ?? '',
      ownerName: podcast.author ?? '',
      image: podcast.artworkUrl ?? '',
      artwork: podcast.artworkUrl ?? '',
      lastUpdateTime: 0,
      categories: podcast.categories != null ? {'0': podcast.categories!} : null,
      explicit: podcast.explicit ?? false,
      episodeCount: podcast.episodeCount ?? 0,
    );
  }

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: () {
        Navigator.push(
          context,
          MaterialPageRoute(
            builder: (context) => PinepodsPodcastDetails(
              podcast: _convertToUnifiedPodcast(),
              isFollowing: true,
            ),
          ),
        );
      },
      child: SizedBox(
        width: 140,
        child: Column(
          children: [
            ClipRRect(
              borderRadius: BorderRadius.circular(12),
              child: Image.network(
                podcast.artworkUrl ?? '',
                width: 140,
                height: 140,
                fit: BoxFit.cover,
                errorBuilder: (context, error, stackTrace) {
                  return Container(
                    width: 140,
                    height: 140,
                    decoration: BoxDecoration(
                      color: Theme.of(context).colorScheme.surfaceVariant,
                      borderRadius: BorderRadius.circular(12),
                    ),
                    child: Icon(
                      Icons.podcasts,
                      size: 48,
                      color: Theme.of(context).colorScheme.onSurfaceVariant,
                    ),
                  );
                },
              ),
            ),
            const SizedBox(height: 8),
            Text(
              podcast.podcastName,
              style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                fontWeight: FontWeight.w600,
              ),
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
              textAlign: TextAlign.center,
            ),
          ],
        ),
      ),
    );
  }
}

class _PlaylistCard extends StatelessWidget {
  final Playlist playlist;

  const _PlaylistCard({required this.playlist});

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 200,
      child: Card(
        child: InkWell(
          onTap: () => _openPlaylist(context),
          borderRadius: BorderRadius.circular(12),
          child: Padding(
            padding: const EdgeInsets.all(16.0),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Row(
                  children: [
                    Icon(
                      _getIconFromName(playlist.iconName),
                      color: Theme.of(context).colorScheme.primary,
                      size: 24,
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        playlist.name,
                        style: Theme.of(context).textTheme.titleMedium?.copyWith(
                          fontWeight: FontWeight.bold,
                        ),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  ],
                ),
                if (playlist.episodeCount != null) ...[
                  const SizedBox(height: 8),
                  Text(
                    '${playlist.episodeCount} episodes',
                    style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: Theme.of(context).colorScheme.onSurface.withOpacity(0.7),
                    ),
                  ),
                ],
              ],
            ),
          ),
        ),
      ),
    );
  }

  Future<void> _openPlaylist(BuildContext context) async {
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    
    if (settings.pinepodsServer == null ||
        settings.pinepodsApiKey == null ||
        settings.pinepodsUserId == null) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text('Not connected to PinePods server. Please connect in Settings.'),
            backgroundColor: Colors.red,
          ),
        );
      }
      return;
    }

    try {
      final pinepodsService = PinepodsService();
      pinepodsService.setCredentials(
        settings.pinepodsServer!,
        settings.pinepodsApiKey!,
      );
      
      final userPlaylists = await pinepodsService.getUserPlaylists(settings.pinepodsUserId!);
      final fullPlaylistData = userPlaylists.firstWhere(
        (p) => p.playlistId == playlist.playlistId,
        orElse: () => throw Exception('Playlist not found'),
      );
      
      if (context.mounted) {
        Navigator.push(
          context,
          MaterialPageRoute(
            builder: (context) => PlaylistEpisodesPage(playlist: fullPlaylistData),
          ),
        );
      }
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Error opening playlist: $e'),
            backgroundColor: Colors.red,
          ),
        );
      }
    }
  }

  IconData _getIconFromName(String iconName) {
    switch (iconName) {
      case 'ph-music-notes':
        return Icons.music_note;
      case 'ph-star':
        return Icons.star;
      case 'ph-clock':
        return Icons.access_time;
      case 'ph-heart':
        return Icons.favorite;
      default:
        return Icons.playlist_play;
    }
  }
}
