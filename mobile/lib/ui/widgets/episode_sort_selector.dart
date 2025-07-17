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
class EpisodeSortSelectorWidget extends StatefulWidget {
  final Podcast? podcast;

  const EpisodeSortSelectorWidget({
    required this.podcast,
    super.key,
  });

  @override
  State<EpisodeSortSelectorWidget> createState() => _EpisodeSortSelectorWidgetState();
}

class _EpisodeSortSelectorWidgetState extends State<EpisodeSortSelectorWidget> {
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
                      Icons.sort,
                      semanticLabel: L.of(context)!.episode_sort_semantic_label,
                    ),
                    visualDensity: VisualDensity.compact,
                    onPressed: widget.podcast != null && widget.podcast!.subscribed
                        ? () {
                            showModalBottomSheet<void>(
                                barrierLabel: L.of(context)!.scrim_episode_sort_selector,
                                isScrollControlled: true,
                                context: context,
                                backgroundColor: theme.secondaryHeaderColor,
                                shape: const RoundedRectangleBorder(
                                  borderRadius: BorderRadius.only(
                                    topLeft: Radius.circular(16.0),
                                    topRight: Radius.circular(16.0),
                                  ),
                                ),
                                builder: (context) {
                                  return EpisodeSortSlider(
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

class EpisodeSortSlider extends StatefulWidget {
  final Podcast podcast;

  const EpisodeSortSlider({
    required this.podcast,
    super.key,
  });

  @override
  State<EpisodeSortSlider> createState() => _EpisodeSortSliderState();
}

class _EpisodeSortSliderState extends State<EpisodeSortSlider> {
  @override
  Widget build(BuildContext context) {
    return Column(
        mainAxisSize: MainAxisSize.min,
        mainAxisAlignment: MainAxisAlignment.start,
        crossAxisAlignment: CrossAxisAlignment.center,
        children: <Widget>[
          const SliderHandle(),
          Semantics(
            header: true,
            child: Padding(
              padding: const EdgeInsets.only(top: 8.0, bottom: 8.0),
              child: Text(
                L.of(context)!.episode_sort_semantic_label,
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
                EpisodeSortSelectorEntry(
                  label: L.of(context)!.episode_sort_none_label,
                  sort: PodcastEpisodeSort.none,
                  selectedSort: widget.podcast.sort,
                ),
                const Divider(),
                EpisodeSortSelectorEntry(
                  label: L.of(context)!.episode_sort_latest_first_label,
                  sort: PodcastEpisodeSort.latestFirst,
                  selectedSort: widget.podcast.sort,
                ),
                const Divider(),
                EpisodeSortSelectorEntry(
                  label: L.of(context)!.episode_sort_earliest_first_label,
                  sort: PodcastEpisodeSort.earliestFirst,
                  selectedSort: widget.podcast.sort,
                ),
                const Divider(),
                EpisodeSortSelectorEntry(
                  label: L.of(context)!.episode_sort_alphabetical_ascending_label,
                  sort: PodcastEpisodeSort.alphabeticalAscending,
                  selectedSort: widget.podcast.sort,
                ),
                const Divider(),
                EpisodeSortSelectorEntry(
                  label: L.of(context)!.episode_sort_alphabetical_descending_label,
                  sort: PodcastEpisodeSort.alphabeticalDescending,
                  selectedSort: widget.podcast.sort,
                ),
                const Divider(),
              ],
            ),
          )
        ]);
  }
}

class EpisodeSortSelectorEntry extends StatelessWidget {
  const EpisodeSortSelectorEntry({
    super.key,
    required this.label,
    required this.sort,
    required this.selectedSort,
  });

  final String label;
  final PodcastEpisodeSort sort;
  final PodcastEpisodeSort selectedSort;

  @override
  Widget build(BuildContext context) {
    final podcastBloc = Provider.of<PodcastBloc>(context);

    return GestureDetector(
      behavior: HitTestBehavior.translucent,
      onTap: () {
        switch (sort) {
          case PodcastEpisodeSort.none:
            podcastBloc.podcastEvent(PodcastEvent.episodeSortDefault);
            break;
          case PodcastEpisodeSort.latestFirst:
            podcastBloc.podcastEvent(PodcastEvent.episodeSortLatest);
            break;
          case PodcastEpisodeSort.earliestFirst:
            podcastBloc.podcastEvent(PodcastEvent.episodeSortEarliest);
            break;
          case PodcastEpisodeSort.alphabeticalAscending:
            podcastBloc.podcastEvent(PodcastEvent.episodeSortAlphabeticalAscending);
            break;
          case PodcastEpisodeSort.alphabeticalDescending:
            podcastBloc.podcastEvent(PodcastEvent.episodeSortAlphabeticalDescending);
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
              selected: sort == selectedSort,
              child: Text(
                label,
                style: Theme.of(context).textTheme.bodyLarge,
              ),
            ),
            if (sort == selectedSort)
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
