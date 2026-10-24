// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/bloc/podcast/audio_bloc.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/ui/podcast/now_playing.dart';
import 'package:pinepods_mobile/ui/podcast/skip_interval_icon.dart';
import 'package:pinepods_mobile/ui/widgets/placeholder_builder.dart';
import 'package:pinepods_mobile/ui/widgets/podcast_image.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';

/// Displays a mini podcast player widget if a podcast is playing or paused.
///
/// If stopped a zero height box is built instead. Tapping on the mini player
/// will open the main player window.
///
/// The app shell builds one instance outside the tab navigator, so it stays
/// mounted while pages are pushed and popped above it.
class MiniPlayer extends StatelessWidget {
  const MiniPlayer({
    super.key,
  });

  static bool _isVisible(AudioState? state, Episode? episode) {
    if (episode == null) return false;
    return state != null && state != AudioState.stopped && state != AudioState.none;
  }

  @override
  Widget build(BuildContext context) {
    final audioBloc = Provider.of<AudioBloc>(context, listen: false);

    return StreamBuilder<AudioState>(
        stream: audioBloc.playingState,
        initialData: audioBloc.playingState?.valueOrNull,
        builder: (context, snapshot) {
          final active = _isVisible(snapshot.data, audioBloc.nowPlaying?.valueOrNull);
          return AnimatedSize(
            duration: const Duration(milliseconds: 250),
            curve: Curves.easeOut,
            child: active
                ? const Material(type: MaterialType.transparency, child: _MiniPlayerBody())
                : const SizedBox(width: double.infinity),
          );
        });
  }
}

class _MiniPlayerBody extends StatelessWidget {
  const _MiniPlayerBody();

  void _openNowPlaying(BuildContext context) => showNowPlaying(context);

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final textTheme = theme.textTheme;
    final audioBloc = Provider.of<AudioBloc>(context, listen: false);
    final placeholderBuilder = PlaceholderBuilder.of(context);
    final fallbackImage = const Image(image: AssetImage('assets/images/favicon.png'));

