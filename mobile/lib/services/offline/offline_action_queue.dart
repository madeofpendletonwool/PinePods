// lib/services/offline/offline_action_queue.dart

import 'dart:async';

import 'package:connectivity_plus/connectivity_plus.dart';
import 'package:logging/logging.dart';
import 'package:pinepods_mobile/entities/pending_action.dart';
import 'package:pinepods_mobile/repository/repository.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/settings/settings_service.dart';

/// Durable "outbox" for episode interactions (progress, completion, save,
/// queue, history). Every interaction for a locally-playable episode is written
/// here and flushed to the PinePods server when the device is online — on
/// app start, on reconnect, and opportunistically after each enqueue.
///
/// This is what lets users listen to downloaded episodes offline and have their
/// progress / completion / saved state reconcile with the server once back
/// online, rather than being silently dropped.
class OfflineActionQueue {
  final log = Logger('OfflineActionQueue');

  final Repository repository;
  final PinepodsService pinepodsService;
  final SettingsService settingsService;

  /// Stop draining after this many consecutive failures for a single item so we
  /// don't spin; the item is retried on the next flush trigger.
  static const int _maxRetries = 8;

  bool _flushing = false;
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
  ///
  /// For [PendingActionType.recordPosition] only the latest position per episode
  /// matters, so any earlier pending position update for the same episode is
  /// collapsed to avoid the queue growing without bound during playback.
  Future<void> enqueue(PendingAction action) async {
    try {
      if (action.type == PendingActionType.recordPosition) {
        final existing = await repository.getPendingActions();
        for (final a in existing) {
          if (a.type == PendingActionType.recordPosition &&
              a.episodeId == action.episodeId &&
              a.id != null) {
            await repository.deletePendingAction(a.id!);
          }
        }
      }

      await repository.savePendingAction(action);
    } catch (e) {
      log.warning('Failed to enqueue offline action: $e');
      return;
    }

    // Opportunistic flush — succeeds immediately when online, harmless offline.
    flush();
  }

  /// Convenience helpers for the common interactions.
  Future<void> enqueuePosition(int episodeId, int userId, double positionSeconds, bool isYoutube) =>
      enqueue(PendingAction(
        type: PendingActionType.recordPosition,
        episodeId: episodeId,
        userId: userId,
        isYoutube: isYoutube,
        payload: {'position': positionSeconds},
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

  /// Attempt to send all pending actions to the server in order. Stops at the
  /// first failure (likely offline) so the remaining items are retried later.
  Future<void> flush() async {
    if (_flushing) return;

    final server = settingsService.pinepodsServer;
    final apiKey = settingsService.pinepodsApiKey;
    if (server == null || apiKey == null) {
      // Not logged in / no credentials — nothing we can send yet.
      return;
    }

    _flushing = true;
    try {
      pinepodsService.setCredentials(server, apiKey);

      final actions = await repository.getPendingActions();
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
          break;
        }
      }
    } finally {
      _flushing = false;
    }
  }

  Future<void> _dispatch(PendingAction a) async {
    switch (a.type) {
      case PendingActionType.recordPosition:
        await pinepodsService.recordListenDuration(a.episodeId, a.userId, a.position ?? 0, a.isYoutube);
        break;
      case PendingActionType.addHistory:
        await pinepodsService.addHistory(a.episodeId, a.position ?? 0, a.userId, a.isYoutube);
        break;
      case PendingActionType.markCompleted:
        await pinepodsService.markEpisodeCompleted(a.episodeId, a.userId, a.isYoutube);
        break;
      case PendingActionType.markUncompleted:
        await pinepodsService.markEpisodeUncompleted(a.episodeId, a.userId, a.isYoutube);
        break;
      case PendingActionType.saveEpisode:
        await pinepodsService.saveEpisode(a.episodeId, a.userId, a.isYoutube);
        break;
      case PendingActionType.removeSaved:
        await pinepodsService.removeSavedEpisode(a.episodeId, a.userId, a.isYoutube);
        break;
      case PendingActionType.queue:
        await pinepodsService.queueEpisode(a.episodeId, a.userId, a.isYoutube);
        break;
    }
  }
}
