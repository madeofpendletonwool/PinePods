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
import 'package:pinepods_mobile/services/auto_download/download_cleanup_service.dart';
import 'package:pinepods_mobile/ui/widgets/pinepods_episode_card.dart';
import 'package:provider/provider.dart';
import 'package:intl/intl.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';
import 'package:pinepods_mobile/ui/widgets/lazy_network_image.dart';
import 'package:pinepods_mobile/ui/widgets/soft_ui.dart';

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

      // TEMP-SCREENSHOT-HOOK
      final _tmp = (futures[0] as HomeOverview).inProgressEpisodes;
      if (_tmp.isNotEmpty && const String.fromEnvironment('SHOT_EP').isNotEmpty) {
        WidgetsBinding.instance.addPostFrameCallback((_) => Navigator.push(context, MaterialPageRoute(
            builder: (context) => PinepodsEpisodeDetails(initialEpisode: _tmp[int.parse(const String.fromEnvironment('SHOT_EP'))].toPinepodsEpisode()))));
      }
      // END-TEMP-SCREENSHOT-HOOK
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

    final episode = homeEpisode.toPinepodsEpisode();

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
      final success = await LocalDownloadUtils.localDownloadEpisode(context, homeEpisode.toPinepodsEpisode());

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
      _showSnackBar('Audio service not available', Colors.red);
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
      _showSnackBar('Failed to play episode: $e', Colors.red);
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
                        'Offline mode',
                        style: TextStyle(
                          fontWeight: FontWeight.bold,
                          color: Colors.orange[800],
                          fontSize: 16,
                        ),
                      ),
                      const SizedBox(height: 4),
                      const Text(
                        'Your PinePods server is unreachable. You can still '
                        'browse and play episodes downloaded to this device.',
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
                return const Padding(
                  padding: EdgeInsets.symmetric(vertical: 16.0),
                  child: Text('No downloaded episodes on this device yet.'),
                );
              }
              final preview = downloads.take(5).toList();
              return Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    'On this device (${downloads.length})',
                    style: theme.textTheme.titleMedium,
                  ),
                  const SizedBox(height: 8),
                  ...preview.map(
                    (e) => ListTile(
                      contentPadding: EdgeInsets.zero,
                      leading: const Icon(PhosphorIconsRegular.checkCircle),
                      title: Text(
                        e.title ?? 'Untitled episode',
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
            label: const Text('Go to Downloads'),
          ),
          const SizedBox(height: 8),
          OutlinedButton.icon(
            onPressed: () {
              ConnectivityService.instance.refresh();
              _loadHomeContent();
            },
            icon: const Icon(PhosphorIconsRegular.arrowClockwise),
            label: const Text('Retry connection'),
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
              title: 'Home Unavailable',
              subtitle: 'Failed to load home content',
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
        const SoftSectionHeader(title: 'Your Library', padding: EdgeInsets.only(bottom: 10)),
        Row(
          children: [
            Expanded(
              child: _StatTile(
                title: 'Saved',
                value: '${_homeData!.savedCount}',
                icon: PhosphorIconsFill.bookmarkSimple,
                color: Colors.orange,
              ),
            ),
            const SizedBox(width: 12),
            Expanded(
              child: _StatTile(
                title: 'Downloaded',
                value: '${_homeData!.downloadedCount}',
                icon: PhosphorIconsRegular.downloadSimple,
                color: Colors.green,
              ),
            ),
            const SizedBox(width: 12),
            Expanded(
              child: _StatTile(
                title: 'Queue',
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

  Widget _buildContinueListeningSection() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const SoftSectionHeader(title: 'Continue Listening', padding: EdgeInsets.only(bottom: 10)),
        ...(_homeData!.inProgressEpisodes.take(3).map((episode) =>
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
        const SoftSectionHeader(title: 'Top Podcasts', padding: EdgeInsets.only(bottom: 10)),
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
        const SoftSectionHeader(title: 'Up Next', padding: EdgeInsets.only(bottom: 10)),
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

  Widget _buildWeeklyStatsSection() {
    final stats = _homeData!.weeklyStats;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const SoftSectionHeader(title: 'This Week', padding: EdgeInsets.only(bottom: 10)),
        Row(
          children: [
            Expanded(
              child: _StatTile(
                title: 'Listened',
                value: stats.formattedListened,
                icon: PhosphorIconsRegular.headphones,
                color: Colors.purple,
              ),
            ),
            const SizedBox(width: 12),
            Expanded(
              child: _StatTile(
                title: 'Completed',
                value: stats.episodesCompleted.toString(),
                icon: PhosphorIconsFill.checkCircle,
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
        const SoftSectionHeader(title: 'Smart Playlists', padding: EdgeInsets.only(bottom: 10)),
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
        const SoftSectionHeader(title: 'Recent Episodes', padding: EdgeInsets.only(bottom: 10)),
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
    return const SoftEmptyState(
      icon: PhosphorIconsRegular.microphoneStage,
      title: 'Welcome to PinePods',
      body: 'Search for podcasts to follow and they\'ll show up here.',
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
                    '${playlist.episodeCount} episodes',
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
