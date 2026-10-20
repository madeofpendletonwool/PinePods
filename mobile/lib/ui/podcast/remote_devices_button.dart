// Remote-devices control: an app-bar button that surfaces the user's other
// active devices and what each is playing, with advisory transport controls,
// "Play here" (pull) and "Send here" (push this device's playback), plus the
// "Play on device" picker used by the episode context menu. Commands are sent over the now-playing WebSocket; the target
// device applies them to its own local, authoritative player. Mirrors the web
// remote_devices.rs panel.

import 'package:flutter/material.dart';

import 'package:pinepods_mobile/services/global_services.dart';
import 'package:pinepods_mobile/services/nowplaying/nowplaying_service.dart';

String _fmtTime(double secs) {
  if (secs.isNaN || secs.isInfinite || secs < 0) return '00:00';
  final total = secs.round();
  final h = total ~/ 3600;
  final m = (total % 3600) ~/ 60;
  final s = total % 60;
  final mm = m.toString().padLeft(2, '0');
  final ss = s.toString().padLeft(2, '0');
  return h > 0 ? '$h:$mm:$ss' : '$mm:$ss';
}

/// Icon for a device row, from the free-form `device_type`.
IconData _deviceIcon(String deviceType) {
  switch (deviceType.toLowerCase()) {
    case 'web':
      return Icons.web;
    case 'ios':
    case 'android':
    case 'mobile':
      return Icons.smartphone;
    case 'firewood':
      return Icons.terminal;
    default:
      return Icons.devices_other;
  }
}

String _deviceLabel(NowPlayingDevice device) => device.deviceName.isNotEmpty
    ? device.deviceName
    : (device.deviceType.isNotEmpty ? device.deviceType : 'Device');

/// "Play on ▸": pick another device and start [episodeId] there (it resumes
/// from the server-stored position). No-op when the service isn't available.
Future<void> showPlayOnDevicePicker(
  BuildContext context, {
  required int episodeId,
  required bool isYoutube,
}) async {
  final service = GlobalServices.nowPlayingService;
  if (service == null) return;
  final messenger = ScaffoldMessenger.maybeOf(context);
  final target = await showModalBottomSheet<NowPlayingDevice>(
    context: context,
    backgroundColor: Theme.of(context).scaffoldBackgroundColor,
    shape: const RoundedRectangleBorder(
      borderRadius: BorderRadius.vertical(top: Radius.circular(16)),
    ),
    builder: (context) => SafeArea(
      child: ValueListenableBuilder<List<NowPlayingDevice>>(
        valueListenable: service.devices,
        builder: (context, deviceList, _) => _playOnList(context, deviceList),
      ),
    ),
  );
  if (target == null) return;
  service.playOnDevice(target.deviceId, episodeId, isYoutube);
  messenger?.showSnackBar(
    SnackBar(content: Text('Playing on ${_deviceLabel(target)}')),
  );
}

/// The picker's rows: only devices that can start an episode.
Widget _playOnList(BuildContext context, List<NowPlayingDevice> deviceList) {
  final targets = deviceList.where((d) => d.supports('play_episode')).toList();
  return Column(
    mainAxisSize: MainAxisSize.min,
    children: [
      Padding(
        padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
        child: Row(
          children: [
            const Icon(Icons.cast),
            const SizedBox(width: 8),
            Text('Play on…', style: Theme.of(context).textTheme.titleLarge),
          ],
        ),
      ),
      if (targets.isEmpty)
        const Padding(
          padding: EdgeInsets.all(24),
          child: Text('No other devices are online.'),
        ),
      for (final d in targets)
        ListTile(
          leading: Icon(_deviceIcon(d.deviceType)),
          title: Text(_deviceLabel(d)),
          subtitle: Text(d.episodeId == 0
              ? 'idle'
              : (d.playing ? 'playing ${d.title}' : 'paused')),
          onTap: () => Navigator.of(context).pop(d),
        ),
    ],
  );
}

/// App-bar action showing a devices icon with a badge for the number of other
/// online devices; tapping opens the device panel. Renders nothing (a zero-size
/// box) when the now-playing service isn't available, or — with
/// [hideWhenEmpty] — while no other device is online.
class RemoteDevicesButton extends StatelessWidget {
  const RemoteDevicesButton({super.key, this.color, this.hideWhenEmpty = false});

  final Color? color;
  final bool hideWhenEmpty;

  @override
  Widget build(BuildContext context) {
    final service = GlobalServices.nowPlayingService;
    if (service == null) return const SizedBox.shrink();

    return ValueListenableBuilder<List<NowPlayingDevice>>(
      valueListenable: service.devices,
      builder: (context, deviceList, _) {
        if (hideWhenEmpty && deviceList.isEmpty) return const SizedBox.shrink();
        final iconColor = color ?? Theme.of(context).primaryIconTheme.color;
        return Stack(
          alignment: Alignment.center,
          children: [
            IconButton(
              tooltip: 'Other devices',
              icon: Icon(Icons.devices_outlined, color: iconColor),
              onPressed: () => _showDevicePanel(context, service),
            ),
            if (deviceList.isNotEmpty)
              Positioned(
                top: 8,
                right: 8,
                child: Container(
                  padding: const EdgeInsets.all(2),
                  constraints: const BoxConstraints(
                    minWidth: 16,
                    minHeight: 16,
                  ),
                  decoration: BoxDecoration(
                    color: Theme.of(context).colorScheme.primary,
                    borderRadius: BorderRadius.circular(8),
                  ),
                  child: Text(
                    '${deviceList.length}',
                    style: const TextStyle(
                      color: Colors.white,
                      fontSize: 10,
                      fontWeight: FontWeight.bold,
                    ),
                    textAlign: TextAlign.center,
                  ),
                ),
              ),
          ],
        );
      },
    );
  }

