// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/bloc/podcast/audio_bloc.dart';
import 'package:pinepods_mobile/bloc/podcast/episode_bloc.dart';
import 'package:pinepods_mobile/bloc/podcast/podcast_bloc.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/app_settings.dart';
import 'package:pinepods_mobile/entities/downloadable.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/ui/podcast/now_playing.dart';
import 'package:pinepods_mobile/ui/widgets/action_text.dart';
import 'package:pinepods_mobile/ui/widgets/download_button.dart';
import 'package:pinepods_mobile/ui/widgets/play_pause_button.dart';
import 'package:flutter/material.dart';
import 'package:flutter_dialogs/flutter_dialogs.dart';
import 'package:provider/provider.dart';
import 'package:rxdart/rxdart.dart';

/// Handles the state of the episode transport controls.
///
/// This currently consists of the [PlayControl] and [DownloadControl]
/// to handle the play/pause and download control state respectively.
class PlayControl extends StatelessWidget {
  final Episode episode;

  const PlayControl({
    super.key,
    required this.episode,
  });

  @override
  Widget build(BuildContext context) {
    final audioBloc = Provider.of<AudioBloc>(context, listen: false);
    final settings = Provider.of<SettingsBloc>(context, listen: false).currentSettings;

    return SizedBox(
      height: 48.0,
      width: 48.0,
      child: StreamBuilder<_PlayerControlState>(
          stream: Rx.combineLatest2(audioBloc.playingState!, audioBloc.nowPlaying!,
              (AudioState audioState, Episode? episode) => _PlayerControlState(audioState, episode)),
          builder: (context, snapshot) {
            if (snapshot.hasData) {
              final audioState = snapshot.data!.audioState;
              final nowPlaying = snapshot.data!.episode;

              if (episode.downloadState != DownloadState.downloading && episode.downloadState != DownloadState.queued) {
                // If this episode is the one we are playing, allow the user
                // to toggle between play and pause.
                if (snapshot.hasData && nowPlaying?.guid == episode.guid) {
                  if (audioState == AudioState.playing) {
                    return InkWell(
                      onTap: () {
                        audioBloc.transitionState(TransitionState.pause);
                      },
                      child: PlayPauseButton(
                        title: episode.title!,
                        label: L.of(context)!.pause_button_label,
                        icon: Icons.pause,
                      ),
                    );
                  } else if (audioState == AudioState.buffering) {
                    return PlayPauseBusyButton(
                      title: episode.title!,
                      label: L.of(context)!.pause_button_label,
                      icon: Icons.pause,
                    );
                  } else if (audioState == AudioState.pausing) {
                    return InkWell(
                      onTap: () {
                        audioBloc.transitionState(TransitionState.play);
                        optionalShowNowPlaying(context, settings);
                      },
                      child: PlayPauseButton(
                        title: episode.title!,
                        label: L.of(context)!.play_button_label,
                        icon: Icons.play_arrow,
                      ),
                    );
                  }
                }

                // If this episode is not the one we are playing, allow the
                // user to start playing this episode.
                return InkWell(
                  onTap: () {
                    audioBloc.play(episode);
                    optionalShowNowPlaying(context, settings);
                  },
                  child: PlayPauseButton(
                    title: episode.title!,
                    label: L.of(context)!.play_button_label,
                    icon: Icons.play_arrow,
                  ),
                );
              } else {
                // We are currently downloading this episode. Do not allow
                // the user to play it until the download is complete.
                return Opacity(
                  opacity: 0.2,
                  child: PlayPauseButton(
                    title: episode.title!,
                    label: L.of(context)!.play_button_label,
                    icon: Icons.play_arrow,
                  ),
                );
              }
            } else {
              // We have no playing information at the moment. Show a play button
              // until the stream wakes up.
              if (episode.downloadState != DownloadState.downloading) {
                return InkWell(
                  onTap: () {
                    audioBloc.play(episode);
                    optionalShowNowPlaying(context, settings);
                  },
                  child: PlayPauseButton(
                    title: episode.title!,
                    label: L.of(context)!.play_button_label,
                    icon: Icons.play_arrow,
                  ),
                );
              } else {
                return Opacity(
                  opacity: 0.2,
                  child: PlayPauseButton(
                    title: episode.title!,
                    label: L.of(context)!.play_button_label,
                    icon: Icons.play_arrow,
                  ),
                );
              }
            }
          }),
    );
  }

