// lib/ui/pinepods/labels.dart
// Episode labels: the shared store, the chips on episode cards, the picker sheet, and the
// "Inbox & labels" settings page.
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/entities/episode_label.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';
import 'package:pinepods_mobile/ui/pinepods/collection_sheets.dart' show collectionsClient;

/// The user's labels and which episodes carry them, shared by every screen.
class LabelStore extends ChangeNotifier {
  LabelStore._();
  static final LabelStore instance = LabelStore._();

  List<EpisodeLabel> labels = const [];
  Map<(int, bool), List<int>> assignments = const {};
  String? _loadedFor;
  bool _loading = false;

  /// The inbox count, for the tab badge; null until known.
  final ValueNotifier<int?> inboxCount = ValueNotifier<int?>(null);

  EpisodeLabel? label(int id) {
    for (final l in labels) {
      if (l.id == id) return l;
    }
    return null;
  }

  List<EpisodeLabel> on(int episodeId, bool isYoutube) =>
      [for (final id in assignments[(episodeId, isYoutube)] ?? const <int>[]) if (label(id) != null) label(id)!];

  /// Load once per signed-in server and user.
  Future<void> ensureLoaded(BuildContext context) async {
    final client = collectionsClient(context);
    if (client == null) return;
    final key = '${client.$1.server}|${client.$2}';
    if (_loadedFor == key || _loading) return;
    await reload(client.$1, key: key);
  }

  Future<void> reload(PinepodsService service, {String? key}) async {
    _loading = true;
    try {
      final results = await Future.wait([service.getLabels(), service.getLabelAssignments()]);
      labels = results[0] as List<EpisodeLabel>;
      assignments = results[1] as Map<(int, bool), List<int>>;
      if (key != null) _loadedFor = key;
      notifyListeners();
    } catch (_) {
      // Labels are an extra; screens work without them.
    } finally {
      _loading = false;
    }
  }

  Future<void> refreshInboxCount(PinepodsService service) async {
    try {
      inboxCount.value = await service.getInboxCount();
    } catch (_) {}
  }

  void _apply(int labelId, List<(int, bool)> episodes, bool assigned) {
    final map = {for (final e in assignments.entries) e.key: List<int>.of(e.value)};
    for (final ep in episodes) {
      final ids = map.putIfAbsent(ep, () => []);
      ids.remove(labelId);
      if (assigned) ids.add(labelId);
      if (ids.isEmpty) map.remove(ep);
    }
    assignments = map;
    final count = map.values.where((v) => v.contains(labelId)).length;
    labels = [for (final l in labels) l.id == labelId ? l.copyWith(episodeCount: count) : l];
    notifyListeners();
  }

  /// Put a label on episodes or take it off; changes at once, and back if the server refuses.
  Future<void> setLabel(PinepodsService service, int labelId, List<(int, bool)> episodes, bool assigned) async {
    _apply(labelId, episodes, assigned);
    try {
      await service.setLabelOnEpisodes(labelId, episodes, assigned);
    } catch (e) {
      _apply(labelId, episodes, !assigned);
      rethrow;
    }
  }

  void addLabel(EpisodeLabel label) {
    labels = [...labels, label]..sort((a, b) => a.name.toLowerCase().compareTo(b.name.toLowerCase()));
    notifyListeners();
  }
}

/// The labels on one episode, as small chips; nothing when it has none.
class LabelChips extends StatelessWidget {
  final int episodeId;
  final bool isYoutube;

  const LabelChips({super.key, required this.episodeId, required this.isYoutube});

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: LabelStore.instance,
      builder: (context, _) {
        final labels = LabelStore.instance.on(episodeId, isYoutube);
        if (labels.isEmpty) return const SizedBox.shrink();
        return Padding(
          padding: const EdgeInsets.only(top: 4),
          child: Wrap(
            spacing: 4,
            runSpacing: 4,
            children: [for (final l in labels) LabelChip(label: l)],
          ),
        );
      },
    );
  }
}

