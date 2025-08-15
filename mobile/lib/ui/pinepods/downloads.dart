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
import 'package:sliver_tools/sliver_tools.dart';

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

  @override
  void initState() {
    super.initState();
    _loadDownloads();
    _searchController.addListener(_onSearchChanged);
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
      
      if (_searchQuery.isEmpty) {
        _filteredServerDownloadsByPodcast[podcastName] = List.from(episodes);
      } else {
        final filteredEpisodes = episodes.where((episode) {
          return episode.episodeTitle.toLowerCase().contains(_searchQuery.toLowerCase());
        }).toList();
        
        if (filteredEpisodes.isNotEmpty) {
          _filteredServerDownloadsByPodcast[podcastName] = filteredEpisodes;
        }
      }
    }

    // Filter local downloads (will be called when local downloads are loaded)
    _filterLocalDownloads();
  }

  void _filterLocalDownloads([Map<String, List<Episode>>? localDownloadsByPodcast]) {
    final downloadsToFilter = localDownloadsByPodcast ?? _localDownloadsByPodcast;
    _filteredLocalDownloadsByPodcast = {};
    
    for (final entry in downloadsToFilter.entries) {
      final podcastName = entry.key;
      final episodes = entry.value;
      
      if (_searchQuery.isEmpty) {
        _filteredLocalDownloadsByPodcast[podcastName] = List.from(episodes);
      } else {
        final filteredEpisodes = episodes.where((episode) {
          return (episode.title ?? '').toLowerCase().contains(_searchQuery.toLowerCase());
        }).toList();
        
        if (filteredEpisodes.isNotEmpty) {
          _filteredLocalDownloadsByPodcast[podcastName] = filteredEpisodes;
        }
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
        
        if (_filteredLocalDownloadsByPodcast.isEmpty && _filteredServerDownloadsByPodcast.isEmpty) {
          if (_searchQuery.isNotEmpty) {
            // Show no search results message
            return MultiSliver(
              children: [
                _buildSearchBar(),
                SliverFillRemaining(
                  hasScrollBody: false,
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
                          'No downloads found',
                          style: Theme.of(context).textTheme.headlineSmall,
                        ),
                        const SizedBox(height: 8),
                        Text(
                          'No downloads match "$_searchQuery"',
                          style: Theme.of(context).textTheme.bodyMedium,
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
            _buildSearchBar(),
            _buildDownloadsList(),
          ],
        );
      },
    );
  }

  Widget _buildSearchBar() {
    return SliverToBoxAdapter(
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: TextField(
          controller: _searchController,
          decoration: InputDecoration(
            hintText: 'Filter episodes...',
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
          ),
        ),
      ),
    );
  }

  Widget _buildDownloadsList() {
    return SliverList(
      delegate: SliverChildListDelegate([
        // Local Downloads Section
        if (_filteredLocalDownloadsByPodcast.isNotEmpty) ...[
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
            child: Row(
              children: [
                Icon(Icons.smartphone, color: Colors.green[600]),
                const SizedBox(width: 8),
                Text(
                  _searchQuery.isEmpty 
                      ? 'Local Downloads' 
                      : 'Local Downloads (${_countFilteredEpisodes(_filteredLocalDownloadsByPodcast)})',
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
        if (_filteredServerDownloadsByPodcast.isNotEmpty) ...[
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 24, 16, 8),
            child: Row(
              children: [
                Icon(Icons.cloud_download, color: Colors.blue[600]),
                const SizedBox(width: 8),
                Text(
                  _searchQuery.isEmpty 
                      ? 'Server Downloads' 
                      : 'Server Downloads (${_countFilteredEpisodes(_filteredServerDownloadsByPodcast)})',
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
              ],
            ),
          ),
        ),
        
        // Search bar for filtering local downloads
        _buildSearchBar(),
        
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