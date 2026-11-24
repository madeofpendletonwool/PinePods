// lib/services/offline/offline_action_queue.dart

import 'dart:async';

import 'package:connectivity_plus/connectivity_plus.dart';
import 'package:logging/logging.dart';
import 'package:pinepods_mobile/entities/pending_action.dart';
import 'package:pinepods_mobile/repository/repository.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/pinepods/progress_tracker.dart';
import 'package:pinepods_mobile/services/settings/settings_service.dart';

/// Durable "outbox" for episode interactions (progress, completion, save,
/// queue, history). Every interaction is written here first and removed only
/// once the server has it; it is sent on app start, on reconnect, and after
/// each enqueue.
///
/// Listening progress is kept as one entry per episode: the latest position
/// with the time it was observed, the seconds played since the last report,
/// and whether the episode finished. All of it goes out in one request, and the
/// server lets the newest observation of an episode win, so progress made
/// offline can be sent hours later without overwriting what another device
/// has done since.
class OfflineActionQueue {
  final log = Logger('OfflineActionQueue');

  final Repository repository;
  final PinepodsService pinepodsService;
  final SettingsService settingsService;

  /// Stop draining after this many consecutive failures for a single item so we
  /// don't spin; the item is retried on the next flush trigger.
  static const int _maxRetries = 8;

  bool _flushing = false;

  /// Something was queued while a flush was running; run another after it.
  bool _flushAgain = false;

  /// The server answered 404 to a progress report: it predates the endpoint,
  /// so progress goes out through the calls it does have.
  bool _oldServer = false;

  /// Played seconds not yet a whole minute, for a server that counts minutes.
  double _oldServerSeconds = 0;

  /// Serialises changes to progress entries so two reports for one episode
  /// can't both read the entry before either writes it.
  Future<void> _progressLock = Future.value();

  StreamSubscription<List<ConnectivityResult>>? _connectivitySub;

  OfflineActionQueue({
    required this.repository,
    required this.pinepodsService,
    required this.settingsService,
  });

  /// Begin listening for connectivity changes and attempt an initial flush.
  void start() {
    _connectivitySub ??= Connectivity().onConnectivityChanged.listen((results) {
      final online = results.any((r) => r != ConnectivityResult.none);
      if (online) {
        log.fine('Connectivity restored - flushing offline action queue');
        flush();
      }
    });

    // Drain anything left over from a previous session.
    flush();
  }

  void dispose() {
    _connectivitySub?.cancel();
    _connectivitySub = null;
  }

  /// Persist an interaction and immediately attempt to flush it.
  Future<void> enqueue(PendingAction action) async {
    try {
      await repository.savePendingAction(action);
    } catch (e) {
      log.warning('Failed to enqueue offline action: $e');
      return;
    }

    // Opportunistic flush — succeeds immediately when online, harmless offline.
    unawaited(flush());
  }

  Future<T> _withProgressLock<T>(Future<T> Function() body) {
    final result = _progressLock.then((_) => body());
    _progressLock = result.then<void>((_) {}, onError: (_) {});
    return result;
  }

  Future<PendingAction?> _progressEntry(int episodeId, bool isYoutube) async {
    final actions = await repository.getPendingActions();
    for (final a in actions) {
      if (a.type == PendingActionType.recordPosition && a.episodeId == episodeId && a.isYoutube == isYoutube) {
        return a;
      }
    }
    return null;
  }

  /// Record listening progress. Folded into the episode's pending entry: the
  /// newer position is kept and the played time added up.
  Future<void> enqueueProgress(ProgressUpdate update) async {
    try {
      await _withProgressLock(() async {
        final existing = await _progressEntry(update.episodeId, update.isYoutube);
        final newer = existing == null || !update.at.isBefore(existing.observedAt);
        final duration = (newer ? update.duration : null) ?? existing?.duration;
        await repository.savePendingAction(PendingAction(
          id: existing?.id,
          type: PendingActionType.recordPosition,
          episodeId: update.episodeId,
          userId: update.userId,
          isYoutube: update.isYoutube,
          createdAt: existing?.createdAt,
          payload: {
            'position': newer ? update.position : existing.position ?? update.position,
            'at': (newer ? update.at : existing.observedAt).millisecondsSinceEpoch,
            'listened': (existing?.listened ?? 0) + update.listened,
            'speed_saved': (existing?.speedSaved ?? 0) + update.speedSaved,
            'silence_skipped': (existing?.silenceSkipped ?? 0) + update.silenceSkipped,
            'ads_skipped': (existing?.adsSkipped ?? 0) + update.adsSkipped,
            'downloaded': newer ? update.downloaded : existing.downloaded,
            'finished': newer ? update.finished : existing.finished,
            if (duration != null) 'duration': duration,
          },
        ));
      });
    } catch (e) {
      log.warning('Failed to queue progress: $e');
      return;
    }
    unawaited(flush());
  }

