// CarPlay service for iOS using flutter_carplay
// Provides browsing interface matching Android Auto structure

import 'dart:async';
import 'dart:io';

import 'package:flutter_carplay/flutter_carplay.dart';
import 'package:logging/logging.dart';
import 'package:pinepods_mobile/entities/downloadable.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/home_data.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/repository/repository.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart' show AudioPlayerService, AudioState;
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/settings/settings_service.dart';

/// Service that provides content for CarPlay browsing
/// Mirrors the functionality of MediaBrowserHelper on Android Auto
class CarPlayService {
  final log = Logger('CarPlayService');
  final Repository repository;
  final SettingsService settingsService;
  final AudioPlayerService audioPlayerService;
  PinepodsService? pinepodsService;

  final FlutterCarplay _flutterCarplay = FlutterCarplay();
  bool _isConnected = false;
  bool _isSettingUp = false;  // Guard against concurrent setup
  Timer? _refreshRetryTimer;  // Retries the initial load until credentials hydrate

  /// True once we have everything needed to fetch content from the server.
  bool get _isReady =>
      pinepodsService != null && settingsService.pinepodsUserId != null;

  // Note: We use FlutterCarplay.showSharedNowPlaying() instead of a custom method channel
  // because flutter_carplay manages its own template stack and interface controller

  // Store tab templates for updating
  CPListTemplate? _currentTab;
  CPListTemplate? _savedTab;
  CPListTemplate? _nowPlayingTab;
  CPTabBarTemplate? _rootTemplate;

  // Playback state tracking
  StreamSubscription? _playbackSubscription;
  AudioState _currentPlaybackState = AudioState.none;

  CarPlayService({
    required this.repository,
    required this.settingsService,
    required this.audioPlayerService,
  }) {
    if (Platform.isIOS) {
      _setupCarPlay();
      _setupPlaybackListener();
    }
  }

  void setPinepodsService(PinepodsService? service) {
    pinepodsService = service;
    log.info('PinepodsService reference set for CarPlay');
    if (_isConnected) {
      _refreshDynamicTabs();
    }
  }

  void _setupCarPlay() {
    log.info('Setting up CarPlay connection listener');

    // Pre-set root template immediately — the plugin stores it statically so that when
    // templateApplicationScene(_:didConnect:) fires natively it applies it right away
    // without waiting for any round-trip through Dart.
    _setInitialRootTemplate();

    _flutterCarplay.addListenerOnConnectionChange((status) async {
      log.info('CarPlay connection status: $status');
      if (status == ConnectionStatusTypes.connected) {
        if (_isConnected) {
          log.info('CarPlay already connected, ignoring duplicate callback');
          return;
        }
        _isConnected = true;
        _setupNowPlayingTemplate();
        // Refresh dynamic tab content now that we have a live connection
        await _refreshDynamicTabs();
        await _refreshNowPlayingTab();
      } else if (status == ConnectionStatusTypes.disconnected) {
        _isConnected = false;
        _isSettingUp = false;
        _refreshRetryTimer?.cancel();
        // Don't clear _rootTemplate — native side keeps it and re-applies on reconnect
      }
    });
  }

  void _setInitialRootTemplate() {
    log.info('Pre-setting CarPlay root template');

    _currentTab = CPListTemplate(
      sections: [],
      title: 'Current',
      systemIcon: 'clock.fill',
      emptyViewTitleVariants: ['Loading…'],
      emptyViewSubtitleVariants: ['Episodes loading'],
    );

    _savedTab = CPListTemplate(
      sections: [],
      title: 'Saved',
      systemIcon: 'bookmark.fill',
      emptyViewTitleVariants: ['Loading…'],
      emptyViewSubtitleVariants: ['Saved episodes loading'],
    );

    _nowPlayingTab = _createNowPlayingTab();

    _rootTemplate = CPTabBarTemplate(
      templates: [
        _nowPlayingTab!,
        _currentTab!,
        _savedTab!,
        _createMoreTab(),
      ],
    );

    FlutterCarplay.setRootTemplate(rootTemplate: _rootTemplate!, animated: false);
    log.info('CarPlay root template pre-set successfully');
  }

