// lib/ui/widgets/episode_context_menu.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';

class EpisodeContextMenu extends StatelessWidget {
  final PinepodsEpisode episode;
  final VoidCallback? onSave;
  final VoidCallback? onRemoveSaved;
  final VoidCallback? onDownload;
  final VoidCallback? onLocalDownload;
  final VoidCallback? onQueue;
  final VoidCallback? onMarkComplete;
  final VoidCallback? onDismiss;

  const EpisodeContextMenu({
    Key? key,
    required this.episode,
    this.onSave,
    this.onRemoveSaved,
    this.onDownload,
    this.onLocalDownload,
    this.onQueue,
    this.onMarkComplete,
    this.onDismiss,
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
                    Text(
                      episode.podcastName,
                      style: TextStyle(
                        fontSize: 14,
                        color: Theme.of(context).primaryColor,
                      ),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                    const SizedBox(height: 16),
                    const Divider(height: 1),
                    const SizedBox(height: 8),
                    
                    // Menu options
                    _buildMenuOption(
                      context,
                      icon: episode.saved ? Icons.bookmark_remove : Icons.bookmark_add,
                      text: episode.saved ? 'Remove from Saved' : 'Save Episode',
                      onTap: episode.saved ? onRemoveSaved : onSave,
                    ),
                    
                    _buildMenuOption(
                      context,
                      icon: episode.downloaded ? Icons.delete_outline : Icons.cloud_download_outlined,
                      text: episode.downloaded ? 'Delete from Server' : 'Download to Server',
                      onTap: onDownload,
                    ),
                    
                    _buildMenuOption(
                      context,
                      icon: Icons.file_download_outlined,
                      text: 'Download Locally',
                      onTap: onLocalDownload,
                    ),
                    
                    _buildMenuOption(
                      context,
                      icon: episode.queued ? Icons.queue_music : Icons.add_to_queue,
                      text: episode.queued ? 'Remove from Queue' : 'Add to Queue',
                      onTap: onQueue,
                    ),
                    
                    _buildMenuOption(
                      context,
                      icon: episode.completed ? Icons.check_circle : Icons.check_circle_outline,
                      text: episode.completed ? 'Mark as Incomplete' : 'Mark as Complete',
                      onTap: onMarkComplete,
                    ),
                  ],
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
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