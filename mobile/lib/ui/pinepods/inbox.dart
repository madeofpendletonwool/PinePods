// lib/ui/pinepods/inbox.dart
// The inbox: new episodes not dealt with yet. Swipe right to queue, left to remove; queueing,
// saving, playing, finishing or dismissing one anywhere takes it out.
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/episode_label.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/services/global_services.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';
import 'package:pinepods_mobile/ui/pinepods/episode_details.dart';
import 'package:pinepods_mobile/ui/pinepods/labels.dart';
import 'package:pinepods_mobile/ui/pinepods/podcast_nav.dart';
import 'package:pinepods_mobile/ui/utils/player_utils.dart';
import 'package:pinepods_mobile/ui/widgets/episode_context_menu.dart';
import 'package:pinepods_mobile/ui/widgets/pinepods_episode_card.dart';
import 'package:pinepods_mobile/ui/widgets/server_error_page.dart';
import 'package:pinepods_mobile/ui/widgets/soft_ui.dart';
import 'package:pinepods_mobile/services/error_handling_service.dart';
import 'package:provider/provider.dart';
import 'package:sliver_tools/sliver_tools.dart';

class PinepodsInbox extends StatefulWidget {
  const PinepodsInbox({super.key});

  @override
  State<PinepodsInbox> createState() => _PinepodsInboxState();
}

class _PinepodsInboxState extends State<PinepodsInbox> {
  static const _pageSize = 50;

