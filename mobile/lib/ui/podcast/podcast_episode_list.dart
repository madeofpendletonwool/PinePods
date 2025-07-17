// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/bloc/podcast/queue_bloc.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/state/queue_event_state.dart';
import 'package:pinepods_mobile/ui/widgets/episode_tile.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

class PodcastEpisodeList extends StatelessWidget {
  final List<Episode?>? episodes;
  final IconData icon;
  final String emptyMessage;
  final bool play;
  final bool download;

  static const _defaultIcon = Icons.add_alert;

  const PodcastEpisodeList({
    super.key,
    required this.episodes,
    required this.play,
    required this.download,
    this.icon = _defaultIcon,
    this.emptyMessage = '',
  });

  @override
  Widget build(BuildContext context) {
    if (episodes != null && episodes!.isNotEmpty) {
      var queueBloc = Provider.of<QueueBloc>(context);

      return StreamBuilder<QueueState>(
          stream: queueBloc.queue,
          builder: (context, snapshot) {
            return SliverList(
                delegate: SliverChildBuilderDelegate(
              (BuildContext context, int index) {
                var queued = false;
                var playing = false;
                var episode = episodes![index]!;

                if (snapshot.hasData) {
                  var playingGuid = snapshot.data!.playing?.guid;

                  queued = snapshot.data!.queue.any((element) => element.guid == episode.guid);

                  playing = playingGuid == episode.guid;
                }

                return EpisodeTile(
                  episode: episode,
                  download: download,
                  play: play,
                  playing: playing,
                  queued: queued,
                );
              },
              childCount: episodes!.length,
              addAutomaticKeepAlives: false,
            ));
          });
    } else {
      return SliverFillRemaining(
        hasScrollBody: false,
        child: Padding(
          padding: const EdgeInsets.all(32.0),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.center,
            children: <Widget>[
              Icon(
                icon,
                size: 75,
                color: Theme.of(context).primaryColor,
              ),
              Text(
                emptyMessage,
                style: Theme.of(context).textTheme.titleLarge,
                textAlign: TextAlign.center,
              ),
            ],
          ),
        ),
      );
    }
  }
}
