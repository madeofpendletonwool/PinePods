// lib/entities/episode_label.dart
import 'package:flutter/material.dart';

/// Colours a label can have; the server accepts only these names.
const List<String> labelColors = [
  'blue', 'green', 'yellow', 'orange', 'red', 'purple', 'pink', 'teal', 'gray', // i18n-ignore
];

Color labelColor(String name) {
  switch (name) {
    case 'green':
      return const Color(0xFF22C55E);
    case 'yellow':
      return const Color(0xFFEAB308);
    case 'orange':
      return const Color(0xFFF97316);
    case 'red':
      return const Color(0xFFEF4444);
    case 'purple':
      return const Color(0xFFA855F7);
    case 'pink':
      return const Color(0xFFEC4899);
    case 'teal':
      return const Color(0xFF14B8A6);
    case 'gray':
      return const Color(0xFF94A3B8);
    case 'blue':
    default:
      return const Color(0xFF3B82F6);
  }
}

/// One of the user's own labels for sorting episodes.
class EpisodeLabel {
  final int id;
  final String name;
  final String color;
  final int episodeCount;

  const EpisodeLabel({required this.id, required this.name, required this.color, this.episodeCount = 0});

  factory EpisodeLabel.fromJson(Map<String, dynamic> json) => EpisodeLabel(
        id: (json['label_id'] as num).toInt(),
        name: json['name'] as String? ?? '',
        color: json['color'] as String? ?? 'blue',
        episodeCount: (json['episode_count'] as num?)?.toInt() ?? 0,
      );

  EpisodeLabel copyWith({String? name, String? color, int? episodeCount}) => EpisodeLabel(
        id: id,
        name: name ?? this.name,
        color: color ?? this.color,
        episodeCount: episodeCount ?? this.episodeCount,
      );
}

/// Which part of the inbox to list or clear.
class InboxQuery {
  final int? labelId;
  final bool unlabeled;
  final int? podcastId;
  final bool oldestFirst;

  const InboxQuery({this.labelId, this.unlabeled = false, this.podcastId, this.oldestFirst = false});

  bool get isEverything => labelId == null && !unlabeled && podcastId == null;

  InboxQuery copyWith({int? Function()? labelId, bool? unlabeled, int? Function()? podcastId, bool? oldestFirst}) => InboxQuery(
        labelId: labelId != null ? labelId() : this.labelId,
        unlabeled: unlabeled ?? this.unlabeled,
        podcastId: podcastId != null ? podcastId() : this.podcastId,
        oldestFirst: oldestFirst ?? this.oldestFirst,
      );
}
