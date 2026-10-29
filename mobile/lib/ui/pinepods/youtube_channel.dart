// lib/ui/pinepods/youtube_channel.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/youtube_channel.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';
import 'package:pinepods_mobile/ui/pinepods/podcast_nav.dart';
import 'package:pinepods_mobile/ui/widgets/lazy_network_image.dart';
import 'package:pinepods_mobile/ui/widgets/soft_ui.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';

/// Subscribes to [channel]; returns its podcast ID, or null after showing why it failed.
Future<int?> subscribeToChannel(BuildContext context, YouTubeChannelResult channel) async {
  final settings = Provider.of<SettingsBloc>(context, listen: false).currentSettings;
  final messenger = ScaffoldMessenger.of(context);
  final userId = settings.pinepodsUserId;
  if (userId == null || settings.pinepodsServer == null || settings.pinepodsApiKey == null) {
    messenger.showSnackBar(const SnackBar(content: Text('Not logged in to PinePods server')));
    return null;
  }
  final service = PinepodsService()..setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
  try {
    final podcastId = await service.subscribeToYouTubeChannel(channel.channelId, userId);
    messenger.showSnackBar(
      SnackBar(content: Text('Subscribed to ${channel.name}. Videos are being added in the background.')),
    );
    return podcastId;
  } catch (e) {
    messenger.showSnackBar(SnackBar(content: Text("Couldn't subscribe: ${_reason(e)}")));
    return null;
  }
}

/// Unsubscribes from [channel]; returns whether it worked.
Future<bool> unsubscribeFromChannel(BuildContext context, YouTubeChannelResult channel) async {
  final confirmed = await showDialog<bool>(
    context: context,
    builder: (context) => AlertDialog(
      title: const Text('Unsubscribe'),
      content: Text('Unsubscribe from "${channel.name}"? Its videos and your history with them are removed.'),
      actions: [
        TextButton(onPressed: () => Navigator.pop(context, false), child: const Text('Cancel')),
        TextButton(
          onPressed: () => Navigator.pop(context, true),
          style: TextButton.styleFrom(foregroundColor: Colors.red),
          child: const Text('Unsubscribe'),
        ),
      ],
    ),
  );
  if (confirmed != true || !context.mounted) return false;

  final settings = Provider.of<SettingsBloc>(context, listen: false).currentSettings;
  final messenger = ScaffoldMessenger.of(context);
  final userId = settings.pinepodsUserId;
  if (userId == null || settings.pinepodsServer == null || settings.pinepodsApiKey == null) return false;
  final service = PinepodsService()..setCredentials(settings.pinepodsServer!, settings.pinepodsApiKey!);
  try {
    final ok = await service.removeYouTubeChannel(channel.name, channel.url, userId);
    messenger.showSnackBar(SnackBar(content: Text(ok ? 'Unsubscribed' : "Couldn't unsubscribe")));
    return ok;
  } catch (e) {
    messenger.showSnackBar(SnackBar(content: Text("Couldn't unsubscribe: ${_reason(e)}")));
    return false;
  }
}

String _reason(Object e) => e.toString().replaceFirst('Exception: ', '');

String formatVideoLength(int seconds) {
  final h = seconds ~/ 3600;
  final m = (seconds % 3600) ~/ 60;
  final s = (seconds % 60).toString().padLeft(2, '0');
  return h > 0 ? '$h:${m.toString().padLeft(2, '0')}:$s' : '$m:$s';
}

/// A YouTube channel from search: what it posts, and a button to subscribe.
class YouTubeChannelPage extends StatefulWidget {
  final YouTubeChannelResult channel;

  /// Podcast ID when the user already subscribes.
  final int? podcastId;
  final ValueChanged<int?>? onSubscriptionChanged;

  const YouTubeChannelPage({super.key, required this.channel, this.podcastId, this.onSubscriptionChanged});

  @override
  State<YouTubeChannelPage> createState() => _YouTubeChannelPageState();
}

class _YouTubeChannelPageState extends State<YouTubeChannelPage> {
  late int? _podcastId = widget.podcastId;
  bool _busy = false;
  bool _descriptionExpanded = false;

  YouTubeChannelResult get _channel => widget.channel;

  Future<void> _subscribe() async {
    setState(() => _busy = true);
    final id = await subscribeToChannel(context, _channel);
    if (!mounted) return;
    setState(() {
      _busy = false;
      if (id != null) _podcastId = id;
    });
    if (id != null) widget.onSubscriptionChanged?.call(id);
  }

  Future<void> _unsubscribe() async {
    setState(() => _busy = true);
    final ok = await unsubscribeFromChannel(context, _channel);
    if (!mounted) return;
    setState(() {
      _busy = false;
      if (ok) _podcastId = null;
    });
    if (ok) widget.onSubscriptionChanged?.call(null);
  }