  /// A bare position observed now, for callers with nothing else to say.
  Future<void> enqueuePosition(int episodeId, int userId, double positionSeconds, bool isYoutube) =>
      enqueueProgress(ProgressUpdate(
        episodeId: episodeId,
        userId: userId,
        isYoutube: isYoutube,
        position: positionSeconds,
        at: DateTime.now(),
      ));

  Future<void> enqueueHistory(int episodeId, int userId, double positionSeconds, bool isYoutube) =>
      enqueue(PendingAction(
        type: PendingActionType.addHistory,
        episodeId: episodeId,
        userId: userId,
        isYoutube: isYoutube,
        payload: {'position': positionSeconds},
      ));

  Future<void> enqueueSimple(PendingActionType type, int episodeId, int userId, bool isYoutube) =>
      enqueue(PendingAction(type: type, episodeId: episodeId, userId: userId, isYoutube: isYoutube));

  /// Progress for an episode that has not reached the server yet, or null.
  /// Resume uses it: what was heard on this device and not yet sent is newer
  /// than anything the server can say.
  Future<PendingAction?> pendingProgress(int episodeId, bool isYoutube) async {
    try {
      return await _progressEntry(episodeId, isYoutube);
    } catch (e) {
      log.warning('Failed to read pending progress: $e');
      return null;
    }
  }

  /// The position (seconds) of [pendingProgress], or null when nothing is pending.
  Future<double?> pendingPosition(int episodeId, bool isYoutube) async =>
      (await pendingProgress(episodeId, isYoutube))?.position;

  /// Send everything pending. Stops at the first failure (likely offline) so
  /// the rest is retried later. A call during a flush runs another after it,
  /// so what was queued meanwhile doesn't wait for the next trigger.
  Future<void> flush() async {
    if (_flushing) {
      _flushAgain = true;
      return;
    }

    _flushing = true;
    try {
      do {
        _flushAgain = false;
        final server = settingsService.pinepodsServer;
        final apiKey = settingsService.pinepodsApiKey;
        // Not logged in / no credentials — nothing we can send yet.
        if (server == null || apiKey == null) return;
        pinepodsService.setCredentials(server, apiKey);
        if (!await _flushProgress()) return;
        if (!await _flushActions()) return;
      } while (_flushAgain);
    } finally {
      _flushing = false;
    }
  }