  Future<void> _refreshDynamicTabs() async {
    if (_isSettingUp) return;

    // If credentials/services aren't ready yet, leave the tabs showing their
    // "Loading…" empty view (rather than overwriting them with a genuinely-empty
    // section) and retry shortly. This avoids the "no episodes" flash when the
    // car connects before login/settings have hydrated.
    if (!_isReady) {
      log.info('CarPlay not ready to load content yet, will retry');
      _scheduleRefreshRetry();
      return;
    }

    _isSettingUp = true;
    _refreshRetryTimer?.cancel();
    log.info('Refreshing CarPlay dynamic tab content');

    try {
      // Update Current tab
      final currentItems = await _loadCurrentEpisodes();
      if (_currentTab != null) {
        await _flutterCarplay.updateListTemplateSections(
          elementId: _currentTab!.uniqueId,
          sections: currentItems.isEmpty ? [] : [CPListSection(items: currentItems)],
        );
      }

      // Update Saved tab
      final savedItems = await _loadSavedEpisodes();
      if (_savedTab != null) {
        await _flutterCarplay.updateListTemplateSections(
          elementId: _savedTab!.uniqueId,
          sections: savedItems.isEmpty ? [] : [CPListSection(items: savedItems)],
        );
      }
    } catch (e) {
      log.warning('Failed to refresh dynamic CarPlay tabs: $e');
    } finally {
      _isSettingUp = false;
    }
  }

  /// Retry loading the tabs once credentials/services become available. Only
  /// runs while connected; stops itself once ready or on disconnect.
  void _scheduleRefreshRetry() {
    _refreshRetryTimer?.cancel();
    _refreshRetryTimer = Timer.periodic(const Duration(seconds: 2), (timer) {
      if (!_isConnected) {
        timer.cancel();
        return;
      }
      if (_isReady) {
        timer.cancel();
        _refreshDynamicTabs();
      }
    });
  }

  void _setupPlaybackListener() {
    // Listen to playback state changes - only log on actual changes
    _playbackSubscription = audioPlayerService.playingState?.listen((state) {
      if (state != _currentPlaybackState) {
        _currentPlaybackState = state;
        if (_isConnected) {
          log.info('Playback state changed to $state');
          // The native audio player handles updating MPNowPlayingInfoCenter
          // which CarPlay uses to display now playing info. Refresh the Now
          // Playing tab so it reflects the current episode / play state.
          _refreshNowPlayingTab();
        }
      }
    });
  }

  void _setupNowPlayingTemplate() {
    log.info('Setting up Now Playing - using system now playing screen');
    // The now playing info is handled by the native audio player via MPNowPlayingInfoCenter
    // flutter_carplay provides showSharedNowPlaying() to navigate to the system now playing screen
  }

  /// Show the now playing screen in CarPlay
  /// Uses CPGridTemplate for a proper button-centered layout
  Future<void> showNowPlaying() async {
    if (_isConnected && audioPlayerService.nowPlaying != null) {
      log.info('Showing CarPlay now playing screen');
      _showNowPlayingGrid();
    }
  }

  bool get _isPlaying => _currentPlaybackState == AudioState.playing;

  /// Push the native CPNowPlayingTemplate.shared via flutter_carplay
  /// This shows the proper CarPlay Now Playing UI with artwork, progress, and controls
  Future<void> _showNowPlayingGrid() async {
    log.info('Pushing CPNowPlayingTemplate.shared via flutter_carplay');
    try {
      // Debug: Check what's in MPNowPlayingInfoCenter before showing the template
      final nativeService = audioPlayerService as dynamic;
      if (nativeService.getNowPlayingInfo != null) {
        try {
          final nowPlayingInfo = await nativeService.getNowPlayingInfo();
          log.info('MPNowPlayingInfoCenter state: $nowPlayingInfo');
        } catch (e) {
          log.warning('Could not get now playing info: $e');
        }
      }

      // Ensure CarPlay Now Playing template is configured before showing
      try {
        await nativeService.configureCarPlayNowPlaying();
      } catch (e) {
        log.warning('Could not configure CarPlay Now Playing: $e');
      }

      // Use flutter_carplay's built-in method which properly manages the template stack
      final result = await FlutterCarplay.showSharedNowPlaying(animated: true);
      if (result) {
        log.info('Now Playing template shown successfully via flutter_carplay');
      } else {
        log.warning('flutter_carplay showSharedNowPlaying returned false, using fallback');
        _showNowPlayingList();
      }
    } catch (e) {
      log.severe('Error showing Now Playing via flutter_carplay: $e');
      // Fall back to list template if native fails
      _showNowPlayingList();
    }
  }

