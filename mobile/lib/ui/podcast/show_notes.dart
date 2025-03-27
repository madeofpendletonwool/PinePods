// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/ui/widgets/podcast_html.dart';
import 'package:flutter/material.dart';
import 'package:flutter_html/flutter_html.dart';

/// This class displays the show notes for the selected podcast.
///
/// We make use of [Html] to render the notes and, if in HTML format, display the
/// correct formatting, links etc.
class ShowNotes extends StatelessWidget {
  final ScrollController _sliverScrollController = ScrollController();
  final Episode episode;

  ShowNotes({
    super.key,
    required this.episode,
  });

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;

    return Scaffold(
        backgroundColor: Theme.of(context).scaffoldBackgroundColor,
        body: CustomScrollView(controller: _sliverScrollController, slivers: <Widget>[
          SliverAppBar(
            title: Text(episode.podcast!),
            floating: false,
            pinned: true,
            snap: false,
          ),
          SliverToBoxAdapter(
            child: Column(
              mainAxisAlignment: MainAxisAlignment.start,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Padding(
                  padding: const EdgeInsets.fromLTRB(16.0, 8.0, 16.0, 8.0),
                  child: Text(episode.title ?? '', style: textTheme.titleLarge),
                ),
                Padding(
                  padding: const EdgeInsets.fromLTRB(8.0, 0.0, 8.0, 0.0),
                  child: PodcastHtml(content: episode.content ?? episode.description!),
                ),
              ],
            ),
          ),
        ]));
  }
}
