// Copyright 2020 Ben Hills. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

enum LastState { none, completed, stopped, paused }

/// This class is used to persist information about the currently playing episode to disk.
///
/// This allows the background audio service to persist state (whilst the UI is not visible)
/// and for the episode play and position details to be restored when the UI becomes visible
/// again - either when bringing it to the foreground or upon next start.
class Persistable {
  /// The Podcast GUID.
  String pguid;

  /// The episode ID (provided by the DB layer).
  int episodeId;

  /// The current position in seconds;
  int position;

  /// The current playback state.
  LastState state;

  /// Date & time episode was last updated.
  DateTime? lastUpdated;

  Persistable({
    required this.pguid,
    required this.episodeId,
    required this.position,
    required this.state,
    this.lastUpdated,
  });

  Persistable.empty()
      : pguid = '',
        episodeId = 0,
        position = 0,
        state = LastState.none,
        lastUpdated = DateTime.now();

  Map<String, dynamic> toMap() {
    return <String, dynamic>{
      'pguid': pguid,
      'episodeId': episodeId,
      'position': position,
      'state': state.toString(),
      'lastUpdated': lastUpdated == null ? DateTime.now().millisecondsSinceEpoch : lastUpdated!.millisecondsSinceEpoch,
    };
  }

  static Persistable fromMap(Map<String, dynamic> persistable) {
    var stateString = persistable['state'] as String?;
    var state = LastState.none;

    if (stateString != null) {
      switch (stateString) {
        case 'LastState.completed':
          state = LastState.completed;
          break;
        case 'LastState.stopped':
          state = LastState.stopped;
          break;
        case 'LastState.paused':
          state = LastState.paused;
          break;
      }
    }

    var lastUpdated = persistable['lastUpdated'] as int?;

    return Persistable(
      pguid: persistable['pguid'] as String,
      episodeId: persistable['episodeId'] as int,
      position: persistable['position'] as int,
      state: state,
      lastUpdated: lastUpdated == null ? DateTime.now() : DateTime.fromMillisecondsSinceEpoch(lastUpdated),
    );
  }
}