  /// Fallback list-based Now Playing screen
  void _showNowPlayingList() {
    final episode = audioPlayerService.nowPlaying;
    if (episode == null) {
      log.warning('No episode playing, cannot show Now Playing');
      return;
    }

    final isPlaying = _isPlaying;

    // Format current position and duration
    final positionState = audioPlayerService.playPosition?.value;
    final position = positionState?.position.inMilliseconds ?? 0;
    final duration = episode.duration > 0 ? episode.duration : 1;
    final positionMin = (position ~/ 60000);
    final positionSec = ((position % 60000) ~/ 1000);
    final durationMin = (duration ~/ 60000);
    final durationSec = ((duration % 60000) ~/ 1000);
    final progress = duration > 0 ? (position / duration * 100).round() : 0;

    // Create progress bar visualization
    final progressBarLength = 15;
    final filledLength = (progress / 100 * progressBarLength).round();
    final progressBar = '${'●' * filledLength}${'○' * (progressBarLength - filledLength)}';

    final template = CPListTemplate(
      sections: [
        CPListSection(
          items: [
            // Episode info with artwork and progress
            CPListItem(
              text: episode.title ?? 'Unknown Episode',
              detailText: '${episode.podcast ?? ''}\n$progressBar $progress%\n$positionMin:${positionSec.toString().padLeft(2, '0')} / $durationMin:${durationSec.toString().padLeft(2, '0')}',
              image: episode.imageUrl,
              playbackProgress: duration > 0 ? position / duration : null,
              isPlaying: isPlaying,
              playingIndicatorLocation: CPListItemPlayingIndicatorLocation.trailing,
            ),
            // Play/Pause - no screen refresh, just toggle
            CPListItem(
              text: isPlaying ? '⏸  Pause' : '▶  Play',
              detailText: isPlaying ? 'Tap to pause' : 'Tap to play',
              onPress: (complete, item) async {
                if (_isPlaying) {
                  await audioPlayerService.pause();
                } else {
                  await audioPlayerService.play();
                }
                complete();
              },
            ),
            // Rewind
            CPListItem(
              text: '⏪  Rewind ${settingsService.rewindInterval} seconds',
              onPress: (complete, item) async {
                await audioPlayerService.rewind();
                complete();
              },
            ),
            // Fast forward
            CPListItem(
              text: '⏩  Forward ${settingsService.fastForwardInterval} seconds',
              onPress: (complete, item) async {
                await audioPlayerService.fastForward();
                complete();
              },
            ),
          ],
        ),
      ],
      title: 'Now Playing',
      systemIcon: 'play.circle.fill',
    );

    FlutterCarplay.push(template: template, animated: true);
    log.info('List Now Playing screen shown for: ${episode.title}');
  }

  // MARK: - Tab Creation

  CPListTemplate _createNowPlayingTab() {
    log.info('Creating Now Playing tab');

    return CPListTemplate(
      sections: _buildNowPlayingSections(),
      title: 'Now Playing',
      systemIcon: 'play.circle.fill',
      emptyViewTitleVariants: ['Nothing Playing'],
      emptyViewSubtitleVariants: ['Start playing an episode'],
    );
  }

