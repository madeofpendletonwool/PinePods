// lib/ui/widgets/draggable_queue_episode_card.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';

class DraggableQueueEpisodeCard extends StatelessWidget {
  final PinepodsEpisode episode;
  final VoidCallback? onTap;
  final VoidCallback? onLongPress;
  final VoidCallback? onPlayPressed;
  final int index; // Add index for drag listener

  const DraggableQueueEpisodeCard({
    Key? key,
    required this.episode,
    required this.index,
    this.onTap,
    this.onLongPress,
    this.onPlayPressed,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return Card(
      margin: const EdgeInsets.symmetric(horizontal: 12.0, vertical: 4.0),
      elevation: 1,
      child: InkWell(
        onTap: onTap,
        onLongPress: onLongPress,
        borderRadius: BorderRadius.circular(8),
        child: Padding(
          padding: const EdgeInsets.all(12.0),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              // Drag handle
              ReorderableDragStartListener(
                index: index,
                child: Container(
                  width: 24,
                  height: 50,
                  margin: const EdgeInsets.only(right: 12),
                  child: Center(
                    child: Icon(
                      Icons.drag_indicator,
                      color: Colors.grey[600],
                      size: 20,
                    ),
                  ),
                ),
              ),
              
              // Episode artwork
              ClipRRect(
                borderRadius: BorderRadius.circular(6),
                child: episode.episodeArtwork.isNotEmpty
                    ? Image.network(
                        episode.episodeArtwork,
                        width: 50,
                        height: 50,
                        fit: BoxFit.cover,
                        cacheWidth: 100,
                        cacheHeight: 100,
                        errorBuilder: (context, error, stackTrace) {
                          return Container(
                            width: 50,
                            height: 50,
                            decoration: BoxDecoration(
                              color: Colors.grey[300],
                              borderRadius: BorderRadius.circular(6),
                            ),
                            child: Icon(
                              Icons.podcasts,
                              color: Colors.grey[600],
                              size: 24,
                            ),
                          );
                        },
                      )
                    : Container(
                        width: 50,
                        height: 50,
                        decoration: BoxDecoration(
                          color: Colors.grey[300],
                          borderRadius: BorderRadius.circular(6),
                        ),
                        child: Icon(
                          Icons.podcasts,
                          color: Colors.grey[600],
                          size: 24,
                        ),
                      ),
              ),
              
              const SizedBox(width: 12),
              
              // Episode info
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      episode.episodeTitle,
                      style: const TextStyle(
                        fontWeight: FontWeight.w600,
                        fontSize: 14,
                      ),
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                    ),
                    const SizedBox(height: 4),
                    Text(
                      episode.podcastName,
                      style: TextStyle(
                        color: Colors.grey[600],
                        fontSize: 13,
                      ),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                    if (episode.episodePubDate.isNotEmpty) ...[
                      const SizedBox(height: 4),
                      Row(
                        children: [
                          Icon(
                            Icons.calendar_today,
                            size: 12,
                            color: Colors.grey[500],
                          ),
                          const SizedBox(width: 4),
                          Text(
                            _formatDate(episode.episodePubDate),
                            style: TextStyle(
                              color: Colors.grey[500],
                              fontSize: 12,
                            ),
                          ),
                          if (episode.episodeDuration > 0) ...[
                            const SizedBox(width: 12),
                            Icon(
                              Icons.access_time,
                              size: 12,
                              color: Colors.grey[500],
                            ),
                            const SizedBox(width: 4),
                            Text(
                              _formatDuration(episode.episodeDuration),
                              style: TextStyle(
                                color: Colors.grey[500],
                                fontSize: 12,
                              ),
                            ),
                          ],
                        ],
                      ),
                    ],
                    // Progress bar if episode has been started
                    if (episode.listenDuration != null && episode.listenDuration! > 0 && episode.episodeDuration > 0) ...[
                      const SizedBox(height: 8),
                      LinearProgressIndicator(
                        value: episode.listenDuration! / episode.episodeDuration,
                        backgroundColor: Colors.grey[300],
                        valueColor: AlwaysStoppedAnimation<Color>(
                          Theme.of(context).primaryColor.withOpacity(0.7),
                        ),
                      ),
                    ],
                  ],
                ),
              ),
              
              // Status indicators and play button
              Column(
                children: [
                  if (onPlayPressed != null)
                    IconButton(
                      onPressed: onPlayPressed,
                      icon: Icon(
                        episode.completed 
                          ? Icons.check_circle 
                          : ((episode.listenDuration != null && episode.listenDuration! > 0) ? Icons.play_circle_filled : Icons.play_circle_outline),
                        color: episode.completed 
                          ? Colors.green 
                          : Theme.of(context).primaryColor,
                        size: 28,
                      ),
                      padding: EdgeInsets.zero,
                      constraints: const BoxConstraints(
                        minWidth: 32,
                        minHeight: 32,
                      ),
                    ),
                  const SizedBox(height: 4),
                  Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      if (episode.saved)
                        Icon(
                          Icons.bookmark,
                          size: 16,
                          color: Colors.orange[600],
                        ),
                      if (episode.downloaded)
                        Padding(
                          padding: const EdgeInsets.only(left: 4),
                          child: Icon(
                            Icons.download_done,
                            size: 16,
                            color: Colors.green[600],
                          ),
                        ),
                      if (episode.queued)
                        Padding(
                          padding: const EdgeInsets.only(left: 4),
                          child: Icon(
                            Icons.queue_music,
                            size: 16,
                            color: Colors.blue[600],
                          ),
                        ),
                    ],
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }

  String _formatDate(String dateString) {
    try {
      final date = DateTime.parse(dateString);
      final now = DateTime.now();
      final difference = now.difference(date).inDays;
      
      if (difference == 0) {
        return 'Today';
      } else if (difference == 1) {
        return 'Yesterday';
      } else if (difference < 7) {
        return '${difference}d ago';
      } else if (difference < 30) {
        return '${(difference / 7).floor()}w ago';
      } else {
        return '${date.day}/${date.month}/${date.year}';
      }
    } catch (e) {
      return dateString;
    }
  }

  String _formatDuration(int seconds) {
    if (seconds <= 0) return '';
    
    final hours = seconds ~/ 3600;
    final minutes = (seconds % 3600) ~/ 60;
    
    if (hours > 0) {
      return '${hours}h ${minutes}m';
    } else {
      return '${minutes}m';
    }
  }
}