import 'dart:async';
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/home_data.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_audio_service.dart';
import 'package:pinepods_mobile/services/global_services.dart';
import 'package:pinepods_mobile/services/search_history_service.dart';
import 'package:pinepods_mobile/ui/widgets/episode_context_menu.dart';
import 'package:pinepods_mobile/ui/pinepods/podcast_nav.dart';
import 'package:pinepods_mobile/ui/widgets/paginated_episode_list.dart';
import 'package:pinepods_mobile/ui/pinepods/episode_details.dart';
import 'package:provider/provider.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';
import 'package:pinepods_mobile/ui/widgets/lazy_network_image.dart';
import 'package:pinepods_mobile/ui/widgets/soft_ui.dart';
import 'package:pinepods_mobile/ui/widgets/transcript_hit_card.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:pinepods_mobile/l10n/L.dart';

enum _StatusFilter { all, unplayed, inProgress, saved, downloaded }

class EpisodeSearchPage extends StatefulWidget {
  const EpisodeSearchPage({Key? key}) : super(key: key);

  @override
  State<EpisodeSearchPage> createState() => _EpisodeSearchPageState();
}

class _EpisodeSearchPageState extends State<EpisodeSearchPage> with TickerProviderStateMixin {
  final PinepodsService _pinepodsService = PinepodsService();
  final SearchHistoryService _searchHistoryService = SearchHistoryService();
  final TextEditingController _searchController = TextEditingController();
  final FocusNode _focusNode = FocusNode();
  Timer? _debounceTimer;

  List<SearchEpisodeResult> _searchResults = [];
  List<String> _searchHistory = [];
  bool _isLoading = false;
  bool _hasSearched = false;
  String? _errorMessage;
  String _currentQuery = '';

  // Category state
  List<String> _selectedCategories = [];
  List<String> _availableCategories = [];

  // Discovery surface
  List<HomePodcast> _mostPlayed = [];
  bool _discoveryLoaded = false;

  // Status filter (local, no API round-trip)
  _StatusFilter _activeStatusFilter = _StatusFilter.all;

  // Pagination state
  int _searchTotal = 0;
  int _searchOffset = 0;
  bool _isLoadingMore = false;

  int? _contextMenuEpisodeIndex;

  // Search inside transcripts too (remembered).
  static const _transcriptsPref = 'search_include_transcripts';
  bool _includeTranscripts = false;
  TranscriptSearchPage? _transcriptPage;
  bool _transcriptLoading = false;
  bool _transcriptUnsupported = false;
  bool _transcriptFailed = false;
  String _transcriptQuery = '';

  // Animation controllers
  late AnimationController _fadeAnimationController;
  late AnimationController _slideAnimationController;
  late Animation<double> _fadeAnimation;
  late Animation<Offset> _slideAnimation;

  @override
  void initState() {
    super.initState();
    _setupAnimations();
    _setupSearch();
    _loadDiscoveryData();
    SharedPreferences.getInstance().then((prefs) {
      if (mounted) setState(() => _includeTranscripts = prefs.getBool(_transcriptsPref) ?? false);
    });
  }

  Future<void> _toggleTranscripts() async {
    final on = !_includeTranscripts;
    setState(() => _includeTranscripts = on);
    (await SharedPreferences.getInstance()).setBool(_transcriptsPref, on);
    if (on && _currentQuery.isNotEmpty) _searchTranscripts(_currentQuery);
  }

  Future<void> _searchTranscripts(String query, {bool more = false}) async {
    if (query.isEmpty) return;
    final offset = more ? _transcriptPage?.results.length ?? 0 : 0;
    setState(() {
      _transcriptQuery = query;
      _transcriptLoading = true;
      _transcriptFailed = false;
      if (!more) _transcriptPage = null;
    });
    try {
      final page = await _pinepodsService.searchTranscripts(query, offset: offset);
      if (!mounted || _transcriptQuery != query) return;
      setState(() {
        _transcriptUnsupported = false;
        _transcriptPage = more && _transcriptPage != null
            ? TranscriptSearchPage(
                results: [..._transcriptPage!.results, ...page.results],
                total: page.total,
                terms: page.terms,
                searchableEpisodes: page.searchableEpisodes,
              )
            : page;
      });
    } on TranscriptSearchUnsupported {
      if (mounted) setState(() => _transcriptUnsupported = true);
    } catch (_) {
      if (mounted && _transcriptQuery == query) setState(() => _transcriptFailed = true);
    } finally {
      if (mounted && _transcriptQuery == query) setState(() => _transcriptLoading = false);
    }
  }