  /// The Now Playing tab shows the current episode directly (no intermediate
  /// "Open Now Playing" button). Tapping the episode opens the full CarPlay
  /// player. When nothing is playing the tab shows its empty view.
  List<CPListSection> _buildNowPlayingSections() {
    final nowPlaying = audioPlayerService.nowPlaying;
    if (nowPlaying == null) {
      return [];
    }

    return [
      CPListSection(
        items: [
          CPListItem(
            text: nowPlaying.title ?? 'Unknown Episode',
            detailText: nowPlaying.podcast ?? 'Unknown Podcast',
            image: nowPlaying.imageUrl,
            onPress: (complete, item) async {
              log.info('Now Playing tab item pressed - opening player');
              _showNowPlayingGrid();
              complete();
            },
            playingIndicatorLocation: CPListItemPlayingIndicatorLocation.trailing,
            isPlaying: _isPlaying,
          ),
        ],
      ),
    ];
  }

  /// Refresh the Now Playing tab so it reflects the currently-playing episode.
  Future<void> _refreshNowPlayingTab() async {
    if (_nowPlayingTab == null) return;
    try {
      await _flutterCarplay.updateListTemplateSections(
        elementId: _nowPlayingTab!.uniqueId,
        sections: _buildNowPlayingSections(),
      );
    } catch (e) {
      log.warning('Failed to refresh Now Playing tab: $e');
    }
  }

  CPListTemplate _createMoreTab() {
    // Create the "More" submenu items
    // Now Playing and Saved are top-level tabs, so not included here
    final moreItems = [
      CPListItem(
        text: 'Queue',
        detailText: 'Up next',
        onPress: (complete, item) {
          _showQueue();
          complete();
        },
        accessoryType: CPListItemAccessoryType.disclosureIndicator,
      ),
      CPListItem(
        text: 'Downloads',
        detailText: 'Downloaded episodes',
        onPress: (complete, item) {
          _showDownloads();
          complete();
        },
        accessoryType: CPListItemAccessoryType.disclosureIndicator,
      ),
      CPListItem(
        text: 'History',
        detailText: 'Recently played',
        onPress: (complete, item) {
          _showHistory();
          complete();
        },
        accessoryType: CPListItemAccessoryType.disclosureIndicator,
      ),
      CPListItem(
        text: 'Podcasts',
        detailText: 'Your subscriptions',
        onPress: (complete, item) {
          _showPodcasts();
          complete();
        },
        accessoryType: CPListItemAccessoryType.disclosureIndicator,
      ),
      CPListItem(
        text: 'Playlists',
        detailText: 'Your playlists',
        onPress: (complete, item) {
          _showPlaylists();
          complete();
        },
        accessoryType: CPListItemAccessoryType.disclosureIndicator,
      ),
    ];

    return CPListTemplate(
      sections: [CPListSection(items: moreItems)],
      title: 'More',
      systemIcon: 'ellipsis.circle.fill',
    );
  }

  // MARK: - Content Loading

  Future<List<CPListItem>> _loadCurrentEpisodes() async {
    log.info('Loading current episodes for CarPlay');

    try {
      final items = <CPListItem>[];
      final episodes = <Episode>[];

      // Get the currently playing episode - add as special "Now Playing" item
      final nowPlaying = audioPlayerService.nowPlaying;
      if (nowPlaying != null) {
        // Add a "Now Playing" header item that navigates to the now playing screen
        items.add(CPListItem(
          text: '▶ Now Playing',
          detailText: nowPlaying.title ?? 'Unknown Episode',
          image: nowPlaying.imageUrl,
          onPress: (complete, item) async {
            log.info('Now Playing item pressed - navigating to Now Playing screen');
            _showNowPlayingGrid();
            complete();
          },
          playingIndicatorLocation: CPListItemPlayingIndicatorLocation.trailing,
        ));
        episodes.add(nowPlaying);
      }

      // Also get recent/home episodes if available
      if (pinepodsService != null && settingsService.pinepodsUserId != null) {
        try {
          final homeData = await pinepodsService!.getHomeOverview(settingsService.pinepodsUserId!);
          // Add in-progress episodes first (limit to 5 for initial load)
          for (final ep in homeData.inProgressEpisodes.take(5)) {
            if (!episodes.any((e) => e.guid == 'pinepods_${ep.episodeId}')) {
              episodes.add(_convertHomeEpisode(ep));
            }
          }
          // Then recent episodes (limit to 10 total for CarPlay - keep it light)
          final remainingSlots = 10 - episodes.length;
          for (final ep in homeData.recentEpisodes.take(remainingSlots)) {
            if (!episodes.any((e) => e.guid == 'pinepods_${ep.episodeId}')) {
              episodes.add(_convertHomeEpisode(ep));
            }
          }
        } catch (e) {
          log.warning('Failed to get home data: $e');
        }
      }

      // Add episode items (skip the first one if it's the now playing episode)
      final startIndex = nowPlaying != null ? 1 : 0;
      for (var i = startIndex; i < episodes.length; i++) {
        try {
          items.add(_createEpisodeItem(episodes[i]));
        } catch (e) {
          log.warning('Failed to create item for ${episodes[i].title}: $e');
        }
      }
      log.info('Loaded ${items.length} current items');
      return items;
    } catch (e) {
      log.severe('Failed to load current episodes: $e');
      return [];
    }
  }

