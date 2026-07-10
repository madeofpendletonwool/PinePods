// lib/ui/pinepods/home.dart
import 'dart:async';

import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/bloc/podcast/audio_bloc.dart';
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
import 'package:pinepods_mobile/ui/pinepods/podcast_nav.dart';
import 'package:pinepods_mobile/ui/utils/player_utils.dart';
import 'package:pinepods_mobile/ui/widgets/server_error_page.dart';
import 'package:pinepods_mobile/services/error_handling_service.dart';
import 'package:pinepods_mobile/services/auto_download/auto_download_service.dart';
import 'package:pinepods_mobile/services/auto_download/queue_download_service.dart';
import 'package:pinepods_mobile/services/auto_download/mirror_download_service.dart';
import 'package:pinepods_mobile/ui/utils/live_progress.dart';
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

  // Refreshes stats/"Continue Listening"/"Up Next" whenever a *different*
  // episode becomes the now-playing one - covers playback started from
  // anywhere (mini player, Episode Details, Android Auto, auto-advance to
  // the next queued episode), not just navigation that started on this page.
  AudioBloc? _audioBloc;
  StreamSubscription<Episode?>? _nowPlayingSub;
  String? _lastNowPlayingGuid;

  @override
  void initState() {
    super.initState();
    _loadHomeContent();
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final bloc = Provider.of<AudioBloc>(context, listen: false);
    if (_audioBloc != bloc) {
      _nowPlayingSub?.cancel();
      _audioBloc = bloc;
      _lastNowPlayingGuid = bloc.nowPlaying?.valueOrNull?.guid;
      _nowPlayingSub = bloc.nowPlaying?.listen((episode) {
        if (episode?.guid != _lastNowPlayingGuid) {
          _lastNowPlayingGuid = episode?.guid;
          _refreshHomeContentSilently();
        }
      });
    }
  }

  @override
  void dispose() {
    _nowPlayingSub?.cancel();
    super.dispose();
  }

  /// Re-fetches home data in the background, without showing the full-page
  /// loading spinner, so the previously-loaded content stays visible (and
  /// usable) while the refresh is in flight. Used after actions that can
  /// change what should be shown - starting playback, or returning from a
  /// screen (Episode Details, etc.) where the queue may have changed -
  /// unlike [_loadHomeContent] this never surfaces a failure to the user:
  /// it's a best-effort background sync, and the last good snapshot is a
  /// better fallback than an error screen.
  Future<void> _refreshHomeContentSilently() async {
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;

    if (settings.pinepodsServer == null ||
        settings.pinepodsApiKey == null ||
        settings.pinepodsUserId == null) {
      return;
    }

    try {
      _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
      GlobalServices.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);

      final futures = await Future.wait([
        _pinepodsService.getHomeOverview(settings.pinepodsUserId!),
        _pinepodsService.getPlaylists(settings.pinepodsUserId!),
      ]);

      if (!mounted) return;
      setState(() {
        _homeData = futures[0] as HomeOverview;
        _playlistData = futures[1] as PlaylistResponse;
      });
    } catch (e) {
      // Best-effort background refresh - keep showing the last good snapshot.
    }
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

      // Check for new episodes on auto-download podcasts (fire-and-forget)
      AutoDownloadService.checkAndDownloadNewEpisodes(
        context: context,
        pinepodsService: _pinepodsService,
        userId: settings.pinepodsUserId!,
      );

      // Keep the top-N queued episodes downloaded (fire-and-forget)
      QueueDownloadService.syncQueueDownloads(
        context: context,
        pinepodsService: _pinepodsService,
        userId: settings.pinepodsUserId!,
      );

      // Mirror the server's downloaded episodes to this device (fire-and-forget)
      MirrorDownloadService.syncMirror(
        context: context,
        pinepodsService: _pinepodsService,
        userId: settings.pinepodsUserId!,
      );

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
          // The per-card flag above is enough for this card's own icon, but
          // the "Up Next" preview and the Queue stat count are a separate
          // snapshot that needs its own refresh.
          _refreshHomeContentSilently();
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
          _refreshHomeContentSilently();
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
          podcastId: homeEpisode.podcastId,
        );

        final pageContext = context;
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

                // Up Next Section (queue preview) - kept below the fold so the
                // Library / Continue Listening / Top Podcasts overview is unchanged.
                if (_homeData!.queuePreview.isNotEmpty) ...[
                  _buildUpNextSection(),
                  const SizedBox(height: 24),
                ],

                // This Week listening stats (only when there is activity)
                if (_homeData!.weeklyStats.hasActivity) ...[
                  _buildWeeklyStatsSection(),
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
                ).then((_) => _refreshHomeContentSilently());
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

  Widget _buildUpNextSection() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          'Up Next',
          style: Theme.of(context).textTheme.headlineSmall?.copyWith(
            fontWeight: FontWeight.bold,
          ),
        ),
        const SizedBox(height: 16),
        ...(_homeData!.queuePreview.map((episode) =>
          Padding(
            padding: const EdgeInsets.only(bottom: 12),
            child: _EpisodeCard(
              episode: episode,
              onTap: () {
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
                ).then((_) => _refreshHomeContentSilently());
              },
              onPlayPressed: () => _playEpisode(episode),
            ),
          ),
        )),
      ],
    );
  }

  Widget _buildWeeklyStatsSection() {
    final stats = _homeData!.weeklyStats;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          'This Week',
          style: Theme.of(context).textTheme.headlineSmall?.copyWith(
            fontWeight: FontWeight.bold,
          ),
        ),
        const SizedBox(height: 16),
        Row(
          children: [
            Expanded(
              child: _WeeklyStatCard(
                title: 'Listened',
                value: stats.formattedListened,
                icon: Icons.headphones,
                color: Colors.purple,
              ),
            ),
            const SizedBox(width: 12),
            Expanded(
              child: _WeeklyStatCard(
                title: 'Completed',
                value: stats.episodesCompleted.toString(),
                icon: Icons.check_circle,
                color: Colors.green,
              ),
            ),
          ],
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
                ).then((_) => _refreshHomeContentSilently());
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
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(16),
      ),
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
          ],
        ),
      ),
    );
  }
}