class LabelChip extends StatelessWidget {
  final EpisodeLabel label;
  final bool selected;

  const LabelChip({super.key, required this.label, this.selected = false});

  @override
  Widget build(BuildContext context) {
    final color = labelColor(label.color);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 7, vertical: 1),
      decoration: BoxDecoration(
        color: color.withValues(alpha: selected ? 0.35 : 0.16),
        borderRadius: BorderRadius.circular(999),
        border: Border.all(color: color.withValues(alpha: 0.4)),
      ),
      child: Text(
        label.name,
        style: TextStyle(fontSize: 11, fontWeight: FontWeight.w600, color: Color.lerp(color, Theme.of(context).colorScheme.onSurface, 0.3)),
      ),
    );
  }
}

class _ColorPicker extends StatelessWidget {
  final String value;
  final ValueChanged<String> onChanged;

  const _ColorPicker({required this.value, required this.onChanged});

  @override
  Widget build(BuildContext context) {
    return Wrap(
      spacing: 6,
      runSpacing: 6,
      children: [
        for (final c in labelColors)
          GestureDetector(
            onTap: () => onChanged(c),
            child: Container(
              width: 26,
              height: 26,
              decoration: BoxDecoration(
                color: labelColor(c),
                shape: BoxShape.circle,
                border: Border.all(
                  color: c == value ? Theme.of(context).colorScheme.onSurface : Colors.transparent,
                  width: 2.5,
                ),
              ),
            ),
          ),
      ],
    );
  }
}

/// Picker for the labels on one or more episodes; a new label can be made here and goes
/// straight onto them.
Future<void> showLabelsSheet(BuildContext context, {required List<(int, bool)> episodes}) async {
  final client = collectionsClient(context);
  if (client == null || episodes.isEmpty) return;
  LabelStore.instance.ensureLoaded(context);
  await showModalBottomSheet<void>(
    context: context,
    useRootNavigator: true,
    isScrollControlled: true,
    backgroundColor: Theme.of(context).scaffoldBackgroundColor,
    shape: const RoundedRectangleBorder(borderRadius: BorderRadius.vertical(top: Radius.circular(16))),
    builder: (_) => _LabelsSheet(service: client.$1, episodes: episodes, messenger: ScaffoldMessenger.maybeOf(context)),
  );
}

class _LabelsSheet extends StatefulWidget {
  final PinepodsService service;
  final List<(int, bool)> episodes;
  final ScaffoldMessengerState? messenger;

  const _LabelsSheet({required this.service, required this.episodes, this.messenger});

  @override
  State<_LabelsSheet> createState() => _LabelsSheetState();
}

class _LabelsSheetState extends State<_LabelsSheet> {
  final _name = TextEditingController();
  String _color = 'blue';
  bool _busy = false;

  LabelStore get _store => LabelStore.instance;

  bool _isOn(int labelId) => widget.episodes.every((e) => (_store.assignments[e] ?? const []).contains(labelId));

  Future<void> _toggle(EpisodeLabel l) async {
    try {
      await _store.setLabel(widget.service, l.id, widget.episodes, !_isOn(l.id));
    } catch (e) {
      widget.messenger?.showSnackBar(SnackBar(content: Text('$e'), backgroundColor: Colors.red));
    }
  }

  Future<void> _create() async {
    final name = _name.text.trim();
    if (name.isEmpty || _busy) return;
    setState(() => _busy = true);
    try {
      final label = await widget.service.createLabel(name, _color);
      _store.addLabel(label);
      await _store.setLabel(widget.service, label.id, widget.episodes, true);
      _name.clear();
    } catch (e) {
      widget.messenger?.showSnackBar(SnackBar(content: Text('$e'), backgroundColor: Colors.red));
    }
    if (mounted) setState(() => _busy = false);
  }

