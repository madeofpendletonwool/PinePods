// lib/ui/pinepods/saved.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_audio_service.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/ui/widgets/episode_context_menu.dart';
import 'package:pinepods_mobile/ui/widgets/pinepods_episode_card.dart';
import 'package:pinepods_mobile/ui/pinepods/episode_details.dart';
import 'package:provider/provider.dart';
import 'package:sliver_tools/sliver_tools.dart';

class PinepodsSaved extends StatefulWidget {
  const PinepodsSaved({Key? key}) : super(key: key);

  @override
  State<PinepodsSaved> createState() => _PinepodsSavedState();
}

class _PinepodsSavedState extends State<PinepodsSaved> {
  bool _isLoading = false;
  String _errorMessage = '';
  List<PinepodsEpisode> _episodes = [];
  List<PinepodsEpisode> _filteredEpisodes = [];
  final PinepodsService _pinepodsService = PinepodsService();
  PinepodsAudioService? _audioService;
  int? _contextMenuEpisodeIndex;
  final TextEditingController _searchController = TextEditingController();
  String _searchQuery = '';

  @override
  void initState() {
    super.initState();
    _loadSavedEpisodes();
    _searchController.addListener(_onSearchChanged);
  }

  @override
  void dispose() {
    _searchController.dispose();
    _audioService?.dispose();
    super.dispose();
  }

  void _onSearchChanged() {
    setState(() {
      _searchQuery = _searchController.text;
      _filterEpisodes();
    });
  }

  void _filterEpisodes() {
    if (_searchQuery.isEmpty) {
      _filteredEpisodes = List.from(_episodes);
    } else {
      _filteredEpisodes = _episodes.where((episode) {
        return episode.episodeTitle.toLowerCase().contains(_searchQuery.toLowerCase()) ||
               episode.podcastName.toLowerCase().contains(_searchQuery.toLowerCase());
      }).toList();
    }
  }

  void _initializeAudioService() {
    if (_audioService != null) return;
    
    try {
      final audioPlayerService = Provider.of<AudioPlayerService>(context, listen: false);
      final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
      
      _audioService = PinepodsAudioService(
        audioPlayerService,
        _pinepodsService,
        settingsBloc,
      );
    } catch (e) {
      // Provider not available - audio service will remain null
    }
  }

  Future<void> _loadSavedEpisodes() async {
    setState(() {
      _isLoading = true;
      _errorMessage = '';
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
      final userId = settings.pinepodsUserId!;

      final episodes = await _pinepodsService.getSavedEpisodes(userId);
      
      setState(() {
        _episodes = episodes;
        _filterEpisodes(); // Initialize filtered list
        _isLoading = false;
      });
    } catch (e) {
      setState(() {
        _errorMessage = 'Failed to load saved episodes: ${e.toString()}';
        _isLoading = false;
      });
    }
  }

  Future<void> _refresh() async {
    await _loadSavedEpisodes();
  }

  Future<void> _playEpisode(PinepodsEpisode episode) async {
    _initializeAudioService();
    
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
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Row(
            children: [
              const SizedBox(
                width: 16,
                height: 16,
                child: CircularProgressIndicator(strokeWidth: 2),
              ),
              const SizedBox(width: 12),
              Text('Starting ${episode.episodeTitle}...'),
            ],
          ),
          duration: const Duration(seconds: 2),
        ),
      );

      await _audioService!.playPinepodsEpisode(
        pinepodsEpisode: episode,
        resume: episode.isStarted,
      );

      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text('Now playing: ${episode.episodeTitle}'),
          backgroundColor: Colors.green,
          duration: const Duration(seconds: 2),
        ),
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
    // This shouldn't be called since all episodes here are already saved
    // But just in case, we'll handle it
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
        // REMOVE the episode from the list since it's no longer saved
        setState(() {
          _episodes.removeAt(episodeIndex);
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
    // Show context menu as a modal overlay if needed
    if (_contextMenuEpisodeIndex != null) {
      final episodeIndex = _contextMenuEpisodeIndex!;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        showDialog(
          context: context,
          barrierColor: Colors.black.withOpacity(0.3),
          builder: (context) => EpisodeContextMenu(
            episode: _episodes[episodeIndex],
            onSave: () {
              Navigator.of(context).pop();
              _saveEpisode(episodeIndex);
            },
            onRemoveSaved: () {
              Navigator.of(context).pop();
              _removeSavedEpisode(episodeIndex);
            },
            onDownload: _episodes[episodeIndex].downloaded 
              ? () {
                  Navigator.of(context).pop();
                  _deleteEpisode(episodeIndex);
                }
              : () {
                  Navigator.of(context).pop();
                  _downloadEpisode(episodeIndex);
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
          ),
        );
      });
      _contextMenuEpisodeIndex = null;
    }
    
    if (_isLoading) {
      return const SliverFillRemaining(
        child: Center(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              CircularProgressIndicator(),
              SizedBox(height: 16),
              Text('Loading saved episodes...'),
            ],
          ),
        ),
      );
    }

    if (_errorMessage.isNotEmpty) {
      return SliverFillRemaining(
        child: Center(
          child: Padding(
            padding: const EdgeInsets.all(16.0),
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Icon(
                  Icons.error_outline,
                  color: Theme.of(context).colorScheme.error,
                  size: 48,
                ),
                const SizedBox(height: 16),
                Text(
                  _errorMessage,
                  style: TextStyle(
                    color: Theme.of(context).colorScheme.error,
                  ),
                  textAlign: TextAlign.center,
                ),
                const SizedBox(height: 16),
                ElevatedButton(
                  onPressed: _refresh,
                  child: const Text('Retry'),
                ),
              ],
            ),
          ),
        ),
      );
    }

    if (_episodes.isEmpty) {
      return const SliverFillRemaining(
        child: Center(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Icon(
                Icons.bookmark_outline,
                size: 64,
                color: Colors.grey,
              ),
              SizedBox(height: 16),
              Text(
                'No saved episodes',
                style: TextStyle(
                  fontSize: 18,
                  color: Colors.grey,
                ),
              ),
              SizedBox(height: 8),
              Text(
                'Episodes you save will appear here',
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
        _buildSearchBar(),
        _buildEpisodesList(),
      ],
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
          if (index == 0) {
            // Header
            return Padding(
              padding: const EdgeInsets.all(16.0),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Text(
                    _searchQuery.isEmpty 
                        ? 'Saved Episodes' 
                        : 'Search Results (${_filteredEpisodes.length})',
                    style: const TextStyle(
                      fontSize: 24,
                      fontWeight: FontWeight.bold,
                    ),
                  ),
                  IconButton(
                    icon: const Icon(Icons.refresh),
                    onPressed: _refresh,
                  ),
                ],
              ),
            );
          }
          // Episodes (index - 1 because of header)
          final episodeIndex = index - 1;
          final episode = _filteredEpisodes[episodeIndex];
          // Find the original index for context menu operations
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
            onLongPress: () => _showContextMenu(originalIndex),
            onPlayPressed: () => _playEpisode(episode),
            onDownloadPressed: () => _downloadEpisode(originalIndex),
          );
        },
        childCount: _filteredEpisodes.length + 1, // +1 for header
      ),
    );
  }
}