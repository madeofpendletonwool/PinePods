// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'dart:async';

import 'package:pinepods_mobile/bloc/podcast/audio_bloc.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/ui/widgets/placeholder_builder.dart';
import 'package:pinepods_mobile/ui/widgets/podcast_image.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

/// This widget is based upon [MiniPlayer] and provides an additional play/pause control when
/// the episode queue is expanded.
///
/// At some point we should try to merge the common code between this and [MiniPlayer].
class FloatingPlayer extends StatelessWidget {
  const FloatingPlayer({
    super.key,
  });

  @override
  Widget build(BuildContext context) {
    final audioBloc = Provider.of<AudioBloc>(context, listen: false);

    return StreamBuilder<AudioState>(
        stream: audioBloc.playingState,
        builder: (context, snapshot) {
          return (snapshot.hasData &&
                  !(snapshot.data == AudioState.stopped ||
                      snapshot.data == AudioState.none ||
                      snapshot.data == AudioState.error))
              ? _FloatingPlayerBuilder()
              : const SizedBox(
                  height: 0.0,
                );
        });
  }
}

class _FloatingPlayerBuilder extends StatefulWidget {
  @override
  _FloatingPlayerBuilderState createState() => _FloatingPlayerBuilderState();
}

class _FloatingPlayerBuilderState extends State<_FloatingPlayerBuilder> with SingleTickerProviderStateMixin {
  late AnimationController _playPauseController;
  late StreamSubscription<AudioState> _audioStateSubscription;

  @override
  void initState() {
    super.initState();

    _playPauseController = AnimationController(vsync: this, duration: const Duration(milliseconds: 300));
    _playPauseController.value = 1;

    _audioStateListener();
  }

  @override
  void dispose() {
    _audioStateSubscription.cancel();
    _playPauseController.dispose();

    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final textTheme = Theme.of(context).textTheme;
    final audioBloc = Provider.of<AudioBloc>(context, listen: false);
    final placeholderBuilder = PlaceholderBuilder.of(context);

    return Container(
      height: 64,
      color: Theme.of(context).canvasColor,
      child: StreamBuilder<Episode?>(
          stream: audioBloc.nowPlaying,
          builder: (context, snapshot) {
            return StreamBuilder<AudioState>(
                stream: audioBloc.playingState,
                builder: (context, stateSnapshot) {
                  var playing = stateSnapshot.data == AudioState.playing;
                  return Row(
                    mainAxisAlignment: MainAxisAlignment.start,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Padding(
                        padding: const EdgeInsets.all(8.0),
                        child: snapshot.hasData
                            ? PodcastImage(
                                key: Key('float${snapshot.data!.imageUrl}'),
                                url: snapshot.data!.imageUrl!,
                                width: 58.0,
                                height: 58.0,
                                borderRadius: 4.0,
                                placeholder: placeholderBuilder != null
                                    ? placeholderBuilder.builder()(context)
                                    : const Image(image: AssetImage('assets/images/favicon.png')),
                                errorPlaceholder: placeholderBuilder != null
                                    ? placeholderBuilder.errorBuilder()(context)
                                    : const Image(image: AssetImage('assets/images/favicon.png')),
                              )
                            : Container(),
                      ),
                      Expanded(
                          flex: 1,
                          child: Column(
                            mainAxisAlignment: MainAxisAlignment.center,
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: <Widget>[
                              Text(
                                snapshot.data?.title ?? '',
                                overflow: TextOverflow.ellipsis,
                                style: textTheme.bodyMedium,
                              ),
                              Padding(
                                padding: const EdgeInsets.only(top: 4.0),
                                child: Text(
                                  snapshot.data?.author ?? '',
                                  overflow: TextOverflow.ellipsis,
                                  style: textTheme.bodySmall,
                                ),
                              ),
                            ],
                          )),
                      SizedBox(
                        height: 52.0,
                        width: 52.0,
                        child: TextButton(
                          style: TextButton.styleFrom(
                            padding: const EdgeInsets.symmetric(horizontal: 0.0),
                            shape: CircleBorder(
                                side: BorderSide(color: Theme.of(context).colorScheme.surface, width: 0.0)),
                          ),
                          onPressed: () {
                            if (playing) {
                              audioBloc.transitionState(TransitionState.fastforward);
                            }
                          },
                          child: const Icon(
                            Icons.forward_30,
                            size: 36.0,
                          ),
                        ),
                      ),
                      SizedBox(
                        height: 52.0,
                        width: 52.0,
                        child: TextButton(
                          style: TextButton.styleFrom(
                            padding: const EdgeInsets.symmetric(horizontal: 0.0),
                            shape: CircleBorder(
                                side: BorderSide(color: Theme.of(context).colorScheme.surface, width: 0.0)),
                          ),
                          onPressed: () {
                            if (playing) {
                              _pause(audioBloc);
                            } else {
                              _play(audioBloc);
                            }
                          },
                          child: AnimatedIcon(
                            semanticLabel:
                                playing ? L.of(context)!.pause_button_label : L.of(context)!.play_button_label,
                            size: 48.0,
                            icon: AnimatedIcons.play_pause,
                            color: Theme.of(context).iconTheme.color,
                            progress: _playPauseController,
                          ),
                        ),
                      ),
                    ],
                  );
                });
          }),
    );
  }

  /// We call this method to setup a listener for changing [AudioState]. This in turns calls upon the [_pauseController]
  /// to animate the play/pause icon. The [AudioBloc] playingState method is backed by a [BehaviorSubject] so we'll
  /// always get the current state when we subscribe. This, however, has a side effect causing the play/pause icon to
  /// animate when returning from the full-size player, which looks a little odd. Therefore, on the first event we move
  /// the controller to the correct state without animating. This feels a little hacky, but stops the UI from looking a
  /// little odd.
  void _audioStateListener() {
    final audioBloc = Provider.of<AudioBloc>(context, listen: false);
    var firstEvent = true;

    _audioStateSubscription = audioBloc.playingState!.listen((event) {
      if (event == AudioState.playing || event == AudioState.buffering) {
        if (firstEvent) {
          _playPauseController.value = 1;
          firstEvent = false;
        } else {
          _playPauseController.forward();
        }
      } else {
        if (firstEvent) {
          _playPauseController.value = 0;
          firstEvent = false;
        } else {
          _playPauseController.reverse();
        }
      }
    });
  }

  void _play(AudioBloc audioBloc) {
    audioBloc.transitionState(TransitionState.play);
  }

  void _pause(AudioBloc audioBloc) {
    audioBloc.transitionState(TransitionState.pause);
  }
}
