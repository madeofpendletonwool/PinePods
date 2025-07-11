// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/entities/podcast.dart';
import 'package:pinepods_mobile/entities/pinepods_search.dart';
import 'package:pinepods_mobile/ui/pinepods/podcast_details.dart';
import 'package:pinepods_mobile/ui/widgets/tile_image.dart';
import 'package:flutter/material.dart';

class PinepodsPodcastTile extends StatelessWidget {
  final Podcast podcast;

  const PinepodsPodcastTile({
    super.key,
    required this.podcast,
  });

  UnifiedPinepodsPodcast _convertToUnifiedPodcast() {
    return UnifiedPinepodsPodcast(
      id: podcast.id ?? 0,
      indexId: 0, // Default value for subscribed podcasts
      title: podcast.title,
      url: podcast.url,
      originalUrl: podcast.url,
      link: podcast.link ?? '',
      description: podcast.description ?? '',
      author: podcast.copyright ?? '',
      ownerName: podcast.copyright ?? '',
      image: podcast.imageUrl ?? '',
      artwork: podcast.imageUrl ?? '',
      lastUpdateTime: 0, // Default value
      categories: null,
      explicit: false, // Default value
      episodeCount: podcast.episodes.length, // Use actual episode count
    );
  }

  @override
  Widget build(BuildContext context) {
    return ListTile(
      onTap: () {
        final unifiedPodcast = _convertToUnifiedPodcast();
        Navigator.push(
          context,
          MaterialPageRoute<void>(
            settings: const RouteSettings(name: 'pinepods_podcast_details'),
            builder: (context) => PinepodsPodcastDetails(
              podcast: unifiedPodcast,
              isFollowing: true, // These are subscribed podcasts
            ),
          ),
        );
      },
      minVerticalPadding: 9,
      leading: ExcludeSemantics(
        child: Hero(
          key: Key('tilehero${podcast.imageUrl}:${podcast.link}'),
          tag: '${podcast.imageUrl}:${podcast.link}',
          child: TileImage(
            url: podcast.imageUrl!,
            size: 60,
          ),
        ),
      ),
      title: Text(
        podcast.title,
        maxLines: 1,
      ),
      subtitle: Text(
        '${podcast.copyright ?? ''}\n',
        maxLines: 2,
      ),
      isThreeLine: false,
    );
  }
}