// lib/ui/widgets/episode_context_menu.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/services/global_services.dart';
import 'package:pinepods_mobile/services/nowplaying/nowplaying_service.dart';
import 'package:pinepods_mobile/ui/podcast/remote_devices_button.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';

class EpisodeContextMenu extends StatelessWidget {
  final PinepodsEpisode episode;
  final VoidCallback? onSave;
  final VoidCallback? onRemoveSaved;
  final VoidCallback? onDownload;
  final VoidCallback? onLocalDownload;
  final VoidCallback? onDeleteLocalDownload;
  final VoidCallback? onQueue;
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
    this.onMarkComplete,
    this.onDismiss,
    this.onPodcastTap,
    this.isDownloadedLocally = false,
  }) : super(key: key);

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
                      icon: episode.queued ? PhosphorIconsRegular.queue : PhosphorIconsRegular.queue,
                      text: episode.queued ? 'Remove from Queue' : 'Add to Queue',
                      onTap: onQueue,
                    ),
                    
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
  }) {
    return InkWell(
      onTap: enabled ? onTap : null,
      borderRadius: BorderRadius.circular(8),
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 12, horizontal: 8),
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
          ],
        ),
      ),
    );
  }
}