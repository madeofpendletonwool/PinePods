// lib/ui/pinepods/home.dart
import 'dart:async';

import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/bloc/podcast/audio_bloc.dart';
import 'package:pinepods_mobile/bloc/ui/pager_bloc.dart';
import 'package:pinepods_mobile/services/network/connectivity_service.dart';
import 'package:pinepods_mobile/services/podcast/podcast_service.dart';
import 'package:pinepods_mobile/ui/utils/local_download_utils.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_audio_service.dart';
import 'package:pinepods_mobile/services/global_services.dart';
import 'package:pinepods_mobile/entities/home_data.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/entities/pinepods_collection.dart';
import 'package:pinepods_mobile/ui/icons/collection_icons.dart';
import 'package:pinepods_mobile/ui/utils/artwork_colors.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/ui/pinepods/feed.dart';
import 'package:pinepods_mobile/ui/pinepods/saved.dart';
import 'package:pinepods_mobile/ui/pinepods/downloads.dart';
import 'package:pinepods_mobile/ui/pinepods/queue.dart';
import 'package:pinepods_mobile/ui/pinepods/history.dart';
import 'package:pinepods_mobile/ui/pinepods/user_stats.dart';
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
import 'package:pinepods_mobile/services/auto_download/download_cleanup_service.dart';
import 'package:pinepods_mobile/ui/widgets/pinepods_episode_card.dart';
import 'package:provider/provider.dart';
import 'package:intl/intl.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';
import 'package:pinepods_mobile/ui/widgets/lazy_network_image.dart';
import 'package:pinepods_mobile/ui/widgets/soft_ui.dart';
import 'package:pinepods_mobile/l10n/L.dart';

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
  List<PinepodsCollection> _collections = const [];
  String? _heroAccentRequested;
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
    // Recover automatically when the server becomes reachable again after an
    // offline launch, so the user doesn't have to manually retry (#935).
    ConnectivityService.instance.addListener(_onConnectivityChanged);
  }

  void _onConnectivityChanged() {
    if (!mounted) return;
    final recovered = !ConnectivityService.instance.isOffline &&
        !_isLoading &&
        (_errorMessage.isNotEmpty || _homeData == null);
    if (recovered) {
      _loadHomeContent();
    }
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
    ConnectivityService.instance.removeListener(_onConnectivityChanged);
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
      _loadCollections(settings.pinepodsUserId!);
    } catch (e) {
      // Best-effort background refresh - keep showing the last good snapshot.
    }
  }

  /// Collections other than Saved, for the home shelf. Best effort: older servers have none.
  Future<void> _loadCollections(int userId) async {
    try {
      final cols = await _pinepodsService.getCollections(userId);
      if (!mounted) return;
      setState(() => _collections = cols.where((c) => !c.isDefault).toList());
    } catch (_) {}
  }

  void _openCollection(PinepodsCollection? collection) {
    Navigator.push(
      context,
      MaterialPageRoute<void>(
        builder: (context) => Scaffold(
          appBar: AppBar(title: Text(L.of(context)!.saved_and_collections)),
          body: CustomScrollView(slivers: [PinepodsSaved(initialCollectionId: collection?.id)]),
        ),
      ),
    ).then((_) => _refreshHomeContentSilently());
  }

  Future<void> _loadHomeContent() async {
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

      // Clean up finished/old device downloads, then mirror the server's
      // downloads to this device (fire-and-forget)
      final userId = settings.pinepodsUserId!;
      DownloadCleanupService.run(
        context: context,
        pinepodsService: _pinepodsService,
        userId: userId,
      ).then((_) {
        if (!mounted) return;
        MirrorDownloadService.syncMirror(
          context: context,
          pinepodsService: _pinepodsService,
          userId: userId,
        );
      });

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
      _loadCollections(settings.pinepodsUserId!);
    } catch (e) {
      setState(() {
        _errorMessage = L.of(context)!.error_loading_home(e.toString());
        _isLoading = false;
      });
    }
  }

  PinepodsAudioService? get _audioService => GlobalServices.pinepodsAudioService;

  Future<void> _playEpisode(HomeEpisode homeEpisode) async {
    if (_audioService == null) {
      _showSnackBar(L.of(context)!.audio_service_not_available, Colors.red);
      return;
    }

    final episode = homeEpisode.toPinepodsEpisode();

    try {
      await playPinepodsEpisodeWithOptionalFullScreen(
        context,
        _audioService!,
        episode,
      );
    } catch (e) {
      if (mounted) {
        _showSnackBar(L.of(context)!.failed_to_play_episode(e.toString()), Colors.red);
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
      _showSnackBar(L.of(context)!.not_logged_in, Colors.red);
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
        _showSnackBar(L.current.episode_saved, Colors.green);
      } else {
        _showSnackBar(L.current.failed_to_save_episode, Colors.red);
      }
    } catch (e) {
      _showSnackBar(L.current.error_saving_episode(e.toString()), Colors.red);
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
      _showSnackBar(L.of(context)!.not_logged_in, Colors.red);
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
        _showSnackBar(L.current.removed_from_saved_episodes, Colors.orange);
      } else {
        _showSnackBar(L.current.failed_to_remove_saved_episode, Colors.red);
      }
    } catch (e) {
      _showSnackBar(L.current.error_removing_saved_episode(e.toString()), Colors.red);
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
      _showSnackBar(L.of(context)!.not_logged_in, Colors.red);
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
        _showSnackBar(L.current.episode_download_queued, Colors.green);
      } else {
        _showSnackBar(L.current.failed_to_queue_download, Colors.red);
      }
    } catch (e) {
      _showSnackBar(L.current.error_downloading_episode(e.toString()), Colors.red);
    }
  }

  Future<void> _localDownloadEpisode(int episodeIndex, bool isContinueListening) async {
    final episodes = isContinueListening
        ? _homeData!.inProgressEpisodes
        : _homeData!.recentEpisodes;
    final homeEpisode = episodes[episodeIndex];
    
    try {
      final success = await LocalDownloadUtils.localDownloadEpisode(context, homeEpisode.toPinepodsEpisode());

      if (success) {
        _showSnackBar(L.current.episode_download_started, Colors.green);
      } else {
        _showSnackBar(L.current.failed_to_start_download, Colors.red);
      }
    } catch (e) {
      _showSnackBar(L.current.error_starting_local_download(e.toString()), Colors.red);
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
      _showSnackBar(L.of(context)!.not_logged_in, Colors.red);
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
        _showSnackBar(L.current.episode_deleted_from_server, Colors.orange);
      } else {
        _showSnackBar(L.current.failed_to_delete_episode, Colors.red);
      }
    } catch (e) {
      _showSnackBar(L.current.error_deleting_episode(e.toString()), Colors.red);
    }
  }

  Future<void> _toggleQueueEpisode(int episodeIndex, bool isContinueListening, {String? position}) async {
    final episodes = isContinueListening
        ? _homeData!.inProgressEpisodes
        : _homeData!.recentEpisodes;
    final homeEpisode = episodes[episodeIndex];
    
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar(L.of(context)!.not_logged_in, Colors.red);
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
          _showSnackBar(L.current.removed_from_queue, Colors.orange);
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
          position: position,
        );
        if (success) {
          setState(() {
            if (isContinueListening) {
              _homeData!.inProgressEpisodes[episodeIndex] = _updateHomeEpisodeProperty(homeEpisode, queued: true);
            } else {
              _homeData!.recentEpisodes[episodeIndex] = _updateHomeEpisodeProperty(homeEpisode, queued: true);
            }
          });
          _showSnackBar(L.current.added_to_queue_excl, Colors.green);
          _refreshHomeContentSilently();
        }
      }

      if (!success) {
        _showSnackBar(L.current.failed_to_update_queue, Colors.red);
      }
    } catch (e) {
      _showSnackBar(L.current.error_updating_queue(e.toString()), Colors.red);
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
      _showSnackBar(L.of(context)!.not_logged_in, Colors.red);
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
          _showSnackBar(L.current.marked_as_incomplete, Colors.orange);
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
          _showSnackBar(L.current.marked_as_complete, Colors.green);
        }
      }

      if (!success) {
        _showSnackBar(L.current.failed_to_update_completion, Colors.red);
      }
    } catch (e) {
      _showSnackBar(L.current.error_updating_completion(e.toString()), Colors.red);
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

  /// Switch the bottom-nav selection to the Downloads tab (the full offline
  /// library). No-op if the user has removed Downloads from their tab bar.
  void _goToDownloads() {
    final pager = Provider.of<PagerBloc>(context, listen: false);
    final order = Provider.of<SettingsBloc>(context, listen: false)
        .currentSettings
        .bottomBarOrder;
    final idx = order.indexOf('Downloads');
    if (idx >= 0) {
      pager.changePage(idx);
    }
  }

  Future<void> _playLocalDownload(Episode episode) async {
    final audioService = GlobalServices.pinepodsAudioService;
    if (audioService == null) {
      _showSnackBar(L.of(context)!.audio_service_not_available, Colors.red);
      return;
    }
    try {
      final pe = LocalDownloadUtils.toPinepodsEpisode(episode);
      await playPinepodsEpisodeWithOptionalFullScreen(
        context,
        audioService,
        pe,
        resume: true,
      );
    } catch (e) {
      _showSnackBar(L.current.failed_to_play_episode(e.toString()), Colors.red);
    }
  }

  /// Offline landing shown on the Home tab when the PinePods server is
  /// unreachable (#935). Rather than a dead error page, surface the on-device
  /// Downloads library so already-downloaded episodes stay browsable/playable,
  /// and offer a jump to the full Downloads tab.
  Widget _buildOfflineHome() {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.all(16.0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Container(
            padding: const EdgeInsets.all(16),
            decoration: BoxDecoration(
              color: Colors.orange.withOpacity(0.12),
              borderRadius: BorderRadius.circular(16),
            ),
            child: Row(
              children: [
                Icon(PhosphorIconsRegular.cloudSlash, color: Colors.orange[800]),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        L.of(context)!.offline_mode,
                        style: TextStyle(
                          fontWeight: FontWeight.bold,
                          color: Colors.orange[800],
                          fontSize: 16,
                        ),
                      ),
                      const SizedBox(height: 4),
                      Text(
                        L.of(context)!.server_unreachable_offline_body,
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 16),
          FutureBuilder<List<Episode>>(
            future: Provider.of<PodcastService>(context, listen: false)
                .loadDownloads(),
            builder: (context, snapshot) {
              if (snapshot.connectionState == ConnectionState.waiting) {
                return const Padding(
                  padding: EdgeInsets.all(24.0),
                  child: Center(child: CircularProgressIndicator()),
                );
              }
              final downloads =
                  (snapshot.data ?? []).where((e) => e.downloaded).toList();
              if (downloads.isEmpty) {
                return Padding(
                  padding: const EdgeInsets.symmetric(vertical: 16.0),
                  child: Text(L.of(context)!.no_downloaded_episodes_on_device),
                );
              }
              final preview = downloads.take(5).toList();
              return Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    L.of(context)!.on_this_device_count(downloads.length),
                    style: theme.textTheme.titleMedium,
                  ),
                  const SizedBox(height: 8),
                  ...preview.map(
                    (e) => ListTile(
                      contentPadding: EdgeInsets.zero,
                      leading: const Icon(PhosphorIconsRegular.checkCircle),
                      title: Text(
                        e.title ?? L.of(context)!.untitled_episode,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                      subtitle: e.podcast == null
                          ? null
                          : Text(
                              e.podcast!,
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                            ),
                      trailing: const Icon(PhosphorIconsFill.play),
                      onTap: () => _playLocalDownload(e),
                    ),
                  ),
                ],
              );
            },
          ),
          const SizedBox(height: 16),
          ElevatedButton.icon(
            onPressed: _goToDownloads,
            icon: const Icon(PhosphorIconsRegular.downloadSimple),
            label: Text(L.of(context)!.go_to_downloads),
          ),
          const SizedBox(height: 8),
          OutlinedButton.icon(
            onPressed: () {
              ConnectivityService.instance.refresh();
              _loadHomeContent();
            },
            icon: const Icon(PhosphorIconsRegular.arrowClockwise),
            label: Text(L.of(context)!.retry_connection),
          ),
        ],
      ),
    );
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
              onQueueAt: (position) {
                Navigator.of(context).pop();
                _toggleQueueEpisode(episodeIndex, _isContextMenuForContinueListening, position: position);
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
            padding: EdgeInsets.all(48.0),
            child: Center(child: CircularProgressIndicator()),
          )
        else if (_errorMessage.isNotEmpty)
          if (_errorMessage.isServerConnectionError ||
              ConnectivityService.instance.isOffline)
            _buildOfflineHome()
          else
            ServerErrorPage(
              errorMessage: _errorMessage,
              onRetry: _loadHomeContent,
              title: L.of(context)!.home_unavailable,
              subtitle: L.of(context)!.failed_to_load_home,
            )
        else if (_homeData != null)
          Padding(
            padding: const EdgeInsets.all(16.0),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                // Stats Overview Section
                _buildStatsSection(),
                if (_homeData!.weeklyStats.hasActivity) ...[
                  const SizedBox(height: 12),
                  _buildWeekSummary(),
                ],
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

                if (_collections.isNotEmpty) ...[
                  _buildCollectionsSection(),
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
        SoftSectionHeader(title: L.of(context)!.your_library, padding: const EdgeInsets.only(bottom: 10)),
        Row(
          children: [
            Expanded(
              child: _StatTile(
                title: L.of(context)!.saved,
                value: '${_homeData!.savedCount}',
                icon: PhosphorIconsFill.bookmarkSimple,
                color: Colors.orange,
              ),
            ),
            const SizedBox(width: 12),
            Expanded(
              child: _StatTile(
                title: L.of(context)!.downloaded,
                value: '${_homeData!.downloadedCount}',
                icon: PhosphorIconsRegular.downloadSimple,
                color: Colors.green,
              ),
            ),
            const SizedBox(width: 12),
            Expanded(
              child: _StatTile(
                title: L.of(context)!.queue,
                value: '${_homeData!.queueCount}',
                icon: PhosphorIconsRegular.queue,
                color: Colors.blue,
              ),
            ),
          ],
        ),
      ],
    );
  }

  void _openEpisode(HomeEpisode episode) {
    Navigator.push(
      context,
      MaterialPageRoute(
        builder: (context) => PinepodsEpisodeDetails(initialEpisode: episode.toPinepodsEpisode()),
      ),
    ).then((_) => _refreshHomeContentSilently());
  }

  /// The first in-progress episode, large, with a resume button.
  Widget _buildContinueHero(HomeEpisode episode) {
    final theme = Theme.of(context);
    final duration = episode.episodeDuration;
    final listened = (episode.listenDuration ?? 0).clamp(0, duration > 0 ? duration : 0);
    final progress = duration > 0 ? listened / duration : 0.0;
    final left = duration - listened;
    final h = left ~/ 3600, m = (left % 3600) ~/ 60;
    final leftText = h > 0 ? L.of(context)!.stats_hm('$h', '$m') : L.of(context)!.stats_m('$m');
    // Tinted with the artwork's colour, like the episode cards, unless that's turned off.
    Color? accent;
    if (Provider.of<SettingsBloc>(context, listen: false).currentSettings.artworkAccentColors) {
      accent = ArtworkColors.cached(episode.episodeArtwork, null, theme.brightness);
      final key = '${theme.brightness.name}|${episode.episodeArtwork}';
      if (accent == null && _heroAccentRequested != key) {
        _heroAccentRequested = key;
        ArtworkColors.resolve(episode.episodeArtwork, null, theme.brightness).then((c) {
          if (mounted && c != null) setState(() {});
        });
      }
    }
    final primary = accent ?? theme.colorScheme.primary;

    return Material(
      color: primary.withValues(alpha: 0.12),
      borderRadius: BorderRadius.circular(20),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: () => _openEpisode(episode),
        onLongPress: () => _showContextMenu(0, true),
        child: Padding(
          padding: const EdgeInsets.all(14),
          child: Row(
            children: [
              LazyNetworkImage(
                imageUrl: episode.episodeArtwork,
                width: 96,
                height: 96,
                borderRadius: BorderRadius.circular(14),
              ),
              const SizedBox(width: 14),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(episode.podcastName,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: theme.textTheme.bodySmall?.copyWith(color: primary, fontWeight: FontWeight.w700)),
                    const SizedBox(height: 2),
                    Text(episode.episodeTitle,
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                        style: theme.textTheme.titleMedium?.copyWith(fontWeight: FontWeight.w800, height: 1.2)),
                    const SizedBox(height: 10),
                    ClipRRect(
                      borderRadius: BorderRadius.circular(3),
                      child: LinearProgressIndicator(
                        value: progress.toDouble(),
                        minHeight: 4,
                        backgroundColor: primary.withValues(alpha: 0.18),
                        valueColor: AlwaysStoppedAnimation(primary),
                      ),
                    ),
                    const SizedBox(height: 8),
                    Row(
                      children: [
                        if (duration > 0)
                          Expanded(
                            child: Text(L.of(context)!.home_time_left(leftText),
                                style: theme.textTheme.bodySmall?.copyWith(color: theme.hintColor)),
                          )
                        else
                          const Spacer(),
                        FilledButton.icon(
                          onPressed: () => _playEpisode(episode),
                          icon: const Icon(PhosphorIconsFill.play, size: 16),
                          label: Text(L.of(context)!.home_resume),
                          style: FilledButton.styleFrom(
                            backgroundColor: primary,
                            foregroundColor: onColor(primary),
                            visualDensity: VisualDensity.compact,
                            padding: const EdgeInsets.symmetric(horizontal: 14),
                          ),
                        ),
                      ],
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

  Widget _buildContinueListeningSection() {
    final episodes = _homeData!.inProgressEpisodes.take(3).toList();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SoftSectionHeader(title: L.of(context)!.continue_listening, padding: const EdgeInsets.only(bottom: 10)),
        _buildContinueHero(episodes.first),
        const SizedBox(height: 12),
        for (var i = 1; i < episodes.length; i++)
          Padding(
            padding: const EdgeInsets.only(bottom: 12),
            child: PinepodsEpisodeCard(
              episode: episodes[i].toPinepodsEpisode(),
              margin: EdgeInsets.zero,
              onTap: () => _openEpisode(episodes[i]),
              onLongPress: () => _showContextMenu(i, true),
              onPlayPressed: () => _playEpisode(episodes[i]),
            ),
          ),
      ],
    );
  }

  Widget _buildTopPodcastsSection() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SoftSectionHeader(title: L.of(context)!.top_podcasts, padding: const EdgeInsets.only(bottom: 10)),
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
        SoftSectionHeader(title: L.of(context)!.up_next_queue_label, padding: const EdgeInsets.only(bottom: 10)),
        ...(_homeData!.queuePreview.map((episode) =>
          Padding(
            padding: const EdgeInsets.only(bottom: 12),
            child: PinepodsEpisodeCard(
              episode: episode.toPinepodsEpisode(),
              margin: EdgeInsets.zero,
              onTap: () {
                Navigator.push(
                  context,
                  MaterialPageRoute(
                    builder: (context) => PinepodsEpisodeDetails(
                      initialEpisode: episode.toPinepodsEpisode(),
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

  /// One line: this week's listening and completions.
  Widget _buildWeekSummary() {
    final theme = Theme.of(context);
    final stats = _homeData!.weeklyStats;
    return Material(
      color: softFill(theme),
      borderRadius: BorderRadius.circular(14),
      child: InkWell(
        borderRadius: BorderRadius.circular(14),
        onTap: () => Navigator.push(context, MaterialPageRoute<void>(builder: (_) => const PinepodsUserStats())),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
          child: Row(
            children: [
              Icon(PhosphorIconsRegular.chartBar, size: 18, color: theme.colorScheme.primary),
              const SizedBox(width: 10),
              Expanded(
                child: Text(
                  L.of(context)!.home_week_summary(stats.formattedListened, '${stats.episodesCompleted}'),
                  style: theme.textTheme.bodyMedium?.copyWith(fontWeight: FontWeight.w600),
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              Icon(PhosphorIconsRegular.caretRight, size: 16, color: theme.hintColor),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildCollectionsSection() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SoftSectionHeader(
          title: L.of(context)!.collections_title,
          padding: const EdgeInsets.only(bottom: 10),
          trailing: [
            TextButton(onPressed: () => _openCollection(null), child: Text(L.of(context)!.see_all)),
          ],
        ),
        SizedBox(
          height: 96,
          child: ListView.separated(
            scrollDirection: Axis.horizontal,
            itemCount: _collections.length,
            separatorBuilder: (_, _) => const SizedBox(width: 12),
            itemBuilder: (context, index) {
              final c = _collections[index];
              final theme = Theme.of(context);
              return SizedBox(
                width: 180,
                child: Material(
                  color: softFill(theme),
                  borderRadius: BorderRadius.circular(16),
                  child: InkWell(
                    onTap: () => _openCollection(c),
                    borderRadius: BorderRadius.circular(16),
                    child: Padding(
                      padding: const EdgeInsets.all(14),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        mainAxisAlignment: MainAxisAlignment.center,
                        children: [
                          Row(
                            children: [
                              Icon(collectionIcon(c.icon), color: theme.colorScheme.primary, size: 22),
                              const SizedBox(width: 8),
                              Expanded(
                                child: Text(c.name,
                                    maxLines: 1,
                                    overflow: TextOverflow.ellipsis,
                                    style: theme.textTheme.titleSmall?.copyWith(fontWeight: FontWeight.bold)),
                              ),
                            ],
                          ),
                          const SizedBox(height: 6),
                          Text(L.of(context)!.episodes_count(c.episodeCount),
                              style: theme.textTheme.bodySmall?.copyWith(color: theme.hintColor)),
                        ],
                      ),
                    ),
                  ),
                ),
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
        SoftSectionHeader(title: L.of(context)!.smart_playlists, padding: const EdgeInsets.only(bottom: 10)),
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
        SoftSectionHeader(title: L.of(context)!.recent_episodes, padding: const EdgeInsets.only(bottom: 10)),
        ...(_homeData!.recentEpisodes.take(5).map((episode) =>
          Padding(
            padding: const EdgeInsets.only(bottom: 12),
            child: PinepodsEpisodeCard(
              episode: episode.toPinepodsEpisode(),
              margin: EdgeInsets.zero,
              onTap: () {
                Navigator.push(
                  context,
                  MaterialPageRoute(
                    builder: (context) => PinepodsEpisodeDetails(
                      initialEpisode: episode.toPinepodsEpisode(),
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
    return SoftEmptyState(
      icon: PhosphorIconsRegular.microphoneStage,
      title: L.of(context)!.welcome_to_pinepods,
      body: L.of(context)!.home_empty_body,
    );
  }
}

class _StatTile extends StatelessWidget {
  final String title;
  final String value;
  final IconData icon;
  final Color color;

  const _StatTile({
    required this.title,
    required this.value,
    required this.icon,
    required this.color,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Container(
      padding: const EdgeInsets.fromLTRB(14, 12, 14, 12),
      decoration: BoxDecoration(
        color: color.withOpacity(0.12),
        borderRadius: BorderRadius.circular(16),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(icon, color: color, size: 20),
          const SizedBox(height: 8),
          Text(
            value,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: theme.textTheme.titleLarge?.copyWith(fontWeight: FontWeight.bold),
          ),
          Text(title, style: theme.textTheme.bodySmall?.copyWith(color: theme.hintColor)),
        ],
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
            LazyNetworkImage(
              imageUrl: podcast.artworkUrl ?? '',
              width: 140,
              height: 140,
              borderRadius: BorderRadius.circular(14),
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
      child: Material(
        color: softFill(Theme.of(context)),
        borderRadius: BorderRadius.circular(16),
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
                    L.of(context)!.episodes_count(playlist.episodeCount ?? 0),
                    style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: Theme.of(context).hintColor,
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
          SnackBar(
            content: Text(L.of(context)!.not_connected_connect_in_settings),
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
            content: Text(L.of(context)!.error_opening_playlist(e.toString())),
            backgroundColor: Colors.red,
          ),
        );
      }
    }
  }

  IconData _getIconFromName(String iconName) {
    switch (iconName) {
      case 'ph-music-notes':
        return PhosphorIconsRegular.musicNote;
      case 'ph-star':
        return PhosphorIconsFill.star;
      case 'ph-clock':
        return PhosphorIconsRegular.clock;
      case 'ph-heart':
        return PhosphorIconsFill.heart;
      default:
        return PhosphorIconsRegular.playlist;
    }
  }
}
