// lib/entities/youtube_channel.dart

/// Feed URL the server stores for a subscribed YouTube channel.
String youtubeChannelUrl(String channelId) => 'https://www.youtube.com/channel/$channelId';

bool isYoutubeChannelUrl(String url) => url.startsWith('https://www.youtube.com/channel/');

/// A channel from a YouTube search.
class YouTubeChannelResult {
  final String channelId;
  final String name;
  final String description;
  final String thumbnailUrl;
  final int? subscriberCount;
  final int? videoCount;
  final List<YouTubeVideoPreview> recentVideos;

  const YouTubeChannelResult({
    required this.channelId,
    required this.name,
    required this.description,
    required this.thumbnailUrl,
    this.subscriberCount,
    this.videoCount,
    this.recentVideos = const [],
  });

  String get url => youtubeChannelUrl(channelId);

  // The search service answers in camelCase, the server's own yt-dlp search in snake_case.
  factory YouTubeChannelResult.fromJson(Map<String, dynamic> json) {
    final videos = (json['recentVideos'] ?? json['recent_videos']) as List<dynamic>? ?? const [];
    return YouTubeChannelResult(
      channelId: (json['channelId'] ?? json['channel_id'] ?? '') as String,
      name: json['name'] as String? ?? '',
      description: json['description'] as String? ?? '',
      thumbnailUrl: (json['thumbnailUrl'] ?? json['thumbnail_url'] ?? '') as String,
      subscriberCount: _asInt(json['subscriberCount'] ?? json['subscriber_count']),
      videoCount: _asInt(json['videoCount'] ?? json['video_count']),
      recentVideos: videos
          .whereType<Map<String, dynamic>>()
          .map(YouTubeVideoPreview.fromJson)
          .where((v) => v.id.isNotEmpty)
          .toList(),
    );
  }
}

class YouTubeVideoPreview {
  final String id;
  final String title;
  final int? durationSeconds;

  const YouTubeVideoPreview({required this.id, required this.title, this.durationSeconds});

  String get url => 'https://www.youtube.com/watch?v=$id';
  String get thumbnailUrl => 'https://i.ytimg.com/vi/$id/hqdefault.jpg';

  factory YouTubeVideoPreview.fromJson(Map<String, dynamic> json) {
    return YouTubeVideoPreview(
      id: json['id'] as String? ?? '',
      title: json['title'] as String? ?? '',
      durationSeconds: _asInt(json['duration']),
    );
  }
}

/// Which new videos a channel subscription adds. Null means no limit.
class YouTubeVideoFilter {
  final int? minMinutes;
  final int? maxMinutes;
  final String? titleContains;

  const YouTubeVideoFilter({this.minMinutes, this.maxMinutes, this.titleContains});

  factory YouTubeVideoFilter.fromJson(Map<String, dynamic> json) {
    return YouTubeVideoFilter(
      minMinutes: _asInt(json['min_minutes']),
      maxMinutes: _asInt(json['max_minutes']),
      titleContains: json['title_contains'] as String?,
    );
  }
}

int? _asInt(dynamic v) {
  if (v is int) return v;
  if (v is num) return v.round();
  if (v is String) return int.tryParse(v);
  return null;
}
