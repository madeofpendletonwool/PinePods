// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/bloc/podcast/queue_bloc.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/state/queue_event_state.dart';
import 'package:pinepods_mobile/ui/widgets/action_text.dart';
import 'package:pinepods_mobile/ui/widgets/draggable_episode_tile.dart';
import 'package:flutter/material.dart';
import 'package:flutter_dialogs/flutter_dialogs.dart';
import 'package:provider/provider.dart';

/// This class is responsible for rendering the Up Next queue feature.
///
/// The user can see the currently playing item and the current queue. The user can
/// re-arrange items in the queue, remove individual items or completely clear the queue.
class UpNextView extends StatelessWidget {
  const UpNextView({
    super.key,
  });

  @override
  Widget build(BuildContext context) {
    final queueBloc = Provider.of<QueueBloc>(context, listen: false);

    return StreamBuilder<QueueState>(
        initialData: QueueEmptyState(),
        stream: queueBloc.queue,
        builder: (context, snapshot) {
          return Column(
            mainAxisSize: MainAxisSize.min,
            mainAxisAlignment: MainAxisAlignment.start,
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              Row(
                children: [
                  Padding(
                    padding: const EdgeInsets.fromLTRB(16.0, 8.0, 24.0, 8.0),
                    child: Text(
                      L.of(context)!.now_playing_queue_label,
                      style: Theme.of(context).textTheme.titleLarge,
                    ),
                  ),
                ],
              ),
              Padding(
                padding: const EdgeInsets.fromLTRB(8.0, 8.0, 8.0, 0.0),
                child: DraggableEpisodeTile(
                  key: const Key('detileplaying'),
                  episode: snapshot.data!.playing!,
                  draggable: false,
                ),
              ),
              Row(
                mainAxisAlignment: MainAxisAlignment.start,
                crossAxisAlignment: CrossAxisAlignment.center,
                children: [
                  Padding(
                    padding: const EdgeInsets.fromLTRB(16.0, 0.0, 24.0, 8.0),
                    child: Text(
                      L.of(context)!.up_next_queue_label,
                      style: Theme.of(context).textTheme.titleLarge,
                    ),
                  ),
                  const Spacer(),
                  Padding(
                    padding: const EdgeInsets.fromLTRB(16.0, 0.0, 24.0, 8.0),
                    child: TextButton(
                      onPressed: snapshot.hasData && snapshot.data!.queue.isEmpty
                          ? null
                          : () {
                              showPlatformDialog<void>(
                                context: context,
                                useRootNavigator: false,
                                builder: (_) => BasicDialogAlert(
                                  title: Text(
                                    L.of(context)!.queue_clear_label_title,
                                  ),
                                  content: Text(L.of(context)!.queue_clear_label),
                                  actions: <Widget>[
                                    BasicDialogAction(
                                      title: ActionText(
                                        L.of(context)!.cancel_button_label,
                                      ),
                                      onPressed: () {
                                        Navigator.pop(context);
                                      },
                                    ),
                                    BasicDialogAction(
                                      title: ActionText(
                                        Theme.of(context).platform == TargetPlatform.iOS
                                            ? L.of(context)!.queue_clear_button_label.toUpperCase()
                                            : L.of(context)!.queue_clear_button_label,
                                      ),
                                      iosIsDefaultAction: true,
                                      iosIsDestructiveAction: true,
                                      onPressed: () {
                                        queueBloc.queueEvent(QueueClearEvent());
                                        Navigator.pop(context);
                                      },
                                    ),
                                  ],
                                ),
                              );
                            },
                      child: snapshot.hasData && snapshot.data!.queue.isEmpty
                          ? Text(
                              L.of(context)!.clear_queue_button_label,
                              style: Theme.of(context).textTheme.titleSmall!.copyWith(
                                    fontSize: 12.0,
                                    color: Theme.of(context).disabledColor,
                                  ),
                            )
                          : Text(
                              L.of(context)!.clear_queue_button_label,
                              style: Theme.of(context).textTheme.titleSmall!.copyWith(
                                    fontSize: 12.0,
                                    color: Theme.of(context).primaryColor,
                                  ),
                            ),
                    ),
                  ),
                ],
              ),
              snapshot.hasData && snapshot.data!.queue.isEmpty
                  ? Padding(
                      padding: const EdgeInsets.all(24.0),
                      child: Container(
                        decoration: BoxDecoration(
                            color: Theme.of(context).dividerColor,
                            border: Border.all(
                              color: Theme.of(context).dividerColor,
                            ),
                            borderRadius: const BorderRadius.all(Radius.circular(10))),
                        child: Padding(
                          padding: const EdgeInsets.all(24.0),
                          child: Text(
                            L.of(context)!.empty_queue_message,
                            style: Theme.of(context).textTheme.titleMedium,
                          ),
                        ),
                      ),
                    )
                  : Expanded(
                      child: ReorderableListView.builder(
                        buildDefaultDragHandles: false,
                        shrinkWrap: true,
                        padding: const EdgeInsets.all(8),
                        itemCount: snapshot.hasData ? snapshot.data!.queue.length : 0,
                        itemBuilder: (BuildContext context, int index) {
                          return Dismissible(
                            key: ValueKey('disqueue${snapshot.data!.queue[index].guid}'),
                            direction: DismissDirection.endToStart,
                            onDismissed: (direction) {
                              queueBloc.queueEvent(QueueRemoveEvent(episode: snapshot.data!.queue[index]));
                            },
                            child: DraggableEpisodeTile(
                              key: ValueKey('tilequeue${snapshot.data!.queue[index].guid}'),
                              index: index,
                              episode: snapshot.data!.queue[index],
                              playable: true,
                            ),
                          );
                        },
                        onReorder: (int oldIndex, int newIndex) {
                          /// Seems odd to have to do this, but this -1 was taken from
                          /// the Flutter docs.
                          if (oldIndex < newIndex) {
                            newIndex -= 1;
                          }

                          queueBloc.queueEvent(QueueMoveEvent(
                            episode: snapshot.data!.queue[oldIndex],
                            oldIndex: oldIndex,
                            newIndex: newIndex,
                          ));
                        },
                      ),
                    ),
            ],
          );
        });
  }
}