    return StreamBuilder<Episode?>(
        stream: audioBloc.nowPlaying,
        initialData: audioBloc.nowPlaying?.valueOrNull,
        builder: (context, snapshot) {
          final episode = snapshot.data;
          return Dismissible(
            key: ValueKey('mini-${episode?.guid}'),
            confirmDismiss: (direction) async {
              audioBloc.transitionState(TransitionState.stop);
              return true;
            },
            direction: DismissDirection.startToEnd,
            background: Container(color: theme.colorScheme.surface),
            child: GestureDetector(
              key: const Key('miniplayergesture'),
              behavior: HitTestBehavior.opaque,
              onTap: () => _openNowPlaying(context),
              child: Semantics(
                header: true,
                label: L.of(context)!.semantics_mini_player_header,
                child: Container(
                  height: 66,
                  decoration: BoxDecoration(
                    color: theme.colorScheme.surface,
                    border: Border(
                      top: Divider.createBorderSide(context, width: 1.0, color: theme.dividerColor),
                    ),
                  ),
                  child: Column(
                    children: [
                      _ProgressLine(audioBloc: audioBloc),
                      Expanded(
                        child: StreamBuilder<AudioState>(
                            stream: audioBloc.playingState,
                            initialData: audioBloc.playingState?.valueOrNull,
                            builder: (context, stateSnapshot) {
                              final state = stateSnapshot.data;
                              final playing = state == AudioState.playing;
                              final loading = state == AudioState.buffering || state == AudioState.starting;
                              final failed = state == AudioState.error;

                              return Row(
                                children: <Widget>[
                                  Padding(
                                    padding: const EdgeInsets.fromLTRB(10, 8, 12, 8),
                                    child: ExcludeSemantics(
                                      child: SizedBox(
                                        height: 46,
                                        width: 46,
                                        child: episode?.imageUrl != null
                                            ? PodcastImage(
                                                key: Key('mini${episode!.imageUrl}'),
                                                url: episode.imageUrl!,
                                                width: 46,
                                                height: 46,
                                                borderRadius: 8.0,
                                                placeholder: placeholderBuilder?.builder()(context) ?? fallbackImage,
                                                errorPlaceholder:
                                                    placeholderBuilder?.errorBuilder()(context) ?? fallbackImage,
                                              )
                                            : const SizedBox.shrink(),
                                      ),
                                    ),
                                  ),
                                  Expanded(
                                    child: Column(
                                      mainAxisAlignment: MainAxisAlignment.center,
                                      crossAxisAlignment: CrossAxisAlignment.start,
                                      children: <Widget>[
                                        Text(
                                          episode?.title ?? '',
                                          maxLines: 1,
                                          overflow: TextOverflow.ellipsis,
                                          style: textTheme.bodyMedium?.copyWith(fontWeight: FontWeight.w600),
                                        ),
                                        const SizedBox(height: 3),
                                        Text(
                                          failed ? "Couldn't play this episode · tap ↻ to retry" : episode?.author ?? '',
                                          maxLines: 1,
                                          overflow: TextOverflow.ellipsis,
                                          style: textTheme.bodySmall?.copyWith(
                                            color: failed ? theme.colorScheme.error : theme.hintColor,
                                          ),
                                        ),
                                      ],
                                    ),
                                  ),
                                  if (!failed)
                                    IconButton(
                                      onPressed: playing
                                          ? () => audioBloc.transitionState(TransitionState.fastforward)
                                          : null,
                                      icon: SkipIntervalIcon(
                                        icon: PhosphorIconsRegular.arrowClockwise,
                                        seconds: Provider.of<SettingsBloc>(context, listen: false)
                                            .currentSettings
                                            .fastForwardInterval,
                                        size: 32,
                                        semanticLabel: L.of(context)!.fast_forward_button_label,
                                      ),
                                    ),
                                  Padding(
                                    padding: const EdgeInsets.only(right: 6),
                                    child: SizedBox(
                                      width: 48,
                                      height: 48,
                                      child: AnimatedSwitcher(
                                        duration: const Duration(milliseconds: 180),
                                        child: loading
                                            ? const Padding(
                                                key: ValueKey('loading'),
                                                padding: EdgeInsets.all(13),
                                                child: CircularProgressIndicator(strokeWidth: 2.4),
                                              )
                                            : IconButton(
                                                key: ValueKey(failed ? 'retry' : playing),
                                                iconSize: 40,
                                                padding: EdgeInsets.zero,
                                                onPressed: () => audioBloc.transitionState(
                                                  playing ? TransitionState.pause : TransitionState.play,
                                                ),
                                                icon: Icon(
                                                  failed
                                                      ? PhosphorIconsRegular.arrowClockwise
                                                      : playing
                                                          ? PhosphorIconsFill.pause
                                                          : PhosphorIconsFill.play,
                                                  semanticLabel: playing
                                                      ? L.of(context)!.pause_button_label
                                                      : L.of(context)!.play_button_label,
                                                ),
                                              ),
                                      ),
                                    ),
                                  ),
                                ],
                              );
                            }),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          );
        });
  }
}

class _ProgressLine extends StatelessWidget {
  final AudioBloc audioBloc;

  const _ProgressLine({required this.audioBloc});

  @override
  Widget build(BuildContext context) {
    return StreamBuilder<PositionState>(
        stream: audioBloc.playPosition,
        initialData: audioBloc.playPosition?.valueOrNull,
        builder: (context, snapshot) {
          final data = snapshot.data;
          final length = data?.length.inMilliseconds ?? 0;
          final fraction = length > 0 ? (data!.position.inMilliseconds / length).clamp(0.0, 1.0) : 0.0;
          return Align(
            alignment: Alignment.centerLeft,
            child: FractionallySizedBox(
              widthFactor: fraction,
              child: Container(height: 2.0, color: Theme.of(context).primaryColor),
            ),
          );
        });
  }
}
