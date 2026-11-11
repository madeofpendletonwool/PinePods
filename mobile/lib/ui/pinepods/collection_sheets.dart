// lib/ui/pinepods/collection_sheets.dart
// "Add to collection" picker and the create/edit collection sheet.
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/pinepods_collection.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/ui/icons/collection_icons.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';
import 'package:pinepods_mobile/ui/widgets/soft_ui.dart';
import 'package:provider/provider.dart';

/// A service with the signed-in credentials, plus the user id; null when signed out.
(PinepodsService, int)? collectionsClient(BuildContext context) {
  final s = Provider.of<SettingsBloc>(context, listen: false).currentSettings;
  if (s.pinepodsServer == null || s.pinepodsApiKey == null || s.pinepodsUserId == null) return null;
  final service = PinepodsService()..setCredentials(s.pinepodsServer!, s.pinepodsApiKey!);
  return (service, s.pinepodsUserId!);
}

void _snack(ScaffoldMessengerState? messenger, String text, {bool error = false}) {
  messenger?.showSnackBar(SnackBar(
    content: Text(text),
    backgroundColor: error ? Colors.red : null,
    duration: const Duration(seconds: 2),
  ));
}

/// Picker for adding episodes to collections. With one episode, ticking a
/// collection adds or removes it straight away; with several, picking a
/// collection adds them all and closes. Saved isn't listed: it has its own action.
Future<void> showAddToCollectionSheet(
  BuildContext context, {
  required List<(int, bool)> episodes,
}) async {
  final client = collectionsClient(context);
  if (client == null || episodes.isEmpty) return;
  await showModalBottomSheet<void>(
    context: context,
    useRootNavigator: true,
    isScrollControlled: true,
    backgroundColor: Theme.of(context).scaffoldBackgroundColor,
    shape: const RoundedRectangleBorder(borderRadius: BorderRadius.vertical(top: Radius.circular(16))),
    builder: (_) => _AddToCollectionSheet(
      service: client.$1,
      userId: client.$2,
      episodes: episodes,
      messenger: ScaffoldMessenger.maybeOf(context),
    ),
  );
}

class _AddToCollectionSheet extends StatefulWidget {
  final PinepodsService service;
  final int userId;
  final List<(int, bool)> episodes;
  final ScaffoldMessengerState? messenger;

  const _AddToCollectionSheet({
    required this.service,
    required this.userId,
    required this.episodes,
    this.messenger,
  });

  @override
  State<_AddToCollectionSheet> createState() => _AddToCollectionSheetState();
}

class _AddToCollectionSheetState extends State<_AddToCollectionSheet> {
  List<PinepodsCollection>? _collections;
  Set<int> _members = {};
  String? _error;
  bool _busy = false;

  bool get _isBulk => widget.episodes.length > 1;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      final cols = await widget.service.getCollections(widget.userId);
      var members = <int>{};
      if (!_isBulk) {
        final (id, yt) = widget.episodes.first;
        members = await widget.service.getEpisodeCollections(widget.userId, id, yt);
      }
      if (!mounted) return;
      setState(() {
        _collections = cols.where((c) => !c.isDefault).toList();
        _members = members;
      });
    } catch (e) {
      if (mounted) setState(() => _error = e.toString());
    }
  }

  Future<void> _toggle(PinepodsCollection c) async {
    final (id, yt) = widget.episodes.first;
    final add = !_members.contains(c.id);
    setState(() => add ? _members.add(c.id) : _members.remove(c.id));
    try {
      await widget.service.setEpisodeInCollection(c.id, widget.userId, id, yt, add);
    } catch (e) {
      if (!mounted) return;
      setState(() => add ? _members.remove(c.id) : _members.add(c.id));
      _snack(widget.messenger, '$e', error: true);
    }
  }

  Future<void> _addAll(int collectionId, String name) async {
    setState(() => _busy = true);
    try {
      await widget.service.bulkAddToCollection(collectionId, widget.userId, widget.episodes);
      if (!mounted) return;
      Navigator.of(context).pop();
      _snack(widget.messenger, L.current.added_episodes_to_collection(widget.episodes.length, name));
    } catch (e) {
      if (!mounted) return;
      setState(() => _busy = false);
      _snack(widget.messenger, '$e', error: true);
    }
  }

  Future<void> _createNew() async {
    final created = await showCollectionEditor(context);
    if (created == null || !mounted) return;
    if (_isBulk) {
      await _addAll(created.id, created.name);
      return;
    }
    final (id, yt) = widget.episodes.first;
    try {
      await widget.service.setEpisodeInCollection(created.id, widget.userId, id, yt, true);
    } catch (e) {
      _snack(widget.messenger, '$e', error: true);
    }
    await _load();
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final collections = _collections;
    return SafeArea(
      child: ConstrainedBox(
        constraints: BoxConstraints(maxHeight: MediaQuery.of(context).size.height * 0.7),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
              child: Row(
                children: [
                  const Icon(PhosphorIconsRegular.folderPlus),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      _isBulk ? L.of(context)!.add_episodes_to_collection_title(widget.episodes.length) : L.of(context)!.add_to_collection,
                      style: theme.textTheme.titleLarge,
                    ),
                  ),
                  if (_busy) const SizedBox(width: 20, height: 20, child: CircularProgressIndicator(strokeWidth: 2)),
                ],
              ),
            ),
            if (_error != null)
              Padding(padding: const EdgeInsets.all(24), child: Text(_error!))
            else if (collections == null)
              const Padding(padding: EdgeInsets.all(24), child: Center(child: CircularProgressIndicator()))
            else
              Flexible(
                child: ListView(
                  shrinkWrap: true,
                  children: [
                    if (collections.isEmpty)
                      Padding(
                        padding: const EdgeInsets.fromLTRB(16, 8, 16, 8),
                        child: Text(L.of(context)!.no_collections_yet, style: TextStyle(color: theme.hintColor)),
                      ),
                    for (final c in collections)
                      ListTile(
                        enabled: !_busy,
                        leading: Icon(collectionIcon(c.icon)),
                        title: Text(c.name),
                        subtitle: Text(L.of(context)!.collection_episode_count(c.episodeCount)),
                        trailing: _isBulk
                            ? null
                            : Icon(
                                _members.contains(c.id)
                                    ? PhosphorIconsFill.checkSquare
                                    : PhosphorIconsRegular.square,
                                color: _members.contains(c.id) ? theme.primaryColor : null,
                              ),
                        onTap: () => _isBulk ? _addAll(c.id, c.name) : _toggle(c),
                      ),
                    ListTile(
                      enabled: !_busy,
                      leading: const Icon(PhosphorIconsRegular.plus),
                      title: Text(L.of(context)!.new_collection),
                      onTap: _createNew,
                    ),
                  ],
                ),
              ),
          ],
        ),
      ),
    );
  }
}

