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

/// Per-podcast lazy-load state for server downloads
class _PodcastPageState {
  List<PinepodsEpisode> episodes;
  int offset;
  int total;
  bool loadingMore;

  _PodcastPageState({
    this.episodes = const [],
    this.offset = 0,
    this.total = 0,
    this.loadingMore = false,
  });
}

class PinepodsDownloads extends StatefulWidget {
  const PinepodsDownloads({super.key});

  @override
  State<PinepodsDownloads> createState() => _PinepodsDownloadsState();
}

class _PinepodsDownloadsState extends State<PinepodsDownloads> {
  static const int _pageSize = 50;

  final log = Logger('PinepodsDownloads');
  final PinepodsService _pinepodsService = PinepodsService();

  // Server side: podcast summaries + per-podcast lazy state
  List<PodcastDownloadSummary> _serverPodcastSummaries = [];
  final Map<int, _PodcastPageState> _podcastPageState = {};

  // Local downloads (unchanged)
  List<Episode> _localDownloads = [];
  Map<String, List<Episode>> _localDownloadsByPodcast = {};

  bool _isLoadingServerSummary = false;
  bool _isLoadingLocalDownloads = false;
  String? _errorMessage;

  Set<String> _expandedPodcasts = {};
  int? _contextMenuEpisodeIndex;
  bool _isServerEpisode = false;

