// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/sleep.dart';
import 'package:rxdart/rxdart.dart';

enum AudioState {
  none,
  buffering,
  starting,
  playing,
  pausing,
  stopped,
  error,
}

class PositionState {
  Duration position;
  late Duration length;
  late int percentage;
  Episode? episode;
  final bool buffering;

  /// How far ahead of the start the player has audio loaded.
  final Duration buffered;

  PositionState({
    required this.position,
    required this.length,
    required this.percentage,
    this.episode,
    this.buffering = false,
    this.buffered = Duration.zero,
  });

  PositionState.emptyState()
      : position = const Duration(seconds: 0),
        length = const Duration(seconds: 0),
        percentage = 0,
        buffering = false,
        buffered = Duration.zero;
}

/// This class defines the audio playback options supported by Pinepods.
///
/// The implementing classes will then handle the specifics for the platform we are running on.
abstract class AudioPlayerService {
  /// Play a new episode, resuming at [Episode.position] when [resume] is set.
  /// [playbackSpeed] is a session override (e.g. per podcast) that isn't saved
  /// as the user's default.
  Future<void> playEpisode({required Episode episode, bool resume = true, double? playbackSpeed});

  /// Apply details that arrive after playback started (chapters, people,
  /// transcript links) to the playing episode, if it is still [guid].
  void updateNowPlayingDetails(String guid, void Function(Episode episode) update);

  /// Look up a locally-downloaded copy of a PinePods episode by its server
  /// episode id. Returns the stored [Episode] (carrying filepath/filename) if a
  /// completed download exists, otherwise null. Used to prefer the on-disk file
  /// over streaming when playing.
  Future<Episode?> findDownloadedEpisode(int episodeId);

  /// Resume playing of current episode
  Future<void> play();

  /// Stop playing of current episode. Set update to false to stop
  /// playback without saving any episode or positional updates.
  Future<void> stop();

  /// Pause the current episode.
  Future<void> pause();

  /// Rewind the current episode by pre-set number of seconds.
  Future<void> rewind();

  /// Fast forward the current episode by pre-set number of seconds.
  Future<void> fastForward();

  /// Seek to the specified position within the current episode.
  Future<void> seek({required int position});

  /// Call when the app is resumed to re-establish the audio service.
  Future<Episode?> resume();

  /// Call when the app is about to be suspended.
  Future<void> suspend();

  /// Use persist: false for podcast overrides that must not change the user's default.
  Future<void> setPlaybackSpeed(double speed, {bool persist = true});

  /// Call to toggle trim silence.
  Future<void> trimSilence(bool trim);

  /// Apply the server-detected skip ranges for the *current* episode without
  /// changing the global user preference (#727 silence per-podcast override,
  /// #790 ad-skip). Ranges are lists of {'start': seconds, 'end': seconds}.
  ///
  /// - [silenceEnabled] / [silenceRanges]: on Android enables the native
  ///   ExoPlayer skip-silence DSP; on iOS the silence ranges are seeked past.
  /// - [adRanges]: ad segments the server resolved as skippable for this user.
  ///   On both platforms these are range-seeked (Android uses a position poll,
  ///   iOS its kind-agnostic seek observer).
  Future<void> applyEpisodeSkipSegments({
    required bool silenceEnabled,
    required List<Map<String, double>> silenceRanges,
    required List<Map<String, double>> adRanges,
  });

  /// Call to toggle trim silence.
  Future<void> volumeBoost(bool boost);

  void sleep(Sleep sleep);

  void setPlaylistContext(int? playlistId);

  Episode? nowPlaying;

  /// Event listeners
  /// Only emits when the state actually changes.
  ValueStream<AudioState>? playingState;

  /// Speed the current episode plays at, including a per-podcast override.
  ValueStream<double>? playbackSpeed;
  ValueStream<PositionState>? playPosition;
  ValueStream<Episode?>? episodeEvent;
  Stream<int>? playbackError;
  Stream<Sleep>? sleepStream;
}
