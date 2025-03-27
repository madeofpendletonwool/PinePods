// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
import 'package:pinepods_mobile/bloc/podcast/podcast_bloc.dart';
import 'package:pinepods_mobile/entities/feed.dart';
import 'package:pinepods_mobile/entities/podcast.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/state/bloc_state.dart';
import 'package:flutter/cupertino.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';

/// This class is responsible for rendering the context menu on the podcast details
/// page.
///
/// It returns either a [_MaterialPodcastMenu] or a [_CupertinoContextMenu}
/// instance depending upon which platform we are running on.
///
/// The target platform is based on the current [Theme]: [ThemeData.platform].
class PodcastContextMenu extends StatelessWidget {
  final Podcast podcast;

  const PodcastContextMenu(
    this.podcast, {
    super.key,
  });

  @override
  Widget build(BuildContext context) {
    var theme = Theme.of(context);

    switch (theme.platform) {
      case TargetPlatform.android:
      case TargetPlatform.fuchsia:
      case TargetPlatform.linux:
      case TargetPlatform.windows:
        return _MaterialPodcastMenu(podcast);
      case TargetPlatform.iOS:
      case TargetPlatform.macOS:
        return _CupertinoContextMenu(podcast);
    }
  }
}

/// This is the material design version of the context menu. This will be rendered
/// for all platforms that are not iOS.
class _MaterialPodcastMenu extends StatelessWidget {
  final Podcast podcast;

  const _MaterialPodcastMenu(this.podcast);

  @override
  Widget build(BuildContext context) {
    final bloc = Provider.of<PodcastBloc>(context);

    return StreamBuilder<BlocState<Podcast>>(
        stream: bloc.details,
        builder: (context, snapshot) {
          return PopupMenuButton<String>(
            onSelected: (event) {
              togglePlayed(value: event, bloc: bloc);
            },
            icon: const Icon(
              Icons.more_vert,
            ),
            itemBuilder: (BuildContext context) {
              return <PopupMenuEntry<String>>[
                PopupMenuItem<String>(
                  value: 'ma',
                  enabled: podcast.subscribed,
                  child: Text(L.of(context)!.mark_episodes_played_label),
                ),
                PopupMenuItem<String>(
                  value: 'ua',
                  enabled: podcast.subscribed,
                  child: Text(L.of(context)!.mark_episodes_not_played_label),
                ),
                const PopupMenuDivider(),
                PopupMenuItem<String>(
                  value: 'refresh',
                  enabled: podcast.link?.isNotEmpty ?? false,
                  child: Text(L.of(context)!.refresh_feed_label),
                ),
                const PopupMenuDivider(),
                PopupMenuItem<String>(
                  value: 'web',
                  enabled: podcast.link?.isNotEmpty ?? false,
                  child: Text(L.of(context)!.open_show_website_label),
                ),
              ];
            },
          );
        });
  }

  void togglePlayed({
    required String value,
    required PodcastBloc bloc,
  }) async {
    if (value == 'ma') {
      bloc.podcastEvent(PodcastEvent.markAllPlayed);
    } else if (value == 'ua') {
      bloc.podcastEvent(PodcastEvent.clearAllPlayed);
    } else if (value == 'refresh') {
      bloc.load(Feed(
        podcast: podcast,
        refresh: true,
      ));
    } else if (value == 'web') {
      final uri = Uri.parse(podcast.link!);

      if (!await launchUrl(
        uri,
        mode: LaunchMode.externalApplication,
      )) {
        throw Exception('Could not launch $uri');
      }
    }
  }
}

/// This is the Cupertino context menu and is rendered only when running on
/// an iOS device.
class _CupertinoContextMenu extends StatelessWidget {
  final Podcast podcast;

  const _CupertinoContextMenu(this.podcast);

  @override
  Widget build(BuildContext context) {
    final bloc = Provider.of<PodcastBloc>(context);

    return StreamBuilder<BlocState<Podcast>>(
        stream: bloc.details,
        builder: (context, snapshot) {
          return IconButton(
            tooltip: L.of(context)!.podcast_options_overflow_menu_semantic_label,
            icon: const Icon(CupertinoIcons.ellipsis),
            onPressed: () => showCupertinoModalPopup<void>(
              context: context,
              builder: (BuildContext context) {
                return CupertinoActionSheet(
                  actions: <Widget>[
                    CupertinoActionSheetAction(
                      isDefaultAction: true,
                      onPressed: () {
                        bloc.podcastEvent(PodcastEvent.markAllPlayed);
                        Navigator.pop(context, 'Cancel');
                      },
                      child: Text(L.of(context)!.mark_episodes_played_label),
                    ),
                    CupertinoActionSheetAction(
                      isDefaultAction: true,
                      onPressed: () {
                        bloc.podcastEvent(PodcastEvent.clearAllPlayed);
                        Navigator.pop(context, 'Cancel');
                      },
                      child: Text(L.of(context)!.mark_episodes_not_played_label),
                    ),
                    CupertinoActionSheetAction(
                      isDefaultAction: true,
                      onPressed: () {
                        bloc.load(Feed(
                          podcast: podcast,
                          refresh: true,
                        ));
                        if (context.mounted) {
                          Navigator.pop(context, 'Cancel');
                        }
                      },
                      child: Text(L.of(context)!.refresh_feed_label),
                    ),
                    CupertinoActionSheetAction(
                      isDefaultAction: true,
                      onPressed: () async {
                        final uri = Uri.parse(podcast.link!);

                        if (!await launchUrl(
                          uri,
                          mode: LaunchMode.externalApplication,
                        )) {
                          throw Exception('Could not launch $uri');
                        }

                        if (context.mounted) {
                          Navigator.pop(context, 'Cancel');
                        }
                      },
                      child: Text(L.of(context)!.open_show_website_label),
                    ),
                  ],
                  cancelButton: CupertinoActionSheetAction(
                    isDefaultAction: true,
                    onPressed: () {
                      Navigator.pop(context, 'Cancel');
                    },
                    child: Text(L.of(context)!.cancel_option_label),
                  ),
                );
              },
            ),
          );
        });
  }
}
