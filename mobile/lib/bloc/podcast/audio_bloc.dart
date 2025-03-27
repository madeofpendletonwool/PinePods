// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'dart:async';

import 'package:pinepods_mobile/bloc/bloc.dart';
import 'package:pinepods_mobile/core/extensions.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/sleep.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/state/transcript_state_event.dart';
import 'package:logging/logging.dart';
import 'package:rxdart/rxdart.dart';

enum TransitionState {
  play,
  pause,
  stop,
  fastforward,
  rewind,
}

enum LifecycleState {
  pause,
  resume,
  detach,
}

/// A BLoC to handle interactions between the audio service and the client.
class AudioBloc extends Bloc {
  final log = Logger('AudioBloc');

  /// Listen for new episode play requests.
  final BehaviorSubject<Episode?> _play = BehaviorSubject<Episode?>();

  /// Move from one playing state to another such as from paused to play
  final PublishSubject<TransitionState> _transitionPlayingState = PublishSubject<TransitionState>();

  /// Sink to update our position
  final PublishSubject<double> _transitionPosition = PublishSubject<double>();

  /// Handles persisting data to storage.
  final AudioPlayerService audioPlayerService;

  /// Listens for playback speed change requests.
  final PublishSubject<double> _playbackSpeedSubject = PublishSubject<double>();

  /// Listen for toggling of trim silence requests.
  final PublishSubject<bool> _trimSilence = PublishSubject<bool>();

  /// Listen for toggling of volume boost silence requests.
  final PublishSubject<bool> _volumeBoost = PublishSubject<bool>();

  /// Listen for transcript filtering events.
  final PublishSubject<TranscriptEvent> _transcriptEvent = PublishSubject<TranscriptEvent>();

  final BehaviorSubject<Sleep> _sleepEvent = BehaviorSubject<Sleep>();

  AudioBloc({
    required this.audioPlayerService,
  }) {
    /// Listen for transition events from the client.
    _handlePlayingStateTransitions();

    /// Listen for events requesting the start of a new episode.
    _handleEpisodeRequests();

    /// Listen for requests to move the play position within the episode.
    _handlePositionTransitions();

    /// Listen for playback speed changes
    _handlePlaybackSpeedTransitions();

    /// Listen to trim silence requests
    _handleTrimSilenceTransitions();

    /// Listen to volume boost silence requests
    _handleVolumeBoostTransitions();

    /// Listen to transcript filtering events
    _handleTranscriptEvents();

    /// Listen to sleep timer events;
    _handleSleepTimer();
  }

  /// Listens to events from the UI (or any client) to transition from one
  /// audio state to another. For example, to pause the current playback
  /// a [TransitionState.pause] event should be sent. To ensure the underlying
  /// audio service processes one state request at a time we push events
  /// on to a queue and execute them sequentially. Each state maps to a call
  /// to the Audio Service plugin.
  void _handlePlayingStateTransitions() {
    _transitionPlayingState.asyncMap((event) => Future.value(event)).listen((state) async {
      switch (state) {
        case TransitionState.play:
          await audioPlayerService.play();
          break;
        case TransitionState.pause:
          await audioPlayerService.pause();
          break;
        case TransitionState.fastforward:
          await audioPlayerService.fastForward();
          break;
        case TransitionState.rewind:
          await audioPlayerService.rewind();
          break;
        case TransitionState.stop:
          await audioPlayerService.stop();
          break;
      }
    });
  }

  /// Setup a listener for episode requests and then connect to the
  /// underlying audio service.
  void _handleEpisodeRequests() async {
    _play.listen((episode) {
      audioPlayerService.playEpisode(episode: episode!, resume: true);
    });
  }

  /// Listen for requests to change the position of the current episode.
  void _handlePositionTransitions() async {
    _transitionPosition.listen((pos) async {
      await audioPlayerService.seek(position: pos.ceil());
    });
  }

  /// Listen for requests to adjust the playback speed.
  void _handlePlaybackSpeedTransitions() {
    _playbackSpeedSubject.listen((double speed) async {
      await audioPlayerService.setPlaybackSpeed(speed.toTenth);
    });
  }

  /// Listen for requests to toggle trim silence mode. This is currently disabled until
  /// [issue](https://github.com/ryanheise/just_audio/issues/558) is resolved.
  void _handleTrimSilenceTransitions() {
    _trimSilence.listen((bool trim) async {
      await audioPlayerService.trimSilence(trim);
    });
  }

  /// Listen for requests to toggle the volume boost feature. Android only.
  void _handleVolumeBoostTransitions() {
    _volumeBoost.listen((bool boost) async {
      await audioPlayerService.volumeBoost(boost);
    });
  }

  void _handleTranscriptEvents() {
    _transcriptEvent.listen((TranscriptEvent event) {
      if (event is TranscriptFilterEvent) {
        audioPlayerService.searchTranscript(event.search);
      } else if (event is TranscriptClearEvent) {
        audioPlayerService.clearTranscript();
      }
    });
  }

  void _handleSleepTimer() {
    _sleepEvent.listen((Sleep sleep) {
      audioPlayerService.sleep(sleep);
    });
  }

  @override
  void pause() async {
    log.fine('Audio lifecycle pause');
    await audioPlayerService.suspend();
  }

  @override
  void resume() async {
    log.fine('Audio lifecycle resume');
    var ep = await audioPlayerService.resume();

    if (ep != null) {
      log.fine('Resuming with episode ${ep.title} - ${ep.position} - ${ep.played}');
    } else {
      log.fine('Resuming without an episode');
    }
  }

  /// Play the specified track now
  void Function(Episode?) get play => _play.add;

  /// Transition the state from connecting, to play, pause, stop etc.
  void Function(TransitionState) get transitionState => _transitionPlayingState.add;

  /// Move the play position.
  void Function(double) get transitionPosition => _transitionPosition.sink.add;

  /// Get the current playing state
  Stream<AudioState>? get playingState => audioPlayerService.playingState;

  /// Listen for any playback errors
  Stream<int>? get playbackError => audioPlayerService.playbackError;

  /// Get the current playing episode
  ValueStream<Episode?>? get nowPlaying => audioPlayerService.episodeEvent;

  /// Get the current transcript (if there is one).
  Stream<TranscriptState>? get nowPlayingTranscript => audioPlayerService.transcriptEvent;

  /// Get position and percentage played of playing episode
  ValueStream<PositionState>? get playPosition => audioPlayerService.playPosition;

  Stream<Sleep>? get sleepStream => audioPlayerService.sleepStream;

  /// Change playback speed
  void Function(double) get playbackSpeed => _playbackSpeedSubject.sink.add;

  /// Toggle trim silence
  void Function(bool) get trimSilence => _trimSilence.sink.add;

  /// Toggle volume boost silence
  void Function(bool) get volumeBoost => _volumeBoost.sink.add;

  /// Handle filtering & searching of the current transcript.
  void Function(TranscriptEvent) get filterTranscript => _transcriptEvent.sink.add;

  void Function(Sleep) get sleep => _sleepEvent.sink.add;

  @override
  void dispose() {
    _play.close();
    _transitionPlayingState.close();
    _transitionPosition.close();
    _playbackSpeedSubject.close();
    _trimSilence.close();
    _volumeBoost.close();

    super.dispose();
  }
}
