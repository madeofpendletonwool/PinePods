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
        child: Center(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Icon(
                Icons.history,
                size: 64,
                color: Colors.grey,
              ),
              SizedBox(height: 16),
              Text(
                'No listening history',
                style: TextStyle(
                  fontSize: 18,
                  color: Colors.grey,
                ),
              ),
              SizedBox(height: 8),
              Text(
                'Episodes you listen to will appear here',
                style: TextStyle(
                  color: Colors.grey,
                ),
                textAlign: TextAlign.center,
              ),
            ],
          ),
        ),
      );
    }

    return MultiSliver(
      children: [
        _buildSearchAndFilterBar(),
        _buildEpisodesList(),
      ],
    );
  }

  Widget _buildSearchAndFilterBar() {
    return SliverToBoxAdapter(
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            // Search and Sort row
            Row(
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
                const SizedBox(width: 12),
                // Sort dropdown
                Container(
                  decoration: BoxDecoration(
                    color: Theme.of(context).cardColor,
                    borderRadius: BorderRadius.circular(12),
                    border: Border.all(
                      color: Theme.of(context).dividerColor,
                    ),
                  ),
                  padding: const EdgeInsets.symmetric(horizontal: 12),
                  child: DropdownButtonHideUnderline(
                    child: DropdownButton<HistorySortDirection>(
                      value: _sortDirection,
                      icon: const Icon(Icons.sort),
                      items: const [
                        DropdownMenuItem(
                          value: HistorySortDirection.newestFirst,
                          child: Text('Newest'),
                        ),
                        DropdownMenuItem(
                          value: HistorySortDirection.oldestFirst,
                          child: Text('Oldest'),
                        ),
                        DropdownMenuItem(
                          value: HistorySortDirection.shortestFirst,
                          child: Text('Shortest'),
                        ),
                        DropdownMenuItem(
                          value: HistorySortDirection.longestFirst,
                          child: Text('Longest'),
                        ),
                        DropdownMenuItem(
                          value: HistorySortDirection.titleAZ,
                          child: Text('Title A-Z'),
                        ),
                        DropdownMenuItem(
                          value: HistorySortDirection.titleZA,
                          child: Text('Title Z-A'),
                        ),
                      ],
                      onChanged: (value) {
                        if (value != null) {
                          _setSortDirection(value);
                        }
                      },
                    ),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 12),
            // Filter chips
            SingleChildScrollView(
              scrollDirection: Axis.horizontal,
              child: Row(
                children: [
                  // Clear all chip
                  _buildFilterChip(
                    label: 'Clear All',
                    icon: Icons.clear_all,
                    isActive: false,
                    onTap: _clearAllFilters,
                  ),
                  const SizedBox(width: 8),
                  // Completed chip
                  _buildFilterChip(
                    label: 'Completed',
                    icon: Icons.check_circle_outline,
                    isActive: _activeFilter == HistoryFilter.completed,
                    onTap: () {
                      _setFilter(_activeFilter == HistoryFilter.completed
                          ? HistoryFilter.all
                          : HistoryFilter.completed);
                    },
                  ),
                  const SizedBox(width: 8),
                  // In Progress chip
                  _buildFilterChip(
                    label: 'In Progress',
                    icon: Icons.hourglass_bottom,
                    isActive: _activeFilter == HistoryFilter.inProgress,
                    onTap: () {
                      _setFilter(_activeFilter == HistoryFilter.inProgress
                          ? HistoryFilter.all
                          : HistoryFilter.inProgress);
                    },
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildFilterChip({
    required String label,
    required IconData icon,
    required bool isActive,
    required VoidCallback onTap,
  }) {
    final theme = Theme.of(context);
    return Material(
      color: isActive ? theme.primaryColor : theme.cardColor,
      borderRadius: BorderRadius.circular(20),
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(20),
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(20),
            border: Border.all(
              color: isActive ? theme.primaryColor : theme.dividerColor,
            ),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(
                icon,
                size: 18,
                color: isActive ? Colors.white : theme.iconTheme.color,
              ),
              const SizedBox(width: 6),
              Text(
                label,
                style: TextStyle(
                  color: isActive ? Colors.white : theme.textTheme.bodyMedium?.color,
                  fontWeight: FontWeight.w500,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildEpisodesList() {
    final hasActiveSearch = _searchQuery.isNotEmpty;
    if (_filteredEpisodes.isEmpty &&
        !_isLoadingMore &&
        (hasActiveSearch || _favoritesOnly)) {
      return SliverFillRemaining(
        child: Center(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Icon(
                Icons.filter_list_off,
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
                _getNoResultsMessage(),
                style: Theme.of(context).textTheme.bodyMedium,
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 16),
              TextButton.icon(
                onPressed: _clearAllFilters,
                icon: const Icon(Icons.clear_all),
                label: const Text('Clear Filters'),
              ),
            ],
          ),
        ),
      );
    }

    final showFooter = _isLoadingMore || _offset < _total;

    return SliverList(
      delegate: SliverChildBuilderDelegate(
        (context, index) {
          if (index == 0) {
            return Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16.0, vertical: 8.0),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Text(
                    _getHeaderTitle(),
                    style: const TextStyle(
                      fontSize: 20,
                      fontWeight: FontWeight.bold,
                    ),
                  ),
                  Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      IconButton(
                        icon: Icon(
                          _favoritesOnly ? Icons.star : Icons.star_border,
                          color: _favoritesOnly
                              ? Colors.amber
                              : Theme.of(context).iconTheme.color,
                        ),
                        tooltip: _favoritesOnly
                            ? 'Show all podcasts'
                            : 'Show favorites only',
                        onPressed: _toggleFavoritesFilter,
                      ),
                      IconButton(
                        icon: const Icon(Icons.refresh),
                        onPressed: _refresh,
                      ),
                    ],
                  ),
                ],
              ),
            );
          }

          final episodeIndex = index - 1;

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
          final originalIndex = _episodes.indexOf(episode);
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
        childCount: _filteredEpisodes.length + 1 + (showFooter ? 1 : 0),
      ),
    );
  }

  String _getHeaderTitle() {
    final count = _filteredEpisodes.length;
    final hasFilters = _searchQuery.isNotEmpty || _activeFilter != HistoryFilter.all;

    if (hasFilters) {
      return 'Results ($count)';
    }
    return 'Listening History ($count)';
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