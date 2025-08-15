// lib/ui/widgets/offline_episode_tile.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/ui/widgets/tile_image.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:intl/intl.dart' show DateFormat;

/// A custom episode tile specifically for offline downloaded episodes.
/// This bypasses the legacy PlayControl system and uses a custom play callback.
class OfflineEpisodeTile extends StatelessWidget {
  final Episode episode;
  final VoidCallback? onPlayPressed;
  final VoidCallback? onTap;

  const OfflineEpisodeTile({
    super.key,
    required this.episode,
    this.onPlayPressed,
    this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;

    return Card(
      margin: const EdgeInsets.symmetric(horizontal: 12.0, vertical: 4.0),
      child: ListTile(
        onTap: onTap,
        leading: Stack(
          alignment: Alignment.bottomLeft,
          children: [
            Opacity(
              opacity: episode.played ? 0.5 : 1.0,
              child: TileImage(
                url: episode.thumbImageUrl ?? episode.imageUrl!,
                size: 56.0,
                highlight: episode.highlight,
              ),
            ),
            // Progress indicator
            SizedBox(
              height: 5.0,
              width: 56.0 * (episode.percentagePlayed / 100),
              child: Container(
                color: Theme.of(context).primaryColor,
              ),
            ),
          ],
        ),
        title: Opacity(
          opacity: episode.played ? 0.5 : 1.0,
          child: Text(
            episode.title!,
            overflow: TextOverflow.ellipsis,
            maxLines: 2,
            style: textTheme.bodyMedium,
          ),
        ),
        subtitle: Opacity(
          opacity: episode.played ? 0.5 : 1.0,
          child: _EpisodeSubtitle(episode),
        ),
        trailing: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            // Offline indicator
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
              decoration: BoxDecoration(
                color: Colors.green[100],
                borderRadius: BorderRadius.circular(8),
              ),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(
                    Icons.offline_pin,
                    size: 12,
                    color: Colors.green[700],
                  ),
                  const SizedBox(width: 4),
                  Text(
                    'Offline',
                    style: TextStyle(
                      fontSize: 10,
                      color: Colors.green[700],
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                ],
              ),
            ),
            const SizedBox(width: 8),
            // Custom play button that bypasses legacy audio system
            SizedBox(
              width: 48,
              height: 48,
              child: IconButton(
                onPressed: onPlayPressed,
                icon: Icon(
                  Icons.play_arrow,
                  color: Theme.of(context).primaryColor,
                ),
                tooltip: L.of(context)?.play_button_label ?? 'Play',
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _EpisodeSubtitle extends StatelessWidget {
  final Episode episode;
  final String date;
  final Duration length;

  _EpisodeSubtitle(this.episode)
      : date = episode.publicationDate == null
            ? ''
            : DateFormat(episode.publicationDate!.year == DateTime.now().year ? 'd MMM' : 'd MMM yyyy')
                .format(episode.publicationDate!),
        length = Duration(seconds: episode.duration);

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    var timeRemaining = episode.timeRemaining;

    String title;

    if (length.inSeconds > 0) {
      if (length.inSeconds < 60) {
        title = '$date • ${length.inSeconds} sec';
      } else {
        title = '$date • ${length.inMinutes} min';
      }
    } else {
      title = date;
    }

    if (timeRemaining.inSeconds > 0) {
      if (timeRemaining.inSeconds < 60) {
        title = '$title / ${timeRemaining.inSeconds} sec left';
      } else {
        title = '$title / ${timeRemaining.inMinutes} min left';
      }
    }

    return Padding(
      padding: const EdgeInsets.only(top: 4.0),
      child: Text(
        title,
        overflow: TextOverflow.ellipsis,
        softWrap: false,
        style: textTheme.bodySmall,
      ),
    );
  }
}