  /// If we have the 'show now playing upon play' option set to true, launch
  /// the [NowPlaying] widget automatically.
  void optionalShowNowPlaying(BuildContext context, AppSettings settings) {
    if (settings.autoOpenNowPlaying) {
      Navigator.push(
        context,
        MaterialPageRoute<void>(
          builder: (context) => const NowPlaying(),
          settings: const RouteSettings(name: 'nowplaying'),
          fullscreenDialog: false,
        ),
      );
    }
  }
}

class DownloadControl extends StatelessWidget {
  final Episode episode;

  const DownloadControl({
    super.key,
    required this.episode,
  });

  @override
  Widget build(BuildContext context) {
    final audioBloc = Provider.of<AudioBloc>(context);
    final podcastBloc = Provider.of<PodcastBloc>(context);

    return SizedBox(
      height: 48.0,
      width: 48.0,
      child: StreamBuilder<_PlayerControlState>(
          stream: Rx.combineLatest2(audioBloc.playingState!, audioBloc.nowPlaying!,
              (AudioState audioState, Episode? episode) => _PlayerControlState(audioState, episode)),
          builder: (context, snapshot) {
            if (snapshot.hasData) {
              final audioState = snapshot.data!.audioState;
              final nowPlaying = snapshot.data!.episode;

              if (nowPlaying?.guid == episode.guid &&
                  (audioState == AudioState.playing || audioState == AudioState.buffering)) {
                if (episode.downloadState != DownloadState.downloaded) {
                  return Opacity(
                    opacity: 0.2,
                    child: DownloadButton(
                      onPressed: () {},
                      title: episode.title!,
                      icon: Icons.save_alt,
                      percent: 0,
                      label: L.of(context)!.download_episode_button_label,
                    ),
                  );
                } else {
                  return Opacity(
                    opacity: 0.2,
                    child: DownloadButton(
                      onPressed: () {},
                      title: episode.title!,
                      icon: Icons.check,
                      percent: 0,
                      label: L.of(context)!.download_episode_button_label,
                    ),
                  );
                }
              }
            }

            if (episode.downloadState == DownloadState.downloaded) {
              return DownloadButton(
                onPressed: () {},
                title: episode.title!,
                icon: Icons.check,
                percent: 0,
                label: L.of(context)!.download_episode_button_label,
              );
            } else if (episode.downloadState == DownloadState.queued) {
              return DownloadButton(
                onPressed: () => _showCancelDialog(context),
                title: episode.title!,
                icon: Icons.timer_outlined,
                percent: 0,
                label: L.of(context)!.download_episode_button_label,
              );
            } else if (episode.downloadState == DownloadState.downloading) {
              return DownloadButton(
                onPressed: () => _showCancelDialog(context),
                title: episode.title!,
                icon: Icons.timer_outlined,
                percent: episode.downloadPercentage!,
                label: L.of(context)!.download_episode_button_label,
              );
            }

            return DownloadButton(
              onPressed: () => podcastBloc.downloadEpisode(episode),
              title: episode.title!,
              icon: Icons.save_alt,
              percent: 0,
              label: L.of(context)!.download_episode_button_label,
            );
          }),
    );
  }

  Future<void> _showCancelDialog(BuildContext context) {
    final episodeBloc = Provider.of<EpisodeBloc>(context, listen: false);

    return showPlatformDialog<void>(
      context: context,
      useRootNavigator: false,
      builder: (_) => BasicDialogAlert(
        title: Text(
          L.of(context)!.stop_download_title,
        ),
        content: Text(L.of(context)!.stop_download_confirmation),
        actions: <Widget>[
          BasicDialogAction(
            title: ActionText(
              L.of(context)!.continue_button_label,
            ),
            onPressed: () {
              Navigator.pop(context);
            },
          ),
          BasicDialogAction(
            title: ActionText(
              L.of(context)!.stop_download_button_label,
            ),
            iosIsDefaultAction: true,
            onPressed: () {
              episodeBloc.deleteDownload(episode);
              Navigator.pop(context);
            },
          ),
        ],
      ),
    );
  }
}

/// This class acts as a wrapper between the current audio state and
/// downloadables. Saves all that nesting of StreamBuilders.
class _PlayerControlState {
  final AudioState audioState;
  final Episode? episode;

  _PlayerControlState(this.audioState, this.episode);
}
