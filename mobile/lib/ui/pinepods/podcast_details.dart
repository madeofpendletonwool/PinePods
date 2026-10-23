// lib/ui/pinepods/podcast_details.dart

import 'dart:async';
import 'dart:convert';
import 'package:extended_image/extended_image.dart';
import 'package:flutter/material.dart';
import 'package:http/http.dart' as http;
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/entities/pinepods_search.dart';
import 'package:pinepods_mobile/entities/podcast.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/people.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/podcast/podcast_service.dart';
import 'package:pinepods_mobile/services/global_services.dart';
import 'package:pinepods_mobile/ui/widgets/pinepods_episode_card.dart';
import 'package:pinepods_mobile/ui/widgets/platform_progress_indicator.dart';
import 'package:pinepods_mobile/ui/widgets/episode_context_menu.dart';
import 'package:pinepods_mobile/ui/pinepods/episode_details.dart';
import 'package:pinepods_mobile/ui/pinepods/people/add_people_sheet.dart';
import 'package:pinepods_mobile/ui/pinepods/people/people_widgets.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_audio_service.dart';
import 'package:pinepods_mobile/bloc/podcast/audio_bloc.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/ui/podcast/mini_player.dart';
import 'package:pinepods_mobile/ui/utils/player_utils.dart';
import 'package:pinepods_mobile/ui/utils/local_download_utils.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:sliver_tools/sliver_tools.dart';

// Sort direction for episodes within a podcast
enum EpisodeSortDirection {
  newestFirst,
  oldestFirst,
  shortestFirst,
  longestFirst,
  titleAZ,
  titleZA,
}

// 3-state completed filter
enum CompletedFilter {
  showAll,
  showOnly,
  hide,
}

class PinepodsPodcastDetails extends StatefulWidget {
  final UnifiedPinepodsPodcast podcast;
  final bool isFollowing;
  final Function(bool)? onFollowChanged;

  const PinepodsPodcastDetails({
    super.key,
    required this.podcast,
    required this.isFollowing,
    this.onFollowChanged,
  });

  @override
  State<PinepodsPodcastDetails> createState() => _PinepodsPodcastDetailsState();
}

class _PinepodsPodcastDetailsState extends State<PinepodsPodcastDetails> {
  final PinepodsService _pinepodsService = PinepodsService();
  bool _isLoading = false;
  bool _isFollowing = false;
  bool _isFollowButtonLoading = false;
  bool _isFavorite = false;

  /// Header collapse state; the action icons sit on the artwork (white) when
  /// expanded and on the plain app bar (theme colour) when collapsed.
  static const double _headerHeight = 300;
  bool _headerCollapsed = false;

  bool _onScroll(ScrollNotification n) {
    if (n.depth != 0 || n.metrics.axis != Axis.vertical) return false;
    final collapsed = n.metrics.pixels > _headerHeight - kToolbarHeight - 24;
    if (collapsed != _headerCollapsed) setState(() => _headerCollapsed = collapsed);
    return false;
  }
  bool _isFavoriteLoading = false;
  String? _errorMessage;
  List<PinepodsEpisode> _episodes = [];
  List<PinepodsEpisode> _filteredEpisodes = [];
  int? _contextMenuEpisodeIndex;
  // Use global audio service instead of creating local instance
  final TextEditingController _searchController = TextEditingController();
  String _searchQuery = '';
  // People on the show. [_peoplePodcastId] is set once they've loaded from a
  // server that supports adding people.
  PodcastCredits _credits = const PodcastCredits();
  int? _peoplePodcastId;

  // Sort and filter state
  EpisodeSortDirection _sortDirection = EpisodeSortDirection.newestFirst;
  CompletedFilter _completedFilter = CompletedFilter.showAll;
  bool _showInProgress = false;
  bool _isAutoDownloadEnabled = false;
  bool _isAutoPlayNextEnabled = false;

  // AI features (#726/#790) per-podcast settings. Only surfaced when the server
  // reports the AI sidecar as available.
  bool _aiAvailable = false;
  bool _autoTranscribe = false;
  bool _autoAdDetect = false;
  bool _adSkipAutoActivate = true; // server default is true
  // Server-side downloads for this podcast (separate from on-device
  // auto-download): ad times only fit the copy they were found on, so the
  // server copy is what makes ad skipping reliable.
  bool _serverAutoDownload = false;

  // Tracks episode being loaded for ghost mini player
  PinepodsEpisode? _pendingEpisode;

  @override
  void initState() {
    super.initState();
    _isFollowing = widget.isFollowing;
    _initializeCredentials();
    _loadSortPreference();
    _loadAutoDownloadPreference();
    _loadAutoPlayNextPreference();
    _loadAiPreferences();
    _checkFollowStatus();
    _checkFavoriteStatus();
    _searchController.addListener(_onSearchChanged);
  }

  Future<void> _loadSortPreference() async {
    final prefs = await SharedPreferences.getInstance();
    // Use podcast URL as key for per-podcast sort preference
    final key = 'episode_sort_${widget.podcast.url.hashCode}';
    final savedSort = prefs.getString(key);
    if (savedSort != null && mounted) {
      setState(() {
        switch (savedSort) {
          case 'newest':
            _sortDirection = EpisodeSortDirection.newestFirst;
            break;
          case 'oldest':
            _sortDirection = EpisodeSortDirection.oldestFirst;
            break;
          case 'shortest':
            _sortDirection = EpisodeSortDirection.shortestFirst;
            break;
          case 'longest':
            _sortDirection = EpisodeSortDirection.longestFirst;
            break;
          case 'title_az':
            _sortDirection = EpisodeSortDirection.titleAZ;
            break;
          case 'title_za':
            _sortDirection = EpisodeSortDirection.titleZA;
            break;
        }
        _filterEpisodes();
      });
    }
  }

  Future<void> _loadAutoDownloadPreference() async {
    final podcastId = widget.podcast.id;
    if (podcastId <= 0) return;
    final prefs = await SharedPreferences.getInstance();
    if (mounted) {
      setState(() {
        _isAutoDownloadEnabled = prefs.getBool('auto_download_podcast_$podcastId') ?? false;
      });
    }
  }

