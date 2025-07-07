// lib/ui/podcast/pinepods_up_next_view.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/ui/widgets/draggable_queue_episode_card.dart';
import 'package:provider/provider.dart';

/// PinePods version of the Up Next queue that shows the server queue.
///
/// This replaces the local queue functionality with server-based queue management.
class PinepodsUpNextView extends StatefulWidget {
  const PinepodsUpNextView({
    Key? key,
  }) : super(key: key);

  @override
  State<PinepodsUpNextView> createState() => _PinepodsUpNextViewState();
}

class _PinepodsUpNextViewState extends State<PinepodsUpNextView> {
  final PinepodsService _pinepodsService = PinepodsService();
  List<PinepodsEpisode> _queuedEpisodes = [];
  bool _isLoading = true;
  String? _errorMessage;

  @override
  void initState() {
    super.initState();
    _loadQueue();
  }

  Future<void> _loadQueue() async {
    setState(() {
      _isLoading = true;
      _errorMessage = null;
    });

    try {
      final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
      final settings = settingsBloc.currentSettings;

      if (settings.pinepodsServer == null || 
          settings.pinepodsApiKey == null || 
          settings.pinepodsUserId == null) {
        setState(() {
          _errorMessage = 'Not connected to PinePods server';
          _isLoading = false;
        });
        return;
      }

      _pinepodsService.setCredentials(
        settings.pinepodsServer!,
        settings.pinepodsApiKey!,
      );

      final episodes = await _pinepodsService.getQueuedEpisodes(settings.pinepodsUserId!);
      
      setState(() {
        _queuedEpisodes = episodes;
        _isLoading = false;
      });
    } catch (e) {
      setState(() {
        _errorMessage = e.toString();
        _isLoading = false;
      });
    }
  }

  Future<void> _reorderQueue(int oldIndex, int newIndex) async {
    // Adjust indices if moving down the list
    if (newIndex > oldIndex) {
      newIndex -= 1;
    }

    // Update local state immediately for smooth UI
    setState(() {
      final episode = _queuedEpisodes.removeAt(oldIndex);
      _queuedEpisodes.insert(newIndex, episode);
    });

    // Get episode IDs in new order
    final episodeIds = _queuedEpisodes.map((e) => e.episodeId).toList();

    // Call API to update order on server
    try {
      final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
      final settings = settingsBloc.currentSettings;
      final userId = settings.pinepodsUserId;

      if (userId == null) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Not logged in')),
        );
        await _loadQueue(); // Reload to restore original order
        return;
      }

      final success = await _pinepodsService.reorderQueue(userId, episodeIds);

