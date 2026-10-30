class UserStats {
  final String userCreated;
  final int podcastsPlayed;
  final int timeListened;
  final int podcastsAdded;
  final int episodesSaved;
  final int episodesDownloaded;
  final String gpodderUrl;
  final String podSyncType;
  // Sent by newer servers only (null on older ones).
  final bool? syncServerEnabled;
  final String? syncServerUrl;
  final String? syncTargetKind;
  final String? syncTargetUrl;

  UserStats({
    required this.userCreated,
    required this.podcastsPlayed,
    required this.timeListened,
    required this.podcastsAdded,
    required this.episodesSaved,
    required this.episodesDownloaded,
    required this.gpodderUrl,
    required this.podSyncType,
    this.syncServerEnabled,
    this.syncServerUrl,
    this.syncTargetKind,
    this.syncTargetUrl,
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
      syncServerEnabled: json['SyncServerEnabled'] as bool?,
      syncServerUrl: json['SyncServerUrl'] as String?,
      syncTargetKind: json['SyncTargetKind'] as String?,
      syncTargetUrl: json['SyncTargetUrl'] as String?,
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
      if (syncServerEnabled != null) 'SyncServerEnabled': syncServerEnabled,
      if (syncServerUrl != null) 'SyncServerUrl': syncServerUrl,
      if (syncTargetKind != null) 'SyncTargetKind': syncTargetKind,
      if (syncTargetUrl != null) 'SyncTargetUrl': syncTargetUrl,
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

  /// Whether any sync is set up.
  bool get isSyncing {
    if (syncServerEnabled != null) {
      return syncServerEnabled! || syncTargetKind != null;
    }
    return podSyncType.toLowerCase() != 'none' && podSyncType.isNotEmpty;
  }

  /// URLs to show under the sync status. The PinePods server's URL is [serverUrl] (the address
  /// this app uses) unless the server has a public URL configured.
  List<String> syncUrls(String? serverUrl) {
    if (syncServerEnabled == null) {
      return gpodderUrl.isEmpty || gpodderUrl == 'http://localhost:8042' ? [] : [gpodderUrl];
    }
    return [
      if (syncServerEnabled!) syncServerUrl ?? serverUrl ?? '',
      ?syncTargetUrl,
    ].where((u) => u.isNotEmpty).toList();
  }

  // Get sync status description
  String get syncStatusDescription {
    if (syncServerEnabled != null) {
      final parts = [
        if (syncServerEnabled!) 'Internal gpodder',
        if (syncTargetKind == 'nextcloud') 'Nextcloud',
        if (syncTargetKind == 'gpodder') 'External gpodder',
      ];
      return parts.isEmpty ? 'Not Syncing' : parts.join(' + ');
    }
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