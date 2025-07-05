// lib/ui/widgets/pinepods_episode_card.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';

class PinepodsEpisodeCard extends StatelessWidget {
  final PinepodsEpisode episode;
  final VoidCallback? onTap;
  final VoidCallback? onLongPress;
  final VoidCallback? onPlayPressed;

  const PinepodsEpisodeCard({
    Key? key,
    required this.episode,
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
              // Episode artwork (smaller)
              ClipRRect(
                borderRadius: BorderRadius.circular(6),
                child: episode.episodeArtwork.isNotEmpty
                    ? Image.network(
                        episode.episodeArtwork,
                        width: 50,
                        height: 50,
                        fit: BoxFit.cover,
                        cacheWidth: 100, // Optimize memory usage
                        cacheHeight: 100,
                        errorBuilder: (context, error, stackTrace) {
                          return Container(
                            width: 50,
                            height: 50,
                            decoration: BoxDecoration(
                              color: Colors.grey[300],
                              borderRadius: BorderRadius.circular(6),
                            ),
                            child: const Icon(
                              Icons.music_note,
                              color: Colors.grey,
                              size: 24,
                            ),
                          );
                        },
                        loadingBuilder: (context, child, loadingProgress) {
                          if (loadingProgress == null) return child;
                          return Container(
                            width: 50,
                            height: 50,
                            decoration: BoxDecoration(
                              color: Colors.grey[200],
                              borderRadius: BorderRadius.circular(6),
                            ),
                            child: const Center(
                              child: SizedBox(
                                width: 20,
                                height: 20,
                                child: CircularProgressIndicator(strokeWidth: 2),
                              ),
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
                        child: const Icon(
                          Icons.music_note,
                          color: Colors.grey,
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
                        fontSize: 14,
                        fontWeight: FontWeight.w600,
                      ),
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                    ),
                    const SizedBox(height: 2),
                    Text(
                      episode.podcastName,
                      style: TextStyle(
                        fontSize: 12,
                        color: Theme.of(context).primaryColor,
                        fontWeight: FontWeight.w500,
                      ),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                    const SizedBox(height: 4),
                    Row(
                      children: [
                        Text(
                          episode.formattedPubDate,
                          style: TextStyle(
                            fontSize: 11,
                            color: Colors.grey[600],
                          ),
                        ),
                        const SizedBox(width: 8),
                        Text(
                          episode.formattedDuration,
                          style: TextStyle(
                            fontSize: 11,
                            color: Colors.grey[600],
                          ),
                        ),
                      ],
                    ),
                    
                    // Progress bar if episode has been started
                    if (episode.isStarted) ...[ 
                      const SizedBox(height: 6),
                      LinearProgressIndicator(
                        value: episode.progressPercentage / 100,
                        backgroundColor: Colors.grey[300],
                        valueColor: AlwaysStoppedAnimation<Color>(
                          Theme.of(context).primaryColor,
                        ),
                        minHeight: 2,
                      ),
                    ],
                  ],
                ),
              ),
              
              // Action button (just play) - only show if callback provided
              if (onPlayPressed != null)
                IconButton(
                  icon: Icon(
                    episode.completed ? Icons.replay : Icons.play_arrow,
                    color: Theme.of(context).primaryColor,
                  ),
                  onPressed: onPlayPressed,
                  iconSize: 24,
                  padding: const EdgeInsets.all(8),
                  constraints: const BoxConstraints(
                    minWidth: 40,
                    minHeight: 40,
                  ),
                ),
              
              // Status indicators (compact)
              if (episode.saved || episode.downloaded || episode.queued)
                SizedBox(
                  width: 20,
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      if (episode.saved)
                        Icon(
                          Icons.bookmark,
                          color: Colors.orange[600],
                          size: 14,
                        ),
                      if (episode.downloaded)
                        Icon(
                          Icons.download_done,
                          color: Colors.blue[600],
                          size: 14,
                        ),
                      if (episode.queued)
                        Icon(
                          Icons.queue_music,
                          color: Colors.purple[600],
                          size: 14,
                        ),
                    ],
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }
}