// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/bloc/bloc.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/services/podcast/podcast_service.dart';
import 'package:pinepods_mobile/state/queue_event_state.dart';
import 'package:rxdart/rxdart.dart';

/// Handles interaction with the Queue via an [AudioPlayerService].
class QueueBloc extends Bloc {
  final AudioPlayerService audioPlayerService;
  final PodcastService podcastService;
  final PublishSubject<QueueEvent> _queueEvent = PublishSubject<QueueEvent>();

  QueueBloc({
    required this.audioPlayerService,
    required this.podcastService,
  }) {
    _handleQueueEvents();
  }

  void _handleQueueEvents() {
    _queueEvent.listen((QueueEvent event) async {
      if (event is QueueAddEvent) {
        var e = event.episode;
        if (e != null) {
          await audioPlayerService.addUpNextEpisode(e);
        }
      } else if (event is QueueRemoveEvent) {
        var e = event.episode;
        if (e != null) {
          await audioPlayerService.removeUpNextEpisode(e);
        }
      } else if (event is QueueMoveEvent) {
        var e = event.episode;
        if (e != null) {
          await audioPlayerService.moveUpNextEpisode(e, event.oldIndex, event.newIndex);
        }
      } else if (event is QueueClearEvent) {
        await audioPlayerService.clearUpNext();
      }
    });

    audioPlayerService.queueState!.debounceTime(const Duration(seconds: 2)).listen((event) {
      podcastService.saveQueue(event.queue).then((value) {
        /// Queue saved.
      });
    });
  }

  Function(QueueEvent) get queueEvent => _queueEvent.sink.add;

  Stream<QueueListState>? get queue => audioPlayerService.queueState;

  @override
  void dispose() {
    _queueEvent.close();

    super.dispose();
  }
}
