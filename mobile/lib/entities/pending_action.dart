// lib/entities/pending_action.dart

import 'package:pinepods_mobile/l10n/L.dart';

/// The kind of server interaction a [PendingAction] represents.
///
/// These map 1:1 onto the relevant PinepodsService calls so the offline queue
/// can dispatch each action when connectivity is restored.
enum PendingActionType {
  recordPosition,
  markCompleted,
  markUncompleted,
  saveEpisode,
  removeSaved,
  queue,
  addHistory,
}

/// A user interaction (progress, completion, save, queue, history) that could
/// not be — or has not yet been — sent to the server, persisted locally so it
/// can be synced later. This is what lets episodes downloaded for offline
/// listening still record interactions and reconcile once back online.
class PendingAction {
  /// Database key (Sembast record id). Null until persisted.
  int? id;

  final PendingActionType type;
  final int episodeId;
  final int userId;
  final bool isYoutube;

  /// Action-specific data, e.g. {'position': 123.0} for [recordPosition].
  final Map<String, dynamic> payload;

  final DateTime createdAt;

  /// Number of failed sync attempts. Used for backoff / surfacing stuck items.
  int retryCount;

  PendingAction({
    this.id,
    required this.type,
    required this.episodeId,
    required this.userId,
    this.isYoutube = false,
    this.payload = const {},
    DateTime? createdAt,
    this.retryCount = 0,
  }) : createdAt = createdAt ?? DateTime.now();

  /// Convenience for the common position payload.
  double? get position {
    final p = payload['position'];
    if (p is num) return p.toDouble();
    return null;
  }

  Map<String, dynamic> toMap() {
    return <String, dynamic>{
      'type': type.name,
      'episodeId': episodeId,
      'userId': userId,
      'isYoutube': isYoutube,
      'payload': payload,
      'createdAt': createdAt.millisecondsSinceEpoch,
      'retryCount': retryCount,
    };
  }

  static PendingAction fromMap(int? key, Map<String, dynamic> map) {
    return PendingAction(
      id: key,
      type: PendingActionType.values.firstWhere(
        (t) => t.name == map['type'],
        orElse: () => PendingActionType.recordPosition,
      ),
      episodeId: map['episodeId'] as int? ?? 0,
      userId: map['userId'] as int? ?? 0,
      isYoutube: map['isYoutube'] as bool? ?? false,
      payload: (map['payload'] as Map?)?.cast<String, dynamic>() ?? const {},
      createdAt: map['createdAt'] == null
          ? DateTime.now()
          : DateTime.fromMillisecondsSinceEpoch(map['createdAt'] as int),
      retryCount: map['retryCount'] as int? ?? 0,
    );
  }

  /// Human-friendly label for the action queue viewer.
  String get description {
    switch (type) {
      case PendingActionType.recordPosition:
        final p = position;
        return p != null ? L.current.action_save_progress_at(p.toInt()) : L.current.action_save_progress;
      case PendingActionType.markCompleted:
        return L.current.action_mark_completed;
      case PendingActionType.markUncompleted:
        return L.current.action_mark_not_completed;
      case PendingActionType.saveEpisode:
        return L.current.action_save_episode;
      case PendingActionType.removeSaved:
        return L.current.action_remove_saved;
      case PendingActionType.queue:
        return L.current.action_add_to_queue;
      case PendingActionType.addHistory:
        return L.current.action_add_to_history;
    }
  }
}
