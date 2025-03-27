// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/core/annotations.dart';
import 'package:pinepods_mobile/core/extensions.dart';
import 'package:pinepods_mobile/entities/chapter.dart';
import 'package:pinepods_mobile/entities/downloadable.dart';
import 'package:pinepods_mobile/entities/person.dart';
import 'package:pinepods_mobile/entities/transcript.dart';
import 'package:flutter/foundation.dart';
import 'package:html/parser.dart' show parseFragment;
import 'package:logging/logging.dart';

/// An object that represents an individual episode of a Podcast.
///
/// An Episode can be used in conjunction with a [Downloadable] to
/// determine if the Episode is available on the local filesystem.
class Episode {
  final log = Logger('Episode');

  /// Database ID
  int? id;

  /// A String GUID for the episode.
  final String guid;

  /// The GUID for an associated podcast. If an episode has been downloaded
  /// without subscribing to a podcast this may be null.
  String? pguid;

  /// If the episode is currently being downloaded, this contains the unique
  /// ID supplied by the download manager for the episode.
  String? downloadTaskId;

  /// The path to the directory containing the download for this episode; or null.
  String? filepath;

  /// The filename of the downloaded episode; or null.
  String? filename;

  /// The current downloading state of the episode.
  DownloadState downloadState = DownloadState.none;

  /// The name of the podcast the episode is part of.
  String? podcast;

  /// The episode title.
  String? title;

  /// The episode description. This could be plain text or HTML.
  String? description;

  /// More detailed description - optional.
  String? content;

  /// External link
  String? link;

  /// URL to the episode artwork image.
  String? imageUrl;

  /// URL to a thumbnail version of the episode artwork image.
  String? thumbImageUrl;

  /// The date the episode was published (if known).
  DateTime? publicationDate;

  /// The URL for the episode location.
  String? contentUrl;

  /// Author of the episode if known.
  String? author;

  /// The season the episode is part of if available.
  int season;

  /// The episode number within a season if available.
  int episode;

  /// The duration of the episode in milliseconds. This can be populated either from
  /// the RSS if available, or determined from the MP3 file at stream/download time.
  int duration;

  /// Stores the current position within the episode in milliseconds. Used for resuming.
  int position;

  /// Stores the progress of the current download progress if available.
  int? downloadPercentage;

  /// True if this episode is 'marked as played'.
  bool played;

  /// URL pointing to a JSON file containing chapter information if available.
  String? chaptersUrl;

  /// List of chapters for the episode if available.
  List<Chapter> chapters;

  /// List of transcript URLs for the episode if available.
  List<TranscriptUrl> transcriptUrls;

  List<Person> persons;

  /// Currently downloaded or in use transcript for the episode.To minimise memory
  /// use, this is cleared when an episode download is deleted, or a streamed episode stopped.
  Transcript? transcript;

  /// Link to a currently stored transcript for this episode.
  int? transcriptId;

  /// Date and time episode was last updated and persisted.
  DateTime? lastUpdated;

  /// Processed version of episode description.
  String? _descriptionText;

  /// Index of the currently playing chapter it available. Transient.
  int? chapterIndex;

  /// Current chapter we are listening to if this episode has chapters.  Transient.
  Chapter? currentChapter;

  /// Set to true if chapter data is currently being loaded.
  @Transient()
  bool chaptersLoading = false;

  @Transient()
  bool highlight = false;

  @Transient()
  bool queued = false;

  @Transient()
  bool streaming = true;

  Episode({
    required this.guid,
    this.pguid,
    required this.podcast,
    this.id,
    this.downloadTaskId,
    this.filepath,
    this.filename,
    this.downloadState = DownloadState.none,
    this.title,
    this.description,
    this.content,
    this.link,
    String? imageUrl,
    String? thumbImageUrl,
    this.publicationDate,
    String? contentUrl,
    this.author,
    this.season = 0,
    this.episode = 0,
    this.duration = 0,
    this.position = 0,
    this.downloadPercentage = 0,
    this.played = false,
    this.highlight = false,
    String? chaptersUrl,
    this.chapters = const <Chapter>[],
    this.transcriptUrls = const <TranscriptUrl>[],
    this.persons = const <Person>[],
    this.transcriptId = 0,
    this.lastUpdated,
  })  : imageUrl = imageUrl?.forceHttps,
        thumbImageUrl = thumbImageUrl?.forceHttps,
        contentUrl = contentUrl?.forceHttps,
        chaptersUrl = chaptersUrl?.forceHttps;