  @override
  void dispose() {
    _name.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: EdgeInsets.only(bottom: MediaQuery.of(context).viewInsets.bottom),
      child: SafeArea(
        child: ConstrainedBox(
          constraints: BoxConstraints(maxHeight: MediaQuery.of(context).size.height * 0.75),
          child: ListenableBuilder(
            listenable: _store,
            builder: (context, _) => Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Padding(
                  padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
                  child: Row(
                    children: [
                      const Icon(PhosphorIconsRegular.tag),
                      const SizedBox(width: 8),
                      Expanded(
                        child: Text(
                          widget.episodes.length > 1
                              ? L.of(context)!.label_episodes_title(widget.episodes.length)
                              : L.of(context)!.labels,
                          style: theme.textTheme.titleLarge,
                        ),
                      ),
                      if (_busy) const SizedBox(width: 20, height: 20, child: CircularProgressIndicator(strokeWidth: 2)),
                    ],
                  ),
                ),
                Flexible(
                  child: ListView(
                    shrinkWrap: true,
                    children: [
                      if (_store.labels.isEmpty)
                        Padding(
                          padding: const EdgeInsets.fromLTRB(16, 8, 16, 8),
                          child: Text(L.of(context)!.labels_none_yet, style: TextStyle(color: theme.hintColor)),
                        ),
                      for (final l in _store.labels)
                        ListTile(
                          leading: Icon(PhosphorIconsFill.circle, color: labelColor(l.color), size: 18),
                          title: Text(l.name),
                          trailing: Icon(
                            _isOn(l.id) ? PhosphorIconsFill.checkSquare : PhosphorIconsRegular.square,
                            color: _isOn(l.id) ? theme.primaryColor : null,
                          ),
                          onTap: () => _toggle(l),
                        ),
                    ],
                  ),
                ),
                const Divider(height: 1),
                Padding(
                  padding: const EdgeInsets.fromLTRB(16, 12, 16, 12),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        children: [
                          Expanded(
                            child: TextField(
                              controller: _name,
                              maxLength: 64,
                              decoration: InputDecoration(hintText: L.of(context)!.new_label, counterText: ''),
                              onSubmitted: (_) => _create(),
                            ),
                          ),
                          const SizedBox(width: 8),
                          FilledButton(onPressed: _busy ? null : _create, child: Text(L.of(context)!.label_add)),
                        ],
                      ),
                      const SizedBox(height: 10),
                      _ColorPicker(value: _color, onChanged: (c) => setState(() => _color = c)),
                    ],
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// Settings: how far back the inbox goes, and the user's labels.
class InboxSettingsPage extends StatefulWidget {
  const InboxSettingsPage({super.key});

  @override
  State<InboxSettingsPage> createState() => _InboxSettingsPageState();
}

class _InboxSettingsPageState extends State<InboxSettingsPage> {
  static const _dayChoices = [7, 14, 30, 60, 90, 180, 365];
  int? _days;
  PinepodsService? _service;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => _load());
  }

  Future<void> _load() async {
    final client = collectionsClient(context);
    if (client == null) return;
    _service = client.$1;
    await LabelStore.instance.reload(client.$1);
    try {
      final days = await client.$1.getInboxDays();
      if (mounted) setState(() => _days = days);
    } catch (_) {}
  }

  Future<void> _setDays(int days) async {
    final service = _service;
    if (service == null) return;
    final messenger = ScaffoldMessenger.of(context);
    try {
      final saved = await service.setInboxDays(days);
      if (mounted) setState(() => _days = saved);
      LabelStore.instance.refreshInboxCount(service);
    } catch (e) {
      messenger.showSnackBar(SnackBar(content: Text('$e'), backgroundColor: Colors.red));
    }
  }

  Future<void> _edit({EpisodeLabel? label}) async {
    final service = _service;
    if (service == null) return;
    final name = TextEditingController(text: label?.name ?? '');
    var color = label?.color ?? 'blue';
    final messenger = ScaffoldMessenger.of(context);
    final ok = await showDialog<bool>(
      context: context,
      builder: (context) => StatefulBuilder(
        builder: (context, setLocal) => AlertDialog(
          title: Text(label == null ? L.of(context)!.new_label : L.of(context)!.label_rename),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              TextField(controller: name, autofocus: true, maxLength: 64),
              const SizedBox(height: 8),
              _ColorPicker(value: color, onChanged: (c) => setLocal(() => color = c)),
            ],
          ),
          actions: [
            TextButton(onPressed: () => Navigator.pop(context, false), child: Text(L.of(context)!.cancel_button_label)),
            FilledButton(onPressed: () => Navigator.pop(context, true), child: Text(L.of(context)!.save)),
          ],
        ),
      ),
    );
    if (ok != true || name.text.trim().isEmpty) return;
    try {
      if (label == null) {
        await service.createLabel(name.text.trim(), color);
      } else {
        await service.updateLabel(label.id, name.text.trim(), color);
      }
      await LabelStore.instance.reload(service);
    } catch (e) {
      messenger.showSnackBar(SnackBar(content: Text('$e'), backgroundColor: Colors.red));
    }
  }

