// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'dart:async';

import 'package:pinepods_mobile/bloc/podcast/audio_bloc.dart';
import 'package:pinepods_mobile/bloc/podcast/queue_bloc.dart';
import 'package:pinepods_mobile/entities/person.dart';
import 'package:pinepods_mobile/entities/transcript.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/state/queue_event_state.dart';
import 'package:pinepods_mobile/state/transcript_state_event.dart';
import 'package:pinepods_mobile/ui/widgets/platform_progress_indicator.dart';
import 'package:extended_image/extended_image.dart';
import 'package:flutter/material.dart';
import 'package:logging/logging.dart';
import 'package:provider/provider.dart';
import 'package:scrollable_positioned_list/scrollable_positioned_list.dart';
import 'package:url_launcher/url_launcher.dart';

/// This class handles the rendering of the podcast transcript (where available).
// ignore: must_be_immutable
class TranscriptView extends StatefulWidget {
  const TranscriptView({
    super.key,
  });

  @override
  State<TranscriptView> createState() => _TranscriptViewState();
}

class _TranscriptViewState extends State<TranscriptView> {
  final log = Logger('TranscriptView');
  final ItemScrollController _itemScrollController = ItemScrollController();
  final ScrollOffsetListener _scrollOffsetListener = ScrollOffsetListener.create(recordProgrammaticScrolls: false);
  final _transcriptSearchController = TextEditingController();
  late StreamSubscription<PositionState> _positionSubscription;
  int position = 0;
  bool autoScroll = true;
  bool autoScrollEnabled = true;
  bool forceTranscriptUpdate = false;
  bool first = true;
  bool scrolling = false;
  String speaker = '';
  RegExp exp = RegExp(r'(^)(\[?)(?<speaker>[A-Za-z0-9\s]+)(\]?)(\s?)(:)');

  @override
  void initState() {
    super.initState();
    final audioBloc = Provider.of<AudioBloc>(context, listen: false);

    Subtitle? subtitle;
    int index = 0;
    // If the user initiates scrolling, disable auto scroll.
    _scrollOffsetListener.changes.listen((event) {
      if (!scrolling) {
        setState(() {
          autoScroll = false;
        });
      }
    });

    // Listen to playback position updates and scroll to the correct items in the transcript
    // if we have auto scroll enabled.
    _positionSubscription = audioBloc.playPosition!.listen((event) {
      if (_itemScrollController.isAttached) {
        var transcript = event.episode?.transcript;

        if (transcript != null && transcript.subtitles.isNotEmpty) {
          subtitle ??= transcript.subtitles[index];

          if (index == 0) {
            var match = exp.firstMatch(subtitle?.data ?? '');

            if (match != null) {
              setState(() {
                speaker = match.namedGroup('speaker') ?? '';
              });
            }
          }

          // Our we outside the range of our current transcript.
          if (event.position.inMilliseconds < subtitle!.start.inMilliseconds ||
              event.position.inMilliseconds > subtitle!.end!.inMilliseconds ||
              forceTranscriptUpdate) {
            forceTranscriptUpdate = false;
            // Will the next in the list do?
            if (transcript.subtitles.length > (index + 1) &&
                event.position.inMilliseconds >= transcript.subtitles[index + 1].start.inMilliseconds &&
                event.position.inMilliseconds < transcript.subtitles[index + 1].end!.inMilliseconds) {
              index++;
              subtitle = transcript.subtitles[index];

              if (subtitle != null && subtitle!.speaker.isNotEmpty) {
                speaker = subtitle!.speaker;
              } else {
                var match = exp.firstMatch(transcript.subtitles[index].data ?? '');

                if (match != null) {
                  speaker = match.namedGroup('speaker') ?? '';
                }
              }
            } else {
              try {
                subtitle = transcript.subtitles
                    .where((a) => (event.position.inMilliseconds >= a.start.inMilliseconds &&
                        event.position.inMilliseconds < a.end!.inMilliseconds))
                    .first;

                index = transcript.subtitles.indexOf(subtitle!);

                /// If we have had to jump more than one position within the transcript, we may
                /// need to back scan the conversation to find the current speaker.
                if (subtitle!.speaker.isNotEmpty) {
                  speaker = subtitle!.speaker;
                } else {
                  /// Scan backwards a maximum of 50 lines to see if we can find a speaker
                  var speakFound = false;
                  var count = 50;
                  var countIndex = index;

                  while (!speakFound && count-- > 0 && countIndex >= 0) {
                    var match = exp.firstMatch(transcript.subtitles[countIndex].data!);

                    countIndex--;

                    if (match != null) {
                      speaker = match.namedGroup('speaker') ?? '';

                      if (speaker.isNotEmpty) {
                        setState(() {
                          speakFound = true;
                        });
                      }
                    }
                  }
                }
              } catch (e) {
                // We don't have a transcript entry for this position.
              }
            }

            if (subtitle != null) {
              setState(() {
                position = subtitle!.start.inMilliseconds;
              });
            }

            if (autoScroll) {
              if (first) {
                _itemScrollController.jumpTo(index: index);
                first = false;
              } else {
                scrolling = true;
                _itemScrollController.scrollTo(index: index, duration: const Duration(milliseconds: 50)).then((value) {
                  scrolling = false;
                });
              }
            }
          }
        }
      }
    });
  }