  Map<String, dynamic> toMap() {
    return <String, dynamic>{
      'guid': guid,
      'pguid': pguid,
      'downloadTaskId': downloadTaskId,
      'filepath': filepath,
      'filename': filename,
      'downloadState': downloadState.index,
      'podcast': podcast,
      'title': title,
      'description': description,
      'content': content,
      'link': link,
      'imageUrl': imageUrl,
      'thumbImageUrl': thumbImageUrl,
      'publicationDate': publicationDate?.millisecondsSinceEpoch.toString(),
      'contentUrl': contentUrl,
      'author': author,
      'season': season.toString(),
      'episode': episode.toString(),
      'duration': duration.toString(),
      'position': position.toString(),
      'downloadPercentage': downloadPercentage.toString(),
      'played': played ? 'true' : 'false',
      'chaptersUrl': chaptersUrl,
      'chapters': (chapters).map((chapter) => chapter.toMap()).toList(growable: false),
      'tid': transcriptId ?? 0,
      'transcriptUrls': (transcriptUrls).map((tu) => tu.toMap()).toList(growable: false),
      'persons': (persons).map((person) => person.toMap()).toList(growable: false),
      'lastUpdated': lastUpdated?.millisecondsSinceEpoch.toString() ?? '',
    };
  }

  static Episode fromMap(int? key, Map<String, dynamic> episode) {
    var chapters = <Chapter>[];
    var transcriptUrls = <TranscriptUrl>[];
    var persons = <Person>[];

    // We need to perform an 'is' on each loop to prevent Dart
    // from complaining that we have not set the type for chapter.
    if (episode['chapters'] != null) {
      for (var chapter in (episode['chapters'] as List)) {
        if (chapter is Map<String, dynamic>) {
          chapters.add(Chapter.fromMap(chapter));
        }
      }
    }

    if (episode['transcriptUrls'] != null) {
      for (var transcriptUrl in (episode['transcriptUrls'] as List)) {
        if (transcriptUrl is Map<String, dynamic>) {
          transcriptUrls.add(TranscriptUrl.fromMap(transcriptUrl));
        }
      }
    }

    if (episode['persons'] != null) {
      for (var person in (episode['persons'] as List)) {
        if (person is Map<String, dynamic>) {
          persons.add(Person.fromMap(person));
        }
      }
    }

    return Episode(
      id: key,
      guid: episode['guid'] as String,
      pguid: episode['pguid'] as String?,
      downloadTaskId: episode['downloadTaskId'] as String?,
      filepath: episode['filepath'] as String?,
      filename: episode['filename'] as String?,
      downloadState: _determineState(episode['downloadState'] as int?),
      podcast: episode['podcast'] as String?,
      title: episode['title'] as String?,
      description: episode['description'] as String?,
      content: episode['content'] as String?,
      link: episode['link'] as String?,
      imageUrl: episode['imageUrl'] as String?,
      thumbImageUrl: episode['thumbImageUrl'] as String?,
      publicationDate: episode['publicationDate'] == null || episode['publicationDate'] == 'null'
          ? DateTime.now()
          : DateTime.fromMillisecondsSinceEpoch(int.parse(episode['publicationDate'] as String)),
      contentUrl: episode['contentUrl'] as String?,
      author: episode['author'] as String?,
      season: int.parse(episode['season'] as String? ?? '0'),
      episode: int.parse(episode['episode'] as String? ?? '0'),
      duration: int.parse(episode['duration'] as String? ?? '0'),
      position: int.parse(episode['position'] as String? ?? '0'),
      downloadPercentage: int.parse(episode['downloadPercentage'] as String? ?? '0'),
      played: episode['played'] == 'true' ? true : false,
      chaptersUrl: episode['chaptersUrl'] as String?,
      chapters: chapters,
      transcriptUrls: transcriptUrls,
      persons: persons,
      transcriptId: episode['tid'] == null ? 0 : episode['tid'] as int?,
      lastUpdated: episode['lastUpdated'] == null || episode['lastUpdated'] == 'null'
          ? DateTime.now()
          : DateTime.fromMillisecondsSinceEpoch(int.parse(episode['lastUpdated'] as String)),
    );
  }

