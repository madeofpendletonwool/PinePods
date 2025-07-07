// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/bloc/podcast/queue_bloc.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/state/queue_event_state.dart';
import 'package:pinepods_mobile/ui/podcast/transcript_view.dart';
import 'package:pinepods_mobile/ui/podcast/up_next_view.dart';
import 'package:pinepods_mobile/ui/podcast/pinepods_up_next_view.dart';
import 'package:pinepods_mobile/ui/widgets/slider_handle.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

/// This class gives us options that can be dragged up from the bottom of the main player
/// window.
///
/// Currently these options are Up Next & Transcript.
///
/// This class is an initial version and should by much simpler than it is; however,
/// a [NestedScrollView] is the widget we need to implement this UI, there is a current
/// issue whereby the scroll view and [DraggableScrollableSheet] clash and therefore cannot
/// be used together.
///
/// See issues [64157](https://github.com/flutter/flutter/issues/64157)
///            [67219](https://github.com/flutter/flutter/issues/67219)
///
/// If anyone can come up with a more elegant solution (and one that does not throw
/// an overflow error in debug) please raise and issue/submit a PR.
///
class NowPlayingOptionsSelector extends StatefulWidget {
  final double? scrollPos;
  static const baseSize = 68.0;

  const NowPlayingOptionsSelector({super.key, this.scrollPos});

  @override
  State<NowPlayingOptionsSelector> createState() => _NowPlayingOptionsSelectorState();
}

class _NowPlayingOptionsSelectorState extends State<NowPlayingOptionsSelector> {
  DraggableScrollableController? draggableController;

