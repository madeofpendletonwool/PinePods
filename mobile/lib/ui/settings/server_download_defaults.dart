// lib/ui/settings/server_download_defaults.dart
//
// The user's defaults for downloads kept on the Pinepods server: delete an
// episode's server download once it's finished, and delete server downloads
// after a number of days. Each podcast's "Server downloads" sheet can override
// them. Hidden until signed in to a server.
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/core/download_cleanup.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:provider/provider.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';

class ServerDownloadDefaults extends StatefulWidget {
  const ServerDownloadDefaults({super.key});

  @override
  State<ServerDownloadDefaults> createState() => _ServerDownloadDefaultsState();
}

class _ServerDownloadDefaultsState extends State<ServerDownloadDefaults> {
  final PinepodsService _service = PinepodsService();
  int? _userId;
  bool _loaded = false;
  bool _deleteWhenFinished = false;
  int _deleteAfterDays = 0;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final settings =
        Provider.of<SettingsBloc>(context, listen: false).currentSettings;
    if (settings.pinepodsServer == null ||
        settings.pinepodsApiKey == null ||
        settings.pinepodsUserId == null) {
      return;
    }
    _service.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
    _userId = settings.pinepodsUserId!;
    try {
      final finished = await _service.getAutoDeleteOnComplete(_userId!);
      final days = await _service.getAutoDeleteDays(_userId!);
      if (!mounted) return;
      setState(() {
        _deleteWhenFinished = finished.enabled;
        _deleteAfterDays = days.days;
        _loaded = true;
      });
    } catch (_) {}
  }

  @override
  Widget build(BuildContext context) {
    if (!_loaded || _userId == null) return const SizedBox.shrink();
    final userId = _userId!;
    return Column(
      children: [
        MergeSemantics(
          child: ListTile(
            leading: const Icon(PhosphorIconsRegular.checks),
            title: const Text('Delete server downloads when finished'),
            subtitle: const Text(
                "Remove an episode's server download about an hour after you "
                'finish it. With server auto-download, the server keeps only '
                "what you haven't heard."),
            trailing: Switch.adaptive(
              value: _deleteWhenFinished,
              onChanged: (value) async {
                if (await _service.setAutoDeleteOnCompleteDefault(userId, value)) {
                  setState(() => _deleteWhenFinished = value);
                }
              },
            ),
          ),
        ),
        MergeSemantics(
          child: ListTile(
            leading: const Icon(PhosphorIconsRegular.clockCountdown),
            title: const Text('Delete server downloads after'),
            subtitle: const Text('Remove server downloads older than this.'),
            trailing: DropdownButton<int>(
              value: _deleteAfterDays,
              onChanged: (value) async {
                if (value == null) return;
                if (await _service.setAutoDeleteDaysDefault(userId, value)) {
                  setState(() => _deleteAfterDays = value);
                }
              },
              items: [
                for (final d in {...deleteAfterDayChoices, _deleteAfterDays})
                  DropdownMenuItem(value: d, child: Text(deleteAfterDaysLabel(d))),
              ],
            ),
          ),
        ),
      ],
    );
  }
}
