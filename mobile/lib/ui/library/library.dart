// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/bloc/podcast/podcast_bloc.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/app_settings.dart';
import 'package:pinepods_mobile/entities/podcast.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/ui/widgets/platform_progress_indicator.dart';
import 'package:pinepods_mobile/ui/widgets/podcast_grid_tile.dart';
import 'package:pinepods_mobile/ui/widgets/podcast_tile.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

/// This class displays the list of podcasts the user is currently following.
class Library extends StatefulWidget {
  const Library({
    super.key,
  });

  @override
  State<Library> createState() => _LibraryState();
}

class _LibraryState extends State<Library> {
  @override
  Widget build(BuildContext context) {
    final podcastBloc = Provider.of<PodcastBloc>(context);
    final settingsBloc = Provider.of<SettingsBloc>(context);

    return StreamBuilder<List<Podcast>>(
        stream: podcastBloc.subscriptions,
        builder: (context, snapshot) {
          if (snapshot.hasData) {
            if (snapshot.data!.isEmpty) {
              return SliverFillRemaining(
                hasScrollBody: false,
                child: Padding(
                  padding: const EdgeInsets.all(32.0),
                  child: Column(
                    mainAxisAlignment: MainAxisAlignment.center,
                    crossAxisAlignment: CrossAxisAlignment.center,
                    children: <Widget>[
                      Icon(
                        Icons.headset,
                        size: 75,
                        color: Theme.of(context).primaryColor,
                      ),
                      Text(
                        L.of(context)!.no_subscriptions_message,
                        style: Theme.of(context).textTheme.titleLarge,
                        textAlign: TextAlign.center,
                      ),
                    ],
                  ),
                ),
              );
            } else {
              return StreamBuilder<AppSettings>(
                  stream: settingsBloc.settings,
                  builder: (context, settingsSnapshot) {
                    if (settingsSnapshot.hasData) {
                      var mode = settingsSnapshot.data!.layout;
                      var size = mode == 1 ? 100.0 : 160.0;

                      if (mode == 0) {
                        return SliverList(
                            delegate: SliverChildBuilderDelegate(
                          (BuildContext context, int index) {
                            return PodcastTile(podcast: snapshot.data!.elementAt(index));
                          },
                          childCount: snapshot.data!.length,
                          addAutomaticKeepAlives: false,
                        ));
                      }
                      return SliverGrid(
                        gridDelegate: SliverGridDelegateWithMaxCrossAxisExtent(
                          maxCrossAxisExtent: size,
                          mainAxisSpacing: 10.0,
                          crossAxisSpacing: 10.0,
                        ),
                        delegate: SliverChildBuilderDelegate(
                          (BuildContext context, int index) {
                            return PodcastGridTile(podcast: snapshot.data!.elementAt(index));
                          },
                          childCount: snapshot.data!.length,
                        ),
                      );
                    } else {
                      return const SliverFillRemaining(
                        hasScrollBody: false,
                        child: SizedBox(
                          height: 0,
                          width: 0,
                        ),
                      );
                    }
                  });
            }
          } else {
            return const SliverFillRemaining(
              hasScrollBody: false,
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                crossAxisAlignment: CrossAxisAlignment.center,
                children: <Widget>[
                  PlatformProgressIndicator(),
                ],
              ),
            );
          }
        });
  }
}
