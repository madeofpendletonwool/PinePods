// lib/ui/pinepods/podcast_details.dart

import 'dart:async';
import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:http/http.dart' as http;
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/core/download_cleanup.dart';
import 'package:pinepods_mobile/core/html_text.dart';
import 'package:pinepods_mobile/services/auto_download/download_cleanup_service.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/entities/pinepods_search.dart';
import 'package:pinepods_mobile/entities/podcast.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/people.dart';
import 'package:pinepods_mobile/entities/youtube_channel.dart';
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
import 'package:pinepods_mobile/ui/utils/player_utils.dart';
import 'package:pinepods_mobile/ui/utils/artwork_colors.dart';
import 'package:pinepods_mobile/ui/utils/local_download_utils.dart';
import 'package:pinepods_mobile/ui/widgets/lazy_network_image.dart';
import 'package:pinepods_mobile/ui/widgets/soft_ui.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/ui/utils/l10n_labels.dart';

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
  // The podcast as shown; replaced when its details are read from the feed again.
  late UnifiedPinepodsPodcast _podcast = widget.podcast;
  bool _isRefreshingDetails = false;
  bool _isLoading = false;
  bool _isFollowing = false;
  bool _isFollowButtonLoading = false;
  bool _isFavorite = false;

  /// Drives the app bar title only, so scrolling never rebuilds the page.
  final ValueNotifier<bool> _collapsed = ValueNotifier<bool>(false);
  Color? _artColor;
  Timer? _searchDebounce;
  bool _isFavoriteLoading = false;
  String? _errorMessage;
  List<PinepodsEpisode> _episodes = [];
  List<PinepodsEpisode> _filteredEpisodes = [];
  // Index into [_episodes] for each filtered entry, for the context menu.
  List<int> _filteredIndexes = [];
  final Expando<_EpisodeKeys> _episodeKeys = Expando<_EpisodeKeys>();
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
  StreamSubscription<({int episodeId, bool isYoutube})>? _completedSub;
  // The scheduled refresh and Refresh all check this podcast.
  bool _isAutoRefreshEnabled = true;
  bool _isRefreshingNow = false;

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

  bool get _isYoutube => isYoutubeChannelUrl(_podcast.url);
  bool get _isLocal => _podcast.url.startsWith('local://');

  @override
  void initState() {
    super.initState();
    _isFollowing = widget.isFollowing;
    _initializeCredentials();
    _loadSortPreference();
    _loadAutoDownloadPreference();
    _loadAutoPlayNextPreference();
    _loadAutoRefreshPreference();
    _loadAiPreferences();
    _checkFollowStatus();
    _checkFavoriteStatus();
    _searchController.addListener(_onSearchChanged);
    _completedSub = GlobalServices.pinepodsAudioService?.completedEpisodes.listen(_onEpisodeCompleted);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _resolveArtColor();
    });
  }

  /// Per-podcast preference key: the podcast id when known, as the feed URL can
  /// differ between the screens that open this page; else the URL.
  String _prefKey(String kind) =>
      _podcast.id > 0 ? '${kind}_id_${_podcast.id}' : '${kind}_${_podcast.url.hashCode}';

  Future<void> _loadSortPreference() async {
    final prefs = await SharedPreferences.getInstance();
    final savedSort = prefs.getString(_prefKey('episode_sort')) ??
        prefs.getString('episode_sort_${_podcast.url.hashCode}');
    final savedFilter = prefs.getString(_prefKey('episode_filter'));
    if (savedFilter != null && mounted) {
      final filter = _EpisodeFilter.values.where((f) => f.name == savedFilter).firstOrNull;
      if (filter != null) _applyEpisodeFilter(filter);
    }
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
    final podcastId = _podcast.id;
    if (podcastId <= 0) return;
    final prefs = await SharedPreferences.getInstance();
    if (mounted) {
      setState(() {
        _isAutoDownloadEnabled = prefs.getBool('auto_download_podcast_$podcastId') ?? false;
      });
    }
  }

  Future<void> _toggleAutoDownload() async {
    final podcastId = _podcast.id;
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
          content: Text(newValue ? L.of(context)!.auto_download_enabled : L.of(context)!.auto_download_disabled),
          duration: const Duration(seconds: 2),
        ),
      );
    }
  }

  Future<void> _loadAutoPlayNextPreference() async {
    final podcastId = _podcast.id;
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

  bool _useServer() {
    final settings = Provider.of<SettingsBloc>(context, listen: false).currentSettings;
    if (settings.pinepodsServer == null || settings.pinepodsApiKey == null) return false;
    _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
    return true;
  }

  Future<void> _loadAutoRefreshPreference() async {
    final podcastId = _podcast.id;
    if (podcastId <= 0 || !_useServer()) return;
    try {
      final enabled = await _pinepodsService.getAutoRefresh(podcastId);
      if (mounted) setState(() => _isAutoRefreshEnabled = enabled);
    } catch (e) {
      debugPrint('Error loading auto-refresh preference: $e');
    }
  }

  Future<void> _toggleAutoRefresh() async {
    final podcastId = _podcast.id;
    if (podcastId <= 0 || !_useServer()) return;
    final newValue = !_isAutoRefreshEnabled;
    final messenger = ScaffoldMessenger.of(context);
    try {
      await _pinepodsService.setAutoRefresh(podcastId, newValue);
      if (!mounted) return;
      setState(() => _isAutoRefreshEnabled = newValue);
      messenger.showSnackBar(SnackBar(
        content: Text(newValue ? L.of(context)!.refreshing_automatically : L.of(context)!.automatic_refresh_off),
        duration: const Duration(seconds: 2),
      ));
    } catch (e) {
      messenger.showSnackBar(SnackBar(content: Text(L.of(context)!.couldnt_change_auto_refresh(e.toString()))));
    }
  }

  // Check this podcast (feed, channel or local folder) for new episodes now.
  Future<void> _refreshNow() async {
    final podcastId = _podcast.id;
    if (podcastId <= 0 || _isRefreshingNow || !_useServer()) return;
    setState(() => _isRefreshingNow = true);
    final messenger = ScaffoldMessenger.of(context);
    try {
      final added = await _pinepodsService.refreshPodcast(podcastId);
      await _loadPodcastFeed();
      final message = added > 0
          ? L.current.new_episodes_added(added)
          : (_isYoutube ? L.current.checked_channel_new_videos : L.current.no_new_episodes);
      messenger.showSnackBar(SnackBar(content: Text(message)));
    } catch (e) {
      messenger.showSnackBar(SnackBar(content: Text(L.current.couldnt_check_new_episodes(e.toString()))));
    } finally {
      if (mounted) setState(() => _isRefreshingNow = false);
    }
  }

  // Read the feed again so a rebrand (new name, artwork, hosts) shows now.
  Future<void> _refreshDetails() async {
    final podcastId = _podcast.id;
    if (podcastId <= 0) return;
    final settings = Provider.of<SettingsBloc>(context, listen: false).currentSettings;
    final userId = settings.pinepodsUserId;
    if (settings.pinepodsServer == null || settings.pinepodsApiKey == null || userId == null) return;
    setState(() => _isRefreshingDetails = true);
    final messenger = ScaffoldMessenger.of(context);
    try {
      _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
      await _pinepodsService.refreshPodcastDetails(podcastId);
      final d = await _pinepodsService.getPodcastDetailsById(podcastId, userId);
      if (!mounted) return;
      if (d != null) {
        final p = _podcast;
        setState(() {
          _podcast = UnifiedPinepodsPodcast(
            id: p.id,
            indexId: p.indexId,
            title: d['podcastname'] as String? ?? p.title,
            url: p.url,
            originalUrl: p.originalUrl,
            link: d['websiteurl'] as String? ?? p.link,
            description: d['description'] as String? ?? p.description,
            author: d['author'] as String? ?? p.author,
            ownerName: p.ownerName,
            image: d['artworkurl'] as String? ?? p.image,
            artwork: d['artworkurl'] as String? ?? p.artwork,
            lastUpdateTime: p.lastUpdateTime,
            categories: p.categories,
            explicit: p.explicit,
            episodeCount: d['episodecount'] as int? ?? p.episodeCount,
          );
        });
        _resolveArtColor();
      }
      await _loadPodcastFeed();
      messenger.showSnackBar(SnackBar(content: Text(L.current.podcast_details_refreshed)));
    } catch (e) {
      messenger.showSnackBar(SnackBar(content: Text(L.current.couldnt_refresh_from_feed(e.toString()))));
    } finally {
      if (mounted) setState(() => _isRefreshingDetails = false);
    }
  }

  Future<void> _toggleAutoPlayNext() async {
    final podcastId = _podcast.id;
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

        if (response.statusCode == 200) {
          GlobalServices.pinepodsAudioService?.autoPlayNextChanged(podcastId);
        }
        if (response.statusCode == 200 && mounted) {
          setState(() {
            _isAutoPlayNextEnabled = newValue;
          });
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(
              content: Text(newValue ? L.of(context)!.auto_play_next_enabled : L.of(context)!.auto_play_next_disabled),
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
    final podcastId = _podcast.id;
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
    final podcastId = _podcast.id;
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
                      L.of(context)!.ai_features_title,
                      style: Theme.of(context)
                          .textTheme
                          .titleLarge!
                          .copyWith(fontWeight: FontWeight.bold),
                    ),
                  ),
                  SwitchListTile(
                    title: Text(L.of(context)!.auto_transcribe),
                    subtitle: Text(
                        L.of(context)!.auto_transcribe_description),
                    value: _autoTranscribe,
                    onChanged: (v) => toggle('transcribe', v),
                  ),
                  SwitchListTile(
                    title: Text(L.of(context)!.auto_detect_ads),
                    subtitle: Text(_autoTranscribe
                        ? L.of(context)!.auto_detect_ads_description
                        : L.of(context)!.enable_auto_transcribe_first),
                    value: _autoAdDetect,
                    onChanged:
                        _autoTranscribe ? (v) => toggle('ad_detect', v) : null,
                  ),
                  if (_autoTranscribe && _autoAdDetect)
                    SwitchListTile(
                      title: Text(L.of(context)!.auto_skip_detected_ads),
                      subtitle: Text(_adSkipAutoActivate
                          ? L.of(context)!.auto_skip_ads_on
                          : L.of(context)!.auto_skip_ads_off),
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
    final podcastId = _podcast.id;
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
                    child: Text(L.of(context)!.server_downloads,
                        style: theme.textTheme.titleLarge!
                            .copyWith(fontWeight: FontWeight.bold)),
                  ),
                  Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 16),
                    child: Text(
                      L.of(context)!.server_downloads_description,
                      style: theme.textTheme.bodySmall,
                    ),
                  ),
                  SwitchListTile(
                    title: Text(L.of(context)!.auto_download_to_server),
                    subtitle: Text(
                        L.of(context)!.auto_download_to_server_description),
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
                    title: Text(L.of(context)!.delete_when_finished),
                    subtitle: Text(
                        L.of(context)!.delete_server_when_finished_description),
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
                                L.of(context)!.default_value(deleteDefault ? L.of(context)!.option_on : L.of(context)!.option_off))),
                        DropdownMenuItem(value: 'on', child: Text(L.of(context)!.option_on)),
                        DropdownMenuItem(value: 'off', child: Text(L.of(context)!.option_off)),
                      ],
                    ),
                  ),
                  ListTile(
                    title: Text(L.of(context)!.delete_after),
                    subtitle: Text(
                        L.of(context)!.delete_server_downloads_after_description),
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
                                Text(L.of(context)!.default_value(deleteAfterDaysLabel(context, daysDefault)))),
                        for (final d in {...deleteAfterDayChoices, if (afterDays != null) afterDays!})
                          DropdownMenuItem<int?>(
                              value: d, child: Text(deleteAfterDaysLabel(context, d))),
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

  /// This device's download settings for this podcast: auto-download new
  /// episodes, delete when finished, and delete after a number of days.
  Future<void> _showDeviceDownloadsSheet() async {
    final podcastId = _podcast.id;
    if (podcastId <= 0) return;
    final settings =
        Provider.of<SettingsBloc>(context, listen: false).currentSettings;
    final finishedOverride =
        await DownloadCleanupService.getFinishedOverride(podcastId);
    int? afterDays = await DownloadCleanupService.getDaysOverride(podcastId);
    if (!mounted) return;
    // 'default' | 'on' | 'off'
    var whenFinished = finishedOverride == null
        ? 'default'
        : (finishedOverride ? 'on' : 'off');

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
                    child: Text(L.of(context)!.device_downloads,
                        style: theme.textTheme.titleLarge!
                            .copyWith(fontWeight: FontWeight.bold)),
                  ),
                  Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 16),
                    child: Text(
                      L.of(context)!.device_downloads_description,
                      style: theme.textTheme.bodySmall,
                    ),
                  ),
                  SwitchListTile(
                    title: Text(L.of(context)!.auto_download_to_device),
                    subtitle: Text(
                        L.of(context)!.auto_download_to_device_description),
                    value: _isAutoDownloadEnabled,
                    onChanged: (_) async {
                      await _toggleAutoDownload();
                      setSheetState(() {});
                    },
                  ),
                  ListTile(
                    title: Text(L.of(context)!.delete_when_finished),
                    subtitle: Text(
                        L.of(context)!.delete_device_when_finished_description),
                    trailing: DropdownButton<String>(
                      value: whenFinished,
                      onChanged: (v) async {
                        if (v == null) return;
                        await DownloadCleanupService.setFinishedOverride(
                            podcastId, v == 'default' ? null : v == 'on');
                        setSheetState(() => whenFinished = v);
                      },
                      items: [
                        DropdownMenuItem(
                            value: 'default',
                            child: Text(
                                L.of(context)!.default_value(settings.deleteDownloadedPlayedEpisodes ? L.of(context)!.option_on : L.of(context)!.option_off))),
                        DropdownMenuItem(value: 'on', child: Text(L.of(context)!.option_on)),
                        DropdownMenuItem(value: 'off', child: Text(L.of(context)!.option_off)),
                      ],
                    ),
                  ),
                  ListTile(
                    title: Text(L.of(context)!.delete_after),
                    subtitle: Text(L.of(context)!.delete_device_downloads_after_description),
                    trailing: DropdownButton<int?>(
                      value: afterDays,
                      onChanged: (v) async {
                        await DownloadCleanupService.setDaysOverride(
                            podcastId, v);
                        setSheetState(() => afterDays = v);
                      },
                      items: [
                        DropdownMenuItem<int?>(
                            value: null,
                            child: Text(
                                L.of(context)!.default_value(deleteAfterDaysLabel(context, settings.deleteDownloadsAfterDays)))),
                        for (final d in {...deleteAfterDayChoices, if (afterDays != null) afterDays!})
                          DropdownMenuItem<int?>(
                              value: d, child: Text(deleteAfterDaysLabel(context, d))),
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

  /// Which videos a YouTube channel subscription keeps and adds.
  Future<void> _showYouTubeSettingsSheet() async {
    final podcastId = _podcast.id;
    final userId = Provider.of<SettingsBloc>(context, listen: false).currentSettings.pinepodsUserId;
    if (podcastId <= 0 || userId == null) return;

    int? cutoff;
    YouTubeVideoFilter filter = const YouTubeVideoFilter();
    try {
      final results = await Future.wait([
        _pinepodsService.getFeedCutoffDays(podcastId, userId),
        _pinepodsService.getYouTubeVideoFilter(podcastId, userId),
      ]);
      cutoff = results[0] as int?;
      filter = results[1] as YouTubeVideoFilter;
    } catch (e) {
      _showSnackBar(L.current.couldnt_load_video_settings(e.toString()), Colors.red);
      return;
    }
    if (!mounted) return;

    String text(int? v) => v == null || v == 0 ? '' : '$v';
    final cutoffCtl = TextEditingController(text: text(cutoff));
    final minCtl = TextEditingController(text: text(filter.minMinutes));
    final maxCtl = TextEditingController(text: text(filter.maxMinutes));
    final titleCtl = TextEditingController(text: filter.titleContains ?? '');

    await showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      showDragHandle: true,
      builder: (sheetContext) {
        bool saving = false;
        return StatefulBuilder(builder: (context, setSheetState) {
          final theme = Theme.of(context);
          InputDecoration deco(String label, {String? hint, String? suffix}) => InputDecoration(
                labelText: label,
                hintText: hint,
                suffixText: suffix,
                border: const OutlineInputBorder(),
                isDense: true,
              );

          Future<void> save() async {
            final messenger = ScaffoldMessenger.of(context);
            final navigator = Navigator.of(sheetContext);
            setSheetState(() => saving = true);
            try {
              await _pinepodsService.updateFeedCutoffDays(
                  podcastId, userId, int.tryParse(cutoffCtl.text.trim()) ?? 0);
              await _pinepodsService.updateYouTubeVideoFilter(
                podcastId,
                userId,
                YouTubeVideoFilter(
                  minMinutes: int.tryParse(minCtl.text.trim()),
                  maxMinutes: int.tryParse(maxCtl.text.trim()),
                  titleContains: titleCtl.text.trim().isEmpty ? null : titleCtl.text.trim(),
                ),
              );
              navigator.pop();
              messenger.showSnackBar(SnackBar(content: Text(L.current.video_settings_saved)));
            } catch (e) {
              setSheetState(() => saving = false);
              messenger.showSnackBar(
                  SnackBar(content: Text(L.current.couldnt_save(e.toString().replaceFirst('Exception: ', '')))));
            }
          }

          return Padding(
            padding: EdgeInsets.fromLTRB(16, 0, 16, 16 + MediaQuery.of(context).viewInsets.bottom),
            child: SafeArea(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Text(L.of(context)!.video_settings,
                      style: theme.textTheme.titleLarge!.copyWith(fontWeight: FontWeight.bold)),
                  const SizedBox(height: 4),
                  Text(
                      L.of(context)!.video_settings_description,
                      style: theme.textTheme.bodySmall),
                  const SizedBox(height: 16),
                  TextField(
                    controller: cutoffCtl,
                    keyboardType: TextInputType.number,
                    decoration: deco(L.of(context)!.keep_videos_for, suffix: L.of(context)!.unit_days),
                  ),
                  const SizedBox(height: 12),
                  Row(
                    children: [
                      Expanded(
                        child: TextField(
                          controller: minCtl,
                          keyboardType: TextInputType.number,
                          decoration: deco(L.of(context)!.shortest, suffix: L.of(context)!.unit_min),
                        ),
                      ),
                      const SizedBox(width: 12),
                      Expanded(
                        child: TextField(
                          controller: maxCtl,
                          keyboardType: TextInputType.number,
                          decoration: deco(L.of(context)!.longest, suffix: L.of(context)!.unit_min),
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: 12),
                  TextField(
                    controller: titleCtl,
                    maxLength: 255,
                    decoration: deco(L.of(context)!.title_contains, hint: L.of(context)!.title_contains_hint),
                  ),
                  const SizedBox(height: 4),
                  FilledButton(
                    onPressed: saving ? null : save,
                    child: saving
                        ? const SizedBox(width: 18, height: 18, child: CircularProgressIndicator(strokeWidth: 2))
                        : Text(L.of(context)!.save),
                  ),
                ],
              ),
            ),
          );
        });
      },
    );
    cutoffCtl.dispose();
    minCtl.dispose();
    maxCtl.dispose();
    titleCtl.dispose();
  }

  void _applyEpisodeFilter(_EpisodeFilter f) {
    setState(() {
      _showInProgress = f == _EpisodeFilter.inProgress;
      _completedFilter = switch (f) {
        _EpisodeFilter.unplayed => CompletedFilter.hide,
        _EpisodeFilter.played => CompletedFilter.showOnly,
        _ => CompletedFilter.showAll,
      };
      _filterEpisodes();
    });
  }

  Future<void> _saveFilterPreference(_EpisodeFilter f) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_prefKey('episode_filter'), f.name);
  }

  Future<void> _saveSortPreference() async {
    final prefs = await SharedPreferences.getInstance();
    final key = _prefKey('episode_sort');
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
    _completedSub?.cancel();
    _searchController.dispose();
    _searchDebounce?.cancel();
    _collapsed.dispose();
    // Don't dispose global audio service - it should persist across pages
    super.dispose();
  }

  /// Tick an episode that finished playing while this page is open.
  void _onEpisodeCompleted(({int episodeId, bool isYoutube}) done) {
    final index = _episodes.indexWhere((e) => e.episodeId == done.episodeId && e.isYoutube == done.isYoutube);
    if (index < 0 || !mounted) return;
    setState(() {
      _episodes[index] = _updateEpisodeProperty(_episodes[index], completed: true, queued: false);
      _filterEpisodes();
    });
  }

  void _onSearchChanged() {
    _searchDebounce?.cancel();
    _searchDebounce = Timer(const Duration(milliseconds: 200), () {
      if (!mounted || _searchQuery == _searchController.text) return;
      setState(() {
        _searchQuery = _searchController.text;
        _filterEpisodes();
      });
    });
  }

  _EpisodeKeys _keysFor(PinepodsEpisode e) => _episodeKeys[e] ??= _EpisodeKeys(e);

  void _filterEpisodes() {
    final query = _searchQuery.trim().toLowerCase();
    final indexes = <int>[];
    for (var i = 0; i < _episodes.length; i++) {
      final e = _episodes[i];
      if (query.isNotEmpty) {
        final keys = _keysFor(e);
        if (!keys.title.contains(query) && !keys.description.contains(query)) continue;
      }
      if (_showInProgress) {
        if (e.completed || (e.listenDuration ?? 0) <= 0) continue;
      } else if (_completedFilter == CompletedFilter.showOnly && !e.completed) {
        continue;
      } else if (_completedFilter == CompletedFilter.hide && e.completed) {
        continue;
      }
      indexes.add(i);
    }

    indexes.sort((ia, ib) {
      final a = _episodes[ia];
      final b = _episodes[ib];
      switch (_sortDirection) {
        case EpisodeSortDirection.newestFirst:
          return _keysFor(b).date.compareTo(_keysFor(a).date);
        case EpisodeSortDirection.oldestFirst:
          return _keysFor(a).date.compareTo(_keysFor(b).date);
        case EpisodeSortDirection.shortestFirst:
          return a.episodeDuration.compareTo(b.episodeDuration);
        case EpisodeSortDirection.longestFirst:
          return b.episodeDuration.compareTo(a.episodeDuration);
        case EpisodeSortDirection.titleAZ:
          return _keysFor(a).title.compareTo(_keysFor(b).title);
        case EpisodeSortDirection.titleZA:
          return _keysFor(b).title.compareTo(_keysFor(a).title);
      }
    });

    _filteredIndexes = indexes;
    _filteredEpisodes = [for (final i in indexes) _episodes[i]];
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
      if (_podcast.id > 0 && widget.isFollowing) {
        print('Using podcast ID ${_podcast.id} - assuming followed');
        setState(() {
          _isFollowing = true;
        });
        _loadPodcastFeed();
        return;
      }

      print('Checking follow status for: ${_podcast.title}');
      final isFollowing = await _pinepodsService.checkPodcastExists(
        _podcast.title,
        _podcast.url,
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
    final podcastId = _podcast.id;
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
    final podcastId = _podcast.id;
    if (podcastId <= 0 || _isFavoriteLoading) return;

    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;
    if (userId == null ||
        settings.pinepodsServer == null ||
        settings.pinepodsApiKey == null) {
      _showSnackBar(L.of(context)!.not_logged_in, Colors.red);
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
      _showSnackBar(L.of(context)!.failed_to_update_favorite, Colors.red);
    } else {
      _showSnackBar(
        newValue ? L.of(context)!.added_to_favorites : L.of(context)!.removed_from_favorites,
        Colors.green,
      );
    }
  }

  // Convert Episode objects to PinepodsEpisode objects
  PinepodsEpisode _convertEpisodeToPinepodsEpisode(Episode episode) {
    return PinepodsEpisode(
      podcastName: episode.podcast ?? _podcast.title,
      episodeTitle: episode.title ?? '',
      episodePubDate: episode.publicationDate?.toIso8601String() ?? '',
      episodeDescription: episode.description ?? '',
      episodeArtwork: episode.imageUrl ?? _podcast.artwork,
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
          print('Loading episodes for followed podcast: ${_podcast.title}');
          
          int? podcastId;
          
          // If we already have a podcast ID (from episode metadata), use it directly
          if (_podcast.id > 0) {
            podcastId = _podcast.id;
            print('Using existing podcast ID: $podcastId');
          } else {
            // Get the actual podcast ID using the dedicated endpoint
            podcastId = await _pinepodsService.getPodcastId(
              userId,
              _podcast.url,
              _podcast.title,
            );
            print('Got podcast ID from lookup: $podcastId');
          }

          if (podcastId != null && podcastId > 0) {
            // People load alongside the episodes rather than after them.
            unawaited(_loadPeople(podcastId));
            // Get episodes from server
            episodes = await _pinepodsService.getPodcastEpisodes(userId, podcastId);
            print('Loaded ${episodes.length} episodes from server for podcastId: $podcastId');
            
            // If server has no episodes, this podcast may need episode sync.
            // A new channel subscription is still adding its videos; there's no RSS to fall back to.
            if (episodes.isEmpty && !_isYoutube) {
              print('Server has no episodes for subscribed podcast. This should not happen.');
              print('Podcast ID: $podcastId, Title: ${_podcast.title}');
              
              // For subscribed podcasts, we should NOT fall back to RSS
              // The server should have episodes. This indicates a server-side sync issue.
              // Fall back to RSS ONLY as emergency backup, but episodes won't be clickable
              try {
                final podcastService = Provider.of<PodcastService>(context, listen: false);
                final rssPodcast = Podcast.fromUrl(url: _podcast.url);
                
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
      } else if (!_isYoutube) {
        try {
          print('Loading episodes from RSS feed for non-followed podcast: ${_podcast.url}');
          
          // Use the existing podcast service to parse RSS feed
          final podcastService = Provider.of<PodcastService>(context, listen: false);
          final rssePodcast = Podcast.fromUrl(url: _podcast.url);
          
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
            _errorMessage = L.of(context)!.failed_to_load_podcast_feed;
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
        _errorMessage = L.of(context)!.failed_to_load_episodes;
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
      _showSnackBar(L.of(context)!.not_logged_in_to_server, Colors.red);
      return;
    }

    // Show confirmation dialog if unfollowing
    if (_isFollowing) {
      final confirmed = await showDialog<bool>(
        context: context,
        barrierDismissible: false, // Prevent dismissing by tapping outside
        builder: (context) => AlertDialog(
          title: Text(_isYoutube ? L.of(context)!.unsubscribe : L.of(context)!.unfollow_podcast),
          content: Text(
            _isYoutube
                ? L.of(context)!.unsubscribe_channel_confirm(_podcast.title)
                : L.of(context)!.unfollow_podcast_confirm(_podcast.title),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(context).pop(false),
              child: Text(L.of(context)!.cancel_button_label),
            ),
            TextButton(
              onPressed: () => Navigator.of(context).pop(true),
              style: TextButton.styleFrom(
                foregroundColor: Colors.red,
              ),
              child: Text(_isYoutube ? L.of(context)!.unsubscribe : L.of(context)!.unfollow),
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
        success = _isYoutube
            ? await _pinepodsService.removeYouTubeChannel(_podcast.title, _podcast.url, userId)
            : await _pinepodsService.removePodcast(
                _podcast.title,
                _podcast.url,
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
          _showSnackBar(_isYoutube ? L.current.unsubscribed : L.current.podcast_removed, Colors.orange);
        }
      } else {
        print('PinePods: Attempting to add podcast');
        if (_isYoutube) {
          final channelId = Uri.parse(_podcast.url).pathSegments.last;
          final podcastId = await _pinepodsService.subscribeToYouTubeChannel(channelId, userId);
          _podcast = _podcastWithId(podcastId);
          success = true;
        } else {
          success = await _pinepodsService.addPodcast(_podcast, userId);
        }
        print('PinePods: Add podcast result: $success');
        if (success) {
          setState(() {
            _isFollowing = true;
          });
          widget.onFollowChanged?.call(true);
          _showSnackBar(
              _isYoutube ? L.current.subscribed_videos_adding : L.current.podcast_added, Colors.green);
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
        _showSnackBar(oldFollowingState ? L.current.failed_to_remove_podcast : L.current.failed_to_add_podcast, Colors.red);
      }
    } catch (e) {
      print('PinePods: Error in _toggleFollow: $e');
      _showSnackBar(L.current.error_with_detail(e.toString()), Colors.red);
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

  UnifiedPinepodsPodcast _podcastWithId(int id) {
    final p = _podcast;
    return UnifiedPinepodsPodcast(
      id: id,
      indexId: p.indexId,
      title: p.title,
      url: p.url,
      originalUrl: p.originalUrl,
      link: p.link,
      description: p.description,
      author: p.author,
      ownerName: p.ownerName,
      image: p.image,
      artwork: p.artwork,
      lastUpdateTime: p.lastUpdateTime,
      categories: p.categories,
      explicit: p.explicit,
      episodeCount: p.episodeCount,
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
        onQueueAt: (position) {
          Navigator.of(context).pop();
          _queueEpisode(episodeIndex, position: position);
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
      _showSnackBar(L.of(context)!.audio_service_not_available, Colors.red);
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
      _showSnackBar(L.current.failed_to_play_episode(e.toString()), Colors.red);
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
      _showSnackBar(L.of(context)!.not_logged_in, Colors.red);
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
        _showSnackBar(L.current.episode_saved, Colors.green);
      } else {
        _showSnackBar(L.current.failed_to_save_episode, Colors.red);
      }
    } catch (e) {
      _showSnackBar(L.current.error_saving_episode(e.toString()), Colors.red);
    }

    _hideEpisodeContextMenu();
  }

  Future<void> _removeSavedEpisode(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar(L.of(context)!.not_logged_in, Colors.red);
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
        _showSnackBar(L.current.removed_from_saved_episodes, Colors.orange);
      } else {
        _showSnackBar(L.current.failed_to_remove_saved_episode, Colors.red);
      }
    } catch (e) {
      _showSnackBar(L.current.error_removing_saved_episode(e.toString()), Colors.red);
    }

    _hideEpisodeContextMenu();
  }

  Future<void> _downloadEpisode(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar(L.of(context)!.not_logged_in, Colors.red);
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
        _showSnackBar(L.current.episode_download_started_excl, Colors.green);
      } else {
        _showSnackBar(L.current.failed_to_download_episode, Colors.red);
      }
    } catch (e) {
      _showSnackBar(L.current.error_downloading_episode(e.toString()), Colors.red);
    }

    _hideEpisodeContextMenu();
  }

  Future<void> _deleteEpisode(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar(L.of(context)!.not_logged_in, Colors.red);
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
        _showSnackBar(L.current.episode_deleted_from_server, Colors.orange);
      } else {
        _showSnackBar(L.current.failed_to_delete_episode, Colors.red);
      }
    } catch (e) {
      _showSnackBar(L.current.error_deleting_episode(e.toString()), Colors.red);
    }

    _hideEpisodeContextMenu();
  }

  Future<void> _queueEpisode(int episodeIndex, {String? position}) async {
    final episode = _episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar(L.of(context)!.not_logged_in, Colors.red);
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
          _showSnackBar(L.current.removed_from_queue, Colors.orange);
        }
      } else {
        success = await _pinepodsService.queueEpisode(
          episode.episodeId,
          userId,
          episode.isYoutube,
          position: position,
        );
        if (success) {
          setState(() {
            _episodes[episodeIndex] = _updateEpisodeProperty(episode, queued: true);
            _filterEpisodes();
          });
          _showSnackBar(L.current.added_to_queue_excl, Colors.green);
        }
      }

      if (!success) {
        _showSnackBar(L.current.failed_to_update_queue, Colors.red);
      }
    } catch (e) {
      _showSnackBar(L.current.error_updating_queue(e.toString()), Colors.red);
    }

    _hideEpisodeContextMenu();
  }

  Future<void> _markEpisodeComplete(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      _showSnackBar(L.of(context)!.not_logged_in, Colors.red);
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
          _showSnackBar(L.current.episode_marked_incomplete, Colors.green);
        } else {
          _showSnackBar(L.current.failed_to_mark_incomplete, Colors.red);
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
          _showSnackBar(L.current.episode_marked_complete, Colors.green);
        } else {
          _showSnackBar(L.current.failed_to_mark_complete, Colors.red);
        }
      }
    } catch (e) {
      _showSnackBar(L.current.error_marking_complete(e.toString()), Colors.red);
    }

    _hideEpisodeContextMenu();
  }

  Future<void> _localDownloadEpisode(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    
    final success = await LocalDownloadUtils.localDownloadEpisode(context, episode);
    
    if (success) {
      _showSnackBar(L.current.episode_download_started, Colors.green);
    } else {
      _showSnackBar(L.current.failed_to_start_download, Colors.red);
    }

    _hideEpisodeContextMenu();
  }

  Future<void> _deleteLocalDownload(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    
    final deletedCount = await LocalDownloadUtils.deleteLocalDownload(context, episode);
    
    if (deletedCount > 0) {
      _showSnackBar(
        L.current.deleted_local_downloads(deletedCount), 
        Colors.orange
      );
    } else {
      _showSnackBar(L.current.local_download_not_found, Colors.red);
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

  /// Artwork-derived accent when that setting is on, else the theme's.
  Color _accentColor(ThemeData theme) {
    final enabled = Provider.of<SettingsBloc>(context, listen: false).currentSettings.artworkAccentColors;
    return (enabled ? _artColor : null) ?? theme.primaryColor;
  }

  void _resolveArtColor() {
    final settings = Provider.of<SettingsBloc>(context, listen: false).currentSettings;
    if (!settings.artworkAccentColors) return;
    final brightness = ThemeData.estimateBrightnessForColor(Theme.of(context).scaffoldBackgroundColor);
    final url = _podcast.artwork;
    _artColor = ArtworkColors.cached(url, null, brightness);
    if (_artColor != null) return;
    ArtworkColors.resolve(url, null, brightness).then((color) {
      if (!mounted || color == null) return;
      setState(() => _artColor = color);
    });
  }

  /// Newest episode that hasn't been finished, for the header's play button.
  PinepodsEpisode? get _latestUnplayed {
    PinepodsEpisode? best;
    var bestDate = -1;
    for (final e in _episodes) {
      if (e.completed || e.episodeId <= 0) continue;
      final date = _keysFor(e).date;
      if (date > bestDate) {
        best = e;
        bestDate = date;
      }
    }
    return best;
  }

  bool _onScroll(ScrollNotification n) {
    if (n.depth != 0 || n.metrics.axis != Axis.vertical) return false;
    _collapsed.value = n.metrics.pixels > 140;
    return false;
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    // Eases the header between colours when the artwork colour arrives.
    return TweenAnimationBuilder<Color?>(
      tween: ColorTween(end: _accentColor(theme)),
      duration: const Duration(milliseconds: 350),
      builder: (context, accent, _) => _buildPage(theme, accent!),
    );
  }

  Widget _buildPage(ThemeData theme, Color accent) {
    final bg = theme.scaffoldBackgroundColor;
    final headerTint = Color.alphaBlend(accent.withOpacity(0.18), bg);

    return Scaffold(
      body: Column(
        children: [
          Expanded(
            child: NotificationListener<ScrollNotification>(
              onNotification: _onScroll,
              child: CustomScrollView(
                slivers: [
                  SliverAppBar(
                    pinned: true,
                    backgroundColor: headerTint,
                    surfaceTintColor: Colors.transparent,
                    elevation: 0,
                    scrolledUnderElevation: 0,
                    title: ValueListenableBuilder<bool>(
                      valueListenable: _collapsed,
                      builder: (context, collapsed, child) => AnimatedOpacity(
                        opacity: collapsed ? 1 : 0,
                        duration: const Duration(milliseconds: 180),
                        child: child,
                      ),
                      child: Text(_podcast.title, maxLines: 1, overflow: TextOverflow.ellipsis),
                    ),
                  ),
                  SliverToBoxAdapter(child: _buildHeader(theme, accent, headerTint, bg)),
                  ..._buildEpisodeSlivers(theme, accent),
                ],
              ),
            ),
          ),
          _buildBottomPlayer(),
        ],
      ),
    );
  }

  Widget _buildHeader(ThemeData theme, Color accent, Color headerTint, Color bg) {
    final muted = theme.hintColor;
    final count = _episodes.isNotEmpty ? _episodes.length : _podcast.episodeCount;
    final meta = [
      if (count > 0) _isYoutube ? L.of(context)!.videos_count(count) : L.of(context)!.episodes_count(count),
      if (_podcast.explicit) L.of(context)!.explicit,
    ].join('  ·  ');

    return DecoratedBox(
      decoration: BoxDecoration(
        gradient: LinearGradient(
          begin: Alignment.topCenter,
          end: Alignment.bottomCenter,
          colors: [headerTint, bg],
        ),
      ),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(16, 4, 16, 8),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                DecoratedBox(
                  decoration: BoxDecoration(
                    borderRadius: BorderRadius.circular(14),
                    boxShadow: [
                      BoxShadow(color: Colors.black.withOpacity(0.25), blurRadius: 18, offset: const Offset(0, 6)),
                    ],
                  ),
                  child: LazyNetworkImage(
                    imageUrl: _podcast.artwork,
                    width: 116,
                    height: 116,
                    borderRadius: BorderRadius.circular(14),
                  ),
                ),
                const SizedBox(width: 14),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        _podcast.title,
                        maxLines: 3,
                        overflow: TextOverflow.ellipsis,
                        style: theme.textTheme.titleLarge!.copyWith(fontWeight: FontWeight.bold, height: 1.2),
                      ),
                      if (_podcast.author.isNotEmpty) ...[
                        const SizedBox(height: 4),
                        Text(
                          _podcast.author,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: theme.textTheme.titleSmall!.copyWith(color: accent, fontWeight: FontWeight.w600),
                        ),
                      ],
                      if (meta.isNotEmpty) ...[
                        const SizedBox(height: 4),
                        Text(meta, style: theme.textTheme.bodySmall!.copyWith(color: muted, fontSize: 12.5)),
                      ],
                    ],
                  ),
                ),
              ],
            ),
            const SizedBox(height: 16),
            _buildActionRow(theme, accent),
            if (_podcast.description.isNotEmpty) ...[
              const SizedBox(height: 14),
              _ExpandableDescription(description: _podcast.description, accent: accent),
            ],
            if (_credits.people.isNotEmpty || _peoplePodcastId != null) ...[
              const SizedBox(height: 16),
              PeopleStrip(
                title: L.of(context)!.hosts,
                people: _credits.people,
                onAdd: _peoplePodcastId != null ? _addPeople : null,
                emptyPrompt: L.of(context)!.hosts_empty_prompt,
              ),
            ],
            if (_credits.guests.isNotEmpty) ...[
              const SizedBox(height: 16),
              PeopleStrip(title: L.of(context)!.guests, people: _credits.guests, showEpisodeCount: true),
            ],
          ],
        ),
      ),
    );
  }

  Widget _buildActionRow(ThemeData theme, Color accent) {
    final onAccent = onColor(accent);
    final pillStyle = FilledButton.styleFrom(
      backgroundColor: accent,
      foregroundColor: onAccent,
      disabledBackgroundColor: accent.withOpacity(0.5),
      disabledForegroundColor: onAccent.withOpacity(0.7),
      shape: const StadiumBorder(),
      minimumSize: const Size(0, 46),
      textStyle: const TextStyle(fontSize: 15, fontWeight: FontWeight.w600),
    );

    final Widget primary;
    if (!_isFollowing) {
      primary = FilledButton.icon(
        style: pillStyle,
        onPressed: _isFollowButtonLoading ? null : _toggleFollow,
        icon: _isFollowButtonLoading
            ? SizedBox(width: 18, height: 18, child: CircularProgressIndicator(strokeWidth: 2.2, color: onAccent))
            : const Icon(PhosphorIconsBold.plus, size: 18),
        label: Text(_isYoutube ? L.of(context)!.subscribe : L.of(context)!.follow),
      );
    } else {
      final latest = _latestUnplayed;
      final starting = latest != null && _pendingEpisode == latest;
      primary = FilledButton.icon(
        style: pillStyle,
        onPressed: latest == null || starting ? null : () => _playEpisode(latest),
        icon: starting
            ? SizedBox(width: 18, height: 18, child: CircularProgressIndicator(strokeWidth: 2.2, color: onAccent))
            : const Icon(PhosphorIconsFill.play, size: 18),
        label: Text(latest != null && latest.isStarted ? L.of(context)!.resume_latest : L.of(context)!.play_latest),
      );
    }

    return Row(
      children: [
        Expanded(child: primary),
        if (_isFollowing) ...[
          const SizedBox(width: 10),
          CircleAction(
            tooltip: _isYoutube ? L.of(context)!.unsubscribe : L.of(context)!.unfollow,
            icon: _isFollowButtonLoading
                ? SizedBox(width: 18, height: 18, child: CircularProgressIndicator(strokeWidth: 2.2, color: accent))
                : Icon(PhosphorIconsBold.check, color: accent, size: 20),
            onTap: _isFollowButtonLoading ? null : _toggleFollow,
          ),
        ],
        if (_podcast.id > 0) ...[
          const SizedBox(width: 10),
          CircleAction(
            tooltip: _isFavorite ? L.of(context)!.remove_from_favorites : L.of(context)!.add_to_favorites,
            icon: Icon(
              _isFavorite ? PhosphorIconsFill.star : PhosphorIconsRegular.star,
              color: _isFavorite ? Colors.amber : softNeutral(theme).withOpacity(0.8),
              size: 20,
            ),
            onTap: _isFavoriteLoading ? null : _toggleFavorite,
          ),
        ],
        const SizedBox(width: 10),
        _buildOptionsMenu(theme),
      ],
    );
  }

  Widget _buildOptionsMenu(ThemeData theme) {
    return PopupMenuButton<String>(
      tooltip: L.of(context)!.podcast_options,
      onSelected: (value) {
        switch (value) {
          case 'device_downloads':
            _showDeviceDownloadsSheet();
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
          case 'refresh_now':
            _refreshNow();
            break;
          case 'auto_refresh':
            _toggleAutoRefresh();
            break;
          case 'refresh_details':
            _refreshDetails();
            break;
          case 'youtube':
            _showYouTubeSettingsSheet();
            break;
        }
      },
      itemBuilder: (context) => [
        if (_isFollowing && _podcast.id > 0) ...[
          PopupMenuItem<String>(
            value: 'device_downloads',
            child: ListTile(
              contentPadding: EdgeInsets.zero,
              leading: const Icon(PhosphorIconsRegular.deviceMobile),
              title: Text(L.of(context)!.device_downloads),
            ),
          ),
          PopupMenuItem<String>(
            value: 'server_downloads',
            child: ListTile(
              contentPadding: EdgeInsets.zero,
              leading: const Icon(PhosphorIconsRegular.hardDrives),
              title: Text(L.of(context)!.server_downloads),
            ),
          ),
          CheckedPopupMenuItem<String>(
            value: 'autoplay',
            checked: _isAutoPlayNextEnabled,
            child: Text(L.of(context)!.auto_play_next),
          ),
          if (_isYoutube)
            PopupMenuItem<String>(
              value: 'youtube',
              child: ListTile(
                contentPadding: EdgeInsets.zero,
                leading: const Icon(PhosphorIconsRegular.slidersHorizontal),
                title: Text(L.of(context)!.video_settings),
              ),
            ),
          PopupMenuItem<String>(
            value: 'refresh_now',
            enabled: !_isRefreshingNow,
            child: ListTile(
              contentPadding: EdgeInsets.zero,
              leading: const Icon(PhosphorIconsRegular.arrowsClockwise),
              title: Text(_isYoutube ? L.of(context)!.check_for_new_videos : L.of(context)!.check_for_new_episodes),
            ),
          ),
          CheckedPopupMenuItem<String>(
            value: 'auto_refresh',
            checked: _isAutoRefreshEnabled,
            child: Text(L.of(context)!.refresh_automatically),
          ),
          if (!_isYoutube && !_isLocal)
            PopupMenuItem<String>(
              value: 'refresh_details',
              enabled: !_isRefreshingDetails,
              child: ListTile(
                contentPadding: EdgeInsets.zero,
                leading: const Icon(PhosphorIconsRegular.article),
                title: Text(L.of(context)!.refresh_details_from_feed),
              ),
            ),
          if (_aiAvailable)
            PopupMenuItem<String>(
              value: 'ai',
              child: ListTile(
                contentPadding: EdgeInsets.zero,
                leading: const Icon(PhosphorIconsRegular.sparkle),
                title: Text(L.of(context)!.ai_features),
              ),
            ),
          const PopupMenuDivider(),
        ],
        PopupMenuItem<String>(
          value: 'follow',
          enabled: !_isFollowButtonLoading,
          child: ListTile(
            contentPadding: EdgeInsets.zero,
            leading: Icon(_isFollowing ? PhosphorIconsRegular.minusCircle : PhosphorIconsRegular.plusCircle),
            title: Text(_isYoutube
                ? (_isFollowing ? L.of(context)!.unsubscribe : L.of(context)!.subscribe)
                : (_isFollowing ? L.of(context)!.unfollow : L.of(context)!.follow)),
          ),
        ),
      ],
      child: CircleAction(
        tooltip: null,
        icon: Icon(PhosphorIconsBold.dotsThree, color: softNeutral(theme).withOpacity(0.8), size: 20),
        onTap: null,
      ),
    );
  }

  List<Widget> _buildEpisodeSlivers(ThemeData theme, Color accent) {
    Widget message(IconData icon, String title, String body, {Widget? action}) => SliverToBoxAdapter(
          child: SoftEmptyState(icon: icon, title: title, body: body, action: action),
        );

    if (_isLoading) {
      return const [
        SliverToBoxAdapter(
          child: Padding(
            padding: EdgeInsets.all(40),
            child: Center(child: PlatformProgressIndicator()),
          ),
        ),
      ];
    }
    if (_errorMessage != null) {
      return [
        message(PhosphorIconsRegular.warningCircle, _errorMessage!, L.of(context)!.check_connection_try_again,
            action: FilledButton.tonal(onPressed: _loadPodcastFeed, child: Text(L.of(context)!.retry))),
      ];
    }
    if (_episodes.isEmpty) {
      return [
        message(
          PhosphorIconsRegular.info,
          _isYoutube
              ? (_isFollowing ? L.of(context)!.no_videos_yet : L.of(context)!.videos_available_after_subscribing)
              : (_isFollowing ? L.of(context)!.no_episodes_found : L.of(context)!.episodes_available_after_following),
          _isYoutube
              ? (_isFollowing
                  ? L.of(context)!.channel_videos_still_adding
                  : L.of(context)!.subscribe_channel_prompt)
              : _isFollowing
                  ? L.of(context)!.library_episodes_appear_here
                  : L.of(context)!.follow_podcast_prompt,
          action: _isFollowing ? FilledButton.tonal(onPressed: _loadPodcastFeed, child: Text(L.of(context)!.retry)) : null,
        ),
      ];
    }

    return [
      ListToolbar.sliver(context, _buildEpisodeToolbar(accent)),
      if (_filteredEpisodes.isEmpty)
        message(
          PhosphorIconsRegular.magnifyingGlassMinus,
          L.of(context)!.no_episodes_found,
          _searchQuery.isNotEmpty ? L.of(context)!.no_episodes_match_query(_searchQuery) : L.of(context)!.no_episodes_match_filter,
        )
      else
        _buildEpisodesList(),
      const SliverToBoxAdapter(child: SizedBox(height: 12)),
    ];
  }

  ListToolbar<EpisodeSortDirection> _buildEpisodeToolbar(Color accent) {
    final _EpisodeFilter current = _showInProgress
        ? _EpisodeFilter.inProgress
        : switch (_completedFilter) {
            CompletedFilter.showAll => _EpisodeFilter.all,
            CompletedFilter.hide => _EpisodeFilter.unplayed,
            CompletedFilter.showOnly => _EpisodeFilter.played,
          };

    return ListToolbar<EpisodeSortDirection>(
      accent: accent,
      searchController: _searchController,
      searchHint: L.of(context)!.search_episodes,
      filters: [
        for (final f in _EpisodeFilter.values)
          ToolbarFilter(
            label: f.label(context),
            selected: f == current,
            onTap: () {
              _applyEpisodeFilter(f);
              _saveFilterPreference(f);
            },
          ),
      ],
      sortOptions: {
        EpisodeSortDirection.newestFirst: L.of(context)!.sort_newest_first,
        EpisodeSortDirection.oldestFirst: L.of(context)!.sort_oldest_first,
        EpisodeSortDirection.shortestFirst: L.of(context)!.sort_shortest_first,
        EpisodeSortDirection.longestFirst: L.of(context)!.sort_longest_first,
        EpisodeSortDirection.titleAZ: L.of(context)!.sort_title_az,
        EpisodeSortDirection.titleZA: L.of(context)!.sort_title_za,
      },
      sortValue: _sortDirection,
      onSort: (value) {
        setState(() {
          _sortDirection = value;
          _filterEpisodes();
        });
        _saveSortPreference();
      },
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

        // The shell's mini player takes over once audio is active.
        if (isAudioActive) {
          return const SizedBox.shrink();
        } else if (_pendingEpisode != null) {
          return _PendingMiniPlayer(episode: _pendingEpisode!);
        }
        return const SizedBox.shrink();
      },
    );
  }

  Widget _buildEpisodesList() {
    return SliverList(
      delegate: SliverChildBuilderDelegate(
        (context, index) {
          final episode = _filteredEpisodes[index];
          final originalIndex = _filteredIndexes[index];
          // RSS fallback episodes have no server id, so no details or actions.
          final hasServerId = episode.episodeId > 0;

          return PinepodsEpisodeCard(
            episode: episode,
            onTap: _isFollowing && hasServerId
                ? () {
                    Navigator.push(
                      context,
                      MaterialPageRoute(
                        builder: (context) => PinepodsEpisodeDetails(initialEpisode: episode),
                      ),
                    );
                  }
                : null,
            onLongPress: _isFollowing && hasServerId ? () => _showEpisodeContextMenu(originalIndex) : null,
            // Unfollowed (feed) episodes play untracked.
            onPlayPressed: () => _playEpisode(episode),
          );
        },
        childCount: _filteredEpisodes.length,
      ),
    );
  }
}

