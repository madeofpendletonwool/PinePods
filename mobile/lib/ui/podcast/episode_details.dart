// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/ui/podcast/person_avatar.dart';
import 'package:pinepods_mobile/ui/podcast/transport_controls.dart';
import 'package:pinepods_mobile/ui/widgets/episode_tile.dart';
import 'package:pinepods_mobile/ui/widgets/podcast_html.dart';
import 'package:pinepods_mobile/ui/widgets/tile_image.dart';
import 'package:flutter/material.dart';

/// This class renders the more info widget that is accessed from the 'more'
/// button on an episode.
///
/// The widget is displayed as a draggable, scrollable sheet. This contains
/// episode icon and play/pause control, below which the episode title, show
/// notes and person(s) details (if available).
class EpisodeDetails extends StatefulWidget {
  final Episode episode;

  const EpisodeDetails({
    super.key,
    required this.episode,
  });

  @override
  State<EpisodeDetails> createState() => _EpisodeDetailsState();
}

class _EpisodeDetailsState extends State<EpisodeDetails> {
  @override
  Widget build(BuildContext context) {
    final episode = widget.episode;

    /// Ensure we do not highlight this as a new episode
    episode.highlight = false;

    return DraggableScrollableSheet(
        initialChildSize: 0.6,
        expand: false,
        builder: (BuildContext context, ScrollController scrollController) {
          return SingleChildScrollView(
            controller: scrollController,
            child: Column(
              mainAxisAlignment: MainAxisAlignment.start,
              crossAxisAlignment: CrossAxisAlignment.center,
              children: [
                ExpansionTile(
                    key: const Key('episodemoreinfo'),
                    trailing: PlayControl(
                      episode: episode,
                    ),
                    leading: TileImage(
                      url: episode.thumbImageUrl ?? episode.imageUrl!,
                      size: 56.0,
                      highlight: episode.highlight,
                    ),
                    subtitle: EpisodeSubtitle(episode),
                    title: Text(
                      episode.title!,
                      overflow: TextOverflow.ellipsis,
                      maxLines: 2,
                      softWrap: false,
                      style: Theme.of(context).textTheme.bodyMedium,
                    )),
                const Divider(),
                Padding(
                  padding: const EdgeInsets.all(16.0),
                  child: Align(
                    alignment: Alignment.centerLeft,
                    child: Text(
                      episode.title!,
                      style: Theme.of(context).textTheme.titleLarge!.copyWith(fontWeight: FontWeight.bold),
                    ),
                  ),
                ),
                if (episode.persons.isNotEmpty)
                  SizedBox(
                    height: 120.0,
                    child: ListView.builder(
                      itemCount: episode.persons.length,
                      scrollDirection: Axis.horizontal,
                      itemBuilder: (BuildContext context, int index) {
                        return PersonAvatar(person: episode.persons[index]);
                      },
                    ),
                  ),
                Padding(
                  padding: const EdgeInsets.only(
                    left: 8.0,
                    right: 8.0,
                  ),
                  child: PodcastHtml(content: episode.content ?? episode.description!),
                )
              ],
            ),
          );
        });
  }
}
