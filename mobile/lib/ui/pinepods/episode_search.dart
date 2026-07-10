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
        const SnackBar(
          content: Text('Audio service not available'),
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
            content: Text('Playing ${episode.episodeTitle}'),
            backgroundColor: Colors.green,
          ),
        );
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text('Failed to play episode: $e'),
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
        _showSnackBar('Failed to save episode', Colors.red);
      }
    } catch (e) {
      if (mounted) {
        _showSnackBar('Error saving episode: $e', Colors.red);
      }
    }
  }

  Future<void> _removeSavedEpisode(int episodeIndex) async {
    final episode = _searchResults[episodeIndex].toPinepodsEpisode();
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
    final episode = _searchResults[episodeIndex].toPinepodsEpisode();
    _showSnackBar('Download started for ${episode.episodeTitle}', Colors.blue);
  }

  Future<void> _deleteEpisode(int episodeIndex) async {
    final episode = _searchResults[episodeIndex].toPinepodsEpisode();
    _showSnackBar('Delete requested for ${episode.episodeTitle}', Colors.orange);
  }

  Future<void> _localDownloadEpisode(int episodeIndex) async {
    final episode = _searchResults[episodeIndex].toPinepodsEpisode();
    _showSnackBar('Local download started for ${episode.episodeTitle}', Colors.blue);
  }

  Future<void> _toggleQueueEpisode(int episodeIndex) async {
    final episode = _searchResults[episodeIndex].toPinepodsEpisode();
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
    final episode = _searchResults[episodeIndex].toPinepodsEpisode();
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
    });
    _fadeAnimationController.reset();
    _slideAnimationController.reverse();
  }

  Widget _buildSectionHeader(String title) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
      child: Text(
        title,
        style: Theme.of(context).textTheme.titleMedium?.copyWith(
          color: Theme.of(context).primaryColor,
          fontWeight: FontWeight.bold,
        ),
      ),
    );
  }

  Widget _buildSearchBar() {
    final showClear = _searchController.text.isNotEmpty || _selectedCategories.isNotEmpty;
    return SlideTransition(
      position: _slideAnimation,
      child: Container(
        padding: const EdgeInsets.all(16),
        child: Card(
          elevation: 4,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(30),
          ),
          child: Container(
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(30),
              gradient: LinearGradient(
                colors: [
                  Theme.of(context).primaryColor.withOpacity(0.1),
                  Theme.of(context).primaryColor.withOpacity(0.05),
                ],
                begin: Alignment.topLeft,
                end: Alignment.bottomRight,
              ),
            ),
            child: TextField(
              controller: _searchController,
              focusNode: _focusNode,
              style: Theme.of(context).textTheme.bodyLarge,
              onTap: () {
                setState(() {});
              },
              decoration: InputDecoration(
                hintText: 'Search for episodes...',
                hintStyle: Theme.of(context).textTheme.bodyMedium?.copyWith(
                  color: Theme.of(context).hintColor,
                ),
                prefixIcon: Icon(
                  Icons.search,
                  color: Theme.of(context).primaryColor,
                ),
                suffixIcon: showClear
                    ? IconButton(
                        icon: Icon(
                          Icons.clear,
                          color: Theme.of(context).primaryColor,
                        ),
                        onPressed: () {
                          _searchController.clear();
                          _clearResults();
                          _focusNode.requestFocus();
                        },
                      )
                    : null,
                border: InputBorder.none,
                contentPadding: const EdgeInsets.symmetric(
                  horizontal: 20,
                  vertical: 16,
                ),
              ),
            ),
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
            avatar: const Icon(Icons.label_outline, size: 14),
            label: Text(cat),
            deleteIcon: const Icon(Icons.close, size: 14),
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
      (_StatusFilter.all, 'All', null),
      (_StatusFilter.unplayed, 'Unplayed', Icons.circle_outlined),
      (_StatusFilter.inProgress, 'In Progress', Icons.hourglass_empty),
      (_StatusFilter.saved, 'Saved', Icons.star_border),
      (_StatusFilter.downloaded, 'Downloaded', Icons.download_outlined),
    ];
    return SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 4),
      child: Row(
        children: chips.map((c) {
          final (filter, label, icon) = c;
          return Padding(
            padding: const EdgeInsets.only(right: 8),
            child: FilterChip(
              avatar: icon != null ? Icon(icon, size: 14) : null,
              label: Text(label),
              selected: _activeStatusFilter == filter,
              onSelected: (_) => setState(() => _activeStatusFilter = filter),
            ),
          );
        }).toList(),
      ),
    );
  }

  Widget _buildPodcastTile(HomePodcast pod) {
    return Container(
      width: 100,
      margin: const EdgeInsets.only(right: 12),
      child: Column(
        children: [
          ClipRRect(
            borderRadius: BorderRadius.circular(8),
            child: Image.network(
              pod.artworkUrl ?? '',
              width: 90,
              height: 90,
              fit: BoxFit.cover,
              errorBuilder: (_, __, ___) => Container(
                width: 90,
                height: 90,
                color: Theme.of(context).colorScheme.surfaceVariant,
                child: const Icon(Icons.podcasts),
              ),
            ),
          ),
          const SizedBox(height: 4),
          Text(
            pod.podcastName,
            maxLines: 2,
            overflow: TextOverflow.ellipsis,
            style: Theme.of(context).textTheme.bodySmall,
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
          _buildSectionHeader('Most Played in Your Library'),
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
          _buildSectionHeader('Browse by Category'),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            child: Wrap(
              spacing: 8,
              runSpacing: 8,
              children: _availableCategories.map((cat) => FilterChip(
                label: Text(cat),
                selected: _selectedCategories.contains(cat),
                onSelected: (_) => _toggleCategory(cat),
              )).toList(),
            ),
          ),
          const SizedBox(height: 16),
        ],
      ],
    );
  }

  Widget _buildLoadingIndicator() {
    return Container(
      padding: const EdgeInsets.all(64),
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          const CircularProgressIndicator(),
          const SizedBox(height: 16),
          Text(
            'Searching...',
            style: Theme.of(context).textTheme.bodyLarge?.copyWith(
              color: Theme.of(context).primaryColor,
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildEmptyState() {
    return Container(
      padding: const EdgeInsets.all(32),
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(
            Icons.search_off,
            size: 64,
            color: Theme.of(context).hintColor,
          ),
          const SizedBox(height: 16),
          Text(
            'No Episodes Found',
            style: Theme.of(context).textTheme.headlineSmall,
          ),
          const SizedBox(height: 8),
          Text(
            'Try adjusting your search terms or categories',
            style: Theme.of(context).textTheme.bodyMedium?.copyWith(
              color: Theme.of(context).hintColor,
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildErrorState() {
    return Container(
      padding: const EdgeInsets.all(32),
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(
            Icons.error_outline,
            size: 64,
            color: Theme.of(context).colorScheme.error,
          ),
          const SizedBox(height: 16),
          Text(
            'Search Error',
            style: Theme.of(context).textTheme.headlineSmall?.copyWith(
              color: Theme.of(context).colorScheme.error,
            ),
          ),
          const SizedBox(height: 8),
          Text(
            _errorMessage ?? 'Unknown error occurred',
            style: Theme.of(context).textTheme.bodyMedium,
            textAlign: TextAlign.center,
          ),
          const SizedBox(height: 16),
          ElevatedButton(
            onPressed: () {
              if (_currentQuery.isNotEmpty || _selectedCategories.isNotEmpty) {
                _performSearch(_currentQuery);
              }
            },
            child: const Text('Try Again'),
          ),
        ],
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
                      icon: const Icon(Icons.expand_more),
                      label: Text('Load more (${_searchTotal - _searchOffset} remaining)'),
                    ),
            ),
        ],
      ),
    );
  }

  Widget _buildSearchHistory() {
    return Container(
      margin: const EdgeInsets.symmetric(horizontal: 16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Text(
                'Recent Searches',
                style: Theme.of(context).textTheme.titleMedium?.copyWith(
                  color: Theme.of(context).primaryColor,
                  fontWeight: FontWeight.bold,
                ),
              ),
              const Spacer(),
              TextButton(
                onPressed: () async {
                  await _searchHistoryService.clearEpisodeSearchHistory();
                  await _loadSearchHistory();
                },
                child: Text(
                  'Clear All',
                  style: TextStyle(
                    color: Theme.of(context).hintColor,
                    fontSize: 12,
                  ),
                ),
              ),
            ],
          ),
          const SizedBox(height: 8),
          ..._searchHistory.take(10).map((searchTerm) => Card(
            margin: const EdgeInsets.symmetric(vertical: 2),
            child: ListTile(
              dense: true,
              leading: Icon(
                Icons.history,
                color: Theme.of(context).hintColor,
                size: 20,
              ),
              title: Text(
                searchTerm,
                style: Theme.of(context).textTheme.bodyMedium,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
              trailing: IconButton(
                icon: Icon(
                  Icons.close,
                  size: 18,
                  color: Theme.of(context).hintColor,
                ),
                onPressed: () => _removeHistoryItem(searchTerm),
              ),
              onTap: () => _selectHistoryItem(searchTerm),
            ),
          )).toList(),
        ],
      ),
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

    return SliverFillRemaining(
      child: GestureDetector(
        onTap: () {
          FocusScope.of(context).unfocus();
        },
        child: Column(
          children: [
            _buildSearchBar(),
            if (_selectedCategories.isNotEmpty) _buildActiveCategories(),
            if (showFilterBar) _buildStatusFilterChips(),
            Expanded(
              child: SingleChildScrollView(
                child: showDiscovery
                    ? _buildDiscoverySurface()
                    : _isLoading
                        ? _buildLoadingIndicator()
                        : _errorMessage != null
                            ? _buildErrorState()
                            : _visibleResults.isEmpty
                                ? _buildEmptyState()
                                : _buildResults(),
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