  void _playFrom(SearchEpisodeResult result, double seconds) {
    final service = _audioService;
    if (service == null) {
      _showSnackBar(L.of(context)!.audio_service_not_available, Colors.red);
      return;
    }
    service.playFrom(result.toPinepodsEpisode(), seconds).catchError((e) {
      _showSnackBar(L.current.failed_to_play_episode(e.toString()), Colors.red);
    });
  }

  void _setupAnimations() {
    _fadeAnimationController = AnimationController(
      duration: const Duration(milliseconds: 500),
      vsync: this,
    );
    _fadeAnimation = Tween<double>(
      begin: 0.0,
      end: 1.0,
    ).animate(CurvedAnimation(
      parent: _fadeAnimationController,
      curve: Curves.easeInOut,
    ));

    _slideAnimationController = AnimationController(
      duration: const Duration(milliseconds: 300),
      vsync: this,
    );
    _slideAnimation = Tween<Offset>(
      begin: const Offset(0, 0),
      end: const Offset(0, -0.2),
    ).animate(CurvedAnimation(
      parent: _slideAnimationController,
      curve: Curves.easeInOut,
    ));
  }

  void _setupSearch() {
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;

    if (settings.pinepodsServer != null && settings.pinepodsApiKey != null) {
      _pinepodsService.setCredentials(
        settings.pinepodsServer!,
        settings.pinepodsApiKey!,
      );
      GlobalServices.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
    }

    _searchController.addListener(_onSearchChanged);
    _loadSearchHistory();
  }

  Future<void> _loadDiscoveryData() async {
    if (_discoveryLoaded) return;
    final settings = Provider.of<SettingsBloc>(context, listen: false).currentSettings;
    final userId = settings.pinepodsUserId;
    if (userId == null) return;

    try {
      final podData = await _pinepodsService.getUserPodcastsWithCategories(userId);
      final catMap = podData['categories'] as Map<int, List<String>>? ?? {};
      final allCats = catMap.values.expand((c) => c).toSet().toList()..sort();

      final overview = await _pinepodsService.getHomeOverview(userId);

      if (mounted) {
        setState(() {
          _availableCategories = allCats;
          _mostPlayed = overview.topPodcasts;
          _discoveryLoaded = true;
        });
      }
    } catch (_) {}
  }

  Future<void> _loadSearchHistory() async {
    final history = await _searchHistoryService.getEpisodeSearchHistory();
    if (mounted) {
      setState(() {
        _searchHistory = history;
      });
    }
  }

  void _selectHistoryItem(String searchTerm) {
    _searchController.text = searchTerm;
    _performSearch(searchTerm);
  }

  Future<void> _removeHistoryItem(String searchTerm) async {
    await _searchHistoryService.removeEpisodeSearchTerm(searchTerm);
    await _loadSearchHistory();
  }

  void _toggleCategory(String cat) {
    setState(() {
      if (_selectedCategories.contains(cat)) {
        _selectedCategories.remove(cat);
      } else {
        _selectedCategories.add(cat);
      }
    });
    final query = _searchController.text.trim();
    _currentQuery = '';
    if (query.isNotEmpty || _selectedCategories.isNotEmpty) {
      _performSearch(query);
    } else {
      _clearResults();
    }
  }