  Future<void> _toggleAutoDownload() async {
    final podcastId = widget.podcast.id;
    if (podcastId <= 0) return;
    final prefs = await SharedPreferences.getInstance();
    final newValue = !_isAutoDownloadEnabled;
    await prefs.setBool('auto_download_podcast_$podcastId', newValue);
    if (newValue) {
      final lastCheckKey = 'auto_download_last_check_$podcastId';
      if (prefs.getString(lastCheckKey) == null) {
        await prefs.setString(lastCheckKey, DateTime.now().toIso8601String());
      }
    }
    if (mounted) {
      setState(() {
        _isAutoDownloadEnabled = newValue;
      });
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(newValue ? 'Auto-download enabled' : 'Auto-download disabled'),
          duration: const Duration(seconds: 2),
        ),
      );
    }
  }

  Future<void> _loadAutoPlayNextPreference() async {
    final podcastId = widget.podcast.id;
    if (podcastId <= 0) return;

    try {
      final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
      final settings = settingsBloc.currentSettings;
      if (settings.pinepodsServer != null && settings.pinepodsApiKey != null && settings.pinepodsUserId != null) {
        _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
        final status = await _pinepodsService.getAutoPlayNextStatus(podcastId, settings.pinepodsUserId!);
        if (mounted) {
          setState(() {
            _isAutoPlayNextEnabled = status;
          });
        }
      }
    } catch (e) {
      debugPrint('Error loading auto-play-next preference: $e');
    }
  }

  Future<void> _toggleAutoPlayNext() async {
    final podcastId = widget.podcast.id;
    if (podcastId <= 0) return;

    final newValue = !_isAutoPlayNextEnabled;

    try {
      final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
      final settings = settingsBloc.currentSettings;
      if (settings.pinepodsServer != null && settings.pinepodsApiKey != null && settings.pinepodsUserId != null) {
        _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);

        final url = Uri.parse('${settings.pinepodsServer}/api/data/enable_auto_play_next');
        final response = await http.post(
          url,
          headers: {
            'Api-Key': settings.pinepodsApiKey!,
            'Content-Type': 'application/json',
          },
          body: jsonEncode({
            'podcast_id': podcastId,
            'user_id': settings.pinepodsUserId,
            'auto_play_next': newValue,
          }),
        );

        if (response.statusCode == 200 && mounted) {
          setState(() {
            _isAutoPlayNextEnabled = newValue;
          });
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(
              content: Text(newValue ? 'Auto-play next enabled' : 'Auto-play next disabled'),
              duration: const Duration(seconds: 2),
            ),
          );
        }
      }
    } catch (e) {
      debugPrint('Error toggling auto-play-next: $e');
    }
  }

  // Load AI availability + the three per-podcast AI toggles (#726/#790).
  Future<void> _loadAiPreferences() async {
    final podcastId = widget.podcast.id;
    if (podcastId <= 0) return;
    try {
      final settings =
          Provider.of<SettingsBloc>(context, listen: false).currentSettings;
      if (settings.pinepodsServer == null ||
          settings.pinepodsApiKey == null ||
          settings.pinepodsUserId == null) {
        return;
      }
      _pinepodsService.setCredentials(
          settings.pinepodsServer!, settings.pinepodsApiKey!);
      final userId = settings.pinepodsUserId!;

      final status = await _pinepodsService.getAiStatus();
      if (!status.available) {
        if (mounted) setState(() => _aiAvailable = false);
        return;
      }

      final results = await Future.wait([
        _pinepodsService.getAutoTranscribe(userId, podcastId),
        _pinepodsService.getAutoAdDetect(userId, podcastId),
        _pinepodsService.getAdSkipAutoActivate(userId, podcastId),
        _pinepodsService.getServerAutoDownload(userId, podcastId),
      ]);
      if (mounted) {
        setState(() {
          _aiAvailable = true;
          _autoTranscribe = results[0];
          _autoAdDetect = results[1];
          _adSkipAutoActivate = results[2];
          _serverAutoDownload = results[3];
        });
      }
    } catch (e) {
      debugPrint('Error loading AI preferences: $e');
    }
  }

  // Persist one AI toggle; returns whether it succeeded. Updates widget state
  // so the bottom sheet and header reflect the new value after it closes.
  Future<bool> _setAiToggle(String which, bool value) async {
    final podcastId = widget.podcast.id;
    if (podcastId <= 0) return false;
    try {
      final settings =
          Provider.of<SettingsBloc>(context, listen: false).currentSettings;
      if (settings.pinepodsUserId == null) return false;
      final userId = settings.pinepodsUserId!;
      _pinepodsService.setCredentials(
          settings.pinepodsServer!, settings.pinepodsApiKey!);

      bool ok;
      switch (which) {
        case 'transcribe':
          ok = await _pinepodsService.adjustAutoTranscribe(userId, podcastId, value);
          if (ok) {
            _autoTranscribe = value;
            // Auto-detect requires auto-transcribe; keep state consistent.
            if (!value) _autoAdDetect = false;
          }
          break;
        case 'ad_detect':
          ok = await _pinepodsService.adjustAutoAdDetect(userId, podcastId, value);
          if (ok) _autoAdDetect = value;
          break;
        case 'ad_skip_auto':
          ok = await _pinepodsService.adjustAdSkipAutoActivate(userId, podcastId, value);
          if (ok) _adSkipAutoActivate = value;
          break;
        case 'server_download':
          ok = await _pinepodsService.setServerAutoDownload(userId, podcastId, value);
          if (ok) _serverAutoDownload = value;
          break;
        default:
          ok = false;
      }
      if (ok && mounted) setState(() {});
      return ok;
    } catch (e) {
      debugPrint('Error setting AI toggle $which: $e');
      return false;
    }
  }

  void _showAiSettingsSheet() {
    showModalBottomSheet(
      context: context,
      showDragHandle: true,
      builder: (sheetContext) {
        return StatefulBuilder(
          builder: (context, setSheetState) {
            Future<void> toggle(String which, bool value) async {
              final ok = await _setAiToggle(which, value);
              if (ok) setSheetState(() {});
            }

            return SafeArea(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Padding(
                    padding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
                    child: Text(
                      'AI Features',
                      style: Theme.of(context)
                          .textTheme
                          .titleLarge!
                          .copyWith(fontWeight: FontWeight.bold),
                    ),
                  ),
                  SwitchListTile(
                    title: const Text('Auto-transcribe'),
                    subtitle: const Text(
                        'Transcribe new episodes automatically as they arrive.'),
                    value: _autoTranscribe,
                    onChanged: (v) => toggle('transcribe', v),
                  ),
                  SwitchListTile(
                    title: const Text('Auto-detect ads'),
                    subtitle: Text(_autoTranscribe
                        ? 'Scan transcripts for ad segments to skip.'
                        : 'Enable auto-transcribe first.'),
                    value: _autoAdDetect,
                    onChanged:
                        _autoTranscribe ? (v) => toggle('ad_detect', v) : null,
                  ),
                  if (_autoTranscribe && _autoAdDetect)
                    SwitchListTile(
                      title: const Text('Auto-skip detected ads'),
                      subtitle: Text(_adSkipAutoActivate
                          ? 'Skip detected ads automatically.'
                          : 'Require confirmation before skipping.'),
                      value: _adSkipAutoActivate,
                      onChanged: (v) => toggle('ad_skip_auto', v),
                    ),
                  if (_autoAdDetect)
                    _AdDownloadNotice(
                      serverAutoDownload: _serverAutoDownload,
                      onEnable: () => toggle('server_download', true),
                      onServerDownloads: () {
                        Navigator.pop(sheetContext);
                        _showServerDownloadsSheet();
                      },
                    ),
                  const SizedBox(height: 8),
                ],
              ),
            );
          },
        );
      },
    );
  }

  /// The server's download settings for this podcast: auto-download to the
  /// server, delete when finished, and delete after a number of days.
  Future<void> _showServerDownloadsSheet() async {
    final podcastId = widget.podcast.id;
    final settings =
        Provider.of<SettingsBloc>(context, listen: false).currentSettings;
    final userId = settings.pinepodsUserId;
    if (podcastId <= 0 || userId == null) return;
    _pinepodsService.setCredentials(
        settings.pinepodsServer!, settings.pinepodsApiKey!);

    final results = await Future.wait<Object>([
      _pinepodsService.getServerAutoDownload(userId, podcastId),
      _pinepodsService.getAutoDeleteOnComplete(userId),
      _pinepodsService.getAutoDeleteOnComplete(userId, podcastId: podcastId),
      _pinepodsService.getAutoDeleteDays(userId),
      _pinepodsService.getAutoDeleteDays(userId, podcastId: podcastId),
    ]);
    if (!mounted) return;
    var autoDownload = results[0] as bool;
    _serverAutoDownload = autoDownload;
    final deleteDefault = (results[1] as ({bool enabled, bool customized})).enabled;
    final finished = results[2] as ({bool enabled, bool customized});
    // 'default' | 'on' | 'off'
    var whenFinished =
        !finished.customized ? 'default' : (finished.enabled ? 'on' : 'off');
    final daysDefault = (results[3] as ({int days, bool customized})).days;
    final days = results[4] as ({int days, bool customized});
    // null = use the default
    int? afterDays = days.customized ? days.days : null;

    String daysLabel(int d) => d == 0 ? 'Never' : '$d day${d == 1 ? '' : 's'}';
    const dayChoices = [0, 1, 3, 7, 14, 30, 60, 90];

    showModalBottomSheet(
      context: context,
      showDragHandle: true,
      isScrollControlled: true,
      builder: (sheetContext) => StatefulBuilder(
        builder: (context, setSheetState) {
          final theme = Theme.of(context);
          return SafeArea(
            child: SingleChildScrollView(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Padding(
                    padding: const EdgeInsets.fromLTRB(16, 8, 16, 4),
                    child: Text('Server downloads',
                        style: theme.textTheme.titleLarge!
                            .copyWith(fontWeight: FontWeight.bold)),
                  ),
                  Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 16),
                    child: Text(
                      'Downloads kept on your Pinepods server (separate from '
                      'downloads on this device).',
                      style: theme.textTheme.bodySmall,
                    ),
                  ),
                  SwitchListTile(
                    title: const Text('Auto-download to server'),
                    subtitle: const Text(
                        'Download new episodes to the server as they arrive.'),
                    value: autoDownload,
                    onChanged: (v) async {
                      if (await _pinepodsService.setServerAutoDownload(
                          userId, podcastId, v)) {
                        autoDownload = v;
                        _serverAutoDownload = v;
                        setSheetState(() {});
                        if (mounted) setState(() {});
                      }
                    },
                  ),
                  ListTile(
                    title: const Text('Delete when finished'),
                    subtitle: const Text(
                        'Remove the server download about an hour after you '
                        'finish the episode.'),
                    trailing: DropdownButton<String>(
                      value: whenFinished,
                      onChanged: (v) async {
                        if (v == null) return;
                        final enabled =
                            v == 'default' ? null : v == 'on';
                        if (await _pinepodsService
                            .setPodcastAutoDeleteOnComplete(
                                userId, podcastId, enabled)) {
                          setSheetState(() => whenFinished = v);
                        }
                      },
                      items: [
                        DropdownMenuItem(
                            value: 'default',
                            child: Text(
                                'Default (${deleteDefault ? 'on' : 'off'})')),
                        const DropdownMenuItem(value: 'on', child: Text('On')),
                        const DropdownMenuItem(value: 'off', child: Text('Off')),
                      ],
                    ),
                  ),
                  ListTile(
                    title: const Text('Delete after'),
                    subtitle: const Text(
                        'Remove server downloads older than this.'),
                    trailing: DropdownButton<int?>(
                      value: afterDays,
                      onChanged: (v) async {
                        final ok = v == null
                            ? await _pinepodsService.clearPodcastAutoDeleteDays(
                                userId, podcastId)
                            : await _pinepodsService.setPodcastAutoDeleteDays(
                                userId, podcastId, v);
                        if (ok) setSheetState(() => afterDays = v);
                      },
                      items: [
                        DropdownMenuItem<int?>(
                            value: null,
                            child:
                                Text('Default (${daysLabel(daysDefault)})')),
                        for (final d in {...dayChoices, if (afterDays != null) afterDays!})
                          DropdownMenuItem<int?>(
                              value: d, child: Text(daysLabel(d))),
                      ],
                    ),
                  ),
                  const SizedBox(height: 8),
                ],
              ),
            ),
          );
        },
      ),
    );
  }

  Future<void> _saveSortPreference() async {
    final prefs = await SharedPreferences.getInstance();
    final key = 'episode_sort_${widget.podcast.url.hashCode}';
    String value;
    switch (_sortDirection) {
      case EpisodeSortDirection.newestFirst:
        value = 'newest';
        break;
      case EpisodeSortDirection.oldestFirst:
        value = 'oldest';
        break;
      case EpisodeSortDirection.shortestFirst:
        value = 'shortest';
        break;
      case EpisodeSortDirection.longestFirst:
        value = 'longest';
        break;
      case EpisodeSortDirection.titleAZ:
        value = 'title_az';
        break;
      case EpisodeSortDirection.titleZA:
        value = 'title_za';
        break;
    }
    await prefs.setString(key, value);
  }

  @override
  void dispose() {
    _searchController.dispose();
    // Don't dispose global audio service - it should persist across pages
    super.dispose();
  }

  void _onSearchChanged() {
    setState(() {
      _searchQuery = _searchController.text;
      _filterEpisodes();
    });
  }

  void _filterEpisodes() {
    // Start with all episodes
    List<PinepodsEpisode> filtered = List.from(_episodes);

    // Apply search filter (search both title and description)
    if (_searchQuery.isNotEmpty) {
      final query = _searchQuery.toLowerCase();
      filtered = filtered.where((episode) {
        return episode.episodeTitle.toLowerCase().contains(query) ||
            episode.episodeDescription.toLowerCase().contains(query);
      }).toList();
    }

    // Apply completed filter (3-state)
    if (_showInProgress) {
      // In Progress: not completed but has some listen duration
      filtered = filtered.where((episode) {
        return !episode.completed && (episode.listenDuration ?? 0) > 0;
      }).toList();
    } else {
      switch (_completedFilter) {
        case CompletedFilter.showOnly:
          filtered = filtered.where((episode) => episode.completed).toList();
          break;
        case CompletedFilter.hide:
          filtered = filtered.where((episode) => !episode.completed).toList();
          break;
        case CompletedFilter.showAll:
          // No filtering
          break;
      }
    }

    // Apply sorting
    filtered.sort((a, b) {
      switch (_sortDirection) {
        case EpisodeSortDirection.newestFirst:
          return _compareDates(b.episodePubDate, a.episodePubDate);
        case EpisodeSortDirection.oldestFirst:
          return _compareDates(a.episodePubDate, b.episodePubDate);
        case EpisodeSortDirection.shortestFirst:
          return (a.episodeDuration ?? 0).compareTo(b.episodeDuration ?? 0);
        case EpisodeSortDirection.longestFirst:
          return (b.episodeDuration ?? 0).compareTo(a.episodeDuration ?? 0);
        case EpisodeSortDirection.titleAZ:
          return a.episodeTitle.toLowerCase().compareTo(b.episodeTitle.toLowerCase());
        case EpisodeSortDirection.titleZA:
          return b.episodeTitle.toLowerCase().compareTo(a.episodeTitle.toLowerCase());
      }
    });

    _filteredEpisodes = filtered;
  }

  int _compareDates(String dateA, String dateB) {
    try {
      final a = DateTime.parse(dateA);
      final b = DateTime.parse(dateB);
      return a.compareTo(b);
    } catch (e) {
      return 0;
    }
  }

  void _initializeCredentials() {
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    
    if (settings.pinepodsServer != null && settings.pinepodsApiKey != null) {
      _pinepodsService.setCredentials(
        settings.pinepodsServer!,
        settings.pinepodsApiKey!,
      );
      GlobalServices.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
    }
  }

  Future<void> _checkFollowStatus() async {
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      setState(() {
        _isFollowing = false;
      });
      _loadPodcastFeed();
      return;
    }

    try {
      // If we have a valid podcast ID (> 0), assume it's followed since we got it from episode metadata
      if (widget.podcast.id > 0 && widget.isFollowing) {
        print('Using podcast ID ${widget.podcast.id} - assuming followed');
        setState(() {
          _isFollowing = true;
        });
        _loadPodcastFeed();
        return;
      }

      print('Checking follow status for: ${widget.podcast.title}');
      final isFollowing = await _pinepodsService.checkPodcastExists(
        widget.podcast.title,
        widget.podcast.url,
        userId,
      );
      
      print('Follow status result: $isFollowing');
      setState(() {
        _isFollowing = isFollowing;
      });
      
      _loadPodcastFeed();
    } catch (e) {
      print('Error checking follow status: $e');
      // Use the passed value as fallback
      _loadPodcastFeed();
    }
  }

  Future<void> _checkFavoriteStatus() async {
    final podcastId = widget.podcast.id;
    if (podcastId <= 0) return;

    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;
    if (userId == null ||
        settings.pinepodsServer == null ||
        settings.pinepodsApiKey == null) {
      return;
    }

    _pinepodsService.setCredentials(
        settings.pinepodsServer!, settings.pinepodsApiKey!);

    try {
      final isFavorite =
          await _pinepodsService.getPodcastFavoriteStatus(podcastId, userId);
      if (mounted) {
        setState(() {
          _isFavorite = isFavorite;
        });
      }
    } catch (e) {
      print('Error checking favorite status: $e');
    }
  }

  Future<void> _toggleFavorite() async {
    final podcastId = widget.podcast.id;
    if (podcastId <= 0 || _isFavoriteLoading) return;

    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;
    if (userId == null ||
        settings.pinepodsServer == null ||
        settings.pinepodsApiKey == null) {
      _showSnackBar('Not logged in', Colors.red);
      return;
    }

    _pinepodsService.setCredentials(
        settings.pinepodsServer!, settings.pinepodsApiKey!);

    // Optimistically flip, revert on failure.
    final newValue = !_isFavorite;
    setState(() {
      _isFavorite = newValue;
      _isFavoriteLoading = true;
    });

    final success = await _pinepodsService.togglePodcastFavorite(
        podcastId, userId, newValue);

    if (!mounted) return;
    setState(() {
      _isFavoriteLoading = false;
      if (!success) {
        _isFavorite = !newValue;
      }
    });

    if (!success) {
      _showSnackBar('Failed to update favorite', Colors.red);
    } else {
      _showSnackBar(
        newValue ? 'Added to favorites' : 'Removed from favorites',
        Colors.green,
      );
    }
  }

  // Convert Episode objects to PinepodsEpisode objects
  PinepodsEpisode _convertEpisodeToPinepodsEpisode(Episode episode) {
    return PinepodsEpisode(
      podcastName: episode.podcast ?? widget.podcast.title,
      episodeTitle: episode.title ?? '',
      episodePubDate: episode.publicationDate?.toIso8601String() ?? '',
      episodeDescription: episode.description ?? '',
      episodeArtwork: episode.imageUrl ?? widget.podcast.artwork,
      episodeUrl: episode.contentUrl ?? '',
      episodeDuration: episode.duration,
      listenDuration: 0, // RSS episodes don't have listen duration
      episodeId: 0, // RSS episodes don't have server IDs
      completed: false,
      saved: false,
      queued: false,
      downloaded: false,
      isYoutube: false,
    );
  }

  Future<void> _loadPodcastFeed() async {
    setState(() {
      _isLoading = true;
      _errorMessage = null;
    });

    try {
      final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
      final settings = settingsBloc.currentSettings;
      final userId = settings.pinepodsUserId;

      List<PinepodsEpisode> episodes = [];
      
      if (_isFollowing && userId != null) {
        try {
          print('Loading episodes for followed podcast: ${widget.podcast.title}');
          
          int? podcastId;
          
          // If we already have a podcast ID (from episode metadata), use it directly
          if (widget.podcast.id > 0) {
            podcastId = widget.podcast.id;
            print('Using existing podcast ID: $podcastId');
          } else {
            // Get the actual podcast ID using the dedicated endpoint
            podcastId = await _pinepodsService.getPodcastId(
              userId,
              widget.podcast.url,
              widget.podcast.title,
            );
            print('Got podcast ID from lookup: $podcastId');
          }

          if (podcastId != null && podcastId > 0) {
            // People load alongside the episodes rather than after them.
            unawaited(_loadPeople(podcastId));
            // Get episodes from server
            episodes = await _pinepodsService.getPodcastEpisodes(userId, podcastId);
            print('Loaded ${episodes.length} episodes from server for podcastId: $podcastId');
            
            // If server has no episodes, this podcast may need episode sync
            if (episodes.isEmpty) {
              print('Server has no episodes for subscribed podcast. This should not happen.');
              print('Podcast ID: $podcastId, Title: ${widget.podcast.title}');
              
              // For subscribed podcasts, we should NOT fall back to RSS
              // The server should have episodes. This indicates a server-side sync issue.
              // Fall back to RSS ONLY as emergency backup, but episodes won't be clickable
              try {
                final podcastService = Provider.of<PodcastService>(context, listen: false);
                final rssPodcast = Podcast.fromUrl(url: widget.podcast.url);
                
                final loadedPodcast = await podcastService.loadPodcast(podcast: rssPodcast);
                
                if (loadedPodcast != null && loadedPodcast.episodes.isNotEmpty) {
                  episodes = loadedPodcast.episodes.map(_convertEpisodeToPinepodsEpisode).toList();
                  print('Emergency RSS fallback: Loaded ${episodes.length} episodes (NOT CLICKABLE)');
                }
              } catch (e) {
                print('Emergency RSS fallback also failed: $e');
              }
            }
            
          } else {
            print('No podcast ID found - podcast may not be properly added');
          }
        } catch (e) {
          print('Error loading episodes for followed podcast: $e');
          // Fall back to empty episodes list
          episodes = [];
        }
      } else {
        try {
          print('Loading episodes from RSS feed for non-followed podcast: ${widget.podcast.url}');
          
          // Use the existing podcast service to parse RSS feed
          final podcastService = Provider.of<PodcastService>(context, listen: false);
          final rssePodcast = Podcast.fromUrl(url: widget.podcast.url);
          
          final loadedPodcast = await podcastService.loadPodcast(podcast: rssePodcast);
          
          if (loadedPodcast != null && loadedPodcast.episodes.isNotEmpty) {
            // Convert Episode objects to PinepodsEpisode objects
            episodes = loadedPodcast.episodes.map(_convertEpisodeToPinepodsEpisode).toList();
            print('Loaded ${episodes.length} episodes from RSS feed');
          } else {
            print('No episodes found in RSS feed');
          }
        } catch (e) {
          print('Error loading episodes from RSS feed: $e');
          setState(() {
            _errorMessage = 'Failed to load podcast feed';
            _isLoading = false;
          });
          return;
        }
      }

      setState(() {
        _episodes = episodes;
        _filterEpisodes(); // Initialize filtered list
        _isLoading = false;
      });
    } catch (e) {
      print('Error in _loadPodcastFeed: $e');
      setState(() {
        _episodes = [];
        _isLoading = false;
        _errorMessage = 'Failed to load episodes';
      });
    }
  }

  Future<void> _loadPeople(int podcastId) async {
    try {
      final credits = await _pinepodsService.getPodcastPeople(podcastId);
      if (credits != null) {
        if (mounted) {
          setState(() {
            _credits = credits;
            _peoplePodcastId = podcastId;
          });
        }
        return;
      }
      // Servers before the people endpoints: read-only hosts from the 2.0 data.
      final userId = Provider.of<SettingsBloc>(context, listen: false).currentSettings.pinepodsUserId;
      if (userId == null) return;
      final data = await _pinepodsService.fetchPodcasting2PodData(podcastId, userId);
      final hosts = (data?['people'] as List<dynamic>? ?? const [])
          .map((e) => CreditedPerson.fromJson(e as Map<String, dynamic>))
          .where((p) => p.name.isNotEmpty && p.name != 'Unknown Host')
          .toList();
      if (mounted) setState(() => _credits = PodcastCredits(people: hosts));
    } catch (e) {
      debugPrint('Error loading podcast people: $e');
    }
  }

  Future<void> _addPeople() async {
    final podcastId = _peoplePodcastId;
    if (podcastId == null) return;
    final added = await showAddPeopleSheet(context, podcastId: podcastId);
    if (added != null && mounted) await _loadPeople(podcastId);
  }

  Future<void> _toggleFollow() async {
    // Prevent concurrent operations
    if (_isFollowButtonLoading) {
      print('PinePods Follow button: Already processing, ignoring click');
      return;
    }

    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar('Not logged in to PinePods server', Colors.red);
      return;
    }

    // Show confirmation dialog if unfollowing
    if (_isFollowing) {
      final confirmed = await showDialog<bool>(
        context: context,
        barrierDismissible: false, // Prevent dismissing by tapping outside
        builder: (context) => AlertDialog(
          title: const Text('Unfollow Podcast'),
          content: Text(
            'Are you sure you want to unfollow "${widget.podcast.title}"?\n\nThis will remove the podcast and all episode history from your library.',
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(context).pop(false),
              child: const Text('Cancel'),
            ),
            TextButton(
              onPressed: () => Navigator.of(context).pop(true),
              style: TextButton.styleFrom(
                foregroundColor: Colors.red,
              ),
              child: const Text('Unfollow'),
            ),
          ],
        ),
      );

      if (confirmed != true) {
        print('PinePods Follow button: User cancelled unfollow');
        return;
      }
    }

    print('PinePods Follow button: CLICKED - Setting loading to true');
    setState(() {
      _isFollowButtonLoading = true;
    });

    try {
      bool success;
      final oldFollowingState = _isFollowing;

      if (oldFollowingState) {
        print('PinePods: Attempting to remove podcast');
        success = await _pinepodsService.removePodcast(
          widget.podcast.title,
          widget.podcast.url,
          userId,
        );
        print('PinePods: Remove podcast result: $success');
        if (success) {
          setState(() {
            _isFollowing = false;
            _credits = const PodcastCredits();
            _peoplePodcastId = null;
          });
          widget.onFollowChanged?.call(false);
          _showSnackBar('Podcast removed', Colors.orange);
        }
      } else {
        print('PinePods: Attempting to add podcast');
        success = await _pinepodsService.addPodcast(widget.podcast, userId);
        print('PinePods: Add podcast result: $success');
        if (success) {
          setState(() {
            _isFollowing = true;
          });
          widget.onFollowChanged?.call(true);
          _showSnackBar('Podcast added', Colors.green);
        }
      }

      if (success) {
        // Always reload episodes when follow status changes
        // This will switch between server episodes (followed) and RSS episodes (unfollowed)
        print('PinePods: Reloading podcast feed after ${oldFollowingState ? 'unfollow' : 'follow'}');
        await _loadPodcastFeed();
      } else {
        // Revert state change if the operation failed
        setState(() {
          _isFollowing = oldFollowingState;
        });
        _showSnackBar('Failed to ${oldFollowingState ? 'remove' : 'add'} podcast', Colors.red);
      }
    } catch (e) {
      print('PinePods: Error in _toggleFollow: $e');
      _showSnackBar('Error: $e', Colors.red);
    } finally {
      // Always reset loading state
      if (mounted) {
        setState(() {
          _isFollowButtonLoading = false;
        });
      }
      print('PinePods Follow button: Loading state reset to false');
    }
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


  Future<void> _showEpisodeContextMenu(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    final isDownloadedLocally = await LocalDownloadUtils.isEpisodeDownloadedLocally(context, episode);
    
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
          _queueEpisode(episodeIndex);
        },
        onMarkComplete: () {
          Navigator.of(context).pop();
          _markEpisodeComplete(episodeIndex);
        },
        onDismiss: () {
          Navigator.of(context).pop();
          _hideEpisodeContextMenu();
        },
      ),
    );
  }

  void _hideEpisodeContextMenu() {
    setState(() {
      _contextMenuEpisodeIndex = null;
    });
  }

  PinepodsAudioService? get _audioService => GlobalServices.pinepodsAudioService;

  Future<void> _playEpisode(PinepodsEpisode episode) async {
    if (_audioService == null) {
      _showSnackBar('Audio service not available', Colors.red);
      return;
    }

    setState(() => _pendingEpisode = episode);

    try {
      await playPinepodsEpisodeWithOptionalFullScreen(
        context,
        _audioService!,
        episode,
        resume: episode.isStarted,
      );
    } catch (e) {
      _showSnackBar('Failed to play episode: $e', Colors.red);
    } finally {
      if (mounted) setState(() => _pendingEpisode = null);
    }
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

    _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);

    try {
      final success = await _pinepodsService.saveEpisode(
        episode.episodeId,
        userId,
        episode.isYoutube,
      );

      if (success) {
        setState(() {
          _episodes[episodeIndex] = _updateEpisodeProperty(episode, saved: true);
          _filterEpisodes();
        });
        _showSnackBar('Episode saved!', Colors.green);
      } else {
        _showSnackBar('Failed to save episode', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error saving episode: $e', Colors.red);
    }

    _hideEpisodeContextMenu();
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

    _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);

    try {
      final success = await _pinepodsService.removeSavedEpisode(
        episode.episodeId,
        userId,
        episode.isYoutube,
      );

      if (success) {
        setState(() {
          _episodes[episodeIndex] = _updateEpisodeProperty(episode, saved: false);
          _filterEpisodes();
        });
        _showSnackBar('Removed from saved episodes', Colors.orange);
      } else {
        _showSnackBar('Failed to remove saved episode', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error removing saved episode: $e', Colors.red);
    }

    _hideEpisodeContextMenu();
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
          _filterEpisodes();
        });
        _showSnackBar('Episode download started!', Colors.green);
      } else {
        _showSnackBar('Failed to download episode', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error downloading episode: $e', Colors.red);
    }

    _hideEpisodeContextMenu();
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
          _filterEpisodes();
        });
        _showSnackBar('Episode deleted from server', Colors.orange);
      } else {
        _showSnackBar('Failed to delete episode', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error deleting episode: $e', Colors.red);
    }

    _hideEpisodeContextMenu();
  }

  Future<void> _queueEpisode(int episodeIndex) async {
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
            _filterEpisodes();
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
            _filterEpisodes();
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

    _hideEpisodeContextMenu();
  }

  Future<void> _markEpisodeComplete(int episodeIndex) async {
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
      if (episode.completed) {
        final success = await _pinepodsService.markEpisodeUncompleted(
          episode.episodeId,
          userId,
          episode.isYoutube,
        );
        if (success) {
          setState(() {
            _episodes[episodeIndex] = _updateEpisodeProperty(episode, completed: false);
            _filterEpisodes();
          });
          _showSnackBar('Episode marked as incomplete', Colors.green);
        } else {
          _showSnackBar('Failed to mark episode incomplete', Colors.red);
        }
      } else {
        final success = await _pinepodsService.markEpisodeCompleted(
          episode.episodeId,
          userId,
          episode.isYoutube,
        );
        if (success) {
          setState(() {
            _episodes[episodeIndex] = _updateEpisodeProperty(episode, completed: true);
            _filterEpisodes();
          });
          _showSnackBar('Episode marked as complete', Colors.green);
        } else {
          _showSnackBar('Failed to mark episode complete', Colors.red);
        }
      }
    } catch (e) {
      _showSnackBar('Error marking episode complete: $e', Colors.red);
    }

    _hideEpisodeContextMenu();
  }

  Future<void> _localDownloadEpisode(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    
    final success = await LocalDownloadUtils.localDownloadEpisode(context, episode);
    
    if (success) {
      _showSnackBar('Episode download started', Colors.green);
    } else {
      _showSnackBar('Failed to start download', Colors.red);
    }

    _hideEpisodeContextMenu();
  }

  Future<void> _deleteLocalDownload(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    
    final deletedCount = await LocalDownloadUtils.deleteLocalDownload(context, episode);
    
    if (deletedCount > 0) {
      _showSnackBar(
        'Deleted $deletedCount local download${deletedCount > 1 ? 's' : ''}', 
        Colors.orange
      );
    } else {
      _showSnackBar('Local download not found', Colors.red);
    }

    _hideEpisodeContextMenu();
  }

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
      podcastId: episode.podcastId,
    );
  }

  Color _headerIconColor(BuildContext context) {
    if (!_headerCollapsed) return Colors.white;
    final theme = Theme.of(context);
    return theme.appBarTheme.foregroundColor ?? theme.colorScheme.onSurface;
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: SafeArea(child: Column(
        children: [
          Expanded(
            child: NotificationListener<ScrollNotification>(
              onNotification: _onScroll,
              child: CustomScrollView(
              slivers: [
          SliverAppBar(
            expandedHeight: _headerHeight,
            pinned: true,
            iconTheme: IconThemeData(color: _headerIconColor(context)),
            flexibleSpace: FlexibleSpaceBar(
              // Leave room for the leading back button and the two trailing
              // actions so the collapsed title never runs under them.
              titlePadding: const EdgeInsetsDirectional.only(start: 56, end: 104, bottom: 16),
              expandedTitleScale: 1.3,
              title: Text(
                widget.podcast.title,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: TextStyle(
                  color: _headerIconColor(context),
                  shadows: _headerCollapsed
                      ? null
                      : const [
                          Shadow(
                            offset: Offset(0, 1),
                            blurRadius: 3,
                            color: Colors.black54,
                          ),
                        ],
                ),
              ),
              background: Stack(
                fit: StackFit.expand,
                children: [
                  widget.podcast.artwork.isNotEmpty
                      ? ExtendedImage.network(
                          widget.podcast.artwork,
                          fit: BoxFit.cover,
                          cache: true,
                          cacheWidth: 900,
                          loadStateChanged: (state) {
                            if (state.extendedImageLoadState != LoadState.failed) return null;
                            return Container(
                              color: Colors.grey[300],
                              child: const Icon(
                                Icons.music_note,
                                size: 80,
                                color: Colors.grey,
                              ),
                            );
                          },
                        )
                      : Container(
                          color: Colors.grey[300],
                          child: const Icon(
                            Icons.music_note,
                            size: 80,
                            color: Colors.grey,
                          ),
                        ),
                  Container(
                    decoration: BoxDecoration(
                      gradient: LinearGradient(
                        begin: Alignment.topCenter,
                        end: Alignment.bottomCenter,
                        colors: [
                          Colors.transparent,
                          Colors.black.withOpacity(0.7),
                        ],
                      ),
                    ),
                  ),
                ],
              ),
            ),
            actions: [
              if (widget.podcast.id > 0)
                IconButton(
                  onPressed: _isFavoriteLoading ? null : _toggleFavorite,
                  icon: _isFavoriteLoading
                      ? SizedBox(
                          width: 24,
                          height: 24,
                          child: CircularProgressIndicator(
                            strokeWidth: 2.0,
                            valueColor:
                                AlwaysStoppedAnimation<Color>(_headerIconColor(context)),
                          ),
                        )
                      : Icon(
                          _isFavorite ? Icons.star : Icons.star_border,
                          color: _isFavorite ? Colors.amber : _headerIconColor(context),
                        ),
                  tooltip: _isFavorite
                      ? 'Remove from favorites'
                      : 'Add to favorites',
                ),
              PopupMenuButton<String>(
                icon: Icon(Icons.more_vert, color: _headerIconColor(context)),
                tooltip: 'Podcast options',
                onSelected: (value) {
                  switch (value) {
                    case 'autodownload':
                      _toggleAutoDownload();
                      break;
                    case 'autoplay':
                      _toggleAutoPlayNext();
                      break;
                    case 'ai':
                      _showAiSettingsSheet();
                      break;
                    case 'server_downloads':
                      _showServerDownloadsSheet();
                      break;
                    case 'follow':
                      _toggleFollow();
                      break;
                  }
                },
                itemBuilder: (context) => [
                  if (_isFollowing && widget.podcast.id > 0) ...[
                    CheckedPopupMenuItem<String>(
                      value: 'autodownload',
                      checked: _isAutoDownloadEnabled,
                      child: const Text('Auto-download to device'),
                    ),
                    const PopupMenuItem<String>(
                      value: 'server_downloads',
                      child: ListTile(
                        contentPadding: EdgeInsets.zero,
                        leading: Icon(Icons.dns_outlined),
                        title: Text('Server downloads'),
                      ),
                    ),
                    CheckedPopupMenuItem<String>(
                      value: 'autoplay',
                      checked: _isAutoPlayNextEnabled,
                      child: const Text('Auto-play next'),
                    ),
                    if (_aiAvailable)
                      const PopupMenuItem<String>(
                        value: 'ai',
                        child: ListTile(
                          contentPadding: EdgeInsets.zero,
                          leading: Icon(Icons.auto_awesome),
                          title: Text('AI features'),
                        ),
                      ),
                    const PopupMenuDivider(),
                  ],
                  PopupMenuItem<String>(
                    value: 'follow',
                    enabled: !_isFollowButtonLoading,
                    child: ListTile(
                      contentPadding: EdgeInsets.zero,
                      leading: Icon(_isFollowing ? Icons.remove_circle_outline : Icons.add_circle_outline),
                      title: Text(_isFollowing ? 'Unfollow' : 'Follow'),
                    ),
                  ),
                ],
              ),
            ],
          ),
          
          SliverToBoxAdapter(
            child: Padding(
              padding: const EdgeInsets.all(16.0),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  // Podcast info with follow/unfollow button
                  Row(
                    children: [
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            if (widget.podcast.author.isNotEmpty)
                              Text(
                                'By ${widget.podcast.author}',
                                style: TextStyle(
                                  fontSize: 16,
                                  color: Theme.of(context).primaryColor,
                                  fontWeight: FontWeight.w500,
                                ),
                              ),
                          ],
                        ),
                      ),
                      _FollowButton(
                        following: _isFollowing,
                        loading: _isFollowButtonLoading,
                        onPressed: _toggleFollow,
                      ),
                    ],
                  ),
                  
                  const SizedBox(height: 8),
                  
                  _ExpandableDescription(description: widget.podcast.description),
                  
                  const SizedBox(height: 16),
                  
                  // Podcast stats
                  Row(
                    children: [
                      Icon(
                        Icons.mic,
                        size: 16,
                        color: Colors.grey[600],
                      ),
                      const SizedBox(width: 4),
                      Text(
                        '${_episodes.length} episode${_episodes.length != 1 ? 's' : ''}',
                        style: TextStyle(
                          fontSize: 14,
                          color: Colors.grey[600],
                        ),
                      ),
                      const SizedBox(width: 16),
                      if (widget.podcast.explicit)
                        Container(
                          padding: const EdgeInsets.symmetric(
                            horizontal: 6,
                            vertical: 2,
                          ),
                          decoration: BoxDecoration(
                            color: Colors.red,
                            borderRadius: BorderRadius.circular(4),
                          ),
                          child: const Text(
                            'Explicit',
                            style: TextStyle(
                              color: Colors.white,
                              fontSize: 12,
                              fontWeight: FontWeight.bold,
                            ),
                          ),
                        ),
                    ],
                  ),
                  
                  if (_credits.people.isNotEmpty || _peoplePodcastId != null) ...[
                    const SizedBox(height: 16),
                    PeopleStrip(
                      title: 'Hosts',
                      people: _credits.people,
                      onAdd: _peoplePodcastId != null ? _addPeople : null,
                      emptyPrompt: 'Who hosts this show? Add them so everyone can find them.',
                    ),
                  ],
                  if (_credits.guests.isNotEmpty) ...[
                    const SizedBox(height: 16),
                    PeopleStrip(title: 'Guests', people: _credits.guests, showEpisodeCount: true),
                  ],

                  const SizedBox(height: 24),
                  
                  // Episodes section header
                  Row(
                    children: [
                      const Text(
                        'Episodes',
                        style: TextStyle(
                          fontSize: 20,
                          fontWeight: FontWeight.bold,
                        ),
                      ),
                      const Spacer(),
                      if (_isLoading)
                        const SizedBox(
                          width: 20,
                          height: 20,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        ),
                    ],
                  ),
                  
                  const SizedBox(height: 16),
                ],
              ),
            ),
          ),
          
          // Episodes list
          if (_isLoading)
            const SliverToBoxAdapter(
              child: Center(
                child: Padding(
                  padding: EdgeInsets.all(32.0),
                  child: PlatformProgressIndicator(),
                ),
              ),
            )
          else if (_errorMessage != null)
            SliverToBoxAdapter(
              child: Center(
                child: Padding(
                  padding: const EdgeInsets.all(32.0),
                  child: Column(
                    children: [
                      Icon(
                        Icons.error_outline,
                        size: 64,
                        color: Colors.red[300],
                      ),
                      const SizedBox(height: 16),
                      Text(
                        _errorMessage!,
                        textAlign: TextAlign.center,
                        style: Theme.of(context).textTheme.bodyLarge,
                      ),
                      const SizedBox(height: 16),
                      ElevatedButton(
                        onPressed: _loadPodcastFeed,
                        child: const Text('Retry'),
                      ),
                    ],
                  ),
                ),
              ),
            )
          else if (_episodes.isEmpty)
            SliverToBoxAdapter(
              child: Center(
                child: Padding(
                  padding: const EdgeInsets.all(32.0),
                  child: Column(
                    children: [
                      Icon(
                        Icons.info_outline,
                        size: 64,
                        color: Colors.blue[300],
                      ),
                      const SizedBox(height: 16),
                      Text(
                        _isFollowing ? 'No episodes found' : 'Episodes available after following',
                        style: Theme.of(context).textTheme.headlineSmall,
                        textAlign: TextAlign.center,
                      ),
                      const SizedBox(height: 8),
                      Text(
                        _isFollowing
                            ? 'Episodes from your PinePods library will appear here'
                            : 'Follow this podcast to add it to your library and view episodes',
                        style: Theme.of(context).textTheme.bodyMedium,
                        textAlign: TextAlign.center,
                      ),
                      const SizedBox(height: 16),
                      ElevatedButton.icon(
                        onPressed: _isFollowing ? _loadPodcastFeed : _toggleFollow,
                        icon: Icon(_isFollowing ? Icons.refresh : Icons.add),
                        label: Text(_isFollowing ? 'Retry' : 'Follow'),
                      ),
                    ],
                  ),
                ),
              ),
            )
          else
            MultiSliver(
              children: [
                _buildSearchAndFilterBar(),
                _buildEpisodesList(),
              ],
            ),
              ],
            ),
            ),
          ),
          _buildBottomPlayer(),
        ],
      )),
    );
  }

  Widget _buildBottomPlayer() {
    final audioBloc = Provider.of<AudioBloc>(context, listen: false);
    return StreamBuilder<AudioState>(
      stream: audioBloc.playingState,
      initialData: audioBloc.playingState?.valueOrNull ?? AudioState.none,
      builder: (context, snapshot) {
        final audioState = snapshot.data ?? AudioState.none;
        // Error keeps the mini player up so it can offer a retry.
        final isAudioActive = audioState != AudioState.none &&
            audioState != AudioState.stopped;

        if (isAudioActive) {
          return const MiniPlayer();
        } else if (_pendingEpisode != null) {
          return _PendingMiniPlayer(episode: _pendingEpisode!);
        }
        return const SizedBox.shrink();
      },
    );
  }

  Widget _buildSearchAndFilterBar() {
    return SliverToBoxAdapter(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          // Search bar with sort dropdown
          Padding(
            padding: const EdgeInsets.fromLTRB(16.0, 16.0, 16.0, 8.0),
            child: Row(
              children: [
                // Search field
                Expanded(
                  child: TextField(
                    controller: _searchController,
                    decoration: InputDecoration(
                      hintText: 'Search episodes...',
                      prefixIcon: const Icon(Icons.search),
                      suffixIcon: _searchQuery.isNotEmpty
                          ? IconButton(
                              icon: const Icon(Icons.clear),
                              onPressed: () {
                                _searchController.clear();
                              },
                            )
                          : null,
                      border: OutlineInputBorder(
                        borderRadius: BorderRadius.circular(12),
                      ),
                      filled: true,
                      fillColor: Theme.of(context).cardColor,
                      contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
                    ),
                  ),
                ),
                const SizedBox(width: 8),
                // Sort dropdown
                Container(
                  padding: const EdgeInsets.symmetric(horizontal: 12),
                  decoration: BoxDecoration(
                    color: Theme.of(context).cardColor,
                    borderRadius: BorderRadius.circular(12),
                    border: Border.all(color: Theme.of(context).dividerColor),
                  ),
                  child: DropdownButtonHideUnderline(
                    child: DropdownButton<EpisodeSortDirection>(
                      value: _sortDirection,
                      icon: const Icon(Icons.sort),
                      items: const [
                        DropdownMenuItem(
                          value: EpisodeSortDirection.newestFirst,
                          child: Text('Newest'),
                        ),
                        DropdownMenuItem(
                          value: EpisodeSortDirection.oldestFirst,
                          child: Text('Oldest'),
                        ),
                        DropdownMenuItem(
                          value: EpisodeSortDirection.shortestFirst,
                          child: Text('Shortest'),
                        ),
                        DropdownMenuItem(
                          value: EpisodeSortDirection.longestFirst,
                          child: Text('Longest'),
                        ),
                        DropdownMenuItem(
                          value: EpisodeSortDirection.titleAZ,
                          child: Text('Title A-Z'),
                        ),
                        DropdownMenuItem(
                          value: EpisodeSortDirection.titleZA,
                          child: Text('Title Z-A'),
                        ),
                      ],
                      onChanged: (value) {
                        if (value != null) {
                          setState(() {
                            _sortDirection = value;
                            _filterEpisodes();
                          });
                          _saveSortPreference();
                        }
                      },
                    ),
                  ),
                ),
              ],
            ),
          ),
          // Filter chips
          Padding(
            padding: const EdgeInsets.fromLTRB(16.0, 0, 16.0, 8.0),
            child: SingleChildScrollView(
              scrollDirection: Axis.horizontal,
              child: Row(
                children: [
                  // Clear all filters chip
                  _buildFilterChip(
                    label: 'Clear All',
                    icon: Icons.clear_all,
                    isActive: false,
                    onTap: () {
                      setState(() {
                        _completedFilter = CompletedFilter.showAll;
                        _showInProgress = false;
                        _filterEpisodes();
                      });
                    },
                  ),
                  const SizedBox(width: 8),
                  // Completed filter chip (3-state)
                  _buildFilterChip(
                    label: _getCompletedFilterLabel(),
                    icon: _getCompletedFilterIcon(),
                    isActive: _completedFilter != CompletedFilter.showAll,
                    isAlert: _completedFilter == CompletedFilter.hide,
                    onTap: () {
                      setState(() {
                        // Cycle through states: showAll -> showOnly -> hide -> showAll
                        switch (_completedFilter) {
                          case CompletedFilter.showAll:
                            _completedFilter = CompletedFilter.showOnly;
                            _showInProgress = false;
                            break;
                          case CompletedFilter.showOnly:
                            _completedFilter = CompletedFilter.hide;
                            _showInProgress = false;
                            break;
                          case CompletedFilter.hide:
                            _completedFilter = CompletedFilter.showAll;
                            break;
                        }
                        _filterEpisodes();
                      });
                    },
                  ),
                  const SizedBox(width: 8),
                  // In Progress filter chip
                  _buildFilterChip(
                    label: 'In Progress',
                    icon: Icons.play_circle_outline,
                    isActive: _showInProgress,
                    onTap: () {
                      setState(() {
                        _showInProgress = !_showInProgress;
                        if (_showInProgress) {
                          _completedFilter = CompletedFilter.showAll;
                        }
                        _filterEpisodes();
                      });
                    },
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }

  String _getCompletedFilterLabel() {
    switch (_completedFilter) {
      case CompletedFilter.showAll:
        return 'Completed';
      case CompletedFilter.showOnly:
        return 'Completed Only';
      case CompletedFilter.hide:
        return 'Hide Completed';
    }
  }

  IconData _getCompletedFilterIcon() {
    switch (_completedFilter) {
      case CompletedFilter.showAll:
        return Icons.circle_outlined;
      case CompletedFilter.showOnly:
        return Icons.check_circle;
      case CompletedFilter.hide:
        return Icons.cancel;
    }
  }

  Widget _buildFilterChip({
    required String label,
    required IconData icon,
    required bool isActive,
    bool isAlert = false,
    required VoidCallback onTap,
  }) {
    final theme = Theme.of(context);
    final Color backgroundColor;
    final Color foregroundColor;

    if (isAlert) {
      backgroundColor = Colors.orange.withOpacity(0.2);
      foregroundColor = Colors.orange;
    } else if (isActive) {
      backgroundColor = theme.primaryColor.withOpacity(0.2);
      foregroundColor = theme.primaryColor;
    } else {
      backgroundColor = theme.cardColor;
      foregroundColor = theme.textTheme.bodyMedium?.color ?? Colors.grey;
    }

    return Material(
      color: backgroundColor,
      borderRadius: BorderRadius.circular(20),
      child: InkWell(
        borderRadius: BorderRadius.circular(20),
        onTap: onTap,
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(20),
            border: Border.all(
              color: isActive || isAlert ? foregroundColor : theme.dividerColor,
            ),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(icon, size: 18, color: foregroundColor),
              const SizedBox(width: 6),
              Text(
                label,
                style: TextStyle(
                  color: foregroundColor,
                  fontWeight: isActive || isAlert ? FontWeight.w600 : FontWeight.normal,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildEpisodesList() {
    // Check if search returned no results
    if (_filteredEpisodes.isEmpty && _searchQuery.isNotEmpty) {
      return SliverFillRemaining(
        child: Center(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Icon(
                Icons.search_off,
                size: 64,
                color: Theme.of(context).primaryColor,
              ),
              const SizedBox(height: 16),
              Text(
                'No episodes found',
                style: Theme.of(context).textTheme.titleLarge,
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 8),
              Text(
                'No episodes match "$_searchQuery"',
                style: Theme.of(context).textTheme.bodyMedium,
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
          final episode = _filteredEpisodes[index];
          // Find the original index for context menu operations
          final originalIndex = _episodes.indexOf(episode);
          final bool hasValidServerEpisodeId = episode.episodeId > 0;
          
          if (!hasValidServerEpisodeId) {
            print('Episode "${episode.episodeTitle}" has no server ID (RSS fallback) - disabling episode details navigation');
          }
          
          return PinepodsEpisodeCard(
            episode: episode,
            onTap: _isFollowing && hasValidServerEpisodeId ? () {
              Navigator.push(
                context,
                MaterialPageRoute(
                  builder: (context) => PinepodsEpisodeDetails(
                    initialEpisode: episode,
                  ),
                ),
              );
            } : null,
            onLongPress: _isFollowing && hasValidServerEpisodeId ? () {
              _showEpisodeContextMenu(originalIndex);
            } : null,
            onPlayPressed: _isFollowing ? () {
              _playEpisode(episode);
            } : null,
          );
        },
        childCount: _filteredEpisodes.length,
      ),
    );
  }
}

class _FollowButton extends StatelessWidget {
  final bool following;
  final bool loading;
  final VoidCallback onPressed;

  const _FollowButton({required this.following, required this.loading, required this.onPressed});

  @override
  Widget build(BuildContext context) {
    final accent = Theme.of(context).primaryColor;
    final onAccent =
        ThemeData.estimateBrightnessForColor(accent) == Brightness.dark ? Colors.white : Colors.black;
    final Widget icon = loading
        ? SizedBox(
            width: 16,
            height: 16,
            child: CircularProgressIndicator(strokeWidth: 2, color: following ? accent : onAccent),
          )
        : Icon(following ? Icons.check_rounded : Icons.add_rounded, size: 18);

    if (following) {
      return OutlinedButton.icon(
        onPressed: loading ? null : onPressed,
        icon: icon,
        label: const Text('Following'),
        style: OutlinedButton.styleFrom(
          foregroundColor: accent,
          side: BorderSide(color: accent.withOpacity(0.5)),
        ),
      );
    }
    return FilledButton.icon(
      onPressed: loading ? null : onPressed,
      icon: icon,
      label: const Text('Follow'),
      style: FilledButton.styleFrom(backgroundColor: accent, foregroundColor: onAccent),
    );
  }
}

class _ExpandableDescription extends StatefulWidget {
  final String description;
  const _ExpandableDescription({required this.description});

  @override
  State<_ExpandableDescription> createState() => _ExpandableDescriptionState();
}

class _ExpandableDescriptionState extends State<_ExpandableDescription> {
  bool _expanded = false;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          widget.description,
          style: const TextStyle(fontSize: 14),
          maxLines: _expanded ? null : 3,
          overflow: _expanded ? TextOverflow.visible : TextOverflow.ellipsis,
        ),
        GestureDetector(
          onTap: () => setState(() => _expanded = !_expanded),
          child: Padding(
            padding: const EdgeInsets.only(top: 4),
            child: Text(
              _expanded ? 'Show less' : 'Show more',
              style: TextStyle(
                fontSize: 13,
                color: Theme.of(context).colorScheme.primary,
                fontWeight: FontWeight.w500,
              ),
            ),
          ),
        ),
      ],
    );
  }
}

class _PendingMiniPlayer extends StatefulWidget {
  final PinepodsEpisode episode;
  const _PendingMiniPlayer({required this.episode});

  @override
  State<_PendingMiniPlayer> createState() => _PendingMiniPlayerState();
}

class _PendingMiniPlayerState extends State<_PendingMiniPlayer>
    with SingleTickerProviderStateMixin {
  late AnimationController _fadeController;
  late Animation<double> _fadeAnimation;

  @override
  void initState() {
    super.initState();
    _fadeController = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 250),
    );
    _fadeAnimation = CurvedAnimation(
      parent: _fadeController,
      curve: Curves.easeOut,
    );
    _fadeController.forward();
  }

  @override
  void dispose() {
    _fadeController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return FadeTransition(
      opacity: _fadeAnimation,
      child: Container(
        height: 66,
        decoration: BoxDecoration(
          color: theme.colorScheme.surface.withOpacity(0.92),
          border: Border(
            top: Divider.createBorderSide(context,
                width: 1.0, color: theme.dividerColor),
          ),
        ),
        child: Padding(
          padding: const EdgeInsets.only(left: 4.0, right: 4.0),
          child: Row(
            children: [
              SizedBox(
                height: 58,
                width: 58,
                child: Padding(
                  padding: const EdgeInsets.all(8.0),
                  child: ClipRRect(
                    borderRadius: BorderRadius.circular(4),
                    child: widget.episode.episodeArtwork.isNotEmpty
                        ? Image.network(
                            widget.episode.episodeArtwork,
                            fit: BoxFit.cover,
                            errorBuilder: (_, __, ___) => Container(
                              color: theme.colorScheme.surfaceVariant,
                              child: Icon(Icons.music_note,
                                  size: 20,
                                  color: theme.colorScheme.onSurfaceVariant),
                            ),
                          )
                        : Container(
                            color: theme.colorScheme.surfaceVariant,
                            child: Icon(Icons.music_note,
                                size: 20,
                                color: theme.colorScheme.onSurfaceVariant),
                          ),
                  ),
                ),
              ),
              Expanded(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      widget.episode.episodeTitle,
                      overflow: TextOverflow.ellipsis,
                      style: theme.textTheme.bodyMedium,
                    ),
                    Padding(
                      padding: const EdgeInsets.only(top: 4.0),
                      child: Text(
                        widget.episode.podcastName,
                        overflow: TextOverflow.ellipsis,
                        style: theme.textTheme.bodySmall,
                      ),
                    ),
                  ],
                ),
              ),
              SizedBox(
                height: 52,
                width: 52,
                child: Center(
                  child: SizedBox(
                    width: 22,
                    height: 22,
                    child: CircularProgressIndicator(
                      strokeWidth: 2.5,
                      valueColor: AlwaysStoppedAnimation<Color>(
                        theme.iconTheme.color ?? theme.primaryColor,
                      ),
                    ),
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Shown under "Auto-detect ads": whether ads will be found on the copy that
/// gets played. Many podcasts insert different ads each time an episode is
/// fetched, so only a server download keeps detection and playback on one copy.
class _AdDownloadNotice extends StatelessWidget {
  final bool serverAutoDownload;
  final VoidCallback onEnable;
  final VoidCallback onServerDownloads;

  const _AdDownloadNotice({
    required this.serverAutoDownload,
    required this.onEnable,
    required this.onServerDownloads,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final color = serverAutoDownload ? Colors.green : Colors.orange;
    return Container(
      margin: const EdgeInsets.fromLTRB(16, 4, 16, 0),
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: color.withOpacity(0.12),
        borderRadius: BorderRadius.circular(8),
        border: Border(left: BorderSide(color: color, width: 3)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Icon(
                  serverAutoDownload
                      ? Icons.check_circle_outline
                      : Icons.warning_amber,
                  size: 18,
                  color: color),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  serverAutoDownload
                      ? 'Episodes download to the server, so ads are found on '
                          "the copy you'll play and skips line up exactly."
                      : 'Server auto-download is off. Many podcasts insert '
                          'different ads every time an episode is fetched, so '
                          'ads found on one copy can land at the wrong times in '
                          'the copy you stream. Pinepods holds off skipping when '
                          "the stream doesn't match, so some ads won't be "
                          'skipped. Turn on server auto-download for exact skips; '
                          '"Delete when finished" keeps it from filling the disk.',
                  style: theme.textTheme.bodySmall,
                ),
              ),
            ],
          ),
          const SizedBox(height: 8),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: [
              if (!serverAutoDownload)
                OutlinedButton.icon(
                  onPressed: onEnable,
                  icon: const Icon(Icons.cloud_download_outlined, size: 18),
                  label: const Text('Turn on server auto-download'),
                ),
              TextButton(
                onPressed: onServerDownloads,
                child: const Text('Server download settings'),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
