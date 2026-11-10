import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';
import 'package:provider/provider.dart';
import 'package:pinepods_mobile/l10n/L.dart';

/// Where Add to queue puts an episode by default. Stored on the server, so it
/// applies to every client.
class QueuePositionSetting extends StatefulWidget {
  const QueuePositionSetting({super.key});

  @override
  State<QueuePositionSetting> createState() => _QueuePositionSettingState();
}

class _QueuePositionSettingState extends State<QueuePositionSetting> {
  final _service = PinepodsService();
  String? _position;
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
    _service.getQueueAddPosition(_userId!).then((position) {
      if (mounted) setState(() => _position = position);
    }).catchError((_) {});
  }

  Future<void> _set(String position) async {
    final previous = _position;
    setState(() => _position = position);
    try {
      await _service.setQueueAddPosition(_userId!, position);
    } catch (_) {
      if (!mounted) return;
      setState(() => _position = previous);
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text(L.of(context)!.could_not_save_queue_position)),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_userId == null) return const SizedBox.shrink();
    return MergeSemantics(
      child: ListTile(
        leading: const Icon(PhosphorIconsRegular.queue),
        title: Text(L.of(context)!.add_to_queue_position),
        subtitle: Text(L.of(context)!.add_to_queue_position_description),
        trailing: _position == null
            ? const SizedBox(width: 20, height: 20, child: CircularProgressIndicator(strokeWidth: 2))
            : DropdownButton<String>(
                value: _position,
                underline: Container(),
                items: [
                  DropdownMenuItem(value: 'bottom', child: Text(L.of(context)!.queue_bottom)),
                  DropdownMenuItem(value: 'top', child: Text(L.of(context)!.queue_top)),
                ],
                onChanged: (value) {
                  if (value != null && value != _position) _set(value);
                },
              ),
      ),
    );
  }
}