  Future<void> _delete(EpisodeLabel label) async {
    final service = _service;
    if (service == null) return;
    final messenger = ScaffoldMessenger.of(context);
    final ok = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        content: Text(L.of(context)!.label_delete_confirm(label.name)),
        actions: [
          TextButton(onPressed: () => Navigator.pop(context, false), child: Text(L.of(context)!.cancel_button_label)),
          FilledButton(onPressed: () => Navigator.pop(context, true), child: Text(L.of(context)!.label_delete)),
        ],
      ),
    );
    if (ok != true) return;
    try {
      await service.deleteLabel(label.id);
      await LabelStore.instance.reload(service);
    } catch (e) {
      messenger.showSnackBar(SnackBar(content: Text('$e'), backgroundColor: Colors.red));
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final days = _days;
    final choices = {..._dayChoices, ?days}.toList()..sort();
    return Scaffold(
      appBar: AppBar(title: Text(L.of(context)!.inbox_and_labels)),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: () => _edit(),
        icon: const Icon(PhosphorIconsRegular.plus),
        label: Text(L.of(context)!.new_label),
      ),
      body: ListenableBuilder(
        listenable: LabelStore.instance,
        builder: (context, _) => ListView(
          padding: const EdgeInsets.only(bottom: 96),
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 16, 16, 4),
              child: Text(L.of(context)!.inbox_description, style: TextStyle(color: theme.hintColor)),
            ),
            ListTile(
              title: Text(L.of(context)!.inbox_window),
              subtitle: Text(L.of(context)!.inbox_window_description),
              trailing: days == null
                  ? const SizedBox(width: 20, height: 20, child: CircularProgressIndicator(strokeWidth: 2))
                  : DropdownButton<int>(
                      value: days,
                      onChanged: (v) => v == null ? null : _setDays(v),
                      items: [
                        for (final d in choices) DropdownMenuItem(value: d, child: Text(L.of(context)!.inbox_days(d))),
                      ],
                    ),
            ),
            const Divider(),
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 8, 16, 4),
              child: Text(L.of(context)!.labels, style: theme.textTheme.titleMedium),
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 0, 16, 8),
              child: Text(L.of(context)!.labels_description, style: TextStyle(color: theme.hintColor)),
            ),
            if (LabelStore.instance.labels.isEmpty)
              Padding(
                padding: const EdgeInsets.all(16),
                child: Text(L.of(context)!.labels_none_yet, style: TextStyle(color: theme.hintColor)),
              ),
            for (final l in LabelStore.instance.labels)
              ListTile(
                leading: Icon(PhosphorIconsFill.circle, color: labelColor(l.color), size: 18),
                title: Text(l.name),
                subtitle: Text(L.of(context)!.label_episode_count(l.episodeCount)),
                onTap: () => _edit(label: l),
                trailing: IconButton(
                  tooltip: L.of(context)!.label_delete,
                  icon: const Icon(PhosphorIconsRegular.trash),
                  onPressed: () => _delete(l),
                ),
              ),
          ],
        ),
      ),
    );
  }
}