  Future<List<CPListItem>> _loadSavedEpisodes() async {
    final items = <CPListItem>[];
    if (pinepodsService != null && settingsService.pinepodsUserId != null) {
      try {
        final saved = await pinepodsService!.getSavedEpisodes(settingsService.pinepodsUserId!);
        for (final ep in saved.take(50)) {
          items.add(_createEpisodeItem(_convertPinepodsEpisode(ep)));
        }
        log.info('Loaded ${items.length} saved episodes');
      } catch (e) {
        log.warning('Failed to load saved episodes: $e');
      }
    }
    return items;
  }

  // MARK: - More Submenu Navigation

  void _showQueue() async {
    log.info('Showing queue');

    final template = CPListTemplate(
      sections: [],
      title: 'Queue',
      systemIcon: 'list.bullet',
      emptyViewTitleVariants: ['Queue Empty'],
      emptyViewSubtitleVariants: ['Add episodes to your queue'],
    );

    FlutterCarplay.push(template: template, animated: true);

    try {
      final queue = await repository.loadQueue();
      final items = queue.take(50).map((ep) => _createEpisodeItem(ep)).toList();

      await _flutterCarplay.updateListTemplateSections(
        elementId: template.uniqueId,
        sections: items.isEmpty ? [] : [CPListSection(items: items)],
      );
      log.info('Loaded ${items.length} queue items');
    } catch (e) {
      log.warning('Failed to load queue: $e');
    }
  }

  void _showDownloads() async {
    log.info('Showing downloads');

    final template = CPListTemplate(
      sections: [],
      title: 'Downloads',
      systemIcon: 'arrow.down.circle.fill',
      emptyViewTitleVariants: ['No Downloads'],
      emptyViewSubtitleVariants: ['Downloaded episodes appear here'],
    );

    FlutterCarplay.push(template: template, animated: true);

    try {
      final downloads = await repository.findDownloads();
      final items = downloads.take(50).map((ep) => _createEpisodeItem(ep)).toList();

      await _flutterCarplay.updateListTemplateSections(
        elementId: template.uniqueId,
        sections: items.isEmpty ? [] : [CPListSection(items: items)],
      );
      log.info('Loaded ${items.length} downloads');
    } catch (e) {
      log.warning('Failed to load downloads: $e');
    }
  }

  void _showHistory() async {
    log.info('Showing history');

    final template = CPListTemplate(
      sections: [],
      title: 'History',
      systemIcon: 'clock.fill',
      emptyViewTitleVariants: ['No History'],
      emptyViewSubtitleVariants: ['Your listening history will appear here'],
    );

    FlutterCarplay.push(template: template, animated: true);

    // Load history
    if (pinepodsService != null && settingsService.pinepodsUserId != null) {
      try {
        final history = await pinepodsService!.getUserHistory(settingsService.pinepodsUserId!);
        final items = history.take(50).map((ep) => _createEpisodeItem(_convertPinepodsEpisode(ep))).toList();

        await _flutterCarplay.updateListTemplateSections(
          elementId: template.uniqueId,
          sections: items.isEmpty ? [] : [CPListSection(items: items)],
        );
        log.info('Loaded ${items.length} history items');
      } catch (e) {
        log.warning('Failed to load history: $e');
      }
    }
  }

