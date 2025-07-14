// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/bloc/podcast/podcast_bloc.dart';
import 'package:pinepods_mobile/entities/podcast.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/state/bloc_state.dart';
import 'package:pinepods_mobile/ui/widgets/slider_handle.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

/// This widget allows the user to filter the episodes.
class EpisodeFilterSelectorWidget extends StatefulWidget {
  final Podcast? podcast;

  const EpisodeFilterSelectorWidget({
    required this.podcast,
    super.key,
  });

  @override
  State<EpisodeFilterSelectorWidget> createState() => _EpisodeFilterSelectorWidgetState();
}

class _EpisodeFilterSelectorWidgetState extends State<EpisodeFilterSelectorWidget> {
  @override
  Widget build(BuildContext context) {
    var podcastBloc = Provider.of<PodcastBloc>(context);
    var theme = Theme.of(context);

    return StreamBuilder<BlocState<Podcast>>(
        stream: podcastBloc.details,
        initialData: BlocEmptyState<Podcast>(),
        builder: (context, snapshot) {
          return Column(
            mainAxisAlignment: MainAxisAlignment.center,
            crossAxisAlignment: CrossAxisAlignment.center,
            mainAxisSize: MainAxisSize.min,
            children: [
              SizedBox(
                height: 48.0,
                width: 48.0,
                child: Center(
                  child: IconButton(
                    icon: Icon(
                      widget.podcast == null || widget.podcast!.filter == PodcastEpisodeFilter.none
                          ? Icons.filter_alt_outlined
                          : Icons.filter_alt_off_outlined,
                      semanticLabel: L.of(context)!.episode_filter_semantic_label,
                    ),
                    visualDensity: VisualDensity.compact,
                    onPressed: widget.podcast != null && widget.podcast!.subscribed
                        ? () {
                            showModalBottomSheet<void>(
                                isScrollControlled: true,
                                barrierLabel: L.of(context)!.scrim_episode_filter_selector,
                                context: context,
                                backgroundColor: theme.secondaryHeaderColor,
                                shape: const RoundedRectangleBorder(
                                  borderRadius: BorderRadius.only(
                                    topLeft: Radius.circular(16.0),
                                    topRight: Radius.circular(16.0),
                                  ),
                                ),
                                builder: (context) {
                                  return EpisodeFilterSlider(
                                    podcast: widget.podcast!,
                                  );
                                });
                          }
                        : null,
                  ),
                ),
              ),
            ],
          );
        });
  }
}

class EpisodeFilterSlider extends StatefulWidget {
  final Podcast podcast;

  const EpisodeFilterSlider({
    required this.podcast,
    super.key,
  });

  @override
  State<EpisodeFilterSlider> createState() => _EpisodeFilterSliderState();
}

class _EpisodeFilterSliderState extends State<EpisodeFilterSlider> {
  @override
  Widget build(BuildContext context) {
    return Column(
        mainAxisSize: MainAxisSize.min,
        mainAxisAlignment: MainAxisAlignment.start,
        crossAxisAlignment: CrossAxisAlignment.center,
        children: <Widget>[
          const SliderHandle(),
          Padding(
            padding: const EdgeInsets.only(top: 8.0, bottom: 8.0),
            child: Semantics(
              header: true,
              child: Text(
                'Episode Filter',
                style: Theme.of(context).textTheme.titleLarge,
              ),
            ),
          ),
          Padding(
            padding: const EdgeInsets.all(16.0),
            child: ListView(
              shrinkWrap: true,
              children: [
                const Divider(),
                EpisodeFilterSelectorEntry(
                  label: L.of(context)!.episode_filter_none_label,
                  filter: PodcastEpisodeFilter.none,
                  selectedFilter: widget.podcast.filter,
                ),
                const Divider(),
                EpisodeFilterSelectorEntry(
                  label: L.of(context)!.episode_filter_started_label,
                  filter: PodcastEpisodeFilter.started,
                  selectedFilter: widget.podcast.filter,
                ),
                const Divider(),
                EpisodeFilterSelectorEntry(
                  label: L.of(context)!.episode_filter_played_label,
                  filter: PodcastEpisodeFilter.played,
                  selectedFilter: widget.podcast.filter,
                ),
                const Divider(),
                EpisodeFilterSelectorEntry(
                  label: L.of(context)!.episode_filter_unplayed_label,
                  filter: PodcastEpisodeFilter.notPlayed,
                  selectedFilter: widget.podcast.filter,
                ),
                const Divider(),
              ],
            ),
          )
        ]);
  }
}

class EpisodeFilterSelectorEntry extends StatelessWidget {
  const EpisodeFilterSelectorEntry({
    super.key,
    required this.label,
    required this.filter,
    required this.selectedFilter,
  });

  final String label;
  final PodcastEpisodeFilter filter;
  final PodcastEpisodeFilter selectedFilter;

  @override
  Widget build(BuildContext context) {
    final podcastBloc = Provider.of<PodcastBloc>(context);

    return GestureDetector(
      behavior: HitTestBehavior.translucent,
      onTap: () {
        switch (filter) {
          case PodcastEpisodeFilter.none:
            podcastBloc.podcastEvent(PodcastEvent.episodeFilterNone);
            break;
          case PodcastEpisodeFilter.started:
            podcastBloc.podcastEvent(PodcastEvent.episodeFilterStarted);
            break;
          case PodcastEpisodeFilter.played:
            podcastBloc.podcastEvent(PodcastEvent.episodeFilterFinished);
            break;
          case PodcastEpisodeFilter.notPlayed:
            podcastBloc.podcastEvent(PodcastEvent.episodeFilterNotFinished);
            break;
        }

        Navigator.pop(context);
      },
      child: Padding(
        padding: const EdgeInsets.only(
          top: 4.0,
          bottom: 4.0,
        ),
        child: Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          mainAxisSize: MainAxisSize.max,
          children: [
            Semantics(
              selected: filter == selectedFilter,
              child: Text(
                label,
                style: Theme.of(context).textTheme.bodyLarge,
              ),
            ),
            if (filter == selectedFilter)
              const Icon(
                Icons.check,
                size: 18.0,
              ),
          ],
        ),
      ),
    );
  }
}