  List<SearchEpisodeResult> get _visibleResults {
    return _searchResults.where((r) {
      switch (_activeStatusFilter) {
        case _StatusFilter.all:
          return true;
        case _StatusFilter.unplayed:
          return !r.completed && (r.listenDuration ?? 0) == 0;
        case _StatusFilter.inProgress:
          return (r.listenDuration ?? 0) > 0 && !r.completed;
        case _StatusFilter.saved:
          return r.saved;
        case _StatusFilter.downloaded:
          return r.downloaded;
      }
    }).toList();
  }

  PinepodsAudioService? get _audioService => GlobalServices.pinepodsAudioService;

  Future<void> _playEpisode(PinepodsEpisode episode) async {
    if (_audioService == null) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(L.of(context)!.audio_service_not_available),
          backgroundColor: Colors.red,
        ),
      );
      return;
    }

    try {
      await _audioService!.playPinepodsEpisode(pinepodsEpisode: episode);

      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(L.of(context)!.playing_title(episode.episodeTitle)),
            backgroundColor: Colors.green,
          ),
        );
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(L.of(context)!.failed_to_play_episode(e.toString())),
            backgroundColor: Colors.red,
          ),
        );
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
    final episode = _searchResults[episodeIndex].toPinepodsEpisode();
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
        setState(() {
          _searchResults[episodeIndex] = SearchEpisodeResult(
            podcastId: _searchResults[episodeIndex].podcastId,
            podcastName: _searchResults[episodeIndex].podcastName,
            artworkUrl: _searchResults[episodeIndex].artworkUrl,
            author: _searchResults[episodeIndex].author,
            categories: _searchResults[episodeIndex].categories,
            description: _searchResults[episodeIndex].description,
            episodeCount: _searchResults[episodeIndex].episodeCount,
            feedUrl: _searchResults[episodeIndex].feedUrl,
            websiteUrl: _searchResults[episodeIndex].websiteUrl,
            explicit: _searchResults[episodeIndex].explicit,
            userId: _searchResults[episodeIndex].userId,
            episodeId: _searchResults[episodeIndex].episodeId,
            episodeTitle: _searchResults[episodeIndex].episodeTitle,
            episodeDescription: _searchResults[episodeIndex].episodeDescription,
            episodePubDate: _searchResults[episodeIndex].episodePubDate,
            episodeArtwork: _searchResults[episodeIndex].episodeArtwork,
            episodeUrl: _searchResults[episodeIndex].episodeUrl,
            episodeDuration: _searchResults[episodeIndex].episodeDuration,
            completed: _searchResults[episodeIndex].completed,
            saved: true,
            queued: _searchResults[episodeIndex].queued,
            downloaded: _searchResults[episodeIndex].downloaded,
            isYoutube: _searchResults[episodeIndex].isYoutube,
            listenDuration: _searchResults[episodeIndex].listenDuration,
          );
        });
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
    final episode = _searchResults[episodeIndex].toPinepodsEpisode();
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
    final episode = _searchResults[episodeIndex].toPinepodsEpisode();
    _showSnackBar(L.of(context)!.download_started_for(episode.episodeTitle), Colors.blue);
  }

  Future<void> _deleteEpisode(int episodeIndex) async {
    final episode = _searchResults[episodeIndex].toPinepodsEpisode();
    _showSnackBar(L.of(context)!.delete_requested_for(episode.episodeTitle), Colors.orange);
  }

  Future<void> _localDownloadEpisode(int episodeIndex) async {
    final episode = _searchResults[episodeIndex].toPinepodsEpisode();
    _showSnackBar(L.of(context)!.local_download_started_for(episode.episodeTitle), Colors.blue);
  }

  Future<void> _toggleQueueEpisode(int episodeIndex, {String? position}) async {
    final episode = _searchResults[episodeIndex].toPinepodsEpisode();
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
    final episode = _searchResults[episodeIndex].toPinepodsEpisode();
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

  void _onSearchChanged() {
    final query = _searchController.text.trim();

    _debounceTimer?.cancel();
    _debounceTimer = Timer(const Duration(milliseconds: 500), () {
      if (query.isEmpty) {
        if (_selectedCategories.isNotEmpty) {
          _currentQuery = '';
          _performSearch('');
        } else {
          _clearResults();
        }
      } else if (query != _currentQuery) {
        _currentQuery = query;
        _performSearch(query);
      }
    });
  }

  Future<void> _performSearch(String query) async {
    setState(() {
      _isLoading = true;
      _errorMessage = null;
    });

    if (query.isNotEmpty) {
      await _searchHistoryService.addEpisodeSearchTerm(query);
      await _loadSearchHistory();
    }

    _slideAnimationController.forward();
    if (_includeTranscripts && query.isNotEmpty) _searchTranscripts(query);

    try {
      final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
      final settings = settingsBloc.currentSettings;
      final userId = settings.pinepodsUserId;

      if (userId == null) throw Exception('Not logged in');

      final page = await _pinepodsService.searchEpisodes(
        userId, query,
        limit: 50, offset: 0,
        categories: _selectedCategories.isEmpty ? null : List.from(_selectedCategories),
      );

      setState(() {
        _searchResults = page.results;
        _searchTotal = page.total;
        _searchOffset = page.results.length;
        _isLoading = false;
        _hasSearched = true;
      });

      _fadeAnimationController.forward();
    } catch (e) {
      setState(() {
        _errorMessage = e.toString();
        _isLoading = false;
        _hasSearched = true;
        _searchResults = [];
        _searchTotal = 0;
        _searchOffset = 0;
      });
    }
  }

  Future<void> _loadMoreResults() async {
    if (_isLoadingMore || _searchOffset >= _searchTotal) return;

    final userId = Provider.of<SettingsBloc>(context, listen: false)
        .currentSettings.pinepodsUserId;
    if (userId == null) return;

    setState(() => _isLoadingMore = true);

    try {
      final page = await _pinepodsService.searchEpisodes(
        userId, _currentQuery,
        limit: 50, offset: _searchOffset,
        categories: _selectedCategories.isEmpty ? null : List.from(_selectedCategories),
      );
      setState(() {
        _searchResults.addAll(page.results);
        _searchTotal = page.total;
        _searchOffset += page.results.length;
        _isLoadingMore = false;
      });
    } catch (e) {
      setState(() => _isLoadingMore = false);
    }
  }

  void _clearResults() {
    setState(() {
      _searchResults = [];
      _hasSearched = false;
      _errorMessage = null;
      _currentQuery = '';
      _selectedCategories = [];
      _activeStatusFilter = _StatusFilter.all;
      _searchTotal = 0;
      _searchOffset = 0;
      _transcriptPage = null;
      _transcriptQuery = '';
      _transcriptLoading = false;
    });
    _fadeAnimationController.reset();
    _slideAnimationController.reverse();
  }

  Widget _buildSectionHeader(String title) {
    return SoftSectionHeader(
      title: title,
      padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
    );
  }

  Widget _buildSearchBar() {
    final showClear = _searchController.text.isNotEmpty || _selectedCategories.isNotEmpty;
    final pill = OutlineInputBorder(borderRadius: BorderRadius.circular(26), borderSide: BorderSide.none);
    return SlideTransition(
      position: _slideAnimation,
      child: Padding(
        padding: const EdgeInsets.fromLTRB(16, 12, 16, 8),
        child: TextField(
          controller: _searchController,
          focusNode: _focusNode,
          style: Theme.of(context).textTheme.bodyLarge,
          textInputAction: TextInputAction.search,
          onTap: () {
            setState(() {});
          },
          decoration: InputDecoration(
            hintText: L.of(context)!.search_episodes_label,
            prefixIcon: const Icon(PhosphorIconsRegular.magnifyingGlass),
            suffixIcon: showClear
                ? IconButton(
                    tooltip: L.of(context)!.clear_search,
                    icon: const Icon(PhosphorIconsRegular.x),
                    onPressed: () {
                      _searchController.clear();
                      _clearResults();
                      _focusNode.requestFocus();
                    },
                  )
                : null,
            border: pill,
            enabledBorder: pill,
            focusedBorder: pill,
            contentPadding: const EdgeInsets.symmetric(horizontal: 20, vertical: 14),
          ),
        ),
      ),
    );
  }

  Widget _buildActiveCategories() {
    return SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 4),
      child: Row(
        children: _selectedCategories.map((cat) => Padding(
          padding: const EdgeInsets.only(right: 8),
          child: Chip(
            avatar: const Icon(PhosphorIconsRegular.tag, size: 14),
            label: Text(cat),
            deleteIcon: const Icon(PhosphorIconsRegular.x, size: 14),
            onDeleted: () => _toggleCategory(cat),
            backgroundColor: Theme.of(context).colorScheme.primaryContainer,
            labelStyle: TextStyle(color: Theme.of(context).colorScheme.onPrimaryContainer),
          ),
        )).toList(),
      ),
    );
  }

  Widget _buildStatusFilterChips() {
    final chips = <(_StatusFilter, String, IconData?)>[
      (_StatusFilter.all, L.of(context)!.filter_all, null),
      (_StatusFilter.unplayed, L.of(context)!.episode_filter_unplayed_label, null),
      (_StatusFilter.inProgress, L.of(context)!.filter_in_progress, null),
      (_StatusFilter.saved, L.of(context)!.saved, PhosphorIconsRegular.bookmarkSimple),
      (_StatusFilter.downloaded, L.of(context)!.downloaded, PhosphorIconsRegular.downloadSimple),
    ];
    return SizedBox(
      height: 52,
      child: ListView(
        scrollDirection: Axis.horizontal,
        padding: const EdgeInsets.fromLTRB(16, 8, 8, 8),
        children: [
          Padding(
            padding: const EdgeInsets.only(right: 8),
            child: _transcriptsPill(),
          ),
          for (final (filter, label, icon) in chips)
            Padding(
              padding: const EdgeInsets.only(right: 8),
              child: FilterPill(
                label: label,
                icon: icon,
                selected: _activeStatusFilter == filter,
                onTap: () => setState(() => _activeStatusFilter = filter),
              ),
            ),
        ],
      ),
    );
  }

  Widget _transcriptsPill() => FilterPill(
        label: L.of(context)!.transcripts,
        icon: _includeTranscripts ? PhosphorIconsRegular.checkSquare : PhosphorIconsRegular.square,
        selected: _includeTranscripts,
        onTap: _toggleTranscripts,
      );

  Widget _buildTranscriptSection() {
    final theme = Theme.of(context);
    final page = _transcriptPage;
    Widget body;
    if (_transcriptUnsupported) {
      body = _note(L.of(context)!.server_needs_update_transcripts);
    } else if (_transcriptFailed) {
      body = _note(L.of(context)!.couldnt_search_transcripts);
    } else if (page == null) {
      body = const Padding(
        padding: EdgeInsets.all(24),
        child: Center(child: SizedBox(width: 24, height: 24, child: CircularProgressIndicator(strokeWidth: 2.5))),
      );
    } else if (page.results.isEmpty) {
      body = _note(page.searchableEpisodes == 0
          ? L.of(context)!.no_transcripts_yet
          : L.of(context)!.not_said_in_transcripts(page.searchableEpisodes));
    } else {
      body = Column(
        children: [
          for (final hit in page.results)
            TranscriptHitCard(
              key: ValueKey('transcript-${hit.episode.episodeId}'),
              hit: hit,
              terms: page.terms,
              onOpen: () => Navigator.push(
                context,
                MaterialPageRoute(builder: (_) => PinepodsEpisodeDetails(initialEpisode: hit.episode.toPinepodsEpisode())),
              ),
              onPlayFrom: (t) => _playFrom(hit.episode, t),
            ),
          if (page.results.length < page.total)
            Padding(
              padding: const EdgeInsets.only(bottom: 8),
              child: _transcriptLoading
                  ? const CircularProgressIndicator()
                  : OutlinedButton.icon(
                      onPressed: () => _searchTranscripts(_transcriptQuery, more: true),
                      icon: const Icon(PhosphorIconsRegular.caretDown),
                      label: Text(L.of(context)!.more_episodes),
                    ),
            ),
        ],
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SoftSectionHeader(
          title: L.of(context)!.said_in_episodes,
          count: page != null && page.total > 0 ? page.total : null,
          padding: const EdgeInsets.fromLTRB(16, 12, 16, 8),
          trailing: [Icon(PhosphorIconsRegular.scroll, size: 18, color: theme.hintColor)],
        ),
        body,
      ],
    );
  }

  Widget _note(String text) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 0, 16, 12),
      child: Text(text, style: theme.textTheme.bodyMedium!.copyWith(color: theme.hintColor)),
    );
  }

  Widget _buildPodcastTile(HomePodcast pod) {
    return Container(
      width: 100,
      margin: const EdgeInsets.only(right: 12),
      child: Column(
        children: [
          LazyNetworkImage(
            imageUrl: pod.artworkUrl ?? '',
            width: 96,
            height: 96,
            borderRadius: BorderRadius.circular(14),
          ),
          const SizedBox(height: 6),
          Text(
            pod.podcastName,
            maxLines: 2,
            overflow: TextOverflow.ellipsis,
            style: Theme.of(context).textTheme.bodySmall?.copyWith(fontWeight: FontWeight.w600),
            textAlign: TextAlign.center,
          ),
        ],
      ),
    );
  }

  Widget _buildDiscoverySurface() {
    final topPodcasts = _mostPlayed.take(8).toList();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (_searchHistory.isNotEmpty) _buildSearchHistory(),
        if (topPodcasts.isNotEmpty) ...[
          _buildSectionHeader(L.of(context)!.most_played_in_library),
          SizedBox(
            height: 140,
            child: ListView.builder(
              scrollDirection: Axis.horizontal,
              padding: const EdgeInsets.symmetric(horizontal: 16),
              itemCount: topPodcasts.length,
              itemBuilder: (context, index) => _buildPodcastTile(topPodcasts[index]),
            ),
          ),
          const SizedBox(height: 16),
        ],
        if (_availableCategories.isNotEmpty) ...[
          _buildSectionHeader(L.of(context)!.browse_by_category),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            child: Wrap(
              spacing: 8,
              runSpacing: 8,
              children: _availableCategories.map((cat) => SizedBox(
                height: 34,
                child: FilterPill(
                  label: cat,
                  selected: _selectedCategories.contains(cat),
                  onTap: () => _toggleCategory(cat),
                ),
              )).toList(),
            ),
          ),
          const SizedBox(height: 16),
        ],
      ],
    );
  }

  Widget _buildLoadingIndicator() {
    return const Padding(
      padding: EdgeInsets.all(64),
      child: Center(child: CircularProgressIndicator()),
    );
  }

  Widget _buildEmptyState() {
    return SoftEmptyState(
      icon: PhosphorIconsRegular.magnifyingGlassMinus,
      title: L.of(context)!.no_episodes_found,
      body: L.of(context)!.try_different_search,
    );
  }

  Widget _buildErrorState() {
    return SoftEmptyState(
      icon: PhosphorIconsRegular.warningCircle,
      title: L.of(context)!.search_failed,
      body: _errorMessage ?? L.of(context)!.unknown_error_occurred,
      action: FilledButton.tonal(
        onPressed: () {
          if (_currentQuery.isNotEmpty || _selectedCategories.isNotEmpty) {
            _performSearch(_currentQuery);
          }
        },
        child: Text(L.of(context)!.try_again),
      ),
    );
  }

  Widget _buildResults() {
    final visible = _visibleResults;
    final episodes = visible.map((result) => result.toPinepodsEpisode()).toList();
    final hasMore = _searchOffset < _searchTotal;

    return FadeTransition(
      opacity: _fadeAnimation,
      child: Column(
        children: [
          PaginatedEpisodeList(
            episodes: episodes,
            isServerEpisodes: true,
            pageSize: 20,
            onEpisodeTap: (episode) {
              Navigator.push(
                context,
                MaterialPageRoute(
                  builder: (context) => PinepodsEpisodeDetails(
                    initialEpisode: episode,
                  ),
                ),
              );
            },
            onEpisodeLongPress: (episode, globalIndex) {
              final originalIndex = _searchResults.indexWhere(
                (result) => result.episodeId == episode.episodeId,
              );
              if (originalIndex != -1) {
                _showContextMenu(originalIndex);
              }
            },
            onPlayPressed: (episode) => _playEpisode(episode),
          ),
          if (hasMore || _isLoadingMore)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 12.0),
              child: _isLoadingMore
                  ? const CircularProgressIndicator()
                  : OutlinedButton.icon(
                      onPressed: _loadMoreResults,
                      icon: const Icon(PhosphorIconsRegular.caretDown),
                      label: Text(L.of(context)!.load_more_remaining(_searchTotal - _searchOffset)),
                    ),
            ),
        ],
      ),
    );
  }

  Widget _buildSearchHistory() {
    final theme = Theme.of(context);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SoftSectionHeader(
          title: L.of(context)!.recent_searches,
          trailing: [
            TextButton(
              onPressed: () async {
                await _searchHistoryService.clearEpisodeSearchHistory();
                await _loadSearchHistory();
              },
              child: Text(L.of(context)!.clear),
            ),
          ],
        ),
        for (final searchTerm in _searchHistory.take(10))
          ListTile(
            contentPadding: const EdgeInsets.only(left: 16, right: 4),
            leading: Icon(PhosphorIconsRegular.clockCounterClockwise, color: theme.hintColor, size: 20),
            title: Text(searchTerm, maxLines: 1, overflow: TextOverflow.ellipsis),
            trailing: IconButton(
              tooltip: L.of(context)!.remove,
              icon: Icon(PhosphorIconsRegular.x, size: 18, color: theme.hintColor),
              onPressed: () => _removeHistoryItem(searchTerm),
            ),
            onTap: () => _selectHistoryItem(searchTerm),
          ),
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    if (_contextMenuEpisodeIndex != null) {
      final episodeIndex = _contextMenuEpisodeIndex!;
      final episode = _searchResults[episodeIndex].toPinepodsEpisode();
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
      _contextMenuEpisodeIndex = null;
    }

    final showDiscovery = _currentQuery.isEmpty && _selectedCategories.isEmpty && !_hasSearched;
    final showFilterBar = _hasSearched || _selectedCategories.isNotEmpty;
    final showTranscripts = _includeTranscripts && _currentQuery.isNotEmpty;
    final transcriptHits = showTranscripts && (_transcriptPage?.results.isNotEmpty ?? false);

    return SliverFillRemaining(
      child: GestureDetector(
        onTap: () {
          FocusScope.of(context).unfocus();
        },
        child: Column(
          children: [
            _buildSearchBar(),
            if (_selectedCategories.isNotEmpty) _buildActiveCategories(),
            if (showFilterBar)
              _buildStatusFilterChips()
            else
              Align(
                alignment: Alignment.centerLeft,
                child: Padding(padding: const EdgeInsets.fromLTRB(16, 4, 16, 4), child: _transcriptsPill()),
              ),
            Expanded(
              child: SingleChildScrollView(
                child: showDiscovery
                    ? _buildDiscoverySurface()
                    : Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          if (showTranscripts) _buildTranscriptSection(),
                          if (_isLoading)
                            _buildLoadingIndicator()
                          else if (_errorMessage != null)
                            _buildErrorState()
                          else if (_visibleResults.isEmpty)
                            transcriptHits ? _note(L.of(context)!.no_matches_titles_notes) : _buildEmptyState()
                          else ...[
                            if (showTranscripts) _buildSectionHeader(L.of(context)!.titles_and_show_notes),
                            _buildResults(),
                          ],
                        ],
                      ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  @override
  void dispose() {
    _debounceTimer?.cancel();
    _searchController.dispose();
    _focusNode.dispose();
    _fadeAnimationController.dispose();
    _slideAnimationController.dispose();
    super.dispose();
  }
}