  @override
  void dispose() {
    super.dispose();
    _positionSubscription.cancel();
  }

  @override
  Widget build(BuildContext context) {
    final audioBloc = Provider.of<AudioBloc>(context, listen: false);
    final queueBloc = Provider.of<QueueBloc>(context, listen: false);

    return StreamBuilder<QueueState>(
        initialData: QueueEmptyState(),
        stream: queueBloc.queue,
        builder: (context, queueSnapshot) {
          return StreamBuilder<TranscriptState>(
              stream: audioBloc.nowPlayingTranscript,
              builder: (context, transcriptSnapshot) {
                if (transcriptSnapshot.hasData) {
                  if (transcriptSnapshot.data is TranscriptLoadingState) {
                    return const Align(
                      alignment: Alignment.center,
                      child: PlatformProgressIndicator(),
                    );
                  } else if (transcriptSnapshot.data is TranscriptUnavailableState ||
                      !transcriptSnapshot.data!.transcript!.transcriptAvailable) {
                    return Padding(
                      padding: const EdgeInsets.all(16.0),
                      child: Align(
                        alignment: Alignment.center,
                        child: Column(
                          mainAxisAlignment: MainAxisAlignment.center,
                          crossAxisAlignment: CrossAxisAlignment.center,
                          children: [
                            Text(
                              L.of(context)!.no_transcript_available_label,
                              style: Theme.of(context).textTheme.titleLarge,
                              textAlign: TextAlign.center,
                            ),
                            Padding(
                              padding: const EdgeInsets.only(top: 32.0, bottom: 32.0),
                              child: OutlinedButton(
                                  onPressed: () {
                                    final uri = Uri.parse(L.of(context)!.transcript_why_not_url);

                                    unawaited(
                                      canLaunchUrl(uri).then((value) => launchUrl(uri)),
                                    );
                                  },
                                  child: Text(
                                    L.of(context)!.transcript_why_not_label,
                                    style: Theme.of(context).textTheme.titleSmall,
                                    textAlign: TextAlign.center,
                                  )),
                            ),
                          ],
                        ),
                      ),
                    );
                  } else {
                    final items = transcriptSnapshot.data!.transcript?.subtitles ?? <Subtitle>[];

                    return Column(
                      children: [
                        Padding(
                          padding: const EdgeInsets.only(top: 8.0, left: 16.0, right: 16.0),
                          child: TextField(
                            controller: _transcriptSearchController,
                            decoration: InputDecoration(
                              contentPadding: const EdgeInsets.all(0.0),
                              prefixIcon: const Icon(Icons.search),
                              suffixIcon: IconButton(
                                icon: const Icon(Icons.close),
                                onPressed: () {
                                  _transcriptSearchController.clear();
                                  audioBloc.filterTranscript(TranscriptClearEvent());
                                  setState(() {
                                    autoScrollEnabled = true;
                                  });
                                },
                              ),
                              isDense: true,
                              filled: true,
                              border: const OutlineInputBorder(
                                borderRadius: BorderRadius.all(Radius.circular(8.0)),
                                borderSide: BorderSide.none,
                                gapPadding: 0.0,
                              ),
                              hintText: L.of(context)!.search_transcript_label,
                            ),
                            onSubmitted: ((search) {
                              if (search.isNotEmpty) {
                                setState(() {
                                  autoScrollEnabled = false;
                                  autoScroll = false;
                                });
                                audioBloc.filterTranscript(TranscriptFilterEvent(search: search));
                              }
                            }),
                          ),
                        ),
                        Padding(
                          padding: const EdgeInsets.only(left: 8.0, right: 8.0),
                          child: Row(
                            mainAxisAlignment: MainAxisAlignment.end,
                            crossAxisAlignment: CrossAxisAlignment.center,
                            children: [
                              Text(L.of(context)!.auto_scroll_transcript_label),
                              Switch(
                                value: autoScroll,
                                onChanged: autoScrollEnabled
                                    ? (bool enableAutoScroll) {
                                        setState(() {
                                          autoScroll = enableAutoScroll;

                                          if (enableAutoScroll) {
                                            forceTranscriptUpdate = true;
                                          }
                                        });
                                      }
                                    : null,
                              ),
                            ],
                          ),
                        ),
                        if (queueSnapshot.hasData &&
                            queueSnapshot.data?.playing != null &&
                            queueSnapshot.data!.playing!.persons.isNotEmpty)
                          Container(
                            padding: const EdgeInsets.only(left: 16.0),
                            width: double.infinity,
                            height: 72.0,
                            child: ListView.builder(
                                scrollDirection: Axis.horizontal,
                                itemCount: queueSnapshot.data!.playing!.persons.length,
                                itemBuilder: (BuildContext context, int index) {
                                  var person = queueSnapshot.data!.playing!.persons[index];
                                  var selected = false;

                                  // Some speakers are - delimited so won't match
                                  speaker = speaker.replaceAll('-', ' ');

                                  if (speaker.isNotEmpty &&
                                      person.name.toLowerCase().startsWith(speaker.toLowerCase())) {
                                    selected = true;
                                  }

                                  return Padding(
                                    padding: const EdgeInsets.only(right: 8.0),
                                    child: Container(
                                      padding: const EdgeInsets.all(4.0),
                                      decoration: BoxDecoration(
                                          color: selected ? Colors.orange : Colors.transparent, shape: BoxShape.circle),
                                      child: CircleAvatar(
                                        radius: 28,
                                        backgroundImage: ExtendedImage.network(
                                          person.image!,
                                          cache: true,
                                        ).image,
                                        child: const Text(''),
                                      ),
                                    ),
                                  );
                                }),
                          ),
                        Expanded(
                          /// A simple way to ensure the builder is visible before attempting to use it.
                          child: LayoutBuilder(builder: (context, constraints) {
                            return constraints.minHeight > 60.0
                                ? Padding(
                                    padding: const EdgeInsets.only(top: 8.0),
                                    child: ScrollablePositionedList.builder(
                                        itemScrollController: _itemScrollController,
                                        scrollOffsetListener: _scrollOffsetListener,
                                        itemCount: items.length,
                                        itemBuilder: (BuildContext context, int index) {
                                          var i = items[index];
                                          return Wrap(
                                            children: [
                                              SubtitleWidget(
                                                subtitle: i,
                                                persons: queueSnapshot.data?.playing?.persons ?? <Person>[],
                                                highlight: i.start.inMilliseconds == position,
                                              ),
                                            ],
                                          );
                                        }),
                                  )
                                : Container();
                          }),
                        ),
                      ],
                    );
                  }
                } else {
                  return Container();
                }
              });
        });
  }
}

