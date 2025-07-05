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
import 'package:pinepods_mobile/ui/widgets/platform_progress_indicator.dart';
import 'package:provider/provider.dart';
import 'package:logging/logging.dart';

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

  @override
  void initState() {
    super.initState();
    _loadDownloads();
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

  Widget _buildPodcastDropdown(String podcastKey, List<Widget> episodes, {bool isServerDownload = false, String? displayName}) {
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
            subtitle: Text('${episodes.length} episode${episodes.length != 1 ? 's' : ''}'),
            trailing: Icon(
              isExpanded ? Icons.expand_less : Icons.expand_more,
            ),
            onTap: () => _togglePodcastExpansion(podcastKey),
          ),
          if (isExpanded) ...episodes,
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
        
        if (_errorMessage != null) {
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
                    _errorMessage!,
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
        
        if (currentLocalDownloadsByPodcast.isEmpty && _serverDownloadsByPodcast.isEmpty) {
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
        
        return SliverList(
          delegate: SliverChildListDelegate([
            // Local Downloads Section
            if (currentLocalDownloadsByPodcast.isNotEmpty) ...[
              Padding(
                padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
                child: Row(
                  children: [
                    Icon(Icons.smartphone, color: Colors.green[600]),
                    const SizedBox(width: 8),
                    Text(
                      'Local Downloads',
                      style: Theme.of(context).textTheme.titleLarge?.copyWith(
                        fontWeight: FontWeight.bold,
                        color: Colors.green[600],
                      ),
                    ),
                  ],
                ),
              ),
              
              ...currentLocalDownloadsByPodcast.entries.map((entry) {
                final podcastName = entry.key;
                final episodes = entry.value;
                final podcastKey = 'local_$podcastName';
                
                return _buildPodcastDropdown(
                  podcastKey,
                  episodes.map((episode) => EpisodeTile(
                    episode: episode,
                    download: false,
                    play: true,
                  )).toList(),
                  isServerDownload: false,
                  displayName: podcastName,
                );
              }).toList(),
            ],
            
            // Server Downloads Section
            if (_serverDownloadsByPodcast.isNotEmpty) ...[
              Padding(
                padding: const EdgeInsets.fromLTRB(16, 24, 16, 8),
                child: Row(
                  children: [
                    Icon(Icons.cloud_download, color: Colors.blue[600]),
                    const SizedBox(width: 8),
                    Text(
                      'Server Downloads',
                      style: Theme.of(context).textTheme.titleLarge?.copyWith(
                        fontWeight: FontWeight.bold,
                        color: Colors.blue[600],
                      ),
                    ),
                  ],
                ),
              ),
              
              ..._serverDownloadsByPodcast.entries.map((entry) {
                final podcastName = entry.key;
                final episodes = entry.value;
                final podcastKey = 'server_$podcastName';
                
                return _buildPodcastDropdown(
                  podcastKey,
                  episodes.map((episode) {
                    // Find the global index of this episode in _serverDownloads
                    final globalIndex = _serverDownloads.indexWhere((e) => e.episodeId == episode.episodeId);
                    return PinepodsEpisodeCard(
                      episode: episode,
                      onLongPress: () => _showContextMenu(globalIndex, true),
                      onPlayPressed: () => _playServerEpisode(episode),
                    );
                  }).toList(),
                  isServerDownload: true,
                  displayName: podcastName,
                );
              }).toList(),
            ],
            
            // Bottom padding
            const SizedBox(height: 100),
          ]),
        );
      },
    );
  }

  void _playServerEpisode(PinepodsEpisode episode) {
    // TODO: Implement server episode playback
    // This would involve getting the stream URL from the server
    // and playing it through the audio service
    log.info('Playing server episode: ${episode.episodeTitle}');
    
    _showErrorSnackBar('Server episode playback not yet implemented');
  }
}