  void _showPodcasts() async {
    log.info('Showing podcasts');

    final template = CPListTemplate(
      sections: [],
      title: 'Podcasts',
      systemIcon: 'mic.fill',
      emptyViewTitleVariants: ['No Podcasts'],
      emptyViewSubtitleVariants: ['Subscribe to podcasts to see them here'],
    );

    FlutterCarplay.push(template: template, animated: true);

    // Load podcasts
    if (pinepodsService != null && settingsService.pinepodsUserId != null) {
      try {
        final podcasts = await pinepodsService!.getUserPodcasts(settingsService.pinepodsUserId!);
        final items = <CPListItem>[];
        // No limit needed without images
        for (final podcast in podcasts) {
          try {
            // Clean the title - remove any problematic characters
            final cleanTitle = _sanitizeForCarPlay(podcast.title);

            items.add(CPListItem(
              text: cleanTitle.isNotEmpty ? cleanTitle : 'Unknown Podcast',
              detailText: 'Podcast',
              // Images disabled - loading many podcast images crashes CarPlay
              image: null,
              onPress: (complete, item) {
                final podcastId = podcast.id?.toString() ?? '';
                _showPodcastEpisodes(podcastId, podcast.title);
                complete();
              },
              accessoryType: CPListItemAccessoryType.disclosureIndicator,
            ));
          } catch (e) {
            log.warning('Failed to create podcast item for ${podcast.title}: $e');
          }
        }

        log.info('Created ${items.length} podcast items, updating template...');

        // Small delay to let UI settle before updating
        await Future.delayed(const Duration(milliseconds: 100));

        await _flutterCarplay.updateListTemplateSections(
          elementId: template.uniqueId,
          sections: items.isEmpty ? [] : [CPListSection(items: items)],
        );
        log.info('Template updated with ${items.length} podcasts');
      } catch (e, stackTrace) {
        log.severe('Failed to load podcasts: $e');
        log.severe('Stack trace: $stackTrace');
      }
    }
  }

  /// Sanitize text for CarPlay display - remove HTML, newlines, and limit length
  String _sanitizeForCarPlay(String? text) {
    if (text == null || text.isEmpty) return '';

    // Remove HTML tags
    var clean = text.replaceAll(RegExp(r'<[^>]*>'), '');
    // Replace newlines and multiple spaces with single space
    clean = clean.replaceAll(RegExp(r'\s+'), ' ');
    // Trim
    clean = clean.trim();
    // Limit length
    if (clean.length > 100) {
      clean = '${clean.substring(0, 100)}...';
    }
    return clean;
  }

  void _showPodcastEpisodes(String podcastId, String title) async {
    log.info('Showing episodes for podcast: $podcastId');

    final template = CPListTemplate(
      sections: [],
      title: title,
      systemIcon: 'list.bullet',
      emptyViewTitleVariants: ['No Episodes'],
      emptyViewSubtitleVariants: ['No episodes found'],
    );

    FlutterCarplay.push(template: template, animated: true);

    // Load podcast episodes
    if (pinepodsService != null && settingsService.pinepodsUserId != null) {
      try {
        final numericId = podcastId.replaceAll(RegExp(r'[^0-9]'), '');
        final id = int.tryParse(numericId);
        if (id != null) {
          final episodes = await pinepodsService!.getPodcastEpisodes(settingsService.pinepodsUserId!, id);
          final items = episodes.map((ep) => _createEpisodeItem(_convertPinepodsEpisode(ep))).toList();

          await _flutterCarplay.updateListTemplateSections(
            elementId: template.uniqueId,
            sections: items.isEmpty ? [] : [CPListSection(items: items)],
          );
          log.info('Loaded ${items.length} podcast episodes');
        }
      } catch (e) {
        log.warning('Failed to load podcast episodes: $e');
      }
    }
  }

