class PinepodsEpisode {
  final String podcastName;
  final String episodeTitle;
  final String episodePubDate;
  final String episodeDescription;
  final String episodeArtwork;
  final String episodeUrl;
  final int episodeDuration;
  final int? listenDuration;
  final int episodeId;
  final bool completed;
  final bool saved;
  final bool queued;
  final bool downloaded;
  final bool isYoutube;

  PinepodsEpisode({
    required this.podcastName,
    required this.episodeTitle,
    required this.episodePubDate,
    required this.episodeDescription,
    required this.episodeArtwork,
    required this.episodeUrl,
    required this.episodeDuration,
    this.listenDuration,
    required this.episodeId,
    required this.completed,
    required this.saved,
    required this.queued,
    required this.downloaded,
    required this.isYoutube,
  });

  factory PinepodsEpisode.fromJson(Map<String, dynamic> json) {
    return PinepodsEpisode(
      podcastName: json['podcastname'] ?? '',
      episodeTitle: json['episodetitle'] ?? '',
      episodePubDate: json['episodepubdate'] ?? '',
      episodeDescription: json['episodedescription'] ?? '',
      episodeArtwork: json['episodeartwork'] ?? '',
      episodeUrl: json['episodeurl'] ?? '',
      episodeDuration: json['episodeduration'] ?? 0,
      listenDuration: json['listenduration'],
      episodeId: json['episodeid'] ?? 0,
      completed: json['completed'] ?? false,
      saved: json['saved'] ?? false,
      queued: json['queued'] ?? false,
      downloaded: json['downloaded'] ?? false,
      isYoutube: json['is_youtube'] ?? false,
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'podcastname': podcastName,
      'episodetitle': episodeTitle,
      'episodepubdate': episodePubDate,
      'episodedescription': episodeDescription,
      'episodeartwork': episodeArtwork,
      'episodeurl': episodeUrl,
      'episodeduration': episodeDuration,
      'listenduration': listenDuration,
      'episodeid': episodeId,
      'completed': completed,
      'saved': saved,
      'queued': queued,
      'downloaded': downloaded,
      'is_youtube': isYoutube,
    };
  }

  /// Format duration from seconds to MM:SS or HH:MM:SS
  String get formattedDuration {
    if (episodeDuration <= 0) return '0:00';
    
    final hours = episodeDuration ~/ 3600;
    final minutes = (episodeDuration % 3600) ~/ 60;
    final seconds = episodeDuration % 60;
    
    if (hours > 0) {
      return '${hours.toString().padLeft(1, '0')}:${minutes.toString().padLeft(2, '0')}:${seconds.toString().padLeft(2, '0')}';
    } else {
      return '${minutes.toString().padLeft(1, '0')}:${seconds.toString().padLeft(2, '0')}';
    }
  }

  /// Get progress percentage (0-100)
  double get progressPercentage {
    if (episodeDuration <= 0 || listenDuration == null) return 0.0;
    return (listenDuration! / episodeDuration * 100).clamp(0.0, 100.0);
  }

  /// Check if episode has been started (has some listen duration)
  bool get isStarted {
    return listenDuration != null && listenDuration! > 0;
  }

  /// Format listen duration from seconds to MM:SS or HH:MM:SS
  String get formattedListenDuration {
    if (listenDuration == null || listenDuration! <= 0) return '0:00';
    
    final duration = listenDuration!;
    final hours = duration ~/ 3600;
    final minutes = (duration % 3600) ~/ 60;
    final seconds = duration % 60;
    
    if (hours > 0) {
      return '${hours.toString().padLeft(1, '0')}:${minutes.toString().padLeft(2, '0')}:${seconds.toString().padLeft(2, '0')}';
    } else {
      return '${minutes.toString().padLeft(1, '0')}:${seconds.toString().padLeft(2, '0')}';
    }
  }

  /// Format the publish date to a more readable format
  String get formattedPubDate {
    try {
      final date = DateTime.parse(episodePubDate);
      final now = DateTime.now();
      final difference = now.difference(date);
      
      if (difference.inDays == 0) {
        return 'Today';
      } else if (difference.inDays == 1) {
        return 'Yesterday';
      } else if (difference.inDays < 7) {
        return '${difference.inDays} days ago';
      } else if (difference.inDays < 30) {
        final weeks = (difference.inDays / 7).floor();
        return weeks == 1 ? '1 week ago' : '$weeks weeks ago';
      } else {
        final months = (difference.inDays / 30).floor();
        return months == 1 ? '1 month ago' : '$months months ago';
      }
    } catch (e) {
      return episodePubDate;
    }
  }
}