      if (!success) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Failed to update queue order')),
        );
        await _loadQueue(); // Reload to restore original order
      }
    } catch (e) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Error updating queue: $e')),
      );
      await _loadQueue(); // Reload to restore original order
    }
  }

  Future<void> _removeFromQueue(int index) async {
    final episode = _queuedEpisodes[index];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Not logged in')),
      );
      return;
    }

    try {
      final success = await _pinepodsService.removeQueuedEpisode(
        episode.episodeId,
        userId,
        episode.isYoutube,
      );

      if (success) {
        setState(() {
          _queuedEpisodes.removeAt(index);
        });
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Removed from queue')),
        );
      } else {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Failed to remove from queue')),
        );
      }
    } catch (e) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Error removing from queue: $e')),
      );
    }
  }

  Future<void> _clearQueue() async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Clear Queue'),
        content: const Text('Are you sure you want to clear the entire queue?'),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: const Text('Clear'),
          ),
        ],
      ),
    );

    if (confirmed != true) return;

    // Remove all episodes from queue
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Not logged in')),
      );
      return;
    }

    try {
      // Remove each episode from the queue
      for (final episode in _queuedEpisodes) {
        await _pinepodsService.removeQueuedEpisode(
          episode.episodeId,
          userId,
          episode.isYoutube,
        );
      }

      setState(() {
        _queuedEpisodes.clear();
      });

      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Queue cleared')),
      );
    } catch (e) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Error clearing queue: $e')),
      );
      await _loadQueue(); // Reload to get current state
    }
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      mainAxisAlignment: MainAxisAlignment.start,
      crossAxisAlignment: CrossAxisAlignment.center,
      children: [
        // Header with title and clear button
        Row(
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(16.0, 8.0, 24.0, 8.0),
              child: Text(
                'Up Next',
                style: Theme.of(context).textTheme.titleLarge,
              ),
            ),
            const Spacer(),
            Padding(
              padding: const EdgeInsets.fromLTRB(16.0, 0.0, 24.0, 8.0),
              child: TextButton(
                onPressed: _queuedEpisodes.isEmpty ? null : _clearQueue,
                child: Text(
                  'Clear',
                  style: Theme.of(context).textTheme.titleSmall!.copyWith(
                        fontSize: 12.0,
                        color: _queuedEpisodes.isEmpty
                            ? Theme.of(context).disabledColor
                            : Theme.of(context).primaryColor,
                      ),
                ),
              ),
            ),
          ],
        ),

        // Content area
        if (_isLoading)
          const Padding(
            padding: EdgeInsets.all(24.0),
            child: Center(
              child: CircularProgressIndicator(),
            ),
          )
        else if (_errorMessage != null)
          Padding(
            padding: const EdgeInsets.all(24.0),
            child: Column(
              children: [
                Text(
                  'Error loading queue',
                  style: Theme.of(context).textTheme.titleMedium,
                ),
                const SizedBox(height: 8),
                Text(
                  _errorMessage!,
                  style: Theme.of(context).textTheme.bodySmall,
                ),
                const SizedBox(height: 16),
                ElevatedButton(
                  onPressed: _loadQueue,
                  child: const Text('Retry'),
                ),
              ],
            ),
          )
        else if (_queuedEpisodes.isEmpty)
          Padding(
            padding: const EdgeInsets.all(24.0),
            child: Container(
              decoration: BoxDecoration(
                color: Theme.of(context).dividerColor,
                border: Border.all(
                  color: Theme.of(context).dividerColor,
                ),
                borderRadius: const BorderRadius.all(Radius.circular(10)),
              ),
              child: Padding(
                padding: const EdgeInsets.all(24.0),
                child: Text(
                  'Your queue is empty. Add episodes to see them here.',
                  style: Theme.of(context).textTheme.titleMedium,
                  textAlign: TextAlign.center,
                ),
              ),
            ),
          )
        else
          Expanded(
            child: ReorderableListView.builder(
              buildDefaultDragHandles: false,
              shrinkWrap: true,
              padding: const EdgeInsets.all(8),
              itemCount: _queuedEpisodes.length,
              itemBuilder: (BuildContext context, int index) {
                final episode = _queuedEpisodes[index];
                return Dismissible(
                  key: ValueKey('queue_${episode.episodeId}'),
                  direction: DismissDirection.endToStart,
                  onDismissed: (direction) {
                    _removeFromQueue(index);
                  },
                  background: Container(
                    color: Colors.red,
                    alignment: Alignment.centerRight,
                    padding: const EdgeInsets.only(right: 20),
                    child: const Icon(
                      Icons.delete,
                      color: Colors.white,
                    ),
                  ),
                  child: Container(
                    key: ValueKey('episode_${episode.episodeId}'),
                    margin: const EdgeInsets.only(bottom: 4),
                    child: DraggableQueueEpisodeCard(
                      episode: episode,
                      index: index,
                      onTap: () {
                        // Could navigate to episode details if needed
                      },
                      onPlayPressed: () {
                        // Could implement play functionality if needed
                        ScaffoldMessenger.of(context).showSnackBar(
                          SnackBar(content: Text('Playing ${episode.episodeTitle}')),
                        );
                      },
                    ),
                  ),
                );
              },
              onReorder: _reorderQueue,
            ),
          ),
      ],
    );
  }
}