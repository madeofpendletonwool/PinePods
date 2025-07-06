class UserStats {
  final String userCreated;
  final int podcastsPlayed;
  final int timeListened;
  final int podcastsAdded;
  final int episodesSaved;
  final int episodesDownloaded;
  final String gpodderUrl;
  final String podSyncType;

  UserStats({
    required this.userCreated,
    required this.podcastsPlayed,
    required this.timeListened,
    required this.podcastsAdded,
    required this.episodesSaved,
    required this.episodesDownloaded,
    required this.gpodderUrl,
    required this.podSyncType,
  });

  factory UserStats.fromJson(Map<String, dynamic> json) {
    return UserStats(
      userCreated: json['UserCreated'] ?? '',
      podcastsPlayed: json['PodcastsPlayed'] ?? 0,
      timeListened: json['TimeListened'] ?? 0,
      podcastsAdded: json['PodcastsAdded'] ?? 0,
      episodesSaved: json['EpisodesSaved'] ?? 0,
      episodesDownloaded: json['EpisodesDownloaded'] ?? 0,
      gpodderUrl: json['GpodderUrl'] ?? '',
      podSyncType: json['Pod_Sync_Type'] ?? '',
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'UserCreated': userCreated,
      'PodcastsPlayed': podcastsPlayed,
      'TimeListened': timeListened,
      'PodcastsAdded': podcastsAdded,
      'EpisodesSaved': episodesSaved,
      'EpisodesDownloaded': episodesDownloaded,
      'GpodderUrl': gpodderUrl,
      'Pod_Sync_Type': podSyncType,
    };
  }

  // Format time listened from minutes to human readable
  String get formattedTimeListened {
    if (timeListened <= 0) return '0 minutes';
    
    final hours = timeListened ~/ 60;
    final minutes = timeListened % 60;
    
    if (hours == 0) {
      return '$minutes minute${minutes != 1 ? 's' : ''}';
    } else if (minutes == 0) {
      return '$hours hour${hours != 1 ? 's' : ''}';
    } else {
      return '$hours hour${hours != 1 ? 's' : ''} $minutes minute${minutes != 1 ? 's' : ''}';
    }
  }

  // Format user created date
  String get formattedUserCreated {
    try {
      final date = DateTime.parse(userCreated);
      return '${date.day}/${date.month}/${date.year}';
    } catch (e) {
      return userCreated;
    }
  }

  // Get sync status description
  String get syncStatusDescription {
    switch (podSyncType.toLowerCase()) {
      case 'none':
        return 'Not Syncing';
      case 'gpodder':
        if (gpodderUrl == 'http://localhost:8042') {
          return 'Internal gpodder';
        } else {
          return 'External gpodder';
        }
      case 'nextcloud':
        return 'Nextcloud';
      default:
        return 'Unknown sync type';
    }
  }
}