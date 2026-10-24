// lib/ui/pinepods/history.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_audio_service.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/ui/widgets/episode_context_menu.dart';
import 'package:pinepods_mobile/ui/pinepods/podcast_nav.dart';
import 'package:pinepods_mobile/ui/widgets/pinepods_episode_card.dart';
import 'package:pinepods_mobile/ui/pinepods/episode_details.dart';
import 'package:pinepods_mobile/ui/utils/local_download_utils.dart';
import 'package:pinepods_mobile/ui/utils/player_utils.dart';
import 'package:pinepods_mobile/ui/utils/position_utils.dart';
import 'package:pinepods_mobile/ui/widgets/server_error_page.dart';
import 'package:pinepods_mobile/services/error_handling_service.dart';
import 'package:pinepods_mobile/services/global_services.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:sliver_tools/sliver_tools.dart';
import 'package:pinepods_mobile/ui/widgets/soft_ui.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';

/// Sort direction options for history episodes
enum HistorySortDirection {
  newestFirst,
  oldestFirst,
  shortestFirst,
  longestFirst,
  titleAZ,
  titleZA,
}

/// Episode filter options for history
enum HistoryFilter {
  all,
  completed,
  inProgress,
}

class PinepodsHistory extends StatefulWidget {
  const PinepodsHistory({Key? key}) : super(key: key);

  @override
  State<PinepodsHistory> createState() => _PinepodsHistoryState();
}

class _PinepodsHistoryState extends State<PinepodsHistory> {
  bool _isLoading = false;
  String _errorMessage = '';
  List<PinepodsEpisode> _episodes = [];
  List<PinepodsEpisode> _filteredEpisodes = [];
  // Position of each episode in [_episodes], for the context menu.
  Map<PinepodsEpisode, int> _originalIndex = Map<PinepodsEpisode, int>.identity();
  final PinepodsService _pinepodsService = PinepodsService();
  // Use global audio service instead of creating local instance
  int? _contextMenuEpisodeIndex;
  final TextEditingController _searchController = TextEditingController();
  String _searchQuery = '';

  // Pagination state
  int _offset = 0;
  int _total = 0;
  bool _isLoadingMore = false;

  // Sort and filter state
  HistorySortDirection _sortDirection = HistorySortDirection.newestFirst;
  HistoryFilter _activeFilter = HistoryFilter.all;
  static const String _sortPreferenceKey = 'history_sort_direction';

  // Favorites-only filter (podcast-level favorite state from return_pods).
  bool _favoritesOnly = false;
  Set<int> _favoritePodcastIds = {};

  @override
  void initState() {
    super.initState();
    _loadSortPreference();
    _loadHistory();
    _searchController.addListener(_onSearchChanged);
  }

  Future<void> _loadSortPreference() async {
    final prefs = await SharedPreferences.getInstance();
    final savedSort = prefs.getString(_sortPreferenceKey);
    if (savedSort != null) {
      setState(() {
        _sortDirection = _sortDirectionFromString(savedSort);
      });
    }
  }