  /// Send every progress entry in one request. False when it didn't get through.
  Future<bool> _flushProgress() async {
    final sent = (await repository.getPendingActions())
        .where((a) => a.type == PendingActionType.recordPosition && a.id != null)
        .toList();
    if (sent.isEmpty) return true;

    try {
      if (!_oldServer) {
        try {
          await pinepodsService.sendProgress([
            for (final a in sent)
              {
                'episode_id': a.episodeId,
                'is_youtube': a.isYoutube,
                'position': a.position ?? 0,
                'at': a.observedAt.millisecondsSinceEpoch,
                'listened': a.listened,
                'finished': a.finished,
                if (a.speedSaved != 0) 'speed_saved': a.speedSaved,
                if (a.silenceSkipped != 0) 'silence_skipped': a.silenceSkipped,
                if (a.adsSkipped != 0) 'ads_skipped': a.adsSkipped,
                if (a.downloaded) 'downloaded': true,
                if (a.duration != null) 'duration': a.duration,
              },
          ]);
        } on ProgressUnsupportedException {
          log.info('Server has no progress endpoint - using the older calls');
          _oldServer = true;
        }
      }
      if (_oldServer) {
        for (final a in sent) {
          await _sendToOldServer(a);
        }
      }
    } catch (e) {
      log.fine('Progress not sent (will retry): $e');
      return false;
    }

    // Entries unchanged since they were read are done. One that moved on while
    // the request was in flight stays, less the played time already delivered.
    await _withProgressLock(() async {
      final now = {for (final a in await repository.getPendingActions()) a.id: a};
      for (final s in sent) {
        final current = now[s.id];
        if (current == null) continue;
        if (!current.observedAt.isAfter(s.observedAt)) {
          await repository.deletePendingAction(current.id!);
        } else {
          final listened = current.listened - s.listened;
          final silence = current.silenceSkipped - s.silenceSkipped;
          final ads = current.adsSkipped - s.adsSkipped;
          await repository.savePendingAction(PendingAction(
            id: current.id,
            type: current.type,
            episodeId: current.episodeId,
            userId: current.userId,
            isYoutube: current.isYoutube,
            createdAt: current.createdAt,
            payload: {
              ...current.payload,
              'listened': listened < 0 ? 0.0 : listened,
              'speed_saved': current.speedSaved - s.speedSaved,
              'silence_skipped': silence < 0 ? 0.0 : silence,
              'ads_skipped': ads < 0 ? 0.0 : ads,
            },
          ));
        }
      }
    });
    return true;
  }

  /// A server from before the progress endpoint: a position call, a completion
  /// call for a finished episode, and a call per minute played.
  Future<void> _sendToOldServer(PendingAction a) async {
    _require(await pinepodsService.recordListenDuration(a.episodeId, a.userId, a.position ?? 0, a.isYoutube));
    if (a.finished) {
      _require(await pinepodsService.markEpisodeCompleted(a.episodeId, a.userId, a.isYoutube));
    }
    _oldServerSeconds += a.listened;
    while (_oldServerSeconds >= 60) {
      _oldServerSeconds -= 60;
      await pinepodsService.incrementListenTime(a.userId);
    }
  }

  /// Send the other actions in the order they happened. False when one failed.
  Future<bool> _flushActions() async {
    final actions = (await repository.getPendingActions()).where((a) => a.type != PendingActionType.recordPosition);
    for (final action in actions) {
      try {
        await _dispatch(action);
        if (action.id != null) {
          await repository.deletePendingAction(action.id!);
        }
      } catch (e) {
        log.warning('Failed to sync pending action (${action.type.name}): $e');
        action.retryCount++;
        if (action.id != null && action.retryCount < _maxRetries) {
          await repository.savePendingAction(action);
        } else if (action.id != null) {
          // Give up on a persistently-failing item rather than blocking the
          // rest of the queue forever.
          log.warning('Dropping pending action after $_maxRetries attempts: ${action.type.name}');
          await repository.deletePendingAction(action.id!);
        }
        // Stop draining; likely offline. Remaining items retry next trigger.
        return false;
      }
    }
    return true;
  }

  /// The service calls report failure by returning false; here that has to
  /// stop the action from being taken as delivered.
  void _require(bool ok) {
    if (!ok) throw Exception('The server did not accept the request');
  }

  Future<void> _dispatch(PendingAction a) async {
    switch (a.type) {
      case PendingActionType.recordPosition:
        // Sent together by _flushProgress.
        break;
      case PendingActionType.addHistory:
        _require(await pinepodsService.addHistory(a.episodeId, a.position ?? 0, a.userId, a.isYoutube));
        break;
      case PendingActionType.markCompleted:
        _require(await pinepodsService.markEpisodeCompleted(a.episodeId, a.userId, a.isYoutube));
        break;
      case PendingActionType.markUncompleted:
        _require(await pinepodsService.markEpisodeUncompleted(a.episodeId, a.userId, a.isYoutube));
        break;
      case PendingActionType.saveEpisode:
        _require(await pinepodsService.saveEpisode(a.episodeId, a.userId, a.isYoutube));
        break;
      case PendingActionType.removeSaved:
        _require(await pinepodsService.removeSavedEpisode(a.episodeId, a.userId, a.isYoutube));
        break;
      case PendingActionType.queue:
        _require(await pinepodsService.queueEpisode(a.episodeId, a.userId, a.isYoutube));
        break;
    }
  }
}