  // Search functionality
  final TextEditingController _searchController = TextEditingController();
  String _searchQuery = '';
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
      _filterLocalDownloads();
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
      _filterLocalDownloads();
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
      _filterLocalDownloads();
    });
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

  int _compareLocalDates(DateTime? dateA, DateTime? dateB) {
    final a = dateA ?? DateTime(1970);
    final b = dateB ?? DateTime(1970);
    return a.compareTo(b);
  }

  Future<void> _loadDownloads() async {
    await Future.wait([
      _loadServerDownloadSummary(),
      _loadLocalDownloads(),
    ]);
  }

  Future<void> _loadServerDownloadSummary() async {
    setState(() {
      _isLoadingServerSummary = true;
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

        final summaries = await _pinepodsService.getPodcastDownloadSummary(settings.pinepodsUserId!);

        setState(() {
          _serverPodcastSummaries = summaries;
          _isLoadingServerSummary = false;
        });
      } else {
        setState(() {
          _isLoadingServerSummary = false;
        });
      }
    } catch (e) {
      log.severe('Error loading server download summary: $e');
      setState(() {
        _errorMessage = 'Failed to load server downloads: $e';
        _isLoadingServerSummary = false;
      });
    }
  }

  Future<void> _loadEpisodesForPodcast(int podcastId) async {
    final current = _podcastPageState[podcastId];
    if (current != null && (current.loadingMore || current.offset >= current.total && current.total > 0)) {
      return;
    }

    final isFirstLoad = current == null;
    final offset = current?.offset ?? 0;

    setState(() {
      _podcastPageState[podcastId] = _PodcastPageState(
        episodes: current?.episodes ?? [],
        offset: offset,
        total: current?.total ?? 0,
        loadingMore: true,
      );
    });

    try {
      final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
      final settings = settingsBloc.currentSettings;

      if (settings.pinepodsUserId != null) {
        final page = await _pinepodsService.getPodcastDownloadsPaged(
          settings.pinepodsUserId!,
          podcastId,
          limit: _pageSize,
          offset: offset,
        );

        setState(() {
          final existingEpisodes = isFirstLoad ? <PinepodsEpisode>[] : (current?.episodes ?? []);
          final allEpisodes = [...existingEpisodes, ...page.episodes];
          _podcastPageState[podcastId] = _PodcastPageState(
            episodes: allEpisodes,
            offset: allEpisodes.length,
            total: page.total,
            loadingMore: false,
          );
        });
      }
    } catch (e) {
      log.severe('Error loading episodes for podcast $podcastId: $e');
      setState(() {
        _podcastPageState[podcastId] = _PodcastPageState(
          episodes: current?.episodes ?? [],
          offset: current?.offset ?? 0,
          total: current?.total ?? 0,
          loadingMore: false,
        );
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

  Map<String, List<Episode>> _groupLocalEpisodesByPodcast(List<Episode> episodes) {
    final grouped = <String, List<Episode>>{};

    for (final episode in episodes) {
      final podcastName = episode.podcast ?? 'Unknown Podcast';
      if (!grouped.containsKey(podcastName)) {
        grouped[podcastName] = [];
      }
      grouped[podcastName]!.add(episode);
    }

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

  void _togglePodcastExpansion(String podcastKey, {int? podcastId}) {
    final wasExpanded = _expandedPodcasts.contains(podcastKey);
    setState(() {
      if (wasExpanded) {
        _expandedPodcasts.remove(podcastKey);
      } else {
        _expandedPodcasts.add(podcastKey);
      }
    });

    // Lazy-load first page when expanding a server podcast for the first time
    if (!wasExpanded && podcastId != null && !_podcastPageState.containsKey(podcastId)) {
      _loadEpisodesForPodcast(podcastId);
    }
  }

  Future<void> _handleServerEpisodeDelete(PinepodsEpisode episode, int podcastId) async {
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
          setState(() {
            final podState = _podcastPageState[podcastId];
            if (podState != null) {
              podState.episodes.removeWhere((e) => e.episodeId == episode.episodeId);
              podState.offset = podState.episodes.length;
              if (podState.total > 0) podState.total--;
            }
            // Remove summary if no more episodes
            _serverPodcastSummaries = _serverPodcastSummaries.map((s) {
              if (s.podcastId == podcastId) {
                return PodcastDownloadSummary(
                  podcastId: s.podcastId,
                  podcastName: s.podcastName,
                  artworkUrl: s.artworkUrl,
                  episodeCount: (s.episodeCount - 1).clamp(0, s.episodeCount),
                );
              }
              return s;
            }).where((s) => s.episodeCount > 0 || (_podcastPageState[s.podcastId]?.episodes.isNotEmpty ?? false)).toList();
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

  Future<void> _localDownloadServerEpisode(PinepodsEpisode episode) async {
    try {
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
      await podcastBloc.podcastService.saveEpisode(localEpisode);
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
      SnackBar(content: Text(message), backgroundColor: Colors.red),
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

  Widget _buildServerPodcastDropdown(PodcastDownloadSummary summary) {
    final podcastKey = 'server_${summary.podcastId}';
    final isExpanded = _expandedPodcasts.contains(podcastKey);
    final pageState = _podcastPageState[summary.podcastId];
    final episodes = pageState?.episodes ?? [];
    final total = pageState?.total ?? summary.episodeCount;
    final offset = pageState?.offset ?? 0;
    final loadingMore = pageState?.loadingMore ?? false;
    final hasMore = offset < total;

    // Filter loaded episodes by search query
    final filtered = _searchQuery.isEmpty
        ? episodes
        : episodes.where((e) =>
            e.episodeTitle.toLowerCase().contains(_searchQuery.toLowerCase()) ||
            e.podcastName.toLowerCase().contains(_searchQuery.toLowerCase())).toList();

    return Card(
      margin: const EdgeInsets.symmetric(horizontal: 12.0, vertical: 4.0),
      child: Column(
        children: [
          ListTile(
            leading: summary.artworkUrl != null
                ? ClipRRect(
                    borderRadius: BorderRadius.circular(4),
                    child: Image.network(
                      summary.artworkUrl!,
                      width: 40,
                      height: 40,
                      fit: BoxFit.cover,
                      errorBuilder: (_, __, ___) => const Icon(Icons.cloud_download, color: Colors.blue),
                    ),
                  )
                : const Icon(Icons.cloud_download, color: Colors.blue),
            title: Text(
              summary.podcastName,
              style: const TextStyle(fontWeight: FontWeight.bold),
            ),
            subtitle: Text('$total episode${total != 1 ? 's' : ''}'),
            trailing: Icon(isExpanded ? Icons.expand_less : Icons.expand_more),
            onTap: () => _togglePodcastExpansion(podcastKey, podcastId: summary.podcastId),
          ),
          if (isExpanded) ...[
            if (loadingMore && episodes.isEmpty)
              const Padding(
                padding: EdgeInsets.all(16),
                child: Center(child: PlatformProgressIndicator()),
              )
            else ...[
              ...filtered.map((episode) => _buildServerEpisodeTile(episode, summary.podcastId)),
              if (loadingMore)
                const Padding(
                  padding: EdgeInsets.all(8),
                  child: Center(child: PlatformProgressIndicator()),
                )
              else if (hasMore)
                Padding(
                  padding: const EdgeInsets.symmetric(vertical: 8),
                  child: TextButton.icon(
                    onPressed: () => _loadEpisodesForPodcast(summary.podcastId),
                    icon: const Icon(Icons.expand_more),
                    label: Text('Load More (${total - offset} remaining)'),
                  ),
                ),
            ],
          ],
        ],
      ),
    );
  }

  Widget _buildServerEpisodeTile(PinepodsEpisode episode, int podcastId) {
    return ListTile(
      contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 4),
      title: Text(
        episode.episodeTitle,
        maxLines: 2,
        overflow: TextOverflow.ellipsis,
      ),
      subtitle: Text(
        episode.episodePubDate.isNotEmpty
            ? DateTime.tryParse(episode.episodePubDate)?.toLocal().toString().split(' ').first ?? ''
            : '',
      ),
      trailing: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          IconButton(
            icon: const Icon(Icons.play_arrow),
            onPressed: () => _playServerEpisode(episode),
          ),
          IconButton(
            icon: const Icon(Icons.more_vert),
            onPressed: () {
              showDialog(
                context: context,
                barrierColor: Colors.black.withOpacity(0.3),
                builder: (context) => EpisodeContextMenu(
                  episode: episode,
                  onDownload: () {
                    Navigator.of(context).pop();
                    _handleServerEpisodeDelete(episode, podcastId);
                  },
                  onLocalDownload: () {
                    Navigator.of(context).pop();
                    _localDownloadServerEpisode(episode);
                  },
                  onDismiss: () => Navigator.of(context).pop(),
                ),
              );
            },
          ),
        ],
      ),
      onTap: () => Navigator.push(
        context,
        MaterialPageRoute(
          builder: (context) => PinepodsEpisodeDetails(initialEpisode: episode),
        ),
      ),
    );
  }

  Widget _buildLocalPodcastDropdown(String podcastKey, List<Episode> episodes, {String? displayName}) {
    final isExpanded = _expandedPodcasts.contains(podcastKey);
    final title = displayName ?? podcastKey;

    return Card(
      margin: const EdgeInsets.symmetric(horizontal: 12.0, vertical: 4.0),
      child: Column(
        children: [
          ListTile(
            leading: Icon(Icons.file_download, color: Colors.green[600]),
            title: Text(title, style: const TextStyle(fontWeight: FontWeight.bold)),
            subtitle: Text('${episodes.length} episode${episodes.length != 1 ? 's' : ''}'),
            trailing: Icon(isExpanded ? Icons.expand_less : Icons.expand_more),
            onTap: () => _togglePodcastExpansion(podcastKey),
          ),
          if (isExpanded)
            PaginatedEpisodeList(
              episodes: episodes,
              isServerEpisodes: false,
              onPlayPressed: (episode) => _playLocalEpisode(episode),
            ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final episodeBloc = Provider.of<EpisodeBloc>(context);

    return StreamBuilder<BlocState>(
      stream: episodeBloc.downloads,
      builder: (context, snapshot) {
        final localDownloadsState = snapshot.data;
        Map<String, List<Episode>> currentLocalDownloadsByPodcast = {};

        if (localDownloadsState is BlocPopulatedState<List<Episode>>) {
          final currentLocalDownloads = localDownloadsState.results ?? [];
          currentLocalDownloadsByPodcast = _groupLocalEpisodesByPodcast(currentLocalDownloads);
        }

        final isLoading = _isLoadingServerSummary ||
                         _isLoadingLocalDownloads ||
                         (localDownloadsState is BlocLoadingState);

        if (isLoading) {
          return const SliverFillRemaining(
            hasScrollBody: false,
            child: Center(child: PlatformProgressIndicator()),
          );
        }

        _filterLocalDownloads(currentLocalDownloadsByPodcast);

        if (_errorMessage != null) {
          if (_errorMessage!.isServerConnectionError) {
            return _buildOfflineDownloadsView(_filteredLocalDownloadsByPodcast);
          } else {
            return SliverFillRemaining(
              hasScrollBody: false,
              child: Center(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Icon(Icons.error_outline, size: 64, color: Colors.red[300]),
                    const SizedBox(height: 16),
                    Text(
                      _errorMessage!.userFriendlyMessage,
                      textAlign: TextAlign.center,
                      style: Theme.of(context).textTheme.bodyLarge,
                    ),
                    const SizedBox(height: 16),
                    ElevatedButton(onPressed: _loadDownloads, child: const Text('Retry')),
                  ],
                ),
              ),
            );
          }
        }

        final showLocal = _typeFilter == DownloadTypeFilter.all || _typeFilter == DownloadTypeFilter.localOnly;
        final showServer = _typeFilter == DownloadTypeFilter.all || _typeFilter == DownloadTypeFilter.serverOnly;
        final visibleServer = showServer ? _serverPodcastSummaries : <PodcastDownloadSummary>[];
        final hasVisibleLocal = showLocal && _filteredLocalDownloadsByPodcast.isNotEmpty;
        final hasVisibleServer = visibleServer.isNotEmpty;
        final hasActiveFilters = _searchQuery.isNotEmpty || _typeFilter != DownloadTypeFilter.all;

        if (!hasVisibleLocal && !hasVisibleServer) {
          if (hasActiveFilters) {
            return MultiSliver(
              children: [
                _buildSearchAndFilterBar(),
                SliverFillRemaining(
                  hasScrollBody: false,
                  child: Center(
                    child: Column(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        Icon(Icons.filter_list_off, size: 64, color: Theme.of(context).primaryColor),
                        const SizedBox(height: 16),
                        Text('No downloads found', style: Theme.of(context).textTheme.headlineSmall),
                        const SizedBox(height: 8),
                        Text(_getNoResultsMessage(), style: Theme.of(context).textTheme.bodyMedium, textAlign: TextAlign.center),
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
            return SliverFillRemaining(
              hasScrollBody: false,
              child: Center(
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Icon(Icons.download_outlined, size: 64, color: Colors.grey[400]),
                    const SizedBox(height: 16),
                    Text('No downloads found', style: Theme.of(context).textTheme.headlineSmall),
                    const SizedBox(height: 8),
                    Text('Downloaded episodes will appear here', style: Theme.of(context).textTheme.bodyMedium),
                  ],
                ),
              ),
            );
          }
        }

        return MultiSliver(
          children: [
            _buildSearchAndFilterBar(),
            _buildDownloadsList(visibleServer, hasVisibleLocal),
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
            Row(
              children: [
                Expanded(
                  child: TextField(
                    controller: _searchController,
                    decoration: InputDecoration(
                      hintText: 'Search downloads...',
                      prefixIcon: const Icon(Icons.search),
                      suffixIcon: _searchQuery.isNotEmpty
                          ? IconButton(icon: const Icon(Icons.clear), onPressed: () => _searchController.clear())
                          : null,
                      border: OutlineInputBorder(borderRadius: BorderRadius.circular(12)),
                      filled: true,
                      fillColor: Theme.of(context).cardColor,
                      contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
                    ),
                  ),
                ),
                const SizedBox(width: 12),
                Container(
                  decoration: BoxDecoration(
                    color: Theme.of(context).cardColor,
                    borderRadius: BorderRadius.circular(12),
                    border: Border.all(color: Theme.of(context).dividerColor),
                  ),
                  padding: const EdgeInsets.symmetric(horizontal: 12),
                  child: DropdownButtonHideUnderline(
                    child: DropdownButton<DownloadSortDirection>(
                      value: _sortDirection,
                      icon: const Icon(Icons.sort),
                      items: const [
                        DropdownMenuItem(value: DownloadSortDirection.newestFirst, child: Text('Newest')),
                        DropdownMenuItem(value: DownloadSortDirection.oldestFirst, child: Text('Oldest')),
                        DropdownMenuItem(value: DownloadSortDirection.titleAZ, child: Text('Title A-Z')),
                        DropdownMenuItem(value: DownloadSortDirection.titleZA, child: Text('Title Z-A')),
                      ],
                      onChanged: (value) {
                        if (value != null) _setSortDirection(value);
                      },
                    ),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 12),
            SingleChildScrollView(
              scrollDirection: Axis.horizontal,
              child: Row(
                children: [
                  _buildFilterChip(label: 'Clear All', icon: Icons.clear_all, isActive: false, onTap: _clearAllFilters),
                  const SizedBox(width: 8),
                  _buildFilterChip(
                    label: 'Server',
                    icon: Icons.cloud_download,
                    isActive: _typeFilter == DownloadTypeFilter.serverOnly,
                    onTap: () => _setTypeFilter(_typeFilter == DownloadTypeFilter.serverOnly ? DownloadTypeFilter.all : DownloadTypeFilter.serverOnly),
                  ),
                  const SizedBox(width: 8),
                  _buildFilterChip(
                    label: 'Local',
                    icon: Icons.smartphone,
                    isActive: _typeFilter == DownloadTypeFilter.localOnly,
                    onTap: () => _setTypeFilter(_typeFilter == DownloadTypeFilter.localOnly ? DownloadTypeFilter.all : DownloadTypeFilter.localOnly),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildFilterChip({required String label, required IconData icon, required bool isActive, required VoidCallback onTap}) {
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
            border: Border.all(color: isActive ? theme.primaryColor : theme.dividerColor),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(icon, size: 18, color: isActive ? Colors.white : theme.iconTheme.color),
              const SizedBox(width: 6),
              Text(label, style: TextStyle(color: isActive ? Colors.white : theme.textTheme.bodyMedium?.color, fontWeight: FontWeight.w500)),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildDownloadsList(List<PodcastDownloadSummary> serverSummaries, bool hasVisibleLocal) {
    return SliverList(
      delegate: SliverChildListDelegate([
        if (hasVisibleLocal) ...[
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
            child: Row(
              children: [
                Icon(Icons.smartphone, color: Colors.green[600]),
                const SizedBox(width: 8),
                Text(
                  'Local Downloads',
                  style: Theme.of(context).textTheme.titleLarge?.copyWith(fontWeight: FontWeight.bold, color: Colors.green[600]),
                ),
              ],
            ),
          ),
          ..._filteredLocalDownloadsByPodcast.entries.map((entry) {
            return _buildLocalPodcastDropdown('local_${entry.key}', entry.value, displayName: entry.key);
          }).toList(),
        ],

        if (serverSummaries.isNotEmpty) ...[
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 24, 16, 8),
            child: Row(
              children: [
                Icon(Icons.cloud_download, color: Colors.blue[600]),
                const SizedBox(width: 8),
                Text(
                  'Server Downloads',
                  style: Theme.of(context).textTheme.titleLarge?.copyWith(fontWeight: FontWeight.bold, color: Colors.blue[600]),
                ),
              ],
            ),
          ),
          ...serverSummaries.map((summary) => _buildServerPodcastDropdown(summary)).toList(),
        ],

        const SizedBox(height: 100),
      ]),
    );
  }

  String _getNoResultsMessage() {
    final parts = <String>[];
    if (_searchQuery.isNotEmpty) parts.add('matching "$_searchQuery"');
    if (_typeFilter == DownloadTypeFilter.serverOnly) parts.add('in server downloads');
    else if (_typeFilter == DownloadTypeFilter.localOnly) parts.add('in local downloads');
    if (parts.isEmpty) return 'No downloads match your filters';
    return 'No downloads ${parts.join(' ')}';
  }

  void _playServerEpisode(PinepodsEpisode episode) {
    log.info('Playing server episode: ${episode.episodeTitle}');
    _showErrorSnackBar('Server episode playback not yet implemented');
  }

  Future<void> _playLocalEpisode(Episode episode) async {
    try {
      log.info('Playing local episode: ${episode.title}');
      final audioPlayerService = Provider.of<AudioPlayerService>(context, listen: false);
      await audioPlayerService.playEpisode(episode: episode, resume: true);
      log.info('Successfully started local episode playback');
    } catch (e) {
      log.severe('Error playing local episode: $e');
      _showErrorSnackBar('Failed to play episode: $e');
    }
  }

  Widget _buildOfflineDownloadsView(Map<String, List<Episode>> localDownloadsByPodcast) {
    return MultiSliver(
      children: [
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
                Icon(Icons.cloud_off, color: Colors.orange[800], size: 24),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text('Offline Mode', style: TextStyle(fontWeight: FontWeight.bold, color: Colors.orange[800], fontSize: 16)),
                      const SizedBox(height: 4),
                      Text('Server unavailable. Showing local downloads only.', style: TextStyle(color: Colors.orange[700], fontSize: 14)),
                    ],
                  ),
                ),
                const SizedBox(width: 12),
                ElevatedButton.icon(
                  onPressed: () {
                    setState(() { _errorMessage = null; });
                    _loadDownloads();
                  },
                  icon: Icon(Icons.refresh, size: 16, color: Colors.orange[800]),
                  label: Text('Retry', style: TextStyle(color: Colors.orange[800], fontSize: 12, fontWeight: FontWeight.w500)),
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
        _buildSearchAndFilterBar(),
        if (localDownloadsByPodcast.isEmpty)
          SliverFillRemaining(
            hasScrollBody: false,
            child: Center(
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Icon(Icons.cloud_off, size: 64, color: Colors.grey[400]),
                  const SizedBox(height: 16),
                  Text('No local downloads', style: Theme.of(context).textTheme.headlineSmall),
                  const SizedBox(height: 8),
                  Text('Download episodes while online to access them here', style: Theme.of(context).textTheme.bodyMedium, textAlign: TextAlign.center),
                ],
              ),
            ),
          )
        else
          SliverList(
            delegate: SliverChildListDelegate([
              Padding(
                padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
                child: Row(
                  children: [
                    Icon(Icons.smartphone, color: Colors.green[600]),
                    const SizedBox(width: 8),
                    Text('Local Downloads', style: Theme.of(context).textTheme.titleLarge?.copyWith(fontWeight: FontWeight.bold, color: Colors.green[600])),
                  ],
                ),
              ),
              ...localDownloadsByPodcast.entries.map((entry) {
                return _buildLocalPodcastDropdown('offline_local_${entry.key}', entry.value, displayName: entry.key);
              }).toList(),
              const SizedBox(height: 100),
            ]),
          ),
      ],
    );
  }
}
