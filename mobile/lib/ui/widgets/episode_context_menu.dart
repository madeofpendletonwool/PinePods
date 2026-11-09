// lib/ui/widgets/episode_context_menu.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/services/global_services.dart';
import 'package:pinepods_mobile/services/nowplaying/nowplaying_service.dart';
import 'package:pinepods_mobile/ui/podcast/remote_devices_button.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';

class EpisodeContextMenu extends StatefulWidget {
  final PinepodsEpisode episode;
  final VoidCallback? onSave;
  final VoidCallback? onRemoveSaved;
  final VoidCallback? onDownload;
  final VoidCallback? onLocalDownload;
  final VoidCallback? onDeleteLocalDownload;
  final VoidCallback? onQueue;
  /// Add to the 'top' or 'bottom' of the queue instead of the user's default.
  final void Function(String position)? onQueueAt;
  final VoidCallback? onMarkComplete;
  final VoidCallback? onDismiss;
  final VoidCallback? onPodcastTap;
  final bool isDownloadedLocally;

  const EpisodeContextMenu({
    Key? key,
    required this.episode,
    this.onSave,
    this.onRemoveSaved,
    this.onDownload,
    this.onLocalDownload,
    this.onDeleteLocalDownload,
    this.onQueue,
    this.onQueueAt,
    this.onMarkComplete,
    this.onDismiss,
    this.onPodcastTap,
    this.isDownloadedLocally = false,
  }) : super(key: key);

  @override
  State<EpisodeContextMenu> createState() => _EpisodeContextMenuState();
}

class _EpisodeContextMenuState extends State<EpisodeContextMenu> {
  bool _queueChoicesOpen = false;

