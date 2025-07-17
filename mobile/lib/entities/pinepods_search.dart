// lib/entities/pinepods_search.dart

class PinepodsSearchResult {
  final String? status;
  final int? resultCount;
  final List<PinepodsPodcast>? feeds;
  final List<PinepodsITunesPodcast>? results;

  PinepodsSearchResult({
    this.status,
    this.resultCount,
    this.feeds,
    this.results,
  });

  factory PinepodsSearchResult.fromJson(Map<String, dynamic> json) {
    return PinepodsSearchResult(
      status: json['status'] as String?,
      resultCount: json['resultCount'] as int?,
      feeds: json['feeds'] != null
          ? (json['feeds'] as List)
              .map((item) => PinepodsPodcast.fromJson(item as Map<String, dynamic>))
              .toList()
          : null,
      results: json['results'] != null
          ? (json['results'] as List)
              .map((item) => PinepodsITunesPodcast.fromJson(item as Map<String, dynamic>))
              .toList()
          : null,
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'status': status,
      'resultCount': resultCount,
      'feeds': feeds?.map((item) => item.toJson()).toList(),
      'results': results?.map((item) => item.toJson()).toList(),
    };
  }

  List<UnifiedPinepodsPodcast> getUnifiedPodcasts() {
    final List<UnifiedPinepodsPodcast> unified = [];
    
    // Add PodcastIndex results
    if (feeds != null) {
      unified.addAll(feeds!.map((podcast) => UnifiedPinepodsPodcast.fromPodcast(podcast)));
    }
    
    // Add iTunes results  
    if (results != null) {
      unified.addAll(results!.map((podcast) => UnifiedPinepodsPodcast.fromITunesPodcast(podcast)));
    }
    
    return unified;
  }
}

class PinepodsPodcast {
  final int id;
  final String title;
  final String url;
  final String originalUrl;
  final String link;
  final String description;
  final String author;
  final String ownerName;
  final String image;
  final String artwork;
  final int lastUpdateTime;
  final Map<String, String>? categories;
  final bool explicit;
  final int episodeCount;

  PinepodsPodcast({
    required this.id,
    required this.title,
    required this.url,
    required this.originalUrl,
    required this.link,
    required this.description,
    required this.author,
    required this.ownerName,
    required this.image,
    required this.artwork,
    required this.lastUpdateTime,
    this.categories,
    required this.explicit,
    required this.episodeCount,
  });

  factory PinepodsPodcast.fromJson(Map<String, dynamic> json) {
    return PinepodsPodcast(
      id: json['id'] as int,
      title: json['title'] as String? ?? '',
      url: json['url'] as String? ?? '',
      originalUrl: json['originalUrl'] as String? ?? '',
      link: json['link'] as String? ?? '',
      description: json['description'] as String? ?? '',
      author: json['author'] as String? ?? '',
      ownerName: json['ownerName'] as String? ?? '',
      image: json['image'] as String? ?? '',
      artwork: json['artwork'] as String? ?? '',
      lastUpdateTime: json['lastUpdateTime'] as int? ?? 0,
      categories: json['categories'] != null
          ? Map<String, String>.from(json['categories'] as Map)
          : null,
      explicit: json['explicit'] as bool? ?? false,
      episodeCount: json['episodeCount'] as int? ?? 0,
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'id': id,
      'title': title,
      'url': url,
      'originalUrl': originalUrl,
      'link': link,
      'description': description,
      'author': author,
      'ownerName': ownerName,
      'image': image,
      'artwork': artwork,
      'lastUpdateTime': lastUpdateTime,
      'categories': categories,
      'explicit': explicit,
      'episodeCount': episodeCount,
    };
  }
}

class PinepodsITunesPodcast {
  final String wrapperType;
  final String kind;
  final int collectionId;
  final int trackId;
  final String artistName;
  final String trackName;
  final String collectionViewUrl;
  final String feedUrl;
  final String artworkUrl100;
  final String releaseDate;
  final List<String> genres;
  final String collectionExplicitness;
  final int? trackCount;

  PinepodsITunesPodcast({
    required this.wrapperType,
    required this.kind,
    required this.collectionId,
    required this.trackId,
    required this.artistName,
    required this.trackName,
    required this.collectionViewUrl,
    required this.feedUrl,
    required this.artworkUrl100,
    required this.releaseDate,
    required this.genres,
    required this.collectionExplicitness,
    this.trackCount,
  });

  factory PinepodsITunesPodcast.fromJson(Map<String, dynamic> json) {
    return PinepodsITunesPodcast(
      wrapperType: json['wrapperType'] as String? ?? '',
      kind: json['kind'] as String? ?? '',
      collectionId: json['collectionId'] as int? ?? 0,
      trackId: json['trackId'] as int? ?? 0,
      artistName: json['artistName'] as String? ?? '',
      trackName: json['trackName'] as String? ?? '',
      collectionViewUrl: json['collectionViewUrl'] as String? ?? '',
      feedUrl: json['feedUrl'] as String? ?? '',
      artworkUrl100: json['artworkUrl100'] as String? ?? '',
      releaseDate: json['releaseDate'] as String? ?? '',
      genres: json['genres'] != null
          ? List<String>.from(json['genres'] as List)
          : [],
      collectionExplicitness: json['collectionExplicitness'] as String? ?? '',
      trackCount: json['trackCount'] as int?,
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'wrapperType': wrapperType,
      'kind': kind,
      'collectionId': collectionId,
      'trackId': trackId,
      'artistName': artistName,
      'trackName': trackName,
      'collectionViewUrl': collectionViewUrl,
      'feedUrl': feedUrl,
      'artworkUrl100': artworkUrl100,
      'releaseDate': releaseDate,
      'genres': genres,
      'collectionExplicitness': collectionExplicitness,
      'trackCount': trackCount,
    };
  }
}

