// lib/ui/pinepods/downloads.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/bloc/podcast/podcast_bloc.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/download/download_service.dart';
import 'package:pinepods_mobile/bloc/podcast/episode_bloc.dart';
import 'package:pinepods_mobile/state/bloc_state.dart';
import 'package:pinepods_mobile/ui/widgets/pinepods_episode_card.dart';
import 'package:pinepods_mobile/ui/widgets/episode_tile.dart';
import 'package:pinepods_mobile/ui/widgets/episode_context_menu.dart';
import 'package:pinepods_mobile/ui/widgets/paginated_episode_list.dart';
import 'package:pinepods_mobile/ui/widgets/platform_progress_indicator.dart';
import 'package:pinepods_mobile/services/error_handling_service.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/ui/pinepods/episode_details.dart';
import 'package:provider/provider.dart';
import 'package:logging/logging.dart';
import 'package:shared_preferences/shared_preferences.dart';
import 'package:sliver_tools/sliver_tools.dart';

/// Sort direction options for downloads
enum DownloadSortDirection {
  newestFirst,
  oldestFirst,
  titleAZ,
  titleZA,
}

/// Download type filter options
enum DownloadTypeFilter {
  all,
  serverOnly,
  localOnly,
}

class PinepodsDownloads extends StatefulWidget {
  const PinepodsDownloads({super.key});

  @override
  State<PinepodsDownloads> createState() => _PinepodsDownloadsState();
}

class _PinepodsDownloadsState extends State<PinepodsDownloads> {
  final log = Logger('PinepodsDownloads');
  final PinepodsService _pinepodsService = PinepodsService();
  
  List<PinepodsEpisode> _serverDownloads = [];
  List<Episode> _localDownloads = [];
  Map<String, List<PinepodsEpisode>> _serverDownloadsByPodcast = {};
  Map<String, List<Episode>> _localDownloadsByPodcast = {};
  
  bool _isLoadingServerDownloads = false;
  bool _isLoadingLocalDownloads = false;
  String? _errorMessage;
  
  Set<String> _expandedPodcasts = {};
  int? _contextMenuEpisodeIndex;
  bool _isServerEpisode = false;
  
  // Search functionality
  final TextEditingController _searchController = TextEditingController();
  String _searchQuery = '';
  Map<String, List<PinepodsEpisode>> _filteredServerDownloadsByPodcast = {};
  Map<String, List<Episode>> _filteredLocalDownloadsByPodcast = {};

  // Sort and filter state
  DownloadSortDirection _sortDirection = DownloadSortDirection.newestFirst;
  DownloadTypeFilter _typeFilter = DownloadTypeFilter.all;
  static const String _sortPreferenceKey = 'downloads_sort_direction';

