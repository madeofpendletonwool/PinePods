// lib/ui/widgets/pinepods_episode_card.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/ui/widgets/lazy_network_image.dart';

class PinepodsEpisodeCard extends StatelessWidget {
  final PinepodsEpisode episode;
  final VoidCallback? onTap;
  final VoidCallback? onLongPress;
  final VoidCallback? onPlayPressed;
  final VoidCallback? onDownloadPressed;

  const PinepodsEpisodeCard({
    Key? key,
    required this.episode,
    this.onTap,
    this.onLongPress,
    this.onPlayPressed,
    this.onDownloadPressed,
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
              // Episode artwork with lazy loading
              LazyNetworkImage(
                imageUrl: episode.episodeArtwork,
                width: 50,
                height: 50,
                fit: BoxFit.cover,
                borderRadius: BorderRadius.circular(6),
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
              
              // Status indicators and play button
              Column(
                children: [
                  if (onPlayPressed != null)
                    IconButton(
                      onPressed: onPlayPressed,
                      icon: Icon(
                        episode.completed 
                          ? Icons.check_circle 
                          : ((episode.listenDuration != null && episode.listenDuration! > 0) 
                              ? Icons.play_circle_filled 
                              : Icons.play_circle_outline),
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
                  
                  // Download button
                  if (onDownloadPressed != null)
                    IconButton(
                      onPressed: onDownloadPressed,
                      icon: Icon(
                        episode.downloaded 
                          ? Icons.download_done 
                          : Icons.download_outlined,
                        color: episode.downloaded 
                          ? Colors.green 
                          : Colors.grey[600],
                        size: 20,
                      ),
                      padding: EdgeInsets.zero,
                      constraints: const BoxConstraints(
                        minWidth: 24,
                        minHeight: 24,
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
}