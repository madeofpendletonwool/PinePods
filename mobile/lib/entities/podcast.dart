// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/core/extensions.dart';
import 'package:pinepods_mobile/entities/funding.dart';
import 'package:pinepods_mobile/entities/person.dart';
import 'package:podcast_search/podcast_search.dart' as search;

import 'episode.dart';

enum PodcastEpisodeFilter {
  none(id: 0),
  started(id: 1),
  played(id: 2),
  notPlayed(id: 3);

  const PodcastEpisodeFilter({required this.id});

  final int id;
}

enum PodcastEpisodeSort {
  none(id: 0),
  latestFirst(id: 1),
  earliestFirst(id: 2),
  alphabeticalAscending(id: 3),
  alphabeticalDescending(id: 4);

  const PodcastEpisodeSort({required this.id});

  final int id;
}

/// A class that represents an instance of a podcast.
///
/// When persisted to disk this represents a podcast that is being followed.
class Podcast {
  /// Database ID
  int? id;

  /// Unique identifier for podcast.
  final String? guid;

  /// The link to the podcast RSS feed.
  final String url;

  /// RSS link URL.
  final String? link;

  /// Podcast title.
  final String title;

  /// Podcast description. Can be either plain text or HTML.
  final String? description;

  /// URL to the full size artwork image.
  final String? imageUrl;

  /// URL for thumbnail version of artwork image. Not contained within
  /// the RSS but may be calculated or provided within search results.
  final String? thumbImageUrl;

  /// Copyright owner of the podcast.
  final String? copyright;

  /// Zero or more funding links.
  final List<Funding>? funding;

  PodcastEpisodeFilter filter;

  PodcastEpisodeSort sort;

  /// Date and time user subscribed to the podcast.
  DateTime? subscribedDate;

  /// Date and time podcast was last updated/refreshed.
  DateTime? _lastUpdated;

  /// One or more episodes for this podcast.
  List<Episode> episodes;

  final List<Person>? persons;

  bool newEpisodes;
  bool updatedEpisodes = false;

  Podcast({
    required this.guid,
    required String url,
    required this.link,
    required this.title,
    this.id,
    this.description,
    String? imageUrl,
    String? thumbImageUrl,
    this.copyright,
    this.subscribedDate,
    this.funding,
    this.filter = PodcastEpisodeFilter.none,
    this.sort = PodcastEpisodeSort.none,
    this.episodes = const <Episode>[],
    this.newEpisodes = false,
    this.persons,
    DateTime? lastUpdated,
  })  : url = url.forceHttps,
        imageUrl = imageUrl?.forceHttps,
        thumbImageUrl = thumbImageUrl?.forceHttps {
    _lastUpdated = lastUpdated;
  }

  factory Podcast.fromUrl({required String url}) => Podcast(
        url: url,
        guid: '',
        link: '',
        title: '',
        description: '',
        thumbImageUrl: null,
        imageUrl: null,
        copyright: '',
        funding: <Funding>[],
        persons: <Person>[],
      );

  factory Podcast.fromSearchResultItem(search.Item item) => Podcast(
        guid: item.guid ?? '',
        url: item.feedUrl ?? '',
        link: item.feedUrl,
        title: item.trackName!,
        description: '',
        imageUrl: item.bestArtworkUrl ?? item.artworkUrl,
        thumbImageUrl: item.thumbnailArtworkUrl,
        funding: const <Funding>[],
        copyright: item.artistName,
      );

  Map<String, dynamic> toMap() {
    return <String, dynamic>{
      'guid': guid,
      'title': title,
      'copyright': copyright ?? '',
      'description': description ?? '',
      'url': url,
      'link': link ?? '',
      'imageUrl': imageUrl ?? '',
      'thumbImageUrl': thumbImageUrl ?? '',
      'subscribedDate': subscribedDate?.millisecondsSinceEpoch.toString() ?? '',
      'filter': filter.id,
      'sort': sort.id,
      'funding': (funding ?? <Funding>[]).map((funding) => funding.toMap()).toList(growable: false),
      'person': (persons ?? <Person>[]).map((persons) => persons.toMap()).toList(growable: false),
      'lastUpdated': _lastUpdated?.millisecondsSinceEpoch ?? DateTime.now().millisecondsSinceEpoch,
    };
  }

  static Podcast fromMap(int key, Map<String, dynamic> podcast) {
    final sds = podcast['subscribedDate'] as String?;
    final lus = podcast['lastUpdated'] as int?;
    final funding = <Funding>[];
    final persons = <Person>[];
    var filter = PodcastEpisodeFilter.none;
    var sort = PodcastEpisodeSort.none;

    var sd = DateTime.now();
    var lastUpdated = DateTime(1971, 1, 1);

    if (sds != null && sds.isNotEmpty && int.tryParse(sds) != null) {
      sd = DateTime.fromMillisecondsSinceEpoch(int.parse(sds));
    }

    if (lus != null) {
      lastUpdated = DateTime.fromMillisecondsSinceEpoch(lus);
    }

    if (podcast['funding'] != null) {
      for (var chapter in (podcast['funding'] as List)) {
        if (chapter is Map<String, dynamic>) {
          funding.add(Funding.fromMap(chapter));
        }
      }
    }

    if (podcast['persons'] != null) {
      for (var person in (podcast['persons'] as List)) {
        if (person is Map<String, dynamic>) {
          persons.add(Person.fromMap(person));
        }
      }
    }

    if (podcast['filter'] != null) {
      var filterValue = (podcast['filter'] as int);

      filter = switch (filterValue) {
        1 => PodcastEpisodeFilter.started,
        2 => PodcastEpisodeFilter.played,
        3 => PodcastEpisodeFilter.notPlayed,
        _ => PodcastEpisodeFilter.none,
      };
    }

    if (podcast['sort'] != null) {
      var sortValue = (podcast['sort'] as int);

      sort = switch (sortValue) {
        1 => PodcastEpisodeSort.latestFirst,
        2 => PodcastEpisodeSort.earliestFirst,
        3 => PodcastEpisodeSort.alphabeticalAscending,
        4 => PodcastEpisodeSort.alphabeticalDescending,
        _ => PodcastEpisodeSort.none,
      };
    }

    return Podcast(
      id: key,
      guid: podcast['guid'] as String,
      link: podcast['link'] as String?,
      title: podcast['title'] as String,
      copyright: podcast['copyright'] as String?,
      description: podcast['description'] as String?,
      url: podcast['url'] as String,
      imageUrl: podcast['imageUrl'] as String?,
      thumbImageUrl: podcast['thumbImageUrl'] as String?,
      filter: filter,
      sort: sort,
      funding: funding,
      persons: persons,
      subscribedDate: sd,
      lastUpdated: lastUpdated,
    );
  }

  bool get subscribed => id != null;

  DateTime get lastUpdated => _lastUpdated ?? DateTime(1970, 1, 1);

  set lastUpdated(DateTime value) {
    _lastUpdated = value;
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is Podcast && runtimeType == other.runtimeType && guid == other.guid && url == other.url;

  @override
  int get hashCode => guid.hashCode ^ url.hashCode;
}
