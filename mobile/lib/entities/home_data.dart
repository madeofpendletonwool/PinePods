// lib/entities/home_data.dart

class HomePodcast {
  final int podcastId;
  final String podcastName;
  final int? podcastIndexId;
  final String? artworkUrl;
  final String? author;
  final String? categories;
  final String? description;
  final int? episodeCount;
  final String? feedUrl;
  final String? websiteUrl;
  final bool? explicit;
  final bool isYoutube;
  final int playCount;
  final int? totalListenTime;

  HomePodcast({
    required this.podcastId,
    required this.podcastName,
    this.podcastIndexId,
    this.artworkUrl,
    this.author,
    this.categories,
    this.description,
    this.episodeCount,
    this.feedUrl,
    this.websiteUrl,
    this.explicit,
    required this.isYoutube,
    required this.playCount,
    this.totalListenTime,
  });

  factory HomePodcast.fromJson(Map<String, dynamic> json) {
    return HomePodcast(
      podcastId: json['podcastid'] ?? 0,
      podcastName: json['podcastname'] ?? '',
      podcastIndexId: json['podcastindexid'],
      artworkUrl: json['artworkurl'],
      author: json['author'],
      categories: json['categories'],
      description: json['description'],
      episodeCount: json['episodecount'],
      feedUrl: json['feedurl'],
      websiteUrl: json['websiteurl'],
      explicit: json['explicit'],
      isYoutube: json['is_youtube'] ?? false,
      playCount: json['play_count'] ?? 0,
      totalListenTime: json['total_listen_time'],
    );
  }
}

class HomeEpisode {
  final int episodeId;
  final int podcastId;
  final String episodeTitle;
  final String episodeDescription;
  final String episodeUrl;
  final String episodeArtwork;
  final String episodePubDate;
  final int episodeDuration;
  final bool completed;
  final String podcastName;
  final bool isYoutube;
  final int? listenDuration;
  final bool saved;
  final bool queued;
  final bool downloaded;

  HomeEpisode({
    required this.episodeId,
    required this.podcastId,
    required this.episodeTitle,
    required this.episodeDescription,
    required this.episodeUrl,
    required this.episodeArtwork,
    required this.episodePubDate,
    required this.episodeDuration,
    required this.completed,
    required this.podcastName,
    required this.isYoutube,
    this.listenDuration,
    this.saved = false,
    this.queued = false,
    this.downloaded = false,
  });

  factory HomeEpisode.fromJson(Map<String, dynamic> json) {
    return HomeEpisode(
      episodeId: json['episodeid'] ?? 0,
      podcastId: json['podcastid'] ?? 0,
      episodeTitle: json['episodetitle'] ?? '',
      episodeDescription: json['episodedescription'] ?? '',
      episodeUrl: json['episodeurl'] ?? '',
      episodeArtwork: json['episodeartwork'] ?? '',
      episodePubDate: json['episodepubdate'] ?? '',
      episodeDuration: json['episodeduration'] ?? 0,
      completed: json['completed'] ?? false,
      podcastName: json['podcastname'] ?? '',
      isYoutube: json['is_youtube'] ?? false,
      listenDuration: json['listenduration'],
      saved: json['saved'] ?? false,
      queued: json['queued'] ?? false,
      downloaded: json['downloaded'] ?? false,
    );
  }

  /// Format duration in seconds to MM:SS or HH:MM:SS format
  String get formattedDuration {
    if (episodeDuration <= 0) return '--:--';
    
    final hours = episodeDuration ~/ 3600;
    final minutes = (episodeDuration % 3600) ~/ 60;
    final seconds = episodeDuration % 60;
    
    if (hours > 0) {
      return '${hours.toString().padLeft(2, '0')}:${minutes.toString().padLeft(2, '0')}:${seconds.toString().padLeft(2, '0')}';
    } else {
      return '${minutes.toString().padLeft(2, '0')}:${seconds.toString().padLeft(2, '0')}';
    }
  }

  /// Format listen duration if available
  String? get formattedListenDuration {
    if (listenDuration == null || listenDuration! <= 0) return null;
    
    final duration = listenDuration!;
    final hours = duration ~/ 3600;
    final minutes = (duration % 3600) ~/ 60;
    final seconds = duration % 60;
    
    if (hours > 0) {
      return '${hours.toString().padLeft(2, '0')}:${minutes.toString().padLeft(2, '0')}:${seconds.toString().padLeft(2, '0')}';
    } else {
      return '${minutes.toString().padLeft(2, '0')}:${seconds.toString().padLeft(2, '0')}';
    }
  }

  /// Calculate progress percentage for progress bar
  double get progressPercentage {
    if (episodeDuration <= 0 || listenDuration == null) return 0.0;
    return (listenDuration! / episodeDuration) * 100.0;
  }
}

class HomeOverview {
  final List<HomeEpisode> recentEpisodes;
  final List<HomeEpisode> inProgressEpisodes;
  final List<HomePodcast> topPodcasts;
  final int savedCount;
  final int downloadedCount;
  final int queueCount;

  HomeOverview({
    required this.recentEpisodes,
    required this.inProgressEpisodes,
    required this.topPodcasts,
    required this.savedCount,
    required this.downloadedCount,
    required this.queueCount,
  });

  factory HomeOverview.fromJson(Map<String, dynamic> json) {
    return HomeOverview(
      recentEpisodes: (json['recent_episodes'] as List<dynamic>? ?? [])
          .map((e) => HomeEpisode.fromJson(e))
          .toList(),
      inProgressEpisodes: (json['in_progress_episodes'] as List<dynamic>? ?? [])
          .map((e) => HomeEpisode.fromJson(e))
          .toList(),
      topPodcasts: (json['top_podcasts'] as List<dynamic>? ?? [])
          .map((p) => HomePodcast.fromJson(p))
          .toList(),
      savedCount: json['saved_count'] ?? 0,
      downloadedCount: json['downloaded_count'] ?? 0,
      queueCount: json['queue_count'] ?? 0,
    );
  }
}

class Playlist {
  final int playlistId;
  final String name;
  final String? description;
  final String iconName;
  final int? episodeCount;

  Playlist({
    required this.playlistId,
    required this.name,
    this.description,
    required this.iconName,
    this.episodeCount,
  });

  factory Playlist.fromJson(Map<String, dynamic> json) {
    return Playlist(
      playlistId: json['playlist_id'] ?? 0,
      name: json['name'] ?? '',
      description: json['description'],
      iconName: json['icon_name'] ?? 'ph-music-notes',
      episodeCount: json['episode_count'],
    );
  }
}

class PlaylistResponse {
  final List<Playlist> playlists;

  PlaylistResponse({required this.playlists});

  factory PlaylistResponse.fromJson(Map<String, dynamic> json) {
    return PlaylistResponse(
      playlists: (json['playlists'] as List<dynamic>? ?? [])
          .map((p) => Playlist.fromJson(p))
          .toList(),
    );
  }
}