enum _EpisodeFilter {
  all,
  unplayed,
  inProgress,
  played;

  String label(BuildContext context) => switch (this) {
        all => L.of(context)!.filter_all,
        unplayed => L.of(context)!.episode_filter_unplayed_label,
        inProgress => L.of(context)!.filter_in_progress,
        played => L.of(context)!.episode_filter_played_label,
      };
}

/// Lowercased search text and parsed date, computed once per episode instead
/// of on every keystroke and sort comparison.
class _EpisodeKeys {
  final String title;
  final String description;
  final int date;

  _EpisodeKeys(PinepodsEpisode e)
      : title = e.episodeTitle.toLowerCase(),
        description = e.episodeDescription.toLowerCase(),
        date = DateTime.tryParse(e.episodePubDate)?.millisecondsSinceEpoch ?? 0;
}

class _ExpandableDescription extends StatefulWidget {
  final String description;
  final Color accent;
  const _ExpandableDescription({required this.description, required this.accent});

  @override
  State<_ExpandableDescription> createState() => _ExpandableDescriptionState();
}

class _ExpandableDescriptionState extends State<_ExpandableDescription> {
  bool _expanded = false;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return GestureDetector(
      behavior: HitTestBehavior.opaque,
      onTap: () => setState(() => _expanded = !_expanded),
      child: AnimatedSize(
        duration: const Duration(milliseconds: 200),
        curve: Curves.easeOut,
        alignment: Alignment.topCenter,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              htmlToPlainText(widget.description),
              style: theme.textTheme.bodyMedium!.copyWith(height: 1.4, color: softNeutral(theme).withOpacity(0.85)),
              maxLines: _expanded ? null : 2,
              overflow: _expanded ? TextOverflow.visible : TextOverflow.ellipsis,
            ),
            const SizedBox(height: 2),
            Text(
              _expanded ? L.of(context)!.less : L.of(context)!.more_label,
              style: TextStyle(fontSize: 13, color: widget.accent, fontWeight: FontWeight.w600),
            ),
          ],
        ),
      ),
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
                              child: Icon(PhosphorIconsRegular.musicNote,
                                  size: 20,
                                  color: theme.colorScheme.onSurfaceVariant),
                            ),
                          )
                        : Container(
                            color: theme.colorScheme.surfaceVariant,
                            child: Icon(PhosphorIconsRegular.musicNote,
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

/// Shown under L.of(context)!.auto_detect_ads: whether ads will be found on the copy that
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
                      ? PhosphorIconsRegular.checkCircle
                      : PhosphorIconsRegular.warning,
                  size: 18,
                  color: color),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  serverAutoDownload
                      ? L.of(context)!.ad_skip_server_copy_ok
                      : L.of(context)!.ad_skip_server_copy_off,
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
                  icon: const Icon(PhosphorIconsRegular.cloudArrowDown, size: 18),
                  label: Text(L.of(context)!.turn_on_server_auto_download),
                ),
              TextButton(
                onPressed: onServerDownloads,
                child: Text(L.of(context)!.server_download_settings),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
