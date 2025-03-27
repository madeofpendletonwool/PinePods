// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/bloc/podcast/audio_bloc.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/ui/widgets/episode_tile.dart';
import 'package:pinepods_mobile/ui/widgets/tile_image.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

/// Renders an episode within the queue which can be dragged to re-order the queue.
class DraggableEpisodeTile extends StatelessWidget {
  final Episode episode;
  final int index;
  final bool draggable;
  final bool playable;

  const DraggableEpisodeTile({
    super.key,
    required this.episode,
    this.index = 0,
    this.draggable = true,
    this.playable = false,
  });

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final audioBloc = Provider.of<AudioBloc>(context, listen: false);

    return ListTile(
      key: Key('DT${episode.guid}'),
      enabled: playable,
      leading: TileImage(
        url: episode.thumbImageUrl ?? episode.imageUrl ?? '',
        size: 56.0,
        highlight: episode.highlight,
      ),
      title: Text(
        episode.title!,
        overflow: TextOverflow.ellipsis,
        maxLines: 2,
        softWrap: false,
        style: textTheme.bodyMedium,
      ),
      subtitle: EpisodeSubtitle(episode),
      trailing: draggable
          ? ReorderableDragStartListener(
              index: index,
              child: const Icon(Icons.drag_handle),
            )
          : const SizedBox(
              width: 0.0,
              height: 0.0,
            ),
      onTap: () {
        if (playable) {
          audioBloc.play(episode);
        }
      },
    );
  }
}
