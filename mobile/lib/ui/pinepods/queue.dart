// lib/ui/pinepods/queue.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_audio_service.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/ui/widgets/episode_context_menu.dart';
import 'package:pinepods_mobile/ui/widgets/pinepods_episode_card.dart';
import 'package:pinepods_mobile/ui/widgets/draggable_queue_episode_card.dart';
import 'package:pinepods_mobile/ui/pinepods/episode_details.dart';
import 'package:pinepods_mobile/ui/utils/local_download_utils.dart';
import 'package:pinepods_mobile/ui/utils/player_utils.dart';
import 'package:pinepods_mobile/ui/utils/position_utils.dart';
import 'package:pinepods_mobile/services/global_services.dart';
import 'package:provider/provider.dart';

class PinepodsQueue extends StatefulWidget {
  const PinepodsQueue({Key? key}) : super(key: key);

  @override
  State<PinepodsQueue> createState() => _PinepodsQueueState();
}

class _PinepodsQueueState extends State<PinepodsQueue> {
  bool _isLoading = false;
  String _errorMessage = '';
  List<PinepodsEpisode> _episodes = [];
  final PinepodsService _pinepodsService = PinepodsService();
  // Use global audio service instead of creating local instance
  int? _contextMenuEpisodeIndex;

  @override
  void initState() {
    super.initState();
    _loadQueuedEpisodes();
  }

  PinepodsAudioService? get _audioService => GlobalServices.pinepodsAudioService;

  Future<void> _loadQueuedEpisodes() async {
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
      GlobalServices.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
      final userId = settings.pinepodsUserId!;

      final episodes = await _pinepodsService.getQueuedEpisodes(userId);
      
      // Enrich episodes with best available positions (local vs server)
      final enrichedEpisodes = await PositionUtils.enrichEpisodesWithBestPositions(
        context,
        _pinepodsService,
        episodes,
        userId,
      );
      
      setState(() {
        _episodes = enrichedEpisodes;
        _isLoading = false;
      });
      
      // After loading episodes, check their local download status
      await LocalDownloadUtils.loadLocalDownloadStatuses(context, enrichedEpisodes);
    } catch (e) {
      setState(() {
        _errorMessage = 'Failed to load queued episodes: ${e.toString()}';
        _isLoading = false;
      });
    }
  }

  Future<void> _refresh() async {
    // Clear local download status cache on refresh
    LocalDownloadUtils.clearCache();
    await _loadQueuedEpisodes();
  }

  Future<void> _reorderEpisodes(int oldIndex, int newIndex) async {
    // Adjust indices if moving down the list
    if (newIndex > oldIndex) {
      newIndex -= 1;
    }

    // Update local state immediately for smooth UI
    setState(() {
      final episode = _episodes.removeAt(oldIndex);
      _episodes.insert(newIndex, episode);
    });

    // Get episode IDs in new order
    final episodeIds = _episodes.map((e) => e.episodeId).toList();

    // Call API to update order on server
    try {
      final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
      final settings = settingsBloc.currentSettings;
      final userId = settings.pinepodsUserId;

      if (userId == null) {
        _showSnackBar('Not logged in', Colors.red);
        // Reload to restore original order if API call fails
        await _loadQueuedEpisodes();
        return;
      }

      _pinepodsService.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
      final success = await _pinepodsService.reorderQueue(userId, episodeIds);

      if (!success) {
        _showSnackBar('Failed to update queue order', Colors.red);
        // Reload to restore original order if API call fails
        await _loadQueuedEpisodes();
      }
    } catch (e) {
      _showSnackBar('Error updating queue order: $e', Colors.red);
      // Reload to restore original order if API call fails
      await _loadQueuedEpisodes();
    }
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

  Future<void> _showContextMenu(int episodeIndex) async {
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
          _toggleQueueEpisode(episodeIndex);
        },
        onMarkComplete: () {
          Navigator.of(context).pop();
          _toggleMarkComplete(episodeIndex);
        },
        onDismiss: () {
          Navigator.of(context).pop();
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
          // REMOVE the episode from the list since it's no longer queued
          setState(() {
            _episodes.removeAt(episodeIndex);
          });
          _showSnackBar('Removed from queue', Colors.orange);
        }
      } else {
        // This shouldn't happen since all episodes here are already queued
        // But just in case, we'll handle it
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
  void dispose() {
    // Don't dispose global audio service - it should persist across pages
    super.dispose();
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
              Text('Loading queue...'),
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
                Icons.queue_music_outlined,
                size: 64,
                color: Colors.grey,
              ),
              SizedBox(height: 16),
              Text(
                'No queued episodes',
                style: TextStyle(
                  fontSize: 18,
                  color: Colors.grey,
                ),
              ),
              SizedBox(height: 8),
              Text(
                'Episodes you queue will appear here',
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

    return _buildEpisodesList();
  }

  Widget _buildEpisodesList() {
    return SliverToBoxAdapter(
      child: Column(
        children: [
          // Header
          Padding(
            padding: const EdgeInsets.all(16.0),
            child: Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                const Text(
                  'Queue',
                  style: TextStyle(
                    fontSize: 24,
                    fontWeight: FontWeight.bold,
                  ),
                ),
                Row(
                  children: [
                    Text(
                      'Drag to reorder',
                      style: TextStyle(
                        fontSize: 12,
                        color: Colors.grey[600],
                      ),
                    ),
                    const SizedBox(width: 8),
                    IconButton(
                      icon: const Icon(Icons.refresh),
                      onPressed: _refresh,
                    ),
                  ],
                ),
              ],
            ),
          ),
          // Reorderable episodes list
          ReorderableListView.builder(
            shrinkWrap: true,
            physics: const NeverScrollableScrollPhysics(),
            buildDefaultDragHandles: false, // Disable automatic drag handles
            onReorder: _reorderEpisodes,
            itemCount: _episodes.length,
            itemBuilder: (context, index) {
              final episode = _episodes[index];
              return Container(
                key: ValueKey(episode.episodeId),
                margin: const EdgeInsets.only(bottom: 4),
                child: DraggableQueueEpisodeCard(
                  episode: episode,
                  index: index,
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
                  onLongPress: () => _showContextMenu(index),
                  onPlayPressed: () => _playEpisode(episode),
                ),
              );
            },
          ),
        ],
      ),
    );
  }
}