  void _showPlaylists() async {
    log.info('Showing playlists');

    final template = CPListTemplate(
      sections: [],
      title: 'Playlists',
      systemIcon: 'music.note.list',
      emptyViewTitleVariants: ['No Playlists'],
      emptyViewSubtitleVariants: ['Create playlists to see them here'],
    );

    FlutterCarplay.push(template: template, animated: true);

    // Load playlists
    if (pinepodsService != null && settingsService.pinepodsUserId != null) {
      try {
        final playlists = await pinepodsService!.getUserPlaylists(settingsService.pinepodsUserId!);
        final items = playlists.map((playlist) => CPListItem(
          text: playlist.name,
          detailText: '${playlist.episodeCount ?? 0} episodes',
          onPress: (complete, item) {
            _showPlaylistEpisodes(playlist.playlistId.toString(), playlist.name);
            complete();
          },
          accessoryType: CPListItemAccessoryType.disclosureIndicator,
        )).toList();

        await _flutterCarplay.updateListTemplateSections(
          elementId: template.uniqueId,
          sections: items.isEmpty ? [] : [CPListSection(items: items)],
        );
        log.info('Loaded ${items.length} playlists');
      } catch (e) {
        log.warning('Failed to load playlists: $e');
      }
    }
  }

  void _showPlaylistEpisodes(String playlistId, String title) async {
    log.info('Showing episodes for playlist: $playlistId');

    final template = CPListTemplate(
      sections: [],
      title: title,
      systemIcon: 'list.bullet',
      emptyViewTitleVariants: ['No Episodes'],
      emptyViewSubtitleVariants: ['No episodes in this playlist'],
    );

    FlutterCarplay.push(template: template, animated: true);

    // Load playlist episodes
    if (pinepodsService != null && settingsService.pinepodsUserId != null) {
      try {
        final id = int.tryParse(playlistId);
        if (id != null) {
          final response = await pinepodsService!.getPlaylistEpisodes(settingsService.pinepodsUserId!, id);
          final items = response.episodes.map((ep) => _createEpisodeItem(_convertPinepodsEpisode(ep))).toList();

          await _flutterCarplay.updateListTemplateSections(
            elementId: template.uniqueId,
            sections: items.isEmpty ? [] : [CPListSection(items: items)],
          );
          log.info('Loaded ${items.length} playlist episodes');
        }
      } catch (e) {
        log.warning('Failed to load playlist episodes: $e');
      }
    }
  }

  // MARK: - Item Creation

  CPListItem _createEpisodeItem(Episode episode) {
    try {
      // Format duration and progress
      String subtitle = episode.podcast ?? 'Unknown Podcast';
      if (episode.duration > 0) {
        final durationSec = episode.duration ~/ 1000;
        final minutes = durationSec ~/ 60;
        final seconds = durationSec % 60;

        if (episode.position > 0) {
          final positionSec = episode.position ~/ 1000;
          final posMinutes = positionSec ~/ 60;
          final posSeconds = positionSec % 60;
          final percent = ((episode.position / episode.duration) * 100).round();
          subtitle += ' • ${posMinutes}:${posSeconds.toString().padLeft(2, '0')} / ${minutes}:${seconds.toString().padLeft(2, '0')} ($percent%)';
        } else {
          subtitle += ' • ${minutes}:${seconds.toString().padLeft(2, '0')}';
        }
      }

      return CPListItem(
        text: episode.title ?? 'Unknown Episode',
        detailText: subtitle,
        image: episode.imageUrl?.isNotEmpty == true ? episode.imageUrl : null,
        onPress: (complete, item) {
          _playEpisode(episode);
          complete();
        },
        playingIndicatorLocation: CPListItemPlayingIndicatorLocation.trailing,
      );
    } catch (e) {
      log.warning('Failed to create episode item for ${episode.title}: $e');
      return CPListItem(
        text: episode.title ?? 'Unknown Episode',
        detailText: episode.podcast ?? 'Unknown Podcast',
        onPress: (complete, item) {
          _playEpisode(episode);
          complete();
        },
      );
    }
  }

  // MARK: - Playback