  void _showDevicePanel(BuildContext context, NowPlayingService service) {
    showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      backgroundColor: Theme.of(context).scaffoldBackgroundColor,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(16)),
      ),
      builder: (context) => _RemoteDevicesSheet(service: service),
    );
  }
}

class _RemoteDevicesSheet extends StatelessWidget {
  const _RemoteDevicesSheet({required this.service});

  final NowPlayingService service;

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 12),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16),
              child: Row(
                children: [
                  const Icon(Icons.devices_outlined),
                  const SizedBox(width: 8),
                  Text(
                    'Other devices',
                    style: Theme.of(context).textTheme.titleLarge,
                  ),
                ],
              ),
            ),
            ValueListenableBuilder<String?>(
              valueListenable: service.lastCommandError,
              builder: (context, error, _) {
                if (error == null) return const SizedBox(height: 8);
                return Padding(
                  padding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
                  child: Text(
                    error,
                    style: TextStyle(
                      color: Theme.of(context).colorScheme.error,
                    ),
                  ),
                );
              },
            ),
            Flexible(
              child: ValueListenableBuilder<List<NowPlayingDevice>>(
                valueListenable: service.devices,
                builder: (context, deviceList, _) {
                  if (deviceList.isEmpty) {
                    return const Padding(
                      padding: EdgeInsets.all(24),
                      child: Text(
                        'No other devices are online.',
                        textAlign: TextAlign.center,
                      ),
                    );
                  }
                  return ListView.separated(
                    shrinkWrap: true,
                    padding: const EdgeInsets.symmetric(vertical: 4),
                    itemCount: deviceList.length,
                    separatorBuilder: (_, _) => const Divider(height: 1),
                    itemBuilder: (context, index) =>
                        _DeviceRow(service: service, device: deviceList[index]),
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

class _DeviceRow extends StatelessWidget {
  const _DeviceRow({required this.service, required this.device});

  final NowPlayingService service;
  final NowPlayingDevice device;

  @override
  Widget build(BuildContext context) {
    final label = _deviceLabel(device);
    final canSend =
        service.hasLocalPlayback && device.supports('play_episode');
    final track = device.title.isEmpty ? 'Idle' : device.title;
    final progress = device.durationSec > 0
        ? '${_fmtTime(device.positionSec)} / ${_fmtTime(device.durationSec)}'
        : _fmtTime(device.positionSec);
    final hasEpisode = device.episodeId != 0;

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Expanded(
                child: Row(
                  children: [
                    Icon(_deviceIcon(device.deviceType), size: 18),
                    const SizedBox(width: 6),
                    Flexible(
                      child: Text(
                        label,
                        overflow: TextOverflow.ellipsis,
                        style: const TextStyle(fontWeight: FontWeight.bold),
                      ),
                    ),
                  ],
                ),
              ),
              Text(
                !hasEpisode
                    ? 'idle'
                    : (device.playing ? '▶ playing' : '⏸ paused'),
                style: TextStyle(
                  fontSize: 12,
                  color: Theme.of(context).textTheme.bodySmall?.color,
                ),
              ),
            ],
          ),
          const SizedBox(height: 4),
          Text(track, maxLines: 1, overflow: TextOverflow.ellipsis),
          // Idle devices (connected, nothing loaded) are listed so they can be
          // targeted, but there's nothing to transport-control or take over.
          if (!hasEpisode && canSend)
            Align(
              alignment: Alignment.centerRight,
              child: _sendHereButton(context, label),
            ),
          if (hasEpisode) ...[
            Text(
              progress,
              style: TextStyle(
                fontSize: 12,
                color: Theme.of(context).textTheme.bodySmall?.color,
              ),
            ),
            const SizedBox(height: 4),
            Row(
              children: [
                if (device.supports('skip_back'))
                  IconButton(
                    tooltip: 'Skip back 15s',
                    icon: const Icon(Icons.replay),
                    onPressed: () => service.sendCommand(
                      device.deviceId,
                      'skip_back',
                      const {'seconds': 15},
                    ),
                  ),
                if (device.supports(device.playing ? 'pause' : 'play'))
                  IconButton(
                    tooltip: device.playing ? 'Pause' : 'Play',
                    icon: Icon(device.playing ? Icons.pause : Icons.play_arrow),
                    onPressed: () => service.sendCommand(
                      device.deviceId,
                      device.playing ? 'pause' : 'play',
                    ),
                  ),
                if (device.supports('skip_forward'))
                  IconButton(
                    tooltip: 'Skip forward 30s',
                    icon: const Icon(Icons.forward_30),
                    onPressed: () => service.sendCommand(
                      device.deviceId,
                      'skip_forward',
                      const {'seconds': 30},
                    ),
                  ),
                const Spacer(),
                if (canSend) _sendHereButton(context, label),
                TextButton.icon(
                  icon: const Icon(Icons.play_circle_outline, size: 18),
                  label: const Text('Play here'),
                  onPressed: () {
                    service.playHere(device);
                    Navigator.of(context).pop();
                  },
                ),
              ],
            ),
          ],
        ],
      ),
    );
  }

  /// Push this device's playback to [device] at the live position; the local
  /// player pauses once the server confirms delivery.
  Widget _sendHereButton(BuildContext context, String label) {
    return TextButton.icon(
      icon: const Icon(Icons.cast, size: 18),
      label: const Text('Send here'),
      onPressed: () {
        if (service.sendPlaybackTo(device.deviceId)) {
          final messenger = ScaffoldMessenger.maybeOf(context);
          Navigator.of(context).pop();
          messenger?.showSnackBar(SnackBar(content: Text('Sent to $label')));
        }
      },
    );
  }
}
