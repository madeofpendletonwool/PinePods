// lib/ui/podcast/pinepods_up_next_view.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/services/global_services.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/ui/widgets/draggable_queue_episode_card.dart';
import 'package:provider/provider.dart';
import 'dart:async';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';
import 'package:pinepods_mobile/l10n/L.dart';

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

  /// The queue's playing episode, kept out of the list (it's what's playing,
  /// not what's next) but still sent first when reordering.
  PinepodsEpisode? _playing;
  bool _isLoading = true;
  String? _errorMessage;
  StreamSubscription? _episodeSubscription;

  @override
  void initState() {
    super.initState();
    _loadQueue();
    _listenToEpisodeChanges();
  }

  @override
  void dispose() {
    _episodeSubscription?.cancel();
    super.dispose();
  }

  /// Listen to episode changes to refresh queue when episodes advance
  void _listenToEpisodeChanges() {
    try {
      final audioPlayerService = Provider.of<AudioPlayerService>(context, listen: false);
      final episodeStream = audioPlayerService.episodeEvent;
      
      // Check if episodeEvent stream is available
      if (episodeStream == null) {
        print('Episode event stream not available');
        return;
      }
      
      String? lastEpisodeGuid;
      
      _episodeSubscription = episodeStream.listen((episode) {
        // Only refresh if the episode actually changed (avoid unnecessary refreshes)
        if (episode != null && episode.guid != lastEpisodeGuid && mounted) {
          lastEpisodeGuid = episode.guid;
          
          // Add a small delay to ensure server queue has been updated
          Future.delayed(const Duration(milliseconds: 500), () {
            if (mounted) {
              _loadQueue();
            }
          });
        }
      });
    } catch (e) {
      // Provider not available, continue without episode listening
      print('Could not set up episode change listener: $e');
    }
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
          _errorMessage = L.of(context)!.not_connected_to_pinepods;
          _isLoading = false;
        });
        return;
      }

      _pinepodsService.setCredentials(
        settings.pinepodsServer!,
        settings.pinepodsApiKey!,
      );

      final episodes = await _pinepodsService.getQueuedEpisodes(settings.pinepodsUserId!);
      final audio = GlobalServices.pinepodsAudioService;
      bool isPlaying(PinepodsEpisode e) =>
          e.episodeId == audio?.currentEpisodeId && e.isYoutube == audio?.isYoutube;

      if (!mounted) return;
      setState(() {
        _playing = episodes.where(isPlaying).firstOrNull;
        _queuedEpisodes = episodes.where((e) => !isPlaying(e)).toList();
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

    // Get episodes in new order, keeping is_youtube so colliding podcast/YouTube
    // ids are repositioned correctly.
    final reorderItems = [if (_playing != null) _playing!, ..._queuedEpisodes]
        .map((e) => (episodeId: e.episodeId, isYoutube: e.isYoutube))
        .toList();

    // Call API to update order on server
    try {
      final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
      final settings = settingsBloc.currentSettings;
      final userId = settings.pinepodsUserId;

      if (userId == null) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(L.of(context)!.not_logged_in)),
        );
        await _loadQueue(); // Reload to restore original order
        return;
      }

      final success = await _pinepodsService.reorderQueue(userId, reorderItems);

      if (!success) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(L.current.failed_to_update_queue_order)),
        );
        await _loadQueue(); // Reload to restore original order
      }
    } catch (e) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(L.current.error_updating_queue(e.toString()))),
      );
      await _loadQueue(); // Reload to restore original order
    }
  }

  Future<void> _play(PinepodsEpisode episode) async {
    try {
      await GlobalServices.pinepodsAudioService?.playPinepodsEpisode(pinepodsEpisode: episode);
    } catch (e) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(L.of(context)!.could_not_play_episode(e.toString()))));
    }
  }

  Future<void> _removeFromQueue(int index) async {
    final episode = _queuedEpisodes[index];
    final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
    final settings = settingsBloc.currentSettings;
    final userId = settings.pinepodsUserId;

    if (userId == null) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(L.of(context)!.not_logged_in)),
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
          SnackBar(content: Text(L.current.removed_from_queue)),
        );
      } else {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(L.current.failed_to_remove_from_queue)),
        );
      }
    } catch (e) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(L.current.error_removing_from_queue(e.toString()))),
      );
    }
  }

  Future<void> _clearQueue() async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(L.of(context)!.clear_queue),
        content: Text(L.of(context)!.clear_queue_confirm),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: Text(L.of(context)!.cancel_button_label),
          ),
          TextButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: Text(L.of(context)!.clear),
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
        SnackBar(content: Text(L.current.not_logged_in)),
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
        SnackBar(content: Text(L.current.queue_cleared)),
      );
    } catch (e) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(L.current.error_clearing_queue(e.toString()))),
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
                L.of(context)!.up_next_queue_label,
                style: Theme.of(context).textTheme.titleLarge,
              ),
            ),
            const Spacer(),
            Padding(
              padding: const EdgeInsets.fromLTRB(16.0, 0.0, 24.0, 8.0),
              child: TextButton(
                onPressed: _queuedEpisodes.isEmpty ? null : _clearQueue,
                child: Text(
                  L.of(context)!.clear,
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
                  L.of(context)!.error_loading_queue,
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
                  child: Text(L.of(context)!.retry),
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
                  L.of(context)!.up_next_empty,
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
                      PhosphorIconsRegular.trash,
                      color: Colors.white,
                    ),
                  ),
                  child: Container(
                    key: ValueKey('episode_${episode.episodeId}'),
                    margin: const EdgeInsets.only(bottom: 4),
                    child: DraggableQueueEpisodeCard(
                      episode: episode,
                      index: index,
                      onTap: () => _play(episode),
                      onPlayPressed: () => _play(episode),
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