class _WeeklyStatCard extends StatelessWidget {
  final String title;
  final String value;
  final IconData icon;
  final Color color;

  const _WeeklyStatCard({
    required this.title,
    required this.value,
    required this.icon,
    required this.color,
  });

  @override
  Widget build(BuildContext context) {
    return Card(
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(16),
      ),
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          children: [
            Icon(icon, color: color, size: 24),
            const SizedBox(height: 8),
            Text(
              value,
              style: Theme.of(context).textTheme.headlineSmall?.copyWith(
                fontWeight: FontWeight.bold,
                color: color,
              ),
            ),
            const SizedBox(height: 4),
            Text(
              title,
              style: Theme.of(context).textTheme.bodySmall?.copyWith(
                color: Theme.of(context).colorScheme.onSurface.withOpacity(0.7),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _EpisodeCard extends StatefulWidget {
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
  State<_EpisodeCard> createState() => _EpisodeCardState();
}

class _EpisodeCardState extends State<_EpisodeCard> {
  bool _isLoading = false;
  AudioState _audioState = AudioState.none;
  Episode? _nowPlaying;
  PositionState? _positionState;
  AudioBloc? _audioBloc;
  StreamSubscription? _nowPlayingSub;
  StreamSubscription? _audioStateSub;
  StreamSubscription? _positionSub;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final bloc = Provider.of<AudioBloc>(context, listen: false);
    if (_audioBloc != bloc) {
      _nowPlayingSub?.cancel();
      _audioStateSub?.cancel();
      _positionSub?.cancel();
      _audioBloc = bloc;

      _nowPlayingSub = bloc.nowPlaying?.listen((ep) {
        if (mounted) setState(() { _nowPlaying = ep; _isLoading = false; });
      });
      _audioStateSub = bloc.playingState?.listen((state) {
        if (mounted) setState(() {
          _audioState = state;
          if (state == AudioState.error) _isLoading = false;
        });
      });
      // Live position ticks for this card's progress bar while it's the
      // episode currently playing - mirrors what mini_player.dart already
      // does with the same stream. Without this the bar was frozen at
      // whatever listenDuration Home's data snapshot had when it loaded.
      // Ticks fire roughly once a second during playback, so only rebuild
      // this card when it's the one actually playing - checking the guid
      // before calling setState avoids every card on Home rebuilding on
      // every tick.
      _positionSub = bloc.playPosition?.listen((state) {
        if (!mounted) return;
        if (state.episode?.guid != widget.episode.episodeUrl) return;
        setState(() => _positionState = state);
      });
    }
  }

  @override
  void dispose() {
    _nowPlayingSub?.cancel();
    _audioStateSub?.cancel();
    _positionSub?.cancel();
    super.dispose();
  }

  bool get _isCurrentEpisode =>
      widget.episode.episodeUrl.isNotEmpty &&
      _nowPlaying?.guid == widget.episode.episodeUrl;

  /// Progress (0-100) to show on the bar: live position while this card is
  /// the episode actually playing, otherwise the static value from Home's
  /// last-loaded snapshot.
  double get _displayProgressPercentage => LiveProgressResolver.percentage(
        isCurrentEpisode: _isCurrentEpisode,
        staticPercentage: widget.episode.progressPercentage,
        livePercentage: _positionState?.percentage,
      );

  String? get _displayListenDurationText => LiveProgressResolver.elapsedText(
        isCurrentEpisode: _isCurrentEpisode,
        staticText: widget.episode.formattedListenDuration,
        livePosition: _positionState?.position,
      );

  bool get _showProgressSection => LiveProgressResolver.shouldShowProgress(
        isCurrentEpisode: _isCurrentEpisode,
        hasStaticProgress: widget.episode.listenDuration != null && widget.episode.listenDuration! > 0,
      );

  bool get _isPlaying =>
      _isCurrentEpisode &&
      (_audioState == AudioState.playing ||
          _audioState == AudioState.buffering ||
          _audioState == AudioState.starting);

  bool get _isPaused =>
      _isCurrentEpisode && _audioState == AudioState.pausing;

  void _onButtonTap() {
    final bloc = _audioBloc;
    if (bloc == null) return;
    if (_isPlaying) {
      bloc.transitionState(TransitionState.pause);
    } else if (_isPaused) {
      bloc.transitionState(TransitionState.play);
    } else {
      if (_isLoading) return;
      setState(() => _isLoading = true);
      widget.onPlayPressed?.call();
    }
  }

  @override
  Widget build(BuildContext context) {
    final showSpinner = _isLoading && !_isCurrentEpisode;
    final IconData playIcon;
    final Color iconColor;
    if (_isPlaying) {
      playIcon = Icons.pause_circle;
      iconColor = Theme.of(context).primaryColor;
    } else if (widget.episode.completed && !_isPaused) {
      playIcon = Icons.check_circle;
      iconColor = Colors.green;
    } else if (widget.episode.listenDuration != null && widget.episode.listenDuration! > 0) {
      playIcon = Icons.play_circle_filled;
      iconColor = Theme.of(context).primaryColor;
    } else {
      playIcon = Icons.play_circle_outline;
      iconColor = Theme.of(context).primaryColor;
    }

    // Status indicators shown inline with the duration so the right column can
    // host a larger play button without growing the card.
    final statusIcons = <Widget>[
      if (widget.episode.saved)
        Padding(
          padding: const EdgeInsets.only(left: 4),
          child: Icon(Icons.bookmark, size: 16, color: Colors.orange[600]),
        ),
      if (widget.episode.downloaded)
        Padding(
          padding: const EdgeInsets.only(left: 4),
          child: Icon(Icons.download_done, size: 16, color: Colors.green[600]),
        ),
      if (widget.episode.queued)
        Padding(
          padding: const EdgeInsets.only(left: 4),
          child: Icon(Icons.queue_music, size: 16, color: Colors.blue[600]),
        ),
    ];

    return Card(
      child: InkWell(
        onTap: widget.onTap,
        onLongPress: widget.onLongPress,
        borderRadius: BorderRadius.circular(8),
        child: Padding(
          padding: const EdgeInsets.all(12.0),
          child: Row(
            children: [
            // Episode artwork
            ClipRRect(
              borderRadius: BorderRadius.circular(8),
              child: Image.network(
                widget.episode.episodeArtwork,
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
                    widget.episode.episodeTitle,
                    style: Theme.of(context).textTheme.titleSmall?.copyWith(
                      fontWeight: FontWeight.w600,
                    ),
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                  ),
                  const SizedBox(height: 4),
                  Text(
                    widget.episode.podcastName,
                    style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: Theme.of(context).colorScheme.onSurface.withOpacity(0.7),
                    ),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                  const SizedBox(height: 8),
                  // Progress bar for in-progress episodes - also shown while
                  // this card is actively playing even if Home's snapshot
                  // hadn't recorded a listen position yet (e.g. just started
                  // from 0 via auto-advance or this card's own play button).
                  if (_showProgressSection) ...[
                    LinearProgressIndicator(
                      value: _displayProgressPercentage / 100,
                      backgroundColor: Theme.of(context).colorScheme.surfaceVariant,
                      valueColor: AlwaysStoppedAnimation<Color>(
                        Theme.of(context).colorScheme.primary,
                      ),
                    ),
                    const SizedBox(height: 4),
                    Row(
                      children: [
                        Text(
                          _displayListenDurationText ?? '',
                          style: Theme.of(context).textTheme.bodySmall,
                        ),
                        const Spacer(),
                        Text(
                          widget.episode.formattedDuration,
                          style: Theme.of(context).textTheme.bodySmall,
                        ),
                        ...statusIcons,
                      ],
                    ),
                  ] else ...[
                    Row(
                      children: [
                        Text(
                          widget.episode.formattedDuration,
                          style: Theme.of(context).textTheme.bodySmall,
                        ),
                        const Spacer(),
                        ...statusIcons,
                      ],
                    ),
                  ],
                ],
              ),
            ),
            // Larger play button (status icons moved next to the duration).
            if (widget.onPlayPressed != null) ...[
              const SizedBox(width: 4),
              SizedBox(
                width: 48,
                height: 48,
                child: AnimatedSwitcher(
                  duration: const Duration(milliseconds: 200),
                  child: showSpinner
                      ? Padding(
                          key: const ValueKey('loading'),
                          padding: const EdgeInsets.all(8.0),
                          child: CircularProgressIndicator(
                            strokeWidth: 2.5,
                            valueColor: AlwaysStoppedAnimation<Color>(
                              Theme.of(context).primaryColor,
                            ),
                          ),
                        )
                      : GestureDetector(
                          key: ValueKey(playIcon),
                          behavior: HitTestBehavior.opaque,
                          onTap: _onButtonTap,
                          child: Icon(playIcon, color: iconColor, size: 40),
                        ),
                ),
              ),
            ],
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
            Flexible(
              child: Text(
                podcast.podcastName,
                style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  fontWeight: FontWeight.w600,
                ),
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
                textAlign: TextAlign.center,
              ),
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
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(16),
        ),
        child: InkWell(
          onTap: () => _openPlaylist(context),
          borderRadius: BorderRadius.circular(16),
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