class UnifiedPinepodsPodcast {
  final int id;
  final int indexId;
  final String title;
  final String url;
  final String originalUrl;
  final String link;
  final String description;
  final String author;
  final String ownerName;
  final String image;
  final String artwork;
  final int lastUpdateTime;
  final Map<String, String>? categories;
  final bool explicit;
  final int episodeCount;

  UnifiedPinepodsPodcast({
    required this.id,
    required this.indexId,
    required this.title,
    required this.url,
    required this.originalUrl,
    required this.link,
    required this.description,
    required this.author,
    required this.ownerName,
    required this.image,
    required this.artwork,
    required this.lastUpdateTime,
    this.categories,
    required this.explicit,
    required this.episodeCount,
  });

  factory UnifiedPinepodsPodcast.fromJson(Map<String, dynamic> json) {
    return UnifiedPinepodsPodcast(
      id: json['id'] as int? ?? 0,
      indexId: json['indexId'] as int? ?? 0,
      title: json['title'] as String? ?? '',
      url: json['url'] as String? ?? '',
      originalUrl: json['originalUrl'] as String? ?? '',
      link: json['link'] as String? ?? '',
      description: json['description'] as String? ?? '',
      author: json['author'] as String? ?? '',
      ownerName: json['ownerName'] as String? ?? '',
      image: json['image'] as String? ?? '',
      artwork: json['artwork'] as String? ?? '',
      lastUpdateTime: json['lastUpdateTime'] as int? ?? 0,
      categories: json['categories'] != null
          ? Map<String, String>.from(json['categories'] as Map)
          : null,
      explicit: json['explicit'] as bool? ?? false,
      episodeCount: json['episodeCount'] as int? ?? 0,
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'id': id,
      'indexId': indexId,
      'title': title,
      'url': url,
      'originalUrl': originalUrl,
      'link': link,
      'description': description,
      'author': author,
      'ownerName': ownerName,
      'image': image,
      'artwork': artwork,
      'lastUpdateTime': lastUpdateTime,
      'categories': categories,
      'explicit': explicit,
      'episodeCount': episodeCount,
    };
  }

  factory UnifiedPinepodsPodcast.fromPodcast(PinepodsPodcast podcast) {
    return UnifiedPinepodsPodcast(
      id: podcast.id,
      indexId: podcast.id,
      title: podcast.title,
      url: podcast.url,
      originalUrl: podcast.originalUrl,
      author: podcast.author,
      ownerName: podcast.ownerName,
      description: podcast.description,
      image: podcast.image,
      link: podcast.link,
      artwork: podcast.artwork,
      lastUpdateTime: podcast.lastUpdateTime,
      categories: podcast.categories,
      explicit: podcast.explicit,
      episodeCount: podcast.episodeCount,
    );
  }

  factory UnifiedPinepodsPodcast.fromITunesPodcast(PinepodsITunesPodcast podcast) {
    // Convert genres list to map
    final Map<String, String> genreMap = {};
    for (int i = 0; i < podcast.genres.length; i++) {
      genreMap[i.toString()] = podcast.genres[i];
    }

    // Parse release date to timestamp
    int timestamp = 0;
    try {
      final dateTime = DateTime.parse(podcast.releaseDate);
      timestamp = dateTime.millisecondsSinceEpoch ~/ 1000;
    } catch (e) {
      // Default to 0 if parsing fails
    }

    return UnifiedPinepodsPodcast(
      id: podcast.trackId,
      indexId: 0,
      title: podcast.trackName,
      url: podcast.feedUrl,
      originalUrl: podcast.feedUrl,
      author: podcast.artistName,
      ownerName: podcast.artistName,
      description: 'Descriptions not provided by iTunes',
      image: podcast.artworkUrl100,
      link: podcast.collectionViewUrl,
      artwork: podcast.artworkUrl100,
      lastUpdateTime: timestamp,
      categories: genreMap,
      explicit: podcast.collectionExplicitness == 'explicit',
      episodeCount: podcast.trackCount ?? 0,
    );
  }
}

enum SearchProvider {
  podcastIndex,
  itunes,
}

extension SearchProviderExtension on SearchProvider {
  String get name {
    switch (this) {
      case SearchProvider.podcastIndex:
        return 'Podcast Index';
      case SearchProvider.itunes:
        return 'iTunes';
    }
  }

  String get value {
    switch (this) {
      case SearchProvider.podcastIndex:
        return 'podcast_index';
      case SearchProvider.itunes:
        return 'itunes';
    }
  }
}