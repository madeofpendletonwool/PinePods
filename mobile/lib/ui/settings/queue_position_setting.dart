import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';
import 'package:provider/provider.dart';

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
        const SnackBar(content: Text('Could not save the queue position')),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_userId == null) return const SizedBox.shrink();
    return MergeSemantics(
      child: ListTile(
        leading: const Icon(PhosphorIconsRegular.queue),
        title: const Text('Add to queue position'),
        subtitle: const Text('Where Add to Queue puts an episode. The arrow beside it picks the other end.'),
        trailing: _position == null
            ? const SizedBox(width: 20, height: 20, child: CircularProgressIndicator(strokeWidth: 2))
            : DropdownButton<String>(
                value: _position,
                underline: Container(),
                items: const [
                  DropdownMenuItem(value: 'bottom', child: Text('Bottom')),
                  DropdownMenuItem(value: 'top', child: Text('Top')),
                ],
                onChanged: (value) {
                  if (value != null && value != _position) _set(value);
                },
              ),
      ),
    );
  }
}