  @override
  void initState() {
    super.initState();
    _loadSortPreference();
    _loadDownloads();
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

  Future<void> _saveSortPreference(DownloadSortDirection direction) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_sortPreferenceKey, _sortDirectionToString(direction));
  }

  String _sortDirectionToString(DownloadSortDirection direction) {
    switch (direction) {
      case DownloadSortDirection.newestFirst:
        return 'newest';
      case DownloadSortDirection.oldestFirst:
        return 'oldest';
      case DownloadSortDirection.titleAZ:
        return 'title_az';
      case DownloadSortDirection.titleZA:
        return 'title_za';
    }
  }

  DownloadSortDirection _sortDirectionFromString(String value) {
    switch (value) {
      case 'oldest':
        return DownloadSortDirection.oldestFirst;
      case 'title_az':
        return DownloadSortDirection.titleAZ;
      case 'title_za':
        return DownloadSortDirection.titleZA;
      case 'newest':
      default:
        return DownloadSortDirection.newestFirst;
    }
  }

  void _setSortDirection(DownloadSortDirection direction) {
    setState(() {
      _sortDirection = direction;
      _filterDownloads();
    });
    _saveSortPreference(direction);
  }

  void _setTypeFilter(DownloadTypeFilter filter) {
    setState(() {
      _typeFilter = filter;
    });
  }

  void _clearAllFilters() {
    setState(() {
      _typeFilter = DownloadTypeFilter.all;
      _searchController.clear();
      _searchQuery = '';
      _filterDownloads();
    });
  }

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }

  void _onSearchChanged() {
    setState(() {
      _searchQuery = _searchController.text;
      _filterDownloads();
    });
  }

  void _filterDownloads() {
    // Filter server downloads
    _filteredServerDownloadsByPodcast = {};
    for (final entry in _serverDownloadsByPodcast.entries) {
      final podcastName = entry.key;
      final episodes = entry.value;

      List<PinepodsEpisode> filtered;
      if (_searchQuery.isEmpty) {
        filtered = List.from(episodes);
      } else {
        filtered = episodes.where((episode) {
          return episode.episodeTitle.toLowerCase().contains(_searchQuery.toLowerCase()) ||
                 episode.podcastName.toLowerCase().contains(_searchQuery.toLowerCase());
        }).toList();
      }

      // Apply sorting
      filtered.sort((a, b) {
        switch (_sortDirection) {
          case DownloadSortDirection.newestFirst:
            return _compareDates(b.episodePubDate, a.episodePubDate);
          case DownloadSortDirection.oldestFirst:
            return _compareDates(a.episodePubDate, b.episodePubDate);
          case DownloadSortDirection.titleAZ:
            return a.episodeTitle.toLowerCase().compareTo(b.episodeTitle.toLowerCase());
          case DownloadSortDirection.titleZA:
            return b.episodeTitle.toLowerCase().compareTo(a.episodeTitle.toLowerCase());
        }
      });

      if (filtered.isNotEmpty) {
        _filteredServerDownloadsByPodcast[podcastName] = filtered;
      }
    }

    // Filter local downloads (will be called when local downloads are loaded)
    _filterLocalDownloads();
  }

  int _compareDates(String dateA, String dateB) {
    final a = DateTime.tryParse(dateA) ?? DateTime(1970);
    final b = DateTime.tryParse(dateB) ?? DateTime(1970);
    return a.compareTo(b);
  }

  int _compareLocalDates(DateTime? dateA, DateTime? dateB) {
    final a = dateA ?? DateTime(1970);
    final b = dateB ?? DateTime(1970);
    return a.compareTo(b);
  }

  void _filterLocalDownloads([Map<String, List<Episode>>? localDownloadsByPodcast]) {
    final downloadsToFilter = localDownloadsByPodcast ?? _localDownloadsByPodcast;
    _filteredLocalDownloadsByPodcast = {};

    for (final entry in downloadsToFilter.entries) {
      final podcastName = entry.key;
      final episodes = entry.value;

      List<Episode> filtered;
      if (_searchQuery.isEmpty) {
        filtered = List.from(episodes);
      } else {
        filtered = episodes.where((episode) {
          return (episode.title ?? '').toLowerCase().contains(_searchQuery.toLowerCase()) ||
                 (episode.podcast ?? '').toLowerCase().contains(_searchQuery.toLowerCase());
        }).toList();
      }

      // Apply sorting
      filtered.sort((a, b) {
        switch (_sortDirection) {
          case DownloadSortDirection.newestFirst:
            return _compareLocalDates(b.publicationDate, a.publicationDate);
          case DownloadSortDirection.oldestFirst:
            return _compareLocalDates(a.publicationDate, b.publicationDate);
          case DownloadSortDirection.titleAZ:
            return (a.title ?? '').toLowerCase().compareTo((b.title ?? '').toLowerCase());
          case DownloadSortDirection.titleZA:
            return (b.title ?? '').toLowerCase().compareTo((a.title ?? '').toLowerCase());
        }
      });

      if (filtered.isNotEmpty) {
        _filteredLocalDownloadsByPodcast[podcastName] = filtered;
      }
    }
  }

  Future<void> _loadDownloads() async {
    await Future.wait([
      _loadServerDownloads(),
      _loadLocalDownloads(),
    ]);
  }

  Future<void> _loadServerDownloads() async {
    setState(() {
      _isLoadingServerDownloads = true;
      _errorMessage = null;
    });

    try {
      final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
      final settings = settingsBloc.currentSettings;
      
      if (settings.pinepodsServer != null && 
          settings.pinepodsApiKey != null && 
          settings.pinepodsUserId != null) {
        
        _pinepodsService.setCredentials(
          settings.pinepodsServer!,
          settings.pinepodsApiKey!,
        );
        
        final downloads = await _pinepodsService.getServerDownloads(settings.pinepodsUserId!);
        
        setState(() {
          _serverDownloads = downloads;
          _serverDownloadsByPodcast = _groupEpisodesByPodcast(downloads);
          _filterDownloads(); // Initialize filtered data
          _isLoadingServerDownloads = false;
        });
      } else {
        setState(() {
          _isLoadingServerDownloads = false;
        });
      }
    } catch (e) {
      log.severe('Error loading server downloads: $e');
      setState(() {
        _errorMessage = 'Failed to load server downloads: $e';
        _isLoadingServerDownloads = false;
      });
    }
  }

  Future<void> _loadLocalDownloads() async {
    setState(() {
      _isLoadingLocalDownloads = true;
    });

    try {
      final episodeBloc = Provider.of<EpisodeBloc>(context, listen: false);
      episodeBloc.fetchDownloads(false);
      
      // Debug: Let's also directly check what the repository returns
      final podcastBloc = Provider.of<PodcastBloc>(context, listen: false);
      final directDownloads = await podcastBloc.podcastService.loadDownloads();
      print('DEBUG: Direct downloads from repository: ${directDownloads.length} episodes');
      for (var episode in directDownloads) {
        print('DEBUG: Episode: ${episode.title}, GUID: ${episode.guid}, Downloaded: ${episode.downloaded}, Percentage: ${episode.downloadPercentage}');
      }
      
      setState(() {
        _isLoadingLocalDownloads = false;
      });
    } catch (e) {
      log.severe('Error loading local downloads: $e');
      setState(() {
        _isLoadingLocalDownloads = false;
      });
    }
  }

  Map<String, List<PinepodsEpisode>> _groupEpisodesByPodcast(List<PinepodsEpisode> episodes) {
    final grouped = <String, List<PinepodsEpisode>>{};
    
    for (final episode in episodes) {
      final podcastName = episode.podcastName;
      if (!grouped.containsKey(podcastName)) {
        grouped[podcastName] = [];
      }
      grouped[podcastName]!.add(episode);
    }
    
    // Sort episodes within each podcast by publication date (newest first)
    for (final episodes in grouped.values) {
      episodes.sort((a, b) {
        try {
          final dateA = DateTime.parse(a.episodePubDate);
          final dateB = DateTime.parse(b.episodePubDate);
          return dateB.compareTo(dateA); // newest first
        } catch (e) {
          return 0;
        }
      });
    }
    
    return grouped;
  }

  Map<String, List<Episode>> _groupLocalEpisodesByPodcast(List<Episode> episodes) {
    final grouped = <String, List<Episode>>{};
    
    for (final episode in episodes) {
      final podcastName = episode.podcast ?? 'Unknown Podcast';
      if (!grouped.containsKey(podcastName)) {
        grouped[podcastName] = [];
      }
      grouped[podcastName]!.add(episode);
    }
    
    // Sort episodes within each podcast by publication date (newest first)
    for (final episodes in grouped.values) {
      episodes.sort((a, b) {
        if (a.publicationDate == null || b.publicationDate == null) {
          return 0;
        }
        return b.publicationDate!.compareTo(a.publicationDate!);
      });
    }
    
    return grouped;
  }

  void _togglePodcastExpansion(String podcastKey) {
    setState(() {
      if (_expandedPodcasts.contains(podcastKey)) {
        _expandedPodcasts.remove(podcastKey);
      } else {
        _expandedPodcasts.add(podcastKey);
      }
    });
  }

  Future<void> _handleServerEpisodeDelete(PinepodsEpisode episode) async {
    try {
      final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
      final settings = settingsBloc.currentSettings;
      
      if (settings.pinepodsUserId != null) {
        final success = await _pinepodsService.deleteEpisode(
          episode.episodeId,
          settings.pinepodsUserId!,
          episode.isYoutube,
        );
        
        if (success) {
          // Remove from local state
          setState(() {
            _serverDownloads.removeWhere((e) => e.episodeId == episode.episodeId);
            _serverDownloadsByPodcast = _groupEpisodesByPodcast(_serverDownloads);
            _filterDownloads(); // Update filtered lists after removal
          });
        } else {
          _showErrorSnackBar('Failed to delete episode from server');
        }
      }
    } catch (e) {
      log.severe('Error deleting server episode: $e');
      _showErrorSnackBar('Error deleting episode: $e');
    }
  }

  void _handleLocalEpisodeDelete(Episode episode) {
    final episodeBloc = Provider.of<EpisodeBloc>(context, listen: false);
    episodeBloc.deleteDownload(episode);
    
    // The episode bloc will automatically update the downloads stream
    // which will trigger a UI refresh
  }

  void _showContextMenu(int episodeIndex, bool isServerEpisode) {
    setState(() {
      _contextMenuEpisodeIndex = episodeIndex;
      _isServerEpisode = isServerEpisode;
    });
  }

  void _hideContextMenu() {
    setState(() {
      _contextMenuEpisodeIndex = null;
      _isServerEpisode = false;
    });
  }

  Future<void> _localDownloadServerEpisode(int episodeIndex) async {
    final episode = _serverDownloads[episodeIndex];
    
    try {
      // Convert PinepodsEpisode to Episode for local download
      final localEpisode = Episode(
        guid: 'pinepods_${episode.episodeId}_${DateTime.now().millisecondsSinceEpoch}',
        pguid: 'pinepods_${episode.podcastName.replaceAll(' ', '_').toLowerCase()}',
        podcast: episode.podcastName,
        title: episode.episodeTitle,
        description: episode.episodeDescription,
        imageUrl: episode.episodeArtwork,
        contentUrl: episode.episodeUrl,
        duration: episode.episodeDuration,
        publicationDate: DateTime.tryParse(episode.episodePubDate),
        author: episode.podcastName,
        season: 0,
        episode: 0,
        position: episode.listenDuration ?? 0,
        played: episode.completed,
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

  void _showErrorSnackBar(String message) {
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(message),
        backgroundColor: Colors.red,
      ),
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

  Widget _buildPodcastDropdown(String podcastKey, List<dynamic> episodes, {bool isServerDownload = false, String? displayName}) {
    final isExpanded = _expandedPodcasts.contains(podcastKey);
    final title = displayName ?? podcastKey;
    
    return Card(
      margin: const EdgeInsets.symmetric(horizontal: 12.0, vertical: 4.0),
      child: Column(
        children: [
          ListTile(
            leading: Icon(
              isServerDownload ? Icons.cloud_download : Icons.file_download,
              color: isServerDownload ? Colors.blue : Colors.green,
            ),
            title: Text(
              title,
              style: const TextStyle(fontWeight: FontWeight.bold),
            ),
            subtitle: Text(
              '${episodes.length} episode${episodes.length != 1 ? 's' : ''}'  +
              (episodes.length > 20 ? ' (showing 20 at a time)' : '')
            ),
            trailing: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                if (episodes.length > 20)
                  Container(
                    padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                    decoration: BoxDecoration(
                      color: Colors.orange[100],
                      borderRadius: BorderRadius.circular(8),
                    ),
                    child: Text(
                      'Large',
                      style: TextStyle(
                        fontSize: 10,
                        color: Colors.orange[800],
                        fontWeight: FontWeight.w500,
                      ),
                    ),
                  ),
                const SizedBox(width: 8),
                Icon(
                  isExpanded ? Icons.expand_less : Icons.expand_more,
                ),
              ],
            ),
            onTap: () => _togglePodcastExpansion(podcastKey),
          ),
          if (isExpanded)
            PaginatedEpisodeList(
              episodes: episodes,
              isServerEpisodes: isServerDownload,
              onEpisodeTap: isServerDownload 
                ? (episode) {
                    Navigator.push(
                      context,
                      MaterialPageRoute(
                        builder: (context) => PinepodsEpisodeDetails(
                          initialEpisode: episode,
                        ),
                      ),
                    );
                  }
                : null,
              onEpisodeLongPress: isServerDownload
                ? (episode, globalIndex) {
                    // Find the index in the full _serverDownloads list
                    final serverIndex = _serverDownloads.indexWhere((e) => e.episodeId == episode.episodeId);
                    _showContextMenu(serverIndex >= 0 ? serverIndex : globalIndex, true);
                  }
                : null,
              onPlayPressed: isServerDownload
                ? (episode) => _playServerEpisode(episode)
                : (episode) => _playLocalEpisode(episode),
            ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final episodeBloc = Provider.of<EpisodeBloc>(context);
    
    // Show context menu as a modal overlay if needed
    if (_contextMenuEpisodeIndex != null) {
      final episodeIndex = _contextMenuEpisodeIndex!;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (_isServerEpisode) {
          // Show server episode context menu
          showDialog(
            context: context,
            barrierColor: Colors.black.withOpacity(0.3),
            builder: (context) => EpisodeContextMenu(
              episode: _serverDownloads[episodeIndex],
              onDownload: () {
                Navigator.of(context).pop();
                _handleServerEpisodeDelete(_serverDownloads[episodeIndex]);
                _hideContextMenu();
              },
              onLocalDownload: () {
                Navigator.of(context).pop();
                _localDownloadServerEpisode(episodeIndex);
              },
              onDismiss: () {
                Navigator.of(context).pop();
                _hideContextMenu();
              },
            ),
          );
        }
      });
      // Reset the context menu index after storing it locally
      _contextMenuEpisodeIndex = null;
    }
    
    return StreamBuilder<BlocState>(
      stream: episodeBloc.downloads,
      builder: (context, snapshot) {
        final localDownloadsState = snapshot.data;
        List<Episode> currentLocalDownloads = [];
        Map<String, List<Episode>> currentLocalDownloadsByPodcast = {};
        
        if (localDownloadsState is BlocPopulatedState<List<Episode>>) {
          currentLocalDownloads = localDownloadsState.results ?? [];
          currentLocalDownloadsByPodcast = _groupLocalEpisodesByPodcast(currentLocalDownloads);
        }
        
        final isLoading = _isLoadingServerDownloads || 
                         _isLoadingLocalDownloads ||
                         (localDownloadsState is BlocLoadingState);
        
        if (isLoading) {
          return const SliverFillRemaining(
            hasScrollBody: false,
            child: Center(child: PlatformProgressIndicator()),
          );
        }
        
        // Update filtered local downloads when local downloads change
        _filterLocalDownloads(currentLocalDownloadsByPodcast);
        
        if (_errorMessage != null) {
          // Check if this is a server connection error - show offline mode for downloads
          if (_errorMessage!.isServerConnectionError) {
            // Show offline downloads only with special UI
            return _buildOfflineDownloadsView(_filteredLocalDownloadsByPodcast);
          } else {
            return SliverFillRemaining(
              hasScrollBody: false,
              child: Center(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Icon(
                      Icons.error_outline,
                      size: 64,
                      color: Colors.red[300],
                    ),
                    const SizedBox(height: 16),
                    Text(
                      _errorMessage!.userFriendlyMessage,
                      textAlign: TextAlign.center,
                      style: Theme.of(context).textTheme.bodyLarge,
                    ),
                    const SizedBox(height: 16),
                    ElevatedButton(
                      onPressed: _loadDownloads,
                      child: const Text('Retry'),
                    ),
                  ],
                ),
              ),
            );
          }
        }
        
        // Check if there are downloads to show based on type filter
        final showLocal = _typeFilter == DownloadTypeFilter.all || _typeFilter == DownloadTypeFilter.localOnly;
        final showServer = _typeFilter == DownloadTypeFilter.all || _typeFilter == DownloadTypeFilter.serverOnly;
        final hasVisibleLocal = showLocal && _filteredLocalDownloadsByPodcast.isNotEmpty;
        final hasVisibleServer = showServer && _filteredServerDownloadsByPodcast.isNotEmpty;
        final hasActiveFilters = _searchQuery.isNotEmpty || _typeFilter != DownloadTypeFilter.all;

        if (!hasVisibleLocal && !hasVisibleServer) {
          if (hasActiveFilters) {
            // Show no results with filters message
            return MultiSliver(
              children: [
                _buildSearchAndFilterBar(),
                SliverFillRemaining(
                  hasScrollBody: false,
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
                          'No downloads found',
                          style: Theme.of(context).textTheme.headlineSmall,
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
                ),
              ],
            );
          } else {
            // Show empty downloads message
            return SliverFillRemaining(
              hasScrollBody: false,
              child: Center(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Icon(
                      Icons.download_outlined,
                      size: 64,
                      color: Colors.grey[400],
                    ),
                    const SizedBox(height: 16),
                    Text(
                      'No downloads found',
                      style: Theme.of(context).textTheme.headlineSmall,
                    ),
                    const SizedBox(height: 8),
                    Text(
                      'Downloaded episodes will appear here',
                      style: Theme.of(context).textTheme.bodyMedium,
                    ),
                  ],
                ),
              ),
            );
          }
        }
        
        return MultiSliver(
          children: [
            _buildSearchAndFilterBar(),
            _buildDownloadsList(),
          ],
        );
      },
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
                      hintText: 'Search downloads...',
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
                    child: DropdownButton<DownloadSortDirection>(
                      value: _sortDirection,
                      icon: const Icon(Icons.sort),
                      items: const [
                        DropdownMenuItem(
                          value: DownloadSortDirection.newestFirst,
                          child: Text('Newest'),
                        ),
                        DropdownMenuItem(
                          value: DownloadSortDirection.oldestFirst,
                          child: Text('Oldest'),
                        ),
                        DropdownMenuItem(
                          value: DownloadSortDirection.titleAZ,
                          child: Text('Title A-Z'),
                        ),
                        DropdownMenuItem(
                          value: DownloadSortDirection.titleZA,
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
                  // Server downloads chip
                  _buildFilterChip(
                    label: 'Server',
                    icon: Icons.cloud_download,
                    isActive: _typeFilter == DownloadTypeFilter.serverOnly,
                    onTap: () {
                      _setTypeFilter(_typeFilter == DownloadTypeFilter.serverOnly
                          ? DownloadTypeFilter.all
                          : DownloadTypeFilter.serverOnly);
                    },
                  ),
                  const SizedBox(width: 8),
                  // Local downloads chip
                  _buildFilterChip(
                    label: 'Local',
                    icon: Icons.smartphone,
                    isActive: _typeFilter == DownloadTypeFilter.localOnly,
                    onTap: () {
                      _setTypeFilter(_typeFilter == DownloadTypeFilter.localOnly
                          ? DownloadTypeFilter.all
                          : DownloadTypeFilter.localOnly);
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

  Widget _buildDownloadsList() {
    final showLocal = _typeFilter == DownloadTypeFilter.all || _typeFilter == DownloadTypeFilter.localOnly;
    final showServer = _typeFilter == DownloadTypeFilter.all || _typeFilter == DownloadTypeFilter.serverOnly;

    return SliverList(
      delegate: SliverChildListDelegate([
        // Local Downloads Section
        if (showLocal && _filteredLocalDownloadsByPodcast.isNotEmpty) ...[
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
            child: Row(
              children: [
                Icon(Icons.smartphone, color: Colors.green[600]),
                const SizedBox(width: 8),
                Text(
                  'Local Downloads (${_countFilteredEpisodes(_filteredLocalDownloadsByPodcast)})',
                  style: Theme.of(context).textTheme.titleLarge?.copyWith(
                    fontWeight: FontWeight.bold,
                    color: Colors.green[600],
                  ),
                ),
              ],
            ),
          ),

          ..._filteredLocalDownloadsByPodcast.entries.map((entry) {
            final podcastName = entry.key;
            final episodes = entry.value;
            final podcastKey = 'local_$podcastName';

            return _buildPodcastDropdown(
              podcastKey,
              episodes,
              isServerDownload: false,
              displayName: podcastName,
            );
          }).toList(),
        ],

        // Server Downloads Section
        if (showServer && _filteredServerDownloadsByPodcast.isNotEmpty) ...[
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 24, 16, 8),
            child: Row(
              children: [
                Icon(Icons.cloud_download, color: Colors.blue[600]),
                const SizedBox(width: 8),
                Text(
                  'Server Downloads (${_countFilteredEpisodes(_filteredServerDownloadsByPodcast)})',
                  style: Theme.of(context).textTheme.titleLarge?.copyWith(
                    fontWeight: FontWeight.bold,
                    color: Colors.blue[600],
                  ),
                ),
              ],
            ),
          ),

          ..._filteredServerDownloadsByPodcast.entries.map((entry) {
            final podcastName = entry.key;
            final episodes = entry.value;
            final podcastKey = 'server_$podcastName';

            return _buildPodcastDropdown(
              podcastKey,
              episodes,
              isServerDownload: true,
              displayName: podcastName,
            );
          }).toList(),
        ],

        // Bottom padding
        const SizedBox(height: 100),
      ]),
    );
  }

  int _countFilteredEpisodes(Map<String, List<dynamic>> downloadsByPodcast) {
    return downloadsByPodcast.values.fold(0, (sum, episodes) => sum + episodes.length);
  }

  String _getNoResultsMessage() {
    final parts = <String>[];

    if (_searchQuery.isNotEmpty) {
      parts.add('matching "$_searchQuery"');
    }

    if (_typeFilter == DownloadTypeFilter.serverOnly) {
      parts.add('in server downloads');
    } else if (_typeFilter == DownloadTypeFilter.localOnly) {
      parts.add('in local downloads');
    }

    if (parts.isEmpty) {
      return 'No downloads match your filters';
    }

    return 'No downloads ${parts.join(' ')}';
  }

  void _playServerEpisode(PinepodsEpisode episode) {
    // TODO: Implement server episode playback
    // This would involve getting the stream URL from the server
    // and playing it through the audio service
    log.info('Playing server episode: ${episode.episodeTitle}');
    
    _showErrorSnackBar('Server episode playback not yet implemented');
  }

  Future<void> _playLocalEpisode(Episode episode) async {
    try {
      log.info('Playing local episode: ${episode.title}');
      
      final audioPlayerService = Provider.of<AudioPlayerService>(context, listen: false);
      
      // Use the regular audio player service for offline playback
      // This bypasses the PinePods service and server dependencies
      await audioPlayerService.playEpisode(episode: episode, resume: true);
      
      log.info('Successfully started local episode playback');
    } catch (e) {
      log.severe('Error playing local episode: $e');
      _showErrorSnackBar('Failed to play episode: $e');
    }
  }

  Widget _buildOfflinePodcastDropdown(String podcastKey, List<Episode> episodes, {String? displayName}) {
    final isExpanded = _expandedPodcasts.contains(podcastKey);
    final title = displayName ?? podcastKey;
    
    return Card(
      margin: const EdgeInsets.symmetric(horizontal: 12.0, vertical: 4.0),
      child: Column(
        children: [
          ListTile(
            leading: Icon(
              Icons.offline_pin,
              color: Colors.green[700],
            ),
            title: Text(
              title,
              style: const TextStyle(fontWeight: FontWeight.bold),
            ),
            subtitle: Text(
              '${episodes.length} episode${episodes.length != 1 ? 's' : ''} available offline'
            ),
            trailing: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Container(
                  padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                  decoration: BoxDecoration(
                    color: Colors.green[100],
                    borderRadius: BorderRadius.circular(8),
                  ),
                  child: Text(
                    'Offline',
                    style: TextStyle(
                      fontSize: 10,
                      color: Colors.green[700],
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                ),
                const SizedBox(width: 8),
                Icon(
                  isExpanded ? Icons.expand_less : Icons.expand_more,
                ),
              ],
            ),
            onTap: () => _togglePodcastExpansion(podcastKey),
          ),
          if (isExpanded)
            PaginatedEpisodeList(
              episodes: episodes,
              isServerEpisodes: false,
              isOfflineMode: true,
              onPlayPressed: (episode) => _playLocalEpisode(episode),
            ),
        ],
      ),
    );
  }

  Widget _buildOfflineDownloadsView(Map<String, List<Episode>> localDownloadsByPodcast) {
    return MultiSliver(
      children: [
        // Offline banner
        SliverToBoxAdapter(
          child: Container(
            width: double.infinity,
            padding: const EdgeInsets.all(16.0),
            margin: const EdgeInsets.all(12.0),
            decoration: BoxDecoration(
              color: Colors.orange[100],
              border: Border.all(color: Colors.orange[300]!),
              borderRadius: BorderRadius.circular(8),
            ),
            child: Row(
              children: [
                Icon(
                  Icons.cloud_off,
                  color: Colors.orange[800],
                  size: 24,
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        'Offline Mode',
                        style: TextStyle(
                          fontWeight: FontWeight.bold,
                          color: Colors.orange[800],
                          fontSize: 16,
                        ),
                      ),
                      const SizedBox(height: 4),
                      Text(
                        'Server unavailable. Showing local downloads only.',
                        style: TextStyle(
                          color: Colors.orange[700],
                          fontSize: 14,
                        ),
                      ),
                    ],
                  ),
                ),
                const SizedBox(width: 12),
                ElevatedButton.icon(
                  onPressed: () {
                    setState(() {
                      _errorMessage = null;
                    });
                    _loadDownloads();
                  },
                  icon: Icon(
                    Icons.refresh,
                    size: 16,
                    color: Colors.orange[800],
                  ),
                  label: Text(
                    'Retry',
                    style: TextStyle(
                      color: Colors.orange[800],
                      fontSize: 12,
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                  style: ElevatedButton.styleFrom(
                    backgroundColor: Colors.orange[50],
                    elevation: 0,
                    padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
                    minimumSize: Size.zero,
                    tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                  ),
                ),
              ],
            ),
          ),
        ),
        
        // Search bar for filtering local downloads
        _buildSearchAndFilterBar(),
        
        // Local downloads content
        if (localDownloadsByPodcast.isEmpty)
          SliverFillRemaining(
            hasScrollBody: false,
            child: Center(
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Icon(
                    Icons.cloud_off,
                    size: 64,
                    color: Colors.grey[400],
                  ),
                  const SizedBox(height: 16),
                  Text(
                    'No local downloads',
                    style: Theme.of(context).textTheme.headlineSmall,
                  ),
                  const SizedBox(height: 8),
                  Text(
                    'Download episodes while online to access them here',
                    style: Theme.of(context).textTheme.bodyMedium,
                    textAlign: TextAlign.center,
                  ),
                ],
              ),
            ),
          )
        else
          SliverList(
            delegate: SliverChildListDelegate([
              // Local downloads header
              Padding(
                padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
                child: Row(
                  children: [
                    Icon(Icons.smartphone, color: Colors.green[600]),
                    const SizedBox(width: 8),
                    Text(
                      _searchQuery.isEmpty 
                          ? 'Local Downloads' 
                          : 'Local Downloads (${_countFilteredEpisodes(localDownloadsByPodcast)})',
                      style: Theme.of(context).textTheme.titleLarge?.copyWith(
                        fontWeight: FontWeight.bold,
                        color: Colors.green[600],
                      ),
                    ),
                  ],
                ),
              ),
              
              // Local downloads by podcast
              ...localDownloadsByPodcast.entries.map((entry) {
                final podcastName = entry.key;
                final episodes = entry.value;
                final podcastKey = 'offline_local_$podcastName';
                
                return _buildOfflinePodcastDropdown(
                  podcastKey,
                  episodes,
                  displayName: podcastName,
                );
              }).toList(),
              
              // Bottom padding
              const SizedBox(height: 100),
            ]),
          ),
      ],
    );
  }
}