  final PinepodsService _service = PinepodsService();
  InboxQuery _query = const InboxQuery();
  List<PinepodsEpisode> _episodes = [];
  int _total = 0;
  int _removed = 0;
  bool _loading = true;
  bool _loadingMore = false;
  String _error = '';
  int? _userId;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => _load());
  }

  bool _signIn() {
    final s = Provider.of<SettingsBloc>(context, listen: false).currentSettings;
    if (s.pinepodsServer == null || s.pinepodsApiKey == null || s.pinepodsUserId == null) return false;
    _service.setCredentials(s.pinepodsServer!, s.pinepodsApiKey!);
    _userId = s.pinepodsUserId;
    return true;
  }

  Future<void> _load() async {
    if (!mounted) return;
    if (!_signIn()) {
      setState(() {
        _error = L.of(context)!.not_connected_please_login;
        _loading = false;
      });
      return;
    }
    setState(() {
      _loading = true;
      _error = '';
    });
    LabelStore.instance.ensureLoaded(context);
    try {
      final page = await _service.getInbox(_query, limit: _pageSize);
      if (!mounted) return;
      setState(() {
        _episodes = page.episodes;
        _total = page.total;
        _removed = 0;
        _loading = false;
      });
      if (_query.isEverything) LabelStore.instance.inboxCount.value = page.total;
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _error = e.toString();
        _loading = false;
      });
    }
  }

  Future<void> _loadMore() async {
    final offset = _episodes.length;
    if (_loadingMore || offset + _removed >= _total) return;
    setState(() => _loadingMore = true);
    try {
      // Removed items no longer count towards the server's offset.
      final page = await _service.getInbox(_query, limit: _pageSize, offset: offset);
      if (!mounted) return;
      final seen = {for (final e in _episodes) (e.episodeId, e.isYoutube)};
      setState(() {
        _episodes.addAll(page.episodes.where((e) => !seen.contains((e.episodeId, e.isYoutube))));
        _total = page.total + _removed;
        _loadingMore = false;
      });
    } catch (_) {
      if (mounted) setState(() => _loadingMore = false);
    }
  }

  void _setQuery(InboxQuery q) {
    setState(() => _query = q);
    _load();
  }

  int get _remaining => (_total - _removed).clamp(0, 1 << 31);

  /// Drop an episode from the list; returns where it was, for undo.
  int _take(PinepodsEpisode ep) {
    final i = _episodes.indexWhere((e) => e.episodeId == ep.episodeId && e.isYoutube == ep.isYoutube);
    if (i >= 0) {
      setState(() {
        _episodes.removeAt(i);
        _removed++;
      });
      final count = LabelStore.instance.inboxCount.value;
      if (count != null && count > 0) LabelStore.instance.inboxCount.value = count - 1;
    }
    return i;
  }

  void _putBack(PinepodsEpisode ep, int index) {
    if (!mounted) return;
    setState(() {
      _episodes.insert(index.clamp(0, _episodes.length), ep);
      _removed = (_removed - 1).clamp(0, _removed);
    });
    final count = LabelStore.instance.inboxCount.value;
    if (count != null) LabelStore.instance.inboxCount.value = count + 1;
  }

  void _snack(String text, {VoidCallback? undo, bool error = false}) {
    final messenger = ScaffoldMessenger.of(context);
    messenger.hideCurrentSnackBar();
    messenger.showSnackBar(SnackBar(
      content: Text(text),
      backgroundColor: error ? Colors.red : null,
      duration: const Duration(seconds: 4),
      action: undo == null ? null : SnackBarAction(label: L.current.inbox_undo, onPressed: undo),
    ));
  }

  Future<void> _dismiss(PinepodsEpisode ep) async {
    final index = _take(ep);
    try {
      await _service.dismissInbox([(ep.episodeId, ep.isYoutube)]);
      if (!mounted) return;
      _snack(L.current.inbox_dismissed, undo: () async {
        try {
          await _service.restoreInbox([(ep.episodeId, ep.isYoutube)]);
          _putBack(ep, index);
        } catch (e) {
          _snack('$e', error: true);
        }
      });
    } catch (e) {
      _putBack(ep, index);
      _snack('$e', error: true);
    }
  }

  Future<void> _queue(PinepodsEpisode ep, {String position = 'bottom'}) async {
    final userId = _userId;
    if (userId == null) return;
    final index = _take(ep);
    try {
      final ok = await _service.queueEpisode(ep.episodeId, userId, ep.isYoutube, position: position);
      if (!ok) throw Exception(L.current.failed_to_update_queue);
      if (!mounted) return;
      _snack(L.current.inbox_queued, undo: () async {
        try {
          await _service.removeQueuedEpisode(ep.episodeId, userId, ep.isYoutube);
          _putBack(ep, index);
        } catch (e) {
          _snack('$e', error: true);
        }
      });
    } catch (e) {
      _putBack(ep, index);
      _snack('$e', error: true);
    }
  }

  /// Run an action that takes the episode out of the inbox; put it back if it fails.
  Future<void> _handled(PinepodsEpisode ep, Future<bool> Function(int userId) action, String done) async {
    final userId = _userId;
    if (userId == null) return;
    final index = _take(ep);
    try {
      if (!await action(userId)) throw Exception(done);
      if (mounted) _snack(done);
    } catch (e) {
      _putBack(ep, index);
      _snack('$e', error: true);
    }
  }

  Future<void> _play(PinepodsEpisode ep) async {
    final audio = GlobalServices.pinepodsAudioService;
    if (audio == null) return;
    try {
      await playPinepodsEpisodeWithOptionalFullScreen(context, audio, ep, resume: ep.isStarted);
    } catch (e) {
      _snack(L.current.failed_to_play_episode(e.toString()), error: true);
    }
  }

  Future<void> _clear() async {
    final count = _remaining;
    final ok = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        content: Text(L.of(context)!.inbox_clear_confirm(count)),
        actions: [
          TextButton(onPressed: () => Navigator.pop(context, false), child: Text(L.of(context)!.cancel_button_label)),
          FilledButton(
            onPressed: () => Navigator.pop(context, true),
            child: Text(_query.isEverything ? L.of(context)!.inbox_clear : L.of(context)!.inbox_clear_shown),
          ),
        ],
      ),
    );
    if (ok != true || !mounted) return;
    final loaded = [for (final e in _episodes) (e.episodeId, e.isYoutube)];
    try {
      final n = await _service.clearInbox(_query);
      if (!mounted) return;
      await _load();
      LabelStore.instance.refreshInboxCount(_service);
      // Undo brings back what was loaded; the rest stays dismissed.
      _snack(L.current.inbox_cleared(n), undo: () async {
        try {
          await _service.restoreInbox(loaded);
          await _load();
          LabelStore.instance.refreshInboxCount(_service);
        } catch (e) {
          _snack('$e', error: true);
        }
      });
    } catch (e) {
      _snack('$e', error: true);
    }
  }

  void _showMenu(PinepodsEpisode ep) {
    final pageContext = context;
    showDialog(
      context: context,
      barrierColor: Colors.black.withValues(alpha: 0.3),
      builder: (context) => EpisodeContextMenu(
        episode: ep,
        onRemoveFromInbox: () {
          Navigator.of(context).pop();
          _dismiss(ep);
        },
        onSave: () {
          Navigator.of(context).pop();
          _handled(ep, (uid) => _service.saveEpisode(ep.episodeId, uid, ep.isYoutube), L.current.episode_saved);
        },
        onDownload: () {
          Navigator.of(context).pop();
          _handled(ep, (uid) => _service.downloadEpisode(ep.episodeId, uid, ep.isYoutube), L.current.episode_download_queued);
        },
        onQueue: () {
          Navigator.of(context).pop();
          _queue(ep);
        },
        onQueueAt: (position) {
          Navigator.of(context).pop();
          _queue(ep, position: position);
        },
        onMarkComplete: () {
          Navigator.of(context).pop();
          _handled(ep, (uid) => _service.markEpisodeCompleted(ep.episodeId, uid, ep.isYoutube), L.current.marked_as_complete);
        },
        onDismiss: () => Navigator.of(context).pop(),
        onPodcastTap: () {
          Navigator.of(context).pop();
          navigateToPodcastById(pageContext, ep.podcastId,
              fallbackTitle: ep.podcastName, fallbackArtwork: ep.episodeArtwork);
        },
      ),
    );
  }

  Widget _swipeBackground(BuildContext context, {required bool start}) {
    final color = start ? Theme.of(context).primaryColor : Colors.red[400]!;
    return Container(
      margin: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
      padding: const EdgeInsets.symmetric(horizontal: 20),
      alignment: start ? Alignment.centerLeft : Alignment.centerRight,
      decoration: BoxDecoration(color: color.withValues(alpha: 0.18), borderRadius: BorderRadius.circular(12)),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(start ? PhosphorIconsRegular.queue : PhosphorIconsRegular.x, color: color),
          const SizedBox(width: 8),
          Text(start ? L.of(context)!.add_to_queue : L.of(context)!.inbox_remove,
              style: TextStyle(color: color, fontWeight: FontWeight.w600)),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    if (_loading) {
      return const SliverFillRemaining(hasScrollBody: false, child: Center(child: CircularProgressIndicator()));
    }
    if (_error.isNotEmpty) {
      return SliverServerErrorPage(
        errorMessage: _error.isServerConnectionError ? null : _error,
        onRetry: _load,
        title: L.of(context)!.inbox_unavailable,
        subtitle: _error.isServerConnectionError ? L.of(context)!.unable_to_connect_server : L.of(context)!.inbox_failed_to_load,
      );
    }

    final store = LabelStore.instance;
    final showFooter = _loadingMore || _episodes.length + _removed < _total;
    return MultiSliver(
      children: [
        SliverToBoxAdapter(
          child: SoftSectionHeader(
            title: L.of(context)!.inbox,
            count: _remaining,
            trailing: [
              IconButton(
                tooltip: _query.oldestFirst ? L.of(context)!.inbox_oldest_first : L.of(context)!.inbox_newest_first,
                icon: Icon(_query.oldestFirst ? PhosphorIconsRegular.sortAscending : PhosphorIconsRegular.sortDescending),
                onPressed: () => _setQuery(_query.copyWith(oldestFirst: !_query.oldestFirst)),
              ),
              PopupMenuButton<String>(
                icon: const Icon(PhosphorIconsRegular.dotsThreeVertical),
                onSelected: (v) {
                  switch (v) {
                    case 'label':
                      showLabelsSheet(context, episodes: [for (final e in _episodes) (e.episodeId, e.isYoutube)]);
                      break;
                    case 'clear':
                      _clear();
                      break;
                    case 'refresh':
                      _load();
                      break;
                    case 'settings':
                      Navigator.push(context, MaterialPageRoute<void>(builder: (_) => const InboxSettingsPage()))
                          .then((_) => _load());
                      break;
                  }
                },
                itemBuilder: (context) => [
                  PopupMenuItem(value: 'refresh', child: Text(L.of(context)!.refresh)),
                  PopupMenuItem(value: 'label', enabled: _episodes.isNotEmpty, child: Text(L.of(context)!.inbox_label_shown)),
                  PopupMenuItem(
                    value: 'clear',
                    enabled: _remaining > 0,
                    child: Text(_query.isEverything ? L.of(context)!.inbox_clear : L.of(context)!.inbox_clear_shown),
                  ),
                  PopupMenuItem(value: 'settings', child: Text(L.of(context)!.inbox_settings)),
                ],
              ),
            ],
          ),
        ),
        ListenableBuilder(
          listenable: store,
          builder: (context, _) => ListToolbar.sliver(
            context,
            ListToolbar<void>(
              filters: [
                ToolbarFilter(
                  label: L.of(context)!.filter_all,
                  selected: _query.labelId == null && !_query.unlabeled,
                  onTap: () => _setQuery(_query.copyWith(labelId: () => null, unlabeled: false)),
                ),
                if (store.labels.isNotEmpty)
                  ToolbarFilter(
                    label: L.of(context)!.inbox_unlabeled,
                    selected: _query.labelId == null && _query.unlabeled,
                    onTap: () => _setQuery(_query.copyWith(labelId: () => null, unlabeled: true)),
                  ),
                for (final l in store.labels)
                  ToolbarFilter(
                    label: l.name,
                    icon: PhosphorIconsFill.tag,
                    selected: _query.labelId == l.id,
                    onTap: () => _setQuery(_query.copyWith(labelId: () => l.id, unlabeled: false)),
                  ),
              ],
            ),
          ),
        ),
        if (_episodes.isEmpty && !showFooter)
          SliverToBoxAdapter(
            child: SoftEmptyState(
              icon: PhosphorIconsRegular.tray,
              title: L.of(context)!.inbox_empty_title,
              body: L.of(context)!.inbox_empty_body,
            ),
          )
        else
          SliverList(
            delegate: SliverChildBuilderDelegate(
              (context, index) {
                if (index == _episodes.length) {
                  if (!_loadingMore) WidgetsBinding.instance.addPostFrameCallback((_) => _loadMore());
                  return const Padding(
                    padding: EdgeInsets.symmetric(vertical: 16),
                    child: Center(child: CircularProgressIndicator()),
                  );
                }
                final ep = _episodes[index];
                return Dismissible(
                  key: ValueKey(('inbox', ep.episodeId, ep.isYoutube)),
                  background: _swipeBackground(context, start: true),
                  secondaryBackground: _swipeBackground(context, start: false),
                  onDismissed: (direction) {
                    if (direction == DismissDirection.startToEnd) {
                      _queue(ep);
                    } else {
                      _dismiss(ep);
                    }
                  },
                  child: PinepodsEpisodeCard(
                    episode: ep,
                    onTap: () => Navigator.push(
                      context,
                      MaterialPageRoute(builder: (_) => PinepodsEpisodeDetails(initialEpisode: ep)),
                    ),
                    onLongPress: () => _showMenu(ep),
                    onPlayPressed: () => _play(ep),
                  ),
                );
              },
              childCount: _episodes.length + (showFooter ? 1 : 0),
            ),
          ),
      ],
    );
  }
}