/// Result of the collection editor.
class CollectionEdit {
  final int id;
  final String name;
  final bool deleted;
  const CollectionEdit(this.id, this.name, {this.deleted = false});
}

/// Create a collection, or edit/delete [existing]. Null when cancelled.
Future<CollectionEdit?> showCollectionEditor(BuildContext context, {PinepodsCollection? existing}) {
  final client = collectionsClient(context);
  if (client == null) return Future.value(null);
  return showModalBottomSheet<CollectionEdit>(
    context: context,
    useRootNavigator: true,
    isScrollControlled: true,
    backgroundColor: Theme.of(context).scaffoldBackgroundColor,
    shape: const RoundedRectangleBorder(borderRadius: BorderRadius.vertical(top: Radius.circular(16))),
    builder: (_) => _CollectionEditor(service: client.$1, userId: client.$2, existing: existing),
  );
}

class _CollectionEditor extends StatefulWidget {
  final PinepodsService service;
  final int userId;
  final PinepodsCollection? existing;

  const _CollectionEditor({required this.service, required this.userId, this.existing});

  @override
  State<_CollectionEditor> createState() => _CollectionEditorState();
}

class _CollectionEditorState extends State<_CollectionEditor> {
  late final TextEditingController _name = TextEditingController(text: widget.existing?.name ?? '');
  late final TextEditingController _description =
      TextEditingController(text: widget.existing?.description ?? '');
  late String _icon = widget.existing?.icon ?? 'ph-bookmark-simple';
  late final Set<String> _categories = {...?widget.existing?.autoAddCategories};
  List<String> _available = [];
  bool _backfill = false;
  bool _saving = false;
  String? _error;

  bool get _isEdit => widget.existing != null;

  @override
  void initState() {
    super.initState();
    widget.service.getUserCategories(widget.userId).then((cats) {
      if (mounted) setState(() => _available = cats);
    }).catchError((_) {});
  }

  @override
  void dispose() {
    _name.dispose();
    _description.dispose();
    super.dispose();
  }

  Future<void> _save() async {
    final name = _name.text.trim();
    if (name.isEmpty) {
      setState(() => _error = L.of(context)!.collection_name_required);
      return;
    }
    setState(() {
      _saving = true;
      _error = null;
    });
    final description = _description.text.trim();
    final categories = _categories.toList()..sort();
    final backfill = _backfill && categories.isNotEmpty;
    try {
      int id;
      if (_isEdit) {
        id = widget.existing!.id;
        await widget.service.updateCollection(id,
            name: name,
            description: description,
            icon: _icon,
            autoAddCategories: categories,
            backfill: backfill);
      } else {
        id = await widget.service.createCollection(widget.userId,
            name: name,
            description: description.isEmpty ? null : description,
            icon: _icon,
            autoAddCategories: categories.isEmpty ? null : categories,
            backfill: backfill);
      }
      if (mounted) Navigator.of(context).pop(CollectionEdit(id, name));
    } catch (e) {
      if (mounted) {
        setState(() {
          _saving = false;
          _error = e.toString().replaceFirst('Exception: ', '');
        });
      }
    }
  }

  Future<void> _delete() async {
    final existing = widget.existing!;
    final ok = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(L.of(context)!.delete_collection_confirm_title),
        content: Text(L.of(context)!.delete_collection_confirm_body(existing.name)),
        actions: [
          TextButton(onPressed: () => Navigator.pop(context, false), child: Text(L.of(context)!.cancel_button_label)),
          TextButton(onPressed: () => Navigator.pop(context, true), child: Text(L.of(context)!.delete_label)),
        ],
      ),
    );
    if (ok != true || !mounted) return;
    setState(() => _saving = true);
    try {
      await widget.service.deleteCollection(existing.id);
      if (mounted) Navigator.of(context).pop(CollectionEdit(existing.id, existing.name, deleted: true));
    } catch (e) {
      if (mounted) {
        setState(() {
          _saving = false;
          _error = e.toString().replaceFirst('Exception: ', '');
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final accent = theme.primaryColor;
    return Padding(
      padding: EdgeInsets.only(bottom: MediaQuery.of(context).viewInsets.bottom),
      child: SafeArea(
        child: ConstrainedBox(
          constraints: BoxConstraints(maxHeight: MediaQuery.of(context).size.height * 0.85),
          child: SingleChildScrollView(
            padding: const EdgeInsets.fromLTRB(16, 16, 16, 16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Text(_isEdit ? L.of(context)!.edit_collection : L.of(context)!.new_collection, style: theme.textTheme.titleLarge),
                const SizedBox(height: 16),
                TextField(
                  controller: _name,
                  autofocus: !_isEdit,
                  textCapitalization: TextCapitalization.sentences,
                  decoration: InputDecoration(labelText: L.of(context)!.collection_name),
                ),
                const SizedBox(height: 12),
                TextField(
                  controller: _description,
                  textCapitalization: TextCapitalization.sentences,
                  decoration: InputDecoration(labelText: L.of(context)!.description_optional),
                ),
                const SizedBox(height: 16),
                Text(L.of(context)!.icon, style: theme.textTheme.titleSmall),
                const SizedBox(height: 8),
                SizedBox(
                  height: 152,
                  child: GridView.count(
                    crossAxisCount: 7,
                    mainAxisSpacing: 6,
                    crossAxisSpacing: 6,
                    children: [
                      for (final entry in kCollectionIcons.entries)
                        Material(
                          color: entry.key == _icon ? accent : softFill(theme),
                          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
                          child: InkWell(
                            borderRadius: BorderRadius.circular(10),
                            onTap: () => setState(() => _icon = entry.key),
                            child: Icon(entry.value,
                                size: 22, color: entry.key == _icon ? onColor(accent) : softNeutral(theme)),
                          ),
                        ),
                    ],
                  ),
                ),
                const SizedBox(height: 16),
                Text(L.of(context)!.auto_add_by_category, style: theme.textTheme.titleSmall),
                const SizedBox(height: 4),
                Text(L.of(context)!.auto_add_by_category_body,
                    style: TextStyle(color: theme.hintColor, fontSize: 13)),
                const SizedBox(height: 8),
                if (_available.isEmpty)
                  Text(L.of(context)!.no_podcast_categories, style: TextStyle(color: theme.hintColor, fontSize: 13))
                else
                  Wrap(
                    spacing: 6,
                    runSpacing: 6,
                    children: [
                      for (final cat in _available)
                        SizedBox(
                          height: 34,
                          child: FilterPill(
                            label: cat,
                            selected: _categories.contains(cat),
                            onTap: () => setState(
                                () => _categories.contains(cat) ? _categories.remove(cat) : _categories.add(cat)),
                          ),
                        ),
                    ],
                  ),
                if (_categories.isNotEmpty)
                  CheckboxListTile(
                    contentPadding: EdgeInsets.zero,
                    value: _backfill,
                    onChanged: (v) => setState(() => _backfill = v ?? false),
                    title: Text(L.of(context)!.add_existing_matching_episodes),
                  ),
                if (_error != null) ...[
                  const SizedBox(height: 8),
                  Text(_error!, style: const TextStyle(color: Colors.red)),
                ],
                const SizedBox(height: 16),
                Row(
                  children: [
                    if (_isEdit && !widget.existing!.isDefault)
                      TextButton(
                        onPressed: _saving ? null : _delete,
                        style: TextButton.styleFrom(foregroundColor: Colors.red),
                        child: Text(L.of(context)!.delete_collection),
                      ),
                    const Spacer(),
                    FilledButton(
                      onPressed: _saving ? null : _save,
                      child: Text(_saving ? L.of(context)!.saving_ellipsis : (_isEdit ? L.of(context)!.save : L.of(context)!.create)),
                    ),
                  ],
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