  @override
  Widget build(BuildContext context) {
    final queueBloc = Provider.of<QueueBloc>(context, listen: false);
    final theme = Theme.of(context);
    final windowHeight = MediaQuery.of(context).size.height;
    final minSize = NowPlayingOptionsSelector.baseSize / (windowHeight - NowPlayingOptionsSelector.baseSize);

    return DraggableScrollableSheet(
      initialChildSize: minSize,
      minChildSize: minSize,
      maxChildSize: 1.0,
      controller: draggableController,
      // Snap doesn't work as the sheet and scroll controller just don't get along
      // snap: true,
      // snapSizes: [minSize, maxSize],
      builder: (BuildContext context, ScrollController scrollController) {
        return DefaultTabController(
          animationDuration: !draggableController!.isAttached || draggableController!.size <= minSize
              ? const Duration(seconds: 0)
              : kTabScrollDuration,
          length: 2,
          child: LayoutBuilder(builder: (BuildContext ctx, BoxConstraints constraints) {
            return SingleChildScrollView(
              controller: scrollController,
              child: ConstrainedBox(
                constraints: BoxConstraints.expand(
                  height: constraints.maxHeight,
                ),
                child: Material(
                  color: theme.secondaryHeaderColor,
                  shape: RoundedRectangleBorder(
                    side: BorderSide(
                      color: Theme.of(context).highlightColor,
                      width: 0.0,
                    ),
                    borderRadius: const BorderRadius.only(
                      topLeft: Radius.circular(18.0),
                      topRight: Radius.circular(18.0),
                    ),
                  ),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    mainAxisAlignment: MainAxisAlignment.start,
                    crossAxisAlignment: CrossAxisAlignment.center,
                    children: <Widget>[
                      SliderHandle(
                        label: optionsSliderOpen()
                            ? L.of(context)!.semantic_playing_options_collapse_label
                            : L.of(context)!.semantic_playing_options_expand_label,
                        onTap: () {
                          if (draggableController != null) {
                            if (draggableController!.size < 1.0) {
                              draggableController!.animateTo(
                                1.0,
                                duration: const Duration(milliseconds: 150),
                                curve: Curves.easeInOut,
                              );
                            } else {
                              draggableController!.animateTo(
                                0.0,
                                duration: const Duration(milliseconds: 150),
                                curve: Curves.easeInOut,
                              );
                            }
                          }
                        },
                      ),
                      DecoratedBox(
                        decoration: BoxDecoration(
                          color: Colors.white.withOpacity(0.0),
                          border: Border(
                            bottom: draggableController != null &&
                                    (!draggableController!.isAttached || draggableController!.size <= minSize)
                                ? BorderSide.none
                                : BorderSide(color: Colors.grey[800]!, width: 1.0),
                          ),
                        ),
                        child: StreamBuilder<QueueState>(
                            initialData: QueueEmptyState(),
                            stream: queueBloc.queue,
                            builder: (context, snapshot) {
                              return TabBar(
                                onTap: (index) {
                                  DefaultTabController.of(ctx).animateTo(index);

                                  if (draggableController != null && draggableController!.size < 1.0) {
                                    draggableController!.animateTo(
                                      1.0,
                                      duration: const Duration(milliseconds: 150),
                                      curve: Curves.easeInOut,
                                    );
                                  }
                                },
                                automaticIndicatorColorAdjustment: false,
                                indicatorPadding: EdgeInsets.zero,

                                /// Little hack to hide the indicator when closed
                                indicatorColor: draggableController != null &&
                                        (!draggableController!.isAttached || draggableController!.size <= minSize)
                                    ? Theme.of(context).secondaryHeaderColor
                                    : null,
                                tabs: [
                                  Padding(
                                    padding: const EdgeInsets.only(top: 8.0, bottom: 8.0),
                                    child: Text(
                                      L.of(context)!.up_next_queue_label.toUpperCase(),
                                      style: Theme.of(context).textTheme.labelLarge,
                                    ),
                                  ),
                                  Padding(
                                    padding: const EdgeInsets.only(top: 8.0, bottom: 8.0),
                                    // If the episode does not support transcripts, grey out
                                    // the option.
                                    child: snapshot.hasData &&
                                            snapshot.data?.playing != null &&
                                            snapshot.data!.playing!.hasTranscripts
                                        ? Text(
                                            L.of(context)!.transcript_label.toUpperCase(),
                                            style: Theme.of(context).textTheme.labelLarge,
                                          )
                                        : Text(
                                            L.of(context)!.transcript_label.toUpperCase(),
                                            style: Theme.of(context)
                                                .textTheme
                                                .labelLarge!
                                                .copyWith(color: theme.disabledColor),
                                          ),
                                  ),
                                ],
                              );
                            }),
                      ),
                      const Padding(padding: EdgeInsets.only(bottom: 12.0)),
                      Expanded(
                        child: Consumer<SettingsBloc>(
                          builder: (context, settingsBloc, child) {
                            final settings = settingsBloc.currentSettings;
                            final isPinepodsConnected = settings.pinepodsServer != null &&
                                settings.pinepodsApiKey != null &&
                                settings.pinepodsUserId != null;

                            return TabBarView(
                              children: [
                                isPinepodsConnected 
                                  ? const PinepodsUpNextView()
                                  : const UpNextView(),
                                const TranscriptView(),
                              ],
                            );
                          },
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            );
          }),
        );
      },
    );
  }

  bool optionsSliderOpen() {
    return (draggableController != null && draggableController!.isAttached && draggableController!.size == 1.0);
  }

  @override
  void initState() {
    draggableController = DraggableScrollableController();
    super.initState();
  }
}

class NowPlayingOptionsScaffold extends StatelessWidget {
  const NowPlayingOptionsScaffold({super.key});

  @override
  Widget build(BuildContext context) {
    return const SizedBox(
      height: NowPlayingOptionsSelector.baseSize - 8.0,
    );
  }
}

/// This implementation displays the additional options in a tab set outside of a
/// draggable sheet.
///
/// Currently these options are Up Next & Transcript.
class NowPlayingOptionsSelectorWide extends StatefulWidget {
  final double? scrollPos;
  static const baseSize = 68.0;

  const NowPlayingOptionsSelectorWide({super.key, this.scrollPos});

  @override
  State<NowPlayingOptionsSelectorWide> createState() => _NowPlayingOptionsSelectorWideState();
}

class _NowPlayingOptionsSelectorWideState extends State<NowPlayingOptionsSelectorWide> {
  DraggableScrollableController? draggableController;

  @override
  Widget build(BuildContext context) {
    final queueBloc = Provider.of<QueueBloc>(context, listen: false);
    final theme = Theme.of(context);
    final scrollController = ScrollController();

    return DefaultTabController(
      length: 2,
      child: LayoutBuilder(builder: (BuildContext ctx, BoxConstraints constraints) {
        return SingleChildScrollView(
          controller: scrollController,
          child: ConstrainedBox(
            constraints: BoxConstraints.expand(
              height: constraints.maxHeight,
            ),
            child: Material(
              color: theme.secondaryHeaderColor,
              child: Column(
                mainAxisSize: MainAxisSize.min,
                mainAxisAlignment: MainAxisAlignment.start,
                crossAxisAlignment: CrossAxisAlignment.center,
                children: <Widget>[
                  DecoratedBox(
                    decoration: BoxDecoration(
                      color: Colors.white.withOpacity(0.0),
                      border: Border(
                        bottom: BorderSide(color: Colors.grey[800]!, width: 1.0),
                      ),
                    ),
                    child: StreamBuilder<QueueState>(
                        initialData: QueueEmptyState(),
                        stream: queueBloc.queue,
                        builder: (context, snapshot) {
                          return TabBar(
                            automaticIndicatorColorAdjustment: false,
                            tabs: [
                              Padding(
                                padding: const EdgeInsets.only(top: 16.0, bottom: 16.0),
                                child: Text(
                                  L.of(context)!.up_next_queue_label.toUpperCase(),
                                  style: Theme.of(context).textTheme.labelLarge,
                                ),
                              ),
                              Padding(
                                padding: const EdgeInsets.only(top: 16.0, bottom: 16.0),
                                child: snapshot.hasData &&
                                        snapshot.data?.playing != null &&
                                        snapshot.data!.playing!.hasTranscripts
                                    ? Text(
                                        L.of(context)!.transcript_label.toUpperCase(),
                                        style: Theme.of(context).textTheme.labelLarge,
                                      )
                                    : Text(
                                        L.of(context)!.transcript_label.toUpperCase(),
                                        style: Theme.of(context)
                                            .textTheme
                                            .labelLarge!
                                            .copyWith(color: theme.disabledColor),
                                      ),
                              ),
                            ],
                          );
                        }),
                  ),
                  const Expanded(
                    child: TabBarView(
                      children: [
                        UpNextView(),
                        TranscriptView(),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          ),
        );
      }),
    );
  }
}
