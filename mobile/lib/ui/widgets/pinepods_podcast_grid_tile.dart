// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/entities/podcast.dart';
import 'package:pinepods_mobile/entities/pinepods_search.dart';
import 'package:pinepods_mobile/ui/pinepods/podcast_details.dart';
import 'package:pinepods_mobile/ui/widgets/tile_image.dart';
import 'package:flutter/material.dart';

class PinepodsPodcastGridTile extends StatelessWidget {
  final Podcast podcast;

  const PinepodsPodcastGridTile({
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
      episodeCount: 0, // Will be loaded from server
    );
  }

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
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
      child: Semantics(
        label: podcast.title,
        child: GridTile(
          child: Hero(
            key: Key('tilehero${podcast.imageUrl}:${podcast.link}'),
            tag: '${podcast.imageUrl}:${podcast.link}',
            child: TileImage(
              url: podcast.imageUrl!,
              size: 18.0,
            ),
          ),
        ),
      ),
    );
  }
}

class PinepodsPodcastTitledGridTile extends StatelessWidget {
  final Podcast podcast;

  const PinepodsPodcastTitledGridTile({
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
      episodeCount: 0, // Will be loaded from server
    );
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return GestureDetector(
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
      child: GridTile(
        child: Hero(
          key: Key('tilehero${podcast.imageUrl}:${podcast.link}'),
          tag: '${podcast.imageUrl}:${podcast.link}',
          child: Column(
            children: [
              TileImage(
                url: podcast.imageUrl!,
                size: 128.0,
              ),
              Padding(
                padding: const EdgeInsets.only(
                  top: 4.0,
                ),
                child: Text(
                  podcast.title,
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                  textAlign: TextAlign.center,
                  style: theme.textTheme.titleSmall,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}