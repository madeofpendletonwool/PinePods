// lib/ui/pinepods/people/add_people_sheet.dart
//
// Bottom sheet for adding people to a podcast or episode. Contributions go
// through the PinePods server to PodPeopleDB, where they're live right away
// (marked unverified) for everyone.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:pinepods_mobile/entities/people.dart';
import 'package:pinepods_mobile/ui/pinepods/people/people_widgets.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';

/// Show the sheet. Resolves to everyone credited afterwards, or null if the
/// user closed it without adding anyone.
Future<List<CreditedPerson>?> showAddPeopleSheet(
  BuildContext context, {
  required int podcastId,
  int? episodeId,
}) {
  return showModalBottomSheet<List<CreditedPerson>>(
    context: context,
    isScrollControlled: true,
    showDragHandle: true,
    useSafeArea: true,
    builder: (context) => _AddPeopleSheet(podcastId: podcastId, episodeId: episodeId),
  );
}

class _Pending {
  final String name;
  final int? personId;
  final int? podpeopleId;

  const _Pending(this.name, {this.personId, this.podpeopleId});

  bool get existing => personId != null || podpeopleId != null;
}

class _AddPeopleSheet extends StatefulWidget {
  final int podcastId;
  final int? episodeId;

  const _AddPeopleSheet({required this.podcastId, this.episodeId});

  @override
  State<_AddPeopleSheet> createState() => _AddPeopleSheetState();
}

class _AddPeopleSheetState extends State<_AddPeopleSheet> {
  final _controller = TextEditingController();
  final _focus = FocusNode();
  final List<_Pending> _pending = [];
  late String _role = widget.episodeId != null ? roleGuest : roleHost;

  List<PersonHit> _suggestions = [];
  Timer? _debounce;
  int _searchSeq = 0;
  String _lastText = '';

  bool _submitting = false;
  String? _error;

  @override
  void dispose() {
    _debounce?.cancel();
    _controller.dispose();
    _focus.dispose();
    super.dispose();
  }

  void _add(_Pending item) {
    final lower = item.name.toLowerCase();
    if (item.name.isEmpty || _pending.any((p) => p.name.toLowerCase() == lower)) return;
    _pending.add(item);
  }

  void _setText(String text) {
    _controller.value = TextEditingValue(
      text: text,
      selection: TextSelection.collapsed(offset: text.length),
    );
    _lastText = text;
  }

  void _onChanged(String text) {
    // A jump of more than one character is a paste: split it completely,
    // including "and"/"&". While typing, only commas and semicolons split.
    final pasted = text.length - _lastText.length > 1;
    _lastText = text;
    final (names, rest) = splitNames(text, all: pasted);
    if (names.isNotEmpty) {
      setState(() {
        for (final n in names) {
          _add(_Pending(n));
        }
        _error = null;
      });
      _setText(rest);
    }
    _scheduleSearch(_controller.text);
  }

  void _scheduleSearch(String text) {
    _debounce?.cancel();
    final query = text.trim();
    if (query.length < 2) {
      if (_suggestions.isNotEmpty) setState(() => _suggestions = []);
      return;
    }
    _debounce = Timer(const Duration(milliseconds: 250), () async {
      final service = signedInService(context);
      if (service == null) return;
      final seq = ++_searchSeq;
      List<PersonHit> hits;
      try {
        hits = await service.searchPeople(query);
      } catch (_) {
        hits = [];
      }
      // Only the latest query's results are kept.
      if (!mounted || seq != _searchSeq) return;
      setState(() => _suggestions = hits.take(6).toList());
    });
  }

  void _pick(PersonHit hit) {
    setState(() {
      _add(_Pending(hit.name, personId: hit.personId, podpeopleId: hit.podpeopleId));
      _suggestions = [];
      _error = null;
    });
    _searchSeq++;
    _setText('');
    _focus.requestFocus();
  }

  /// Move whatever is typed into the pending list.
  void _flushText() {
    final (names, _) = splitNames(_controller.text, all: true);
    for (final n in names) {
      _add(_Pending(n));
    }
    _setText('');
    _suggestions = [];
    _searchSeq++;
  }

  Future<void> _submit() async {
    setState(_flushText);
    if (_pending.isEmpty || _submitting) return;
    final service = signedInService(context);
    if (service == null) return;

    setState(() {
      _submitting = true;
      _error = null;
    });
    try {
      final result = await service.contributePeople(
        podcastId: widget.podcastId,
        episodeId: widget.episodeId,
        people: _pending
            .map((p) => ContributePerson(personId: p.personId, podpeopleId: p.podpeopleId, name: p.name, role: _role))
            .toList(),
      );
      if (!mounted) return;
      final message = _pending.length == 1 ? 'Added ${_pending.first.name}' : 'Added ${_pending.length} people';
      final messenger = ScaffoldMessenger.of(context);
      Navigator.of(context).pop(result);
      messenger.showSnackBar(SnackBar(content: Text(message)));
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _submitting = false;
        _error = e.toString().replaceFirst('Exception: ', '');
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final muted = theme.textTheme.bodySmall!.copyWith(color: theme.colorScheme.onSurfaceVariant);
    final typed = _controller.text.trim().isNotEmpty;
    final canSubmit = !_submitting && (_pending.isNotEmpty || typed);

    return Padding(
      padding: EdgeInsets.only(bottom: MediaQuery.of(context).viewInsets.bottom),
      child: SingleChildScrollView(
        padding: const EdgeInsets.fromLTRB(16, 0, 16, 16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              widget.episodeId != null ? 'Who\'s in this episode?' : 'Who hosts this show?',
              style: theme.textTheme.titleLarge!.copyWith(fontWeight: FontWeight.bold),
            ),
            const SizedBox(height: 4),
            Text(
              'Added to PodPeopleDB for everyone right away, marked unverified until reviewed.',
              style: muted,
            ),
            const SizedBox(height: 16),
            SegmentedButton<String>(
              segments: const [
                ButtonSegment(value: roleHost, label: Text('Host')),
                ButtonSegment(value: roleCoHost, label: Text('Co-host')),
                ButtonSegment(value: roleGuest, label: Text('Guest')),
              ],
              selected: {_role},
              showSelectedIcon: false,
              onSelectionChanged: _submitting ? null : (s) => setState(() => _role = s.first),
            ),
            const SizedBox(height: 12),
            if (_pending.isNotEmpty) ...[
              Wrap(
                spacing: 8,
                runSpacing: 4,
                children: [
                  for (final p in _pending)
                    InputChip(
                      avatar: Icon(p.existing ? PhosphorIconsRegular.user : PhosphorIconsRegular.userPlus, size: 18),
                      label: Text(p.name),
                      onDeleted: _submitting ? null : () => setState(() => _pending.remove(p)),
                    ),
                ],
              ),
              const SizedBox(height: 8),
            ],
            TextField(
              controller: _controller,
              focusNode: _focus,
              autofocus: true,
              enabled: !_submitting,
              textCapitalization: TextCapitalization.words,
              textInputAction: TextInputAction.done,
              decoration: InputDecoration(
                hintText: _pending.isEmpty ? 'Name, or several: Alice, Bob and Carol' : 'Add another',
                prefixIcon: const Icon(PhosphorIconsRegular.magnifyingGlass),
                border: const OutlineInputBorder(),
              ),
              onChanged: _onChanged,
              onSubmitted: (_) {
                if (_suggestions.isNotEmpty && _controller.text.trim().isNotEmpty) {
                  _pick(_suggestions.first);
                } else {
                  setState(_flushText);
                }
                _focus.requestFocus();
              },
            ),
            if (_suggestions.isNotEmpty)
              Card(
                margin: const EdgeInsets.only(top: 4),
                child: Column(
                  children: [
                    for (final hit in _suggestions)
                      ListTile(
                        dense: true,
                        leading: PersonAvatar(url: hit.img, size: 36),
                        title: Text(hit.name),
                        subtitle: hit.knownFor != null
                            ? Text(hit.knownFor!, maxLines: 1, overflow: TextOverflow.ellipsis)
                            : null,
                        onTap: () => _pick(hit),
                      ),
                  ],
                ),
              ),
            if (_error != null) ...[
              const SizedBox(height: 8),
              Text(_error!, style: theme.textTheme.bodyMedium!.copyWith(color: theme.colorScheme.error)),
            ],
            const SizedBox(height: 16),
            FilledButton(
              onPressed: canSubmit ? _submit : null,
              child: _submitting
                  ? const SizedBox(width: 20, height: 20, child: CircularProgressIndicator(strokeWidth: 2))
                  : Text(_pending.length > 1 ? 'Add ${_pending.length} people' : 'Add'),
            ),
          ],
        ),
      ),
    );
  }
}