  Future<void> _openOnYouTube(String url) async {
    final messenger = ScaffoldMessenger.of(context);
    if (!await launchUrl(Uri.parse(url), mode: LaunchMode.externalApplication)) {
      messenger.showSnackBar(const SnackBar(content: Text("Couldn't open YouTube")));
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final accent = theme.primaryColor;
    final onAccent = onColor(accent);
    final subscribed = _podcastId != null;
    final meta = [
      if (_channel.subscriberCount != null) '${_compact(_channel.subscriberCount!)} subscribers',
      if (_channel.videoCount != null) '${_channel.videoCount} videos',
    ].join('  ·  ');

    final pillStyle = FilledButton.styleFrom(
      backgroundColor: accent,
      foregroundColor: onAccent,
      disabledBackgroundColor: accent.withOpacity(0.5),
      disabledForegroundColor: onAccent.withOpacity(0.7),
      shape: const StadiumBorder(),
      minimumSize: const Size(0, 46),
      textStyle: const TextStyle(fontSize: 15, fontWeight: FontWeight.w600),
    );
    final spinner = SizedBox(
      width: 18,
      height: 18,
      child: CircularProgressIndicator(strokeWidth: 2.2, color: subscribed ? accent : onAccent),
    );

    return Scaffold(
      appBar: AppBar(
        title: Text(_channel.name, maxLines: 1, overflow: TextOverflow.ellipsis),
        actions: [
          IconButton(
            tooltip: 'Open on YouTube',
            icon: const Icon(PhosphorIconsRegular.arrowSquareOut),
            onPressed: () => _openOnYouTube(_channel.url),
          ),
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.fromLTRB(16, 8, 16, 24),
        children: [
          Row(
            children: [
              ClipOval(child: LazyNetworkImage(imageUrl: _channel.thumbnailUrl, width: 88, height: 88)),
              const SizedBox(width: 14),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      _channel.name,
                      maxLines: 3,
                      overflow: TextOverflow.ellipsis,
                      style: theme.textTheme.titleLarge!.copyWith(fontWeight: FontWeight.bold, height: 1.2),
                    ),
                    const SizedBox(height: 4),
                    Row(
                      children: [
                        Icon(PhosphorIconsFill.youtubeLogo, size: 16, color: theme.hintColor),
                        const SizedBox(width: 4),
                        Flexible(
                          child: Text(
                            meta.isEmpty ? 'YouTube channel' : meta,
                            style: theme.textTheme.bodySmall!.copyWith(color: theme.hintColor, fontSize: 12.5),
                          ),
                        ),
                      ],
                    ),
                  ],
                ),
              ),
            ],
          ),
          const SizedBox(height: 16),
          Row(
            children: [
              Expanded(
                child: subscribed
                    ? FilledButton.icon(
                        style: pillStyle,
                        onPressed: _busy
                            ? null
                            : () => navigateToPodcastById(
                                context,
                                _podcastId,
                                fallbackTitle: _channel.name,
                                fallbackArtwork: _channel.thumbnailUrl,
                              ),
                        icon: const Icon(PhosphorIconsFill.play, size: 18),
                        label: const Text('Open channel'),
                      )
                    : FilledButton.icon(
                        style: pillStyle,
                        onPressed: _busy ? null : _subscribe,
                        icon: _busy ? spinner : const Icon(PhosphorIconsBold.plus, size: 18),
                        label: const Text('Subscribe'),
                      ),
              ),
              if (subscribed) ...[
                const SizedBox(width: 10),
                CircleAction(
                  tooltip: 'Unsubscribe',
                  icon: _busy ? spinner : Icon(PhosphorIconsBold.check, color: accent, size: 20),
                  onTap: _busy ? null : _unsubscribe,
                ),
              ],
            ],
          ),
          if (_channel.description.isNotEmpty) ...[
            const SizedBox(height: 14),
            GestureDetector(
              behavior: HitTestBehavior.opaque,
              onTap: () => setState(() => _descriptionExpanded = !_descriptionExpanded),
              child: AnimatedSize(
                duration: const Duration(milliseconds: 200),
                alignment: Alignment.topCenter,
                child: Text(
                  _channel.description,
                  maxLines: _descriptionExpanded ? null : 3,
                  overflow: _descriptionExpanded ? null : TextOverflow.ellipsis,
                  style: theme.textTheme.bodyMedium!.copyWith(height: 1.4),
                ),
              ),
            ),
          ],
          if (_channel.recentVideos.isNotEmpty) ...[
            const SizedBox(height: 20),
            Text('Recent videos', style: theme.textTheme.titleMedium!.copyWith(fontWeight: FontWeight.bold)),
            const SizedBox(height: 8),
            for (final video in _channel.recentVideos) _VideoRow(video: video, onTap: () => _openOnYouTube(video.url)),
          ],
          if (!subscribed) ...[
            const SizedBox(height: 16),
            Text(
              'Subscribing adds the channel to your library. Your server downloads the audio of its videos '
              'so you can play them like podcast episodes.',
              style: theme.textTheme.bodySmall!.copyWith(color: theme.hintColor),
            ),
          ],
        ],
      ),
    );
  }
}

class _VideoRow extends StatelessWidget {
  final YouTubeVideoPreview video;
  final VoidCallback onTap;

  const _VideoRow({required this.video, required this.onTap});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(10),
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 6),
        child: Row(
          children: [
            Stack(
              children: [
                LazyNetworkImage(
                  imageUrl: video.thumbnailUrl,
                  width: 128,
                  height: 72,
                  borderRadius: BorderRadius.circular(8),
                ),
                if (video.durationSeconds != null && video.durationSeconds! > 0)
                  Positioned(
                    right: 4,
                    bottom: 4,
                    child: Container(
                      padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 1),
                      decoration: BoxDecoration(
                        color: Colors.black.withOpacity(0.75),
                        borderRadius: BorderRadius.circular(4),
                      ),
                      child: Text(
                        formatVideoLength(video.durationSeconds!),
                        style: const TextStyle(color: Colors.white, fontSize: 11, fontWeight: FontWeight.w600),
                      ),
                    ),
                  ),
              ],
            ),
            const SizedBox(width: 12),
            Expanded(
              child: Text(
                video.title,
                maxLines: 3,
                overflow: TextOverflow.ellipsis,
                style: theme.textTheme.bodyMedium!.copyWith(fontWeight: FontWeight.w600, height: 1.3),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

String _compact(int n) {
  if (n >= 1000000) return '${(n / 1000000).toStringAsFixed(n >= 10000000 ? 0 : 1)}M';
  if (n >= 1000) return '${(n / 1000).toStringAsFixed(n >= 10000 ? 0 : 1)}K';
  return '$n';
}