/// Each transcript is made up of one or more subtitles. Each [Subtitle] represents one
/// line of the transcript. This widget handles rendering the passed line.
class SubtitleWidget extends StatelessWidget {
  final Subtitle subtitle;
  final List<Person>? persons;
  final bool highlight;
  static const margin = Duration(milliseconds: 1000);

  const SubtitleWidget({
    super.key,
    required this.subtitle,
    this.persons,
    this.highlight = false,
  });

  @override
  Widget build(BuildContext context) {
    final audioBloc = Provider.of<AudioBloc>(context, listen: false);

    return GestureDetector(
      behavior: HitTestBehavior.translucent,
      onTap: () {
        final p = subtitle.start + margin;

        audioBloc.transitionPosition(p.inSeconds.toDouble());
      },
      child: Container(
        width: double.infinity,
        padding: const EdgeInsets.fromLTRB(16.0, 16.0, 16.0, 0.0),
        color: highlight ? Theme.of(context).cardTheme.color : Colors.transparent,
        child: Column(
          mainAxisAlignment: MainAxisAlignment.start,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              subtitle.speaker.isEmpty
                  ? _formatDuration(subtitle.start)
                  : '${_formatDuration(subtitle.start)} - ${subtitle.speaker}',
              style: Theme.of(context).textTheme.titleSmall,
            ),
            Text(
              subtitle.data!,
              style: Theme.of(context).textTheme.titleMedium,
            ),
            const Padding(padding: EdgeInsets.fromLTRB(0.0, 0.0, 0.0, 16.0))
          ],
        ),
      ),
    );
  }

  String _formatDuration(Duration duration) {
    final hh = (duration.inHours).toString().padLeft(2, '0');
    final mm = (duration.inMinutes % 60).toString().padLeft(2, '0');
    final ss = (duration.inSeconds % 60).toString().padLeft(2, '0');

    return '$hh:$mm:$ss';
  }
}