  static DownloadState _determineState(int? index) {
    switch (index) {
      case 0:
        return DownloadState.none;
      case 1:
        return DownloadState.queued;
      case 2:
        return DownloadState.downloading;
      case 3:
        return DownloadState.failed;
      case 4:
        return DownloadState.cancelled;
      case 5:
        return DownloadState.paused;
      case 6:
        return DownloadState.downloaded;
    }

    return DownloadState.none;
  }

  @override
  bool operator ==(Object other) {
    return identical(this, other) ||
        other is Episode &&
            runtimeType == other.runtimeType &&
            guid == other.guid &&
            pguid == other.pguid &&
            downloadTaskId == other.downloadTaskId &&
            filepath == other.filepath &&
            filename == other.filename &&
            downloadState == other.downloadState &&
            podcast == other.podcast &&
            title == other.title &&
            description == other.description &&
            content == other.content &&
            link == other.link &&
            imageUrl == other.imageUrl &&
            thumbImageUrl == other.thumbImageUrl &&
            publicationDate?.millisecondsSinceEpoch == other.publicationDate?.millisecondsSinceEpoch &&
            contentUrl == other.contentUrl &&
            author == other.author &&
            season == other.season &&
            episode == other.episode &&
            duration == other.duration &&
            position == other.position &&
            downloadPercentage == other.downloadPercentage &&
            played == other.played &&
            chaptersUrl == other.chaptersUrl &&
            transcriptId == other.transcriptId &&
            listEquals(persons, other.persons) &&
            listEquals(chapters, other.chapters);
  }

  @override
  int get hashCode =>
      id.hashCode ^
      guid.hashCode ^
      pguid.hashCode ^
      downloadTaskId.hashCode ^
      filepath.hashCode ^
      filename.hashCode ^
      downloadState.hashCode ^
      podcast.hashCode ^
      title.hashCode ^
      description.hashCode ^
      content.hashCode ^
      link.hashCode ^
      imageUrl.hashCode ^
      thumbImageUrl.hashCode ^
      publicationDate.hashCode ^
      contentUrl.hashCode ^
      author.hashCode ^
      season.hashCode ^
      episode.hashCode ^
      duration.hashCode ^
      position.hashCode ^
      downloadPercentage.hashCode ^
      played.hashCode ^
      chaptersUrl.hashCode ^
      chapters.hashCode ^
      transcriptId.hashCode ^
      lastUpdated.hashCode;

  @override
  String toString() {
    return 'Episode{id: $id, guid: $guid, pguid: $pguid, filepath: $filepath, title: $title, contentUrl: $contentUrl, episode: $episode, duration: $duration, position: $position, downloadPercentage: $downloadPercentage, played: $played, queued: $queued}';
  }

  bool get downloaded => downloadPercentage == 100;

  Duration get timeRemaining {
    if (position > 0 && duration > 0) {
      var currentPosition = Duration(milliseconds: position);

      var tr = duration - currentPosition.inSeconds;

      return Duration(seconds: tr);
    }

    return const Duration(seconds: 0);
  }

  double get percentagePlayed {
    if (position > 0 && duration > 0) {
      var pc = (position / (duration * 1000)) * 100;

      if (pc > 100.0) {
        pc = 100.0;
      }

      return pc;
    }

    return 0.0;
  }

  String? get descriptionText {
    if (_descriptionText == null || _descriptionText!.isEmpty) {
      if (description == null || description!.isEmpty) {
        _descriptionText = '';
      } else {
        // Replace break tags with space character for readability
        var formattedDescription = description!.replaceAll(RegExp(r'(<br/?>)+'), ' ');
        _descriptionText = parseFragment(formattedDescription).text;
      }
    }

    return _descriptionText;
  }

  bool get hasChapters => chaptersUrl != null && chaptersUrl!.isNotEmpty;

  bool get hasTranscripts => transcriptUrls.isNotEmpty;

  bool get chaptersAreLoaded => chaptersLoading == false && chapters.isNotEmpty;

  bool get chaptersAreNotLoaded => chaptersLoading == true && chapters.isEmpty;

  String? get positionalImageUrl {
    if (currentChapter != null && currentChapter!.imageUrl != null && currentChapter!.imageUrl!.isNotEmpty) {
      return currentChapter!.imageUrl;
    }

    return imageUrl;
  }
}