  PinepodsEpisode get episode => widget.episode;
  VoidCallback? get onSave => widget.onSave;
  VoidCallback? get onRemoveSaved => widget.onRemoveSaved;
  VoidCallback? get onDownload => widget.onDownload;
  VoidCallback? get onLocalDownload => widget.onLocalDownload;
  VoidCallback? get onDeleteLocalDownload => widget.onDeleteLocalDownload;
  VoidCallback? get onQueue => widget.onQueue;
  void Function(String position)? get onQueueAt => widget.onQueueAt;
  VoidCallback? get onMarkComplete => widget.onMarkComplete;
  VoidCallback? get onDismiss => widget.onDismiss;
  VoidCallback? get onPodcastTap => widget.onPodcastTap;
  bool get isDownloadedLocally => widget.isDownloadedLocally;

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: onDismiss, // Dismiss when tapping outside
      child: Container(
        color: Colors.black.withOpacity(0.3), // Semi-transparent overlay
        child: Center(
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 20),
            child: GestureDetector(
              onTap: () {}, // Prevent dismissal when tapping the menu itself
              child: Material(
                color: Theme.of(context).cardColor,
                borderRadius: BorderRadius.circular(12),
                elevation: 10,
                child: Container(
                  padding: const EdgeInsets.all(16),
                  constraints: const BoxConstraints(
                    maxWidth: 300,
                    maxHeight: 400,
                  ),
                  child: SingleChildScrollView(
                  child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    // Episode title
                    Text(
                      episode.episodeTitle,
                      style: const TextStyle(
                        fontSize: 16,
                        fontWeight: FontWeight.w600,
                      ),
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                    ),
                    const SizedBox(height: 8),
                    InkWell(
                      onTap: onPodcastTap,
                      borderRadius: BorderRadius.circular(4),
                      child: Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Flexible(
                            child: Text(
                              episode.podcastName,
                              style: TextStyle(
                                fontSize: 14,
                                color: Theme.of(context).primaryColor,
                                decoration: onPodcastTap != null
                                    ? TextDecoration.underline
                                    : null,
                                decorationColor: Theme.of(context).primaryColor,
                              ),
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                          if (onPodcastTap != null) ...[
                            const SizedBox(width: 4),
                            Icon(
                              PhosphorIconsRegular.caretRight,
                              size: 16,
                              color: Theme.of(context).primaryColor,
                            ),
                          ],
                        ],
                      ),
                    ),
                    const SizedBox(height: 16),
                    const Divider(height: 1),
                    const SizedBox(height: 8),
                    
                    // Menu options
                    _buildMenuOption(
                      context,
                      icon: episode.saved ? PhosphorIconsFill.bookmarkSimple : PhosphorIconsRegular.bookmarkSimple,
                      text: episode.saved ? 'Remove from Saved' : 'Save Episode',
                      onTap: episode.saved ? onRemoveSaved : onSave,
                    ),
                    
                    _buildMenuOption(
                      context,
                      icon: episode.downloaded ? PhosphorIconsRegular.trash : PhosphorIconsRegular.cloudArrowDown,
                      text: episode.downloaded ? 'Delete from Server' : 'Download to Server',
                      onTap: onDownload,
                    ),
                    
                    _buildMenuOption(
                      context,
                      icon: isDownloadedLocally ? PhosphorIconsRegular.trashSimple : PhosphorIconsRegular.fileArrowDown,
                      text: isDownloadedLocally ? 'Delete Local Download' : 'Download Locally',
                      onTap: isDownloadedLocally ? onDeleteLocalDownload : onLocalDownload,
                    ),
                    
                    _buildMenuOption(
                      context,
                      icon: PhosphorIconsRegular.queue,
                      text: episode.queued ? 'Remove from Queue' : 'Add to Queue',
                      onTap: onQueue,
                      trailing: !episode.queued && onQueueAt != null
                          ? IconButton(
                              tooltip: 'Choose top or bottom',
                              visualDensity: VisualDensity.compact,
                              icon: Icon(
                                _queueChoicesOpen
                                    ? PhosphorIconsRegular.caretUp
                                    : PhosphorIconsRegular.caretDown,
                                size: 18,
                              ),
                              onPressed: () => setState(() => _queueChoicesOpen = !_queueChoicesOpen),
                            )
                          : null,
                    ),
                    if (_queueChoicesOpen && !episode.queued && onQueueAt != null) ...[
                      _buildMenuOption(
                        context,
                        icon: PhosphorIconsRegular.arrowLineUp,
                        text: 'Add to top of queue',
                        onTap: () => onQueueAt!('top'),
                        indent: 32,
                      ),
                      _buildMenuOption(
                        context,
                        icon: PhosphorIconsRegular.arrowLineDown,
                        text: 'Add to bottom of queue',
                        onTap: () => onQueueAt!('bottom'),
                        indent: 32,
                      ),
                    ],
                    
                    _buildMenuOption(
                      context,
                      icon: episode.completed ? PhosphorIconsFill.checkCircle : PhosphorIconsRegular.checkCircle,
                      text: episode.completed ? 'Mark as Incomplete' : 'Mark as Complete',
                      onTap: onMarkComplete,
                    ),

                    _buildPlayOnOption(context),
                  ],
                  ),
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  /// "Play on device…" — only shown while another device is online.
  Widget _buildPlayOnOption(BuildContext context) {
    final service = GlobalServices.nowPlayingService;
    if (service == null) return const SizedBox.shrink();
    return ValueListenableBuilder<List<NowPlayingDevice>>(
      valueListenable: service.devices,
      builder: (context, deviceList, _) {
        if (!deviceList.any((d) => d.supports('play_episode'))) {
          return const SizedBox.shrink();
        }
        return _buildMenuOption(
          context,
          icon: PhosphorIconsRegular.screencast,
          text: 'Play on device…',
          onTap: () {
            // Some callers dismiss with Navigator.pop, so dismiss first and open
            // the picker from the root navigator's (still-mounted) context.
            final rootContext = Navigator.of(context, rootNavigator: true).context;
            onDismiss?.call();
            showPlayOnDevicePicker(
              rootContext,
              episodeId: episode.episodeId,
              isYoutube: episode.isYoutube,
            );
          },
        );
      },
    );
  }

  Widget _buildMenuOption(
    BuildContext context, {
    required IconData icon,
    required String text,
    VoidCallback? onTap,
    bool enabled = true,
    Widget? trailing,
    double indent = 0,
  }) {
    return InkWell(
      onTap: enabled ? onTap : null,
      borderRadius: BorderRadius.circular(8),
      child: Padding(
        padding: EdgeInsets.fromLTRB(8 + indent, trailing != null ? 4 : 12, 8, trailing != null ? 4 : 12),
        child: Row(
          children: [
            Icon(
              icon,
              size: 20,
              color: enabled 
                ? Theme.of(context).iconTheme.color 
                : Theme.of(context).disabledColor,
            ),
            const SizedBox(width: 12),
            Expanded(
              child: Text(
                text,
                style: TextStyle(
                  fontSize: 14,
                  color: enabled 
                    ? Theme.of(context).textTheme.bodyLarge?.color 
                    : Theme.of(context).disabledColor,
                ),
              ),
            ),
            if (trailing != null) trailing,
          ],
        ),
      ),
    );
  }
}