import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';
import 'package:provider/provider.dart';

/// Whether podcasts the user subscribes to start with "Add new episodes to queue" on (#648).
/// Stored on the server, so it applies to every client.
class AutoQueueDefaultSetting extends StatefulWidget {
  const AutoQueueDefaultSetting({super.key});

  @override
  State<AutoQueueDefaultSetting> createState() => _AutoQueueDefaultSettingState();
}

class _AutoQueueDefaultSettingState extends State<AutoQueueDefaultSetting> {
  final _service = PinepodsService();
  bool? _enabled;
  int? _userId;

  @override
  void initState() {
    super.initState();
    final settings = Provider.of<SettingsBloc>(context, listen: false).currentSettings;
    if (settings.pinepodsServer == null || settings.pinepodsApiKey == null || settings.pinepodsUserId == null) {
      return;
    }
    _userId = settings.pinepodsUserId;
    _service.setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
    _service.getAutoQueueNewPodcasts(_userId!).then((enabled) {
      if (mounted) setState(() => _enabled = enabled);
    }).catchError((_) {});
  }

  Future<void> _set(bool enabled) async {
    final previous = _enabled;
    setState(() => _enabled = enabled);
    try {
      await _service.setAutoQueueNewPodcasts(_userId!, enabled);
    } catch (_) {
      if (!mounted) return;
      setState(() => _enabled = previous);
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(L.of(context)!.failed_to_update_auto_queue_new)),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_userId == null) return const SizedBox.shrink();
    return MergeSemantics(
      child: ListTile(
        leading: const Icon(PhosphorIconsRegular.queue),
        title: Text(L.of(context)!.auto_queue_new_podcasts),
        subtitle: Text(L.of(context)!.auto_queue_new_podcasts_description),
        trailing: _enabled == null
            ? const SizedBox(width: 20, height: 20, child: CircularProgressIndicator(strokeWidth: 2))
            : Switch(value: _enabled!, onChanged: _set),
        onTap: _enabled == null ? null : () => _set(!_enabled!),
      ),
    );
  }
}