  Future<void> _playEpisode(Episode episode) async {
    log.info('Playing episode from CarPlay: ${episode.title}');

    // Local-first: episodes converted from PinepodsEpisode/HomeEpisode carry a
    // 'pinepods_<id>' guid but no downloadState, so they'd stream even when a
    // local copy exists. Resolve the on-device download here before playing.
    if (episode.downloadState != DownloadState.downloaded) {
      final idStr = episode.guid.replaceFirst('pinepods_', '').split('_').first;
      final episodeId = int.tryParse(idStr);
      if (episodeId != null) {
        final local = await audioPlayerService.findDownloadedEpisode(episodeId);
        if (local != null) {
          episode.downloadState = DownloadState.downloaded;
          episode.filepath = local.filepath;
          episode.filename = local.filename;
          log.info('CarPlay using local download for episode $episodeId');
        }
      }
    }

    log.info('Episode URL: ${episode.contentUrl}');

    try {
      await audioPlayerService.playEpisode(episode: episode, resume: true);
      log.info('Started playback from CarPlay: ${episode.title}');
      // Don't auto-navigate to Now Playing - let user use the system Now Playing button
      // which should appear in CarPlay when audio is playing
    } catch (e) {
      log.severe('Failed to play episode from CarPlay: $e');
    }
  }

  // MARK: - Episode Conversion

  Episode _convertPinepodsEpisode(PinepodsEpisode ep) {
    String contentUrl = ep.episodeUrl;
    if (pinepodsService != null && settingsService.pinepodsUserId != null) {
      if (ep.downloaded || ep.isYoutube) {
        contentUrl = pinepodsService!.getStreamUrl(
          ep.episodeId,
          settingsService.pinepodsUserId!,
          isYoutube: ep.isYoutube,
          isLocal: ep.downloaded,
        );
      }
    }

    return Episode(
      guid: 'pinepods_${ep.episodeId}',
      pguid: 'pinepods_${ep.podcastId}',
      podcast: ep.podcastName,
      title: ep.episodeTitle,
      description: ep.episodeDescription,
      link: ep.episodeUrl,
      publicationDate: DateTime.tryParse(ep.episodePubDate) ?? DateTime.now(),
      author: '',
      duration: (ep.episodeDuration * 1000).round(),
      contentUrl: contentUrl,
      position: ((ep.listenDuration ?? 0) * 1000).round(),
      imageUrl: ep.episodeArtwork,
      played: ep.completed,
      chapters: [],
      chaptersUrl: null,
      persons: [],
      transcriptUrls: [],
    );
  }

  Episode _convertHomeEpisode(HomeEpisode ep) {
    String contentUrl = ep.episodeUrl;
    if (pinepodsService != null && settingsService.pinepodsUserId != null) {
      if (ep.downloaded || ep.isYoutube) {
        contentUrl = pinepodsService!.getStreamUrl(
          ep.episodeId,
          settingsService.pinepodsUserId!,
          isYoutube: ep.isYoutube,
          isLocal: ep.downloaded,
        );
        log.info('Episode "${ep.episodeTitle}" using stream URL (downloaded=${ep.downloaded}, youtube=${ep.isYoutube})');
      } else {
        log.info('Episode "${ep.episodeTitle}" using original URL');
      }
    }

    return Episode(
      guid: 'pinepods_${ep.episodeId}',
      pguid: 'pinepods_${ep.podcastId}',
      podcast: ep.podcastName,
      title: ep.episodeTitle,
      description: ep.episodeDescription,
      link: ep.episodeUrl,
      publicationDate: DateTime.tryParse(ep.episodePubDate) ?? DateTime.now(),
      author: '',
      duration: (ep.episodeDuration * 1000).round(),
      contentUrl: contentUrl,
      position: ((ep.listenDuration ?? 0) * 1000).round(),
      imageUrl: ep.episodeArtwork,
      played: ep.completed,
      chapters: [],
      chaptersUrl: null,
      persons: [],
      transcriptUrls: [],
    );
  }

  void dispose() {
    _playbackSubscription?.cancel();
    _refreshRetryTimer?.cancel();
    _flutterCarplay.removeListenerOnConnectionChange();
  }
}