  Future<void> _saveSortPreference(HistorySortDirection direction) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_sortPreferenceKey, _sortDirectionToString(direction));
  }

  String _sortDirectionToString(HistorySortDirection direction) {
    switch (direction) {
      case HistorySortDirection.newestFirst:
        return 'newest';
      case HistorySortDirection.oldestFirst:
        return 'oldest';
      case HistorySortDirection.shortestFirst:
        return 'shortest';
      case HistorySortDirection.longestFirst:
        return 'longest';
      case HistorySortDirection.titleAZ:
        return 'title_az';
      case HistorySortDirection.titleZA:
        return 'title_za';
    }
  }

  HistorySortDirection _sortDirectionFromString(String value) {
    switch (value) {
      case 'oldest':
        return HistorySortDirection.oldestFirst;
      case 'shortest':
        return HistorySortDirection.shortestFirst;
      case 'longest':
        return HistorySortDirection.longestFirst;
      case 'title_az':
        return HistorySortDirection.titleAZ;
      case 'title_za':
        return HistorySortDirection.titleZA;
      case 'newest':
      default:
        return HistorySortDirection.newestFirst;
    }
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
    // Sort and status filter are handled server-side; only apply search locally
    var filtered = List<PinepodsEpisode>.from(_episodes);
    if (_searchQuery.isNotEmpty) {
      filtered = filtered.where((episode) {
        return episode.episodeTitle.toLowerCase().contains(_searchQuery.toLowerCase()) ||
               episode.podcastName.toLowerCase().contains(_searchQuery.toLowerCase()) ||
               episode.episodeDescription.toLowerCase().contains(_searchQuery.toLowerCase());
      }).toList();
    }
    if (_favoritesOnly) {
      filtered = filtered
          .where((e) =>
              e.podcastId != null && _favoritePodcastIds.contains(e.podcastId))
          .toList();
    }
    _filteredEpisodes = filtered;
    _originalIndex = Map<PinepodsEpisode, int>.identity()
      ..addEntries([for (var i = 0; i < _episodes.length; i++) MapEntry(_episodes[i], i)]);
  }

  Future<void> _loadFavoritePodcastIds(int userId) async {
    try {
      final podcasts = await _pinepodsService.getUserPodcasts(userId);
      if (!mounted) return;
      setState(() {
        _favoritePodcastIds = podcasts
            .where((p) => p.isFavorite && p.id != null)
            .map((p) => p.id!)
            .toSet();
        _filterEpisodes();
      });
    } catch (e) {
      // Non-fatal: favorites filter just won't have data.
    }
  }

  void _toggleFavoritesFilter() {
    setState(() {
      _favoritesOnly = !_favoritesOnly;
      _filterEpisodes();
    });
    if (_favoritesOnly) {
      final userId =
          Provider.of<SettingsBloc>(context, listen: false)
              .currentSettings
              .pinepodsUserId;
      if (userId != null) {
        _loadFavoritePodcastIds(userId);
      }
    }
  }

  String _sortByParam(HistorySortDirection direction) {
    switch (direction) {
      case HistorySortDirection.shortestFirst:
      case HistorySortDirection.longestFirst:
        return 'duration';
      case HistorySortDirection.titleAZ:
      case HistorySortDirection.titleZA:
        return 'title';
      default:
        return 'date';
    }
  }

  String _sortOrderParam(HistorySortDirection direction) {
    switch (direction) {
      case HistorySortDirection.oldestFirst:
      case HistorySortDirection.shortestFirst:
      case HistorySortDirection.titleAZ:
        return 'asc';
      default:
        return 'desc';
    }
  }

  String _filterParam(HistoryFilter filter) {
    switch (filter) {
      case HistoryFilter.completed: return 'completed';
      case HistoryFilter.inProgress: return 'in_progress';
      case HistoryFilter.all: return 'all';
    }
  }

  void _setSortDirection(HistorySortDirection direction) {
    setState(() {
      _sortDirection = direction;
      _episodes = [];
      _filteredEpisodes = [];
      _offset = 0;
      _total = 0;
    });
    _saveSortPreference(direction);
    _loadHistory();
  }

  void _setFilter(HistoryFilter filter) {
    setState(() {
      _activeFilter = filter;
      _episodes = [];
      _filteredEpisodes = [];
      _offset = 0;
      _total = 0;
    });
    _loadHistory();
  }

  void _clearAllFilters() {
    setState(() {
      _activeFilter = HistoryFilter.all;
      _favoritesOnly = false;
      _searchController.clear();
      _searchQuery = '';
      _episodes = [];
      _filteredEpisodes = [];
      _offset = 0;
      _total = 0;
    });
    _loadHistory();
  }

  PinepodsAudioService? get _audioService => GlobalServices.pinepodsAudioService;

  Future<void> _loadHistory() async {
    setState(() {
      _isLoading = true;
      _errorMessage = '';
      _offset = 0;
      _total = 0;
      _episodes = [];
      _filteredEpisodes = [];
    });

    try {
      final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
      final settings = settingsBloc.currentSettings;

      if (settings.pinepodsServer == null ||
          settings.pinepodsApiKey == null ||
          settings.pinepodsUserId == null) {
        setState(() {
          _errorMessage = 'Not connected to PinePods server. Please login first.';
          _isLoading = false;
        });
        return;
      }

      _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
      GlobalServices.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
      final userId = settings.pinepodsUserId!;

      final page = await _pinepodsService.getUserHistoryPaged(
        userId,
        limit: 50,
        offset: 0,
        sortBy: _sortByParam(_sortDirection),
        sortOrder: _sortOrderParam(_sortDirection),
        filter: _filterParam(_activeFilter),
      );

      final enrichedEpisodes = await PositionUtils.enrichEpisodesWithBestPositions(
        context,
        _pinepodsService,
        page.episodes,
        userId,
      );

      setState(() {
        _episodes = enrichedEpisodes;
        _total = page.total;
        _offset = enrichedEpisodes.length;
        _filterEpisodes();
        _isLoading = false;
      });

      await LocalDownloadUtils.loadLocalDownloadStatuses(context, enrichedEpisodes);
      await _loadFavoritePodcastIds(userId);
    } catch (e) {
      setState(() {
        _errorMessage = 'Failed to load listening history: ${e.toString()}';
        _isLoading = false;
      });
    }
  }

  Future<void> _loadMoreEpisodes() async {
    if (_isLoadingMore || _offset >= _total) return;

    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;

    if (settings.pinepodsServer == null ||
        settings.pinepodsApiKey == null ||
        settings.pinepodsUserId == null) {
      return;
    }

    final userId = settings.pinepodsUserId!;
    setState(() => _isLoadingMore = true);

    try {
      final page = await _pinepodsService.getUserHistoryPaged(
        userId,
        limit: 50,
        offset: _offset,
        sortBy: _sortByParam(_sortDirection),
        sortOrder: _sortOrderParam(_sortDirection),
        filter: _filterParam(_activeFilter),
      );

      final enrichedEpisodes = await PositionUtils.enrichEpisodesWithBestPositions(
        context,
        _pinepodsService,
        page.episodes,
        userId,
      );

      setState(() {
        _episodes.addAll(enrichedEpisodes);
        _total = page.total;
        _offset += enrichedEpisodes.length;
        _filterEpisodes();
        _isLoadingMore = false;
      });

      await LocalDownloadUtils.loadLocalDownloadStatuses(context, enrichedEpisodes);
    } catch (e) {
      setState(() => _isLoadingMore = false);
    }
  }

  Future<void> _refresh() async {
    LocalDownloadUtils.clearCache();
    await _loadHistory();
  }

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
      await _audioService!.playPinepodsEpisode(
        pinepodsEpisode: episode,
        resume: episode.isStarted,
      );
    } catch (e) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text('Failed to play episode: ${e.toString()}'),
          backgroundColor: Colors.red,
          duration: const Duration(seconds: 3),
        ),
      );
    }
  }

  Future<void> _showContextMenu(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    final isDownloadedLocally = await LocalDownloadUtils.isEpisodeDownloadedLocally(context, episode);

    if (!mounted) return;

    final pageContext = context;
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
          _toggleQueueEpisode(episodeIndex);
        },
        onMarkComplete: () {
          Navigator.of(context).pop();
          _toggleMarkComplete(episodeIndex);
        },
        onDismiss: () {
          Navigator.of(context).pop();
        },
        onPodcastTap: () {
          Navigator.of(context).pop();
          navigateToPodcastById(
            pageContext,
            episode.podcastId,
            fallbackTitle: episode.podcastName,
            fallbackArtwork: episode.episodeArtwork,
          );
        },
      ),
    );
  }

  Future<void> _localDownloadEpisode(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    
    final success = await LocalDownloadUtils.localDownloadEpisode(context, episode);
    
    if (success) {
      LocalDownloadUtils.showSnackBar(context, 'Episode download started', Colors.green);
    } else {
      LocalDownloadUtils.showSnackBar(context, 'Failed to start download', Colors.red);
    }
  }

  Future<void> _deleteLocalDownload(int episodeIndex) async {
    final episode = _episodes[episodeIndex];
    
    final deletedCount = await LocalDownloadUtils.deleteLocalDownload(context, episode);
    
    if (deletedCount > 0) {
      LocalDownloadUtils.showSnackBar(
        context, 
        'Deleted $deletedCount local download${deletedCount > 1 ? 's' : ''}', 
        Colors.orange
      );
    } else {
      LocalDownloadUtils.showSnackBar(context, 'Local download not found', Colors.red);
    }
  }

  void _hideContextMenu() {
    setState(() {
      _contextMenuEpisodeIndex = null;
    });
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
          _episodes[episodeIndex] = _updateEpisodeProperty(_episodes[episodeIndex], saved: true);
          _filterEpisodes(); // Update filtered list to reflect changes
        });
        _showSnackBar('Episode saved!', Colors.green);
      } else {
        _showSnackBar('Failed to save episode', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error saving episode: $e', Colors.red);
    }

    _hideContextMenu();
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
          _episodes[episodeIndex] = _updateEpisodeProperty(_episodes[episodeIndex], saved: false);
          _filterEpisodes(); // Update filtered list to reflect changes
        });
        _showSnackBar('Removed from saved episodes', Colors.orange);
      } else {
        _showSnackBar('Failed to remove saved episode', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error removing saved episode: $e', Colors.red);
    }

    _hideContextMenu();
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
          _filterEpisodes(); // Update filtered list to reflect changes
        });
        _showSnackBar('Episode download queued!', Colors.green);
      } else {
        _showSnackBar('Failed to queue download', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error downloading episode: $e', Colors.red);
    }

    _hideContextMenu();
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
          _filterEpisodes(); // Update filtered list to reflect changes
        });
        _showSnackBar('Episode deleted from server', Colors.orange);
      } else {
        _showSnackBar('Failed to delete episode', Colors.red);
      }
    } catch (e) {
      _showSnackBar('Error deleting episode: $e', Colors.red);
    }

    _hideContextMenu();
  }

  Future<void> _toggleQueueEpisode(int episodeIndex) async {
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
            _filterEpisodes(); // Update filtered list to reflect changes
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
            _filterEpisodes(); // Update filtered list to reflect changes
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

    _hideContextMenu();
  }

  Future<void> _toggleMarkComplete(int episodeIndex) async {
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
      if (episode.completed) {
        success = await _pinepodsService.markEpisodeUncompleted(
          episode.episodeId,
          userId,
          episode.isYoutube,
        );
        if (success) {
          setState(() {
            _episodes[episodeIndex] = _updateEpisodeProperty(episode, completed: false);
            _filterEpisodes(); // Update filtered list to reflect changes
          });
          _showSnackBar('Marked as incomplete', Colors.orange);
        }
      } else {
        success = await _pinepodsService.markEpisodeCompleted(
          episode.episodeId,
          userId,
          episode.isYoutube,
        );
        if (success) {
          setState(() {
            _episodes[episodeIndex] = _updateEpisodeProperty(episode, completed: true);
            _filterEpisodes(); // Update filtered list to reflect changes
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

    _hideContextMenu();
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
      listenDate: episode.listenDate,
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


  @override
  Widget build(BuildContext context) {
    if (_isLoading) {
      return const SliverFillRemaining(
        child: Center(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              CircularProgressIndicator(),
              SizedBox(height: 16),
              Text('Loading listening history...'),
            ],
          ),
        ),
      );
    }

    if (_errorMessage.isNotEmpty) {
      return SliverServerErrorPage(
        errorMessage: _errorMessage.isServerConnectionError 
          ? null 
          : _errorMessage,
        onRetry: _refresh,
        title: 'History Unavailable',
        subtitle: _errorMessage.isServerConnectionError
          ? 'Unable to connect to the PinePods server'
          : 'Failed to load listening history',
      );
    }

    if (_episodes.isEmpty) {
      return const SliverFillRemaining(
        hasScrollBody: false,
        child: Center(
          child: SoftEmptyState(
            icon: PhosphorIconsRegular.clockCounterClockwise,
            title: 'No listening history',
            body: 'Episodes you listen to will appear here',
          ),
        ),
      );
    }

    return MultiSliver(
      children: [
        SliverToBoxAdapter(
          child: SoftSectionHeader(
            title: 'History',
            count: _filteredEpisodes.length,
            trailing: [
              IconButton(
                tooltip: 'Refresh',
                icon: const Icon(PhosphorIconsRegular.arrowClockwise),
                onPressed: _refresh,
              ),
            ],
          ),
        ),
        ListToolbar.sliver(context, _buildToolbar()),
        _buildEpisodesList(),
      ],
    );
  }

  ListToolbar<HistorySortDirection> _buildToolbar() {
    return ListToolbar<HistorySortDirection>(
      searchController: _searchController,
      searchHint: 'Search episodes',
      filters: [
        ToolbarFilter(
          label: 'All',
          selected: _activeFilter == HistoryFilter.all && !_favoritesOnly,
          onTap: _clearAllFilters,
        ),
        ToolbarFilter(
          label: 'In progress',
          selected: _activeFilter == HistoryFilter.inProgress,
          onTap: () => _setFilter(
              _activeFilter == HistoryFilter.inProgress ? HistoryFilter.all : HistoryFilter.inProgress),
        ),
        ToolbarFilter(
          label: 'Completed',
          selected: _activeFilter == HistoryFilter.completed,
          onTap: () => _setFilter(
              _activeFilter == HistoryFilter.completed ? HistoryFilter.all : HistoryFilter.completed),
        ),
        ToolbarFilter(
          label: 'Favorites',
          icon: _favoritesOnly ? PhosphorIconsFill.star : PhosphorIconsRegular.star,
          selected: _favoritesOnly,
          onTap: _toggleFavoritesFilter,
        ),
      ],
      sortOptions: const {
        HistorySortDirection.newestFirst: 'Newest first',
        HistorySortDirection.oldestFirst: 'Oldest first',
        HistorySortDirection.shortestFirst: 'Shortest first',
        HistorySortDirection.longestFirst: 'Longest first',
        HistorySortDirection.titleAZ: 'Title A–Z',
        HistorySortDirection.titleZA: 'Title Z–A',
      },
      sortValue: _sortDirection,
      onSort: _setSortDirection,
    );
  }

  Widget _buildEpisodesList() {
    final hasActiveSearch = _searchQuery.isNotEmpty;
    if (_filteredEpisodes.isEmpty &&
        !_isLoadingMore &&
        (hasActiveSearch || _favoritesOnly)) {
      return SliverToBoxAdapter(
        child: SoftEmptyState(
          icon: PhosphorIconsRegular.funnelSimpleX,
          title: 'No episodes found',
          body: _getNoResultsMessage(),
          action: TextButton(onPressed: _clearAllFilters, child: const Text('Clear filters')),
        ),
      );
    }

    final showFooter = _isLoadingMore || _offset < _total;

    return SliverList(
      delegate: SliverChildBuilderDelegate(
        (context, index) {
          final episodeIndex = index;

          // Footer: triggers load more when it becomes visible
          if (showFooter && episodeIndex == _filteredEpisodes.length) {
            if (!_isLoadingMore) {
              WidgetsBinding.instance.addPostFrameCallback((_) => _loadMoreEpisodes());
            }
            return const Padding(
              padding: EdgeInsets.symmetric(vertical: 16.0),
              child: Center(child: CircularProgressIndicator()),
            );
          }

          if (episodeIndex >= _filteredEpisodes.length) return null;
          final episode = _filteredEpisodes[episodeIndex];
          final originalIndex = _originalIndex[episode] ?? -1;
          return PinepodsEpisodeCard(
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
            onLongPress: originalIndex >= 0 ? () => _showContextMenu(originalIndex) : null,
            onPlayPressed: () => _playEpisode(episode),
          );
        },
        childCount: _filteredEpisodes.length + (showFooter ? 1 : 0),
      ),
    );
  }

  String _getNoResultsMessage() {
    final parts = <String>[];

    if (_searchQuery.isNotEmpty) {
      parts.add('matching "$_searchQuery"');
    }

    if (_activeFilter == HistoryFilter.completed) {
      parts.add('that are completed');
    } else if (_activeFilter == HistoryFilter.inProgress) {
      parts.add('that are in progress');
    }

    if (parts.isEmpty) {
      return 'No episodes match your filters';
    }

    return 'No episodes ${parts.join(' and ')}';
  }
}