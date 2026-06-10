// lib/ui/pinepods/action_queue.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/podcast/podcast_bloc.dart';
import 'package:pinepods_mobile/entities/pending_action.dart';
import 'package:pinepods_mobile/repository/repository.dart';
import 'package:pinepods_mobile/services/global_services.dart';
import 'package:pinepods_mobile/ui/widgets/platform_progress_indicator.dart';
import 'package:provider/provider.dart';

/// Shows the offline "outbox" — episode interactions (progress, completion,
/// saved, queue) that were recorded while offline and are waiting to sync to
/// the PinePods server. Lets the user trigger a manual sync.
class ActionQueue extends StatefulWidget {
  const ActionQueue({super.key});

  @override
  State<ActionQueue> createState() => _ActionQueueState();
}

class _ActionQueueState extends State<ActionQueue> {
  late final Repository _repository;
  List<PendingAction> _actions = [];
  final Map<int, String> _titleCache = {};
  bool _loading = true;
  bool _syncing = false;

  @override
  void initState() {
    super.initState();
    _repository = Provider.of<PodcastBloc>(context, listen: false).podcastService.repository;
    _load();
  }

  Future<void> _load() async {
    setState(() => _loading = true);
    final actions = await _repository.getPendingActions();

    // Resolve episode titles from local downloads where possible.
    for (final a in actions) {
      if (!_titleCache.containsKey(a.episodeId)) {
        final ep = await _repository.findEpisodeByGuid('pinepods_${a.episodeId}');
        if (ep?.title != null) {
          _titleCache[a.episodeId] = ep!.title!;
        }
      }
    }

    if (mounted) {
      setState(() {
        _actions = actions;
        _loading = false;
      });
    }
  }

  Future<void> _syncNow() async {
    final queue = GlobalServices.offlineActionQueue;
    if (queue == null) return;

    setState(() => _syncing = true);
    await queue.flush();
    await _load();
    if (mounted) {
      setState(() => _syncing = false);
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(_actions.isEmpty ? 'All actions synced' : '${_actions.length} action(s) still pending'),
          duration: const Duration(seconds: 2),
        ),
      );
    }
  }

  IconData _iconFor(PendingActionType type) {
    switch (type) {
      case PendingActionType.recordPosition:
      case PendingActionType.addHistory:
        return Icons.timelapse;
      case PendingActionType.markCompleted:
        return Icons.check_circle;
      case PendingActionType.markUncompleted:
        return Icons.unpublished;
      case PendingActionType.saveEpisode:
        return Icons.bookmark_add;
      case PendingActionType.removeSaved:
        return Icons.bookmark_remove;
      case PendingActionType.queue:
        return Icons.queue_music;
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Action Queue'),
        actions: [
          IconButton(
            icon: const Icon(Icons.sync),
            tooltip: 'Sync now',
            onPressed: _syncing ? null : _syncNow,
          ),
        ],
      ),
      body: _loading
          ? const Center(child: PlatformProgressIndicator())
          : RefreshIndicator(
              onRefresh: _load,
              child: _actions.isEmpty
                  ? ListView(
                      children: [
                        SizedBox(height: MediaQuery.of(context).size.height * 0.3),
                        Icon(Icons.cloud_done, size: 64, color: Colors.green[400]),
                        const SizedBox(height: 16),
                        Center(
                          child: Text('Everything is synced',
                              style: Theme.of(context).textTheme.titleMedium),
                        ),
                        const SizedBox(height: 8),
                        Center(
                          child: Text(
                            'Interactions made offline appear here until they reach the server.',
                            textAlign: TextAlign.center,
                            style: Theme.of(context).textTheme.bodySmall,
                          ),
                        ),
                      ],
                    )
                  : Column(
                      children: [
                        if (_syncing) const LinearProgressIndicator(),
                        Expanded(
                          child: ListView.separated(
                            itemCount: _actions.length,
                            separatorBuilder: (context, index) => const Divider(height: 1),
                            itemBuilder: (context, index) {
                              final a = _actions[index];
                              final title = _titleCache[a.episodeId] ?? 'Episode #${a.episodeId}';
                              return ListTile(
                                leading: Icon(_iconFor(a.type), color: Theme.of(context).primaryColor),
                                title: Text(a.description),
                                subtitle: Text(
                                  title,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                ),
                                trailing: a.retryCount > 0
                                    ? Tooltip(
                                        message: 'Failed ${a.retryCount} time(s)',
                                        child: Icon(Icons.error_outline, size: 18, color: Colors.orange[700]),
                                      )
                                    : null,
                              );
                            },
                          ),
                        ),
                      ],
                    ),
            ),
    );
  }
}
