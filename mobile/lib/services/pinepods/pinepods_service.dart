// Create this file at lib/services/pinepods/pinepods_service.dart

import 'dart:convert';
import 'package:flutter/foundation.dart';
import 'package:http/http.dart' as http;
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/entities/pinepods_search.dart';
import 'package:pinepods_mobile/entities/user_stats.dart';
import 'package:pinepods_mobile/entities/home_data.dart';
import 'package:pinepods_mobile/entities/podcast.dart';

/// Debug-only logger: in release builds this compiles out, so request URLs and
/// other diagnostics never reach the device console in production.
void _devLog(Object? message) {
  if (kDebugMode) {
    print(message);
  }
}

class PinepodsService {
  String? _server;
  String? _apiKey;

  /// Shared by default across every PinepodsService instance (widgets each
  /// construct their own `PinepodsService()`) so requests reuse keep-alive
  /// connections instead of paying a fresh TCP+TLS handshake per call - which
  /// is what happens with the top-level http.get/post/put functions, since
  /// each of those creates and immediately closes its own client.
  static final http.Client _sharedClient = http.Client();
  static const Duration _defaultTimeout = Duration(seconds: 15);

  final http.Client _client;
  final Duration _timeout;

  /// [client] and [timeout] are only ever overridden in tests - production
  /// code always uses the no-arg constructor, sharing [_sharedClient]. Not
  /// annotated @visibleForTesting since (unlike a dedicated named
  /// constructor) that would also flag the plain `PinepodsService()` calls
  /// used throughout the app.
  PinepodsService({http.Client? client, Duration? timeout})
      : _client = client ?? _sharedClient,
        _timeout = timeout ?? _defaultTimeout;

  Future<http.Response> _get(Uri url, {Map<String, String>? headers}) {
    return _client.get(url, headers: headers).timeout(_timeout);
  }

  Future<http.Response> _post(Uri url, {Map<String, String>? headers, Object? body}) {
    return _client.post(url, headers: headers, body: body).timeout(_timeout);
  }

  Future<http.Response> _put(Uri url, {Map<String, String>? headers, Object? body}) {
    return _client.put(url, headers: headers, body: body).timeout(_timeout);
  }

  // Method to initialize with existing credentials
  void initializeWithCredentials(String server, String apiKey) {
    _server = server;
    _apiKey = apiKey;
  }

  String get apiKey => _apiKey ?? '';

  Future<bool> verifyPinepodsInstance(String serverUrl) async {
    // Normalize the URL by removing trailing slashes
    final normalizedUrl = serverUrl.trim().replaceAll(RegExp(r'/$'), '');
    final url = Uri.parse('$normalizedUrl/api/pinepods_check');

    try {
      final response = await _get(url);

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data['pinepods_instance'] == true;
      }
      return false;
    } catch (e) {
      _devLog('Error verifying PinePods instance: $e');
      return false;
    }
  }

  Future<bool> login(String serverUrl, String username, String password) async {
    // Normalize the URL by removing trailing slashes
    final normalizedUrl = serverUrl.trim().replaceAll(RegExp(r'/$'), '');
    _server = normalizedUrl;

    // Create Basic Auth header
    final credentials = base64Encode(utf8.encode('$username:$password'));
    final authHeader = 'Basic $credentials';

    final url = Uri.parse('$normalizedUrl/api/data/get_key');

    try {
      final response = await _get(
        url,
        headers: {'Authorization': authHeader},
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        _apiKey = data['retrieved_key'];

        // Verify the API key
        return await verifyApiKey();
      }
      return false;
    } catch (e) {
      _devLog('Login error: $e');
      return false;
    }
  }

  Future<bool> verifyApiKey() async {
    if (_server == null || _apiKey == null) {
      return false;
    }

    final url = Uri.parse('$_server/api/data/verify_key');

    try {
      final response = await _get(url, headers: {'Api-Key': _apiKey!});

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data['status'] == 'success';
      }
      return false;
    } catch (e) {
      _devLog('Error verifying API key: $e');
      return false;
    }
  }

  // Add method to fetch podcasts from PinePods
  Future<List<Map<String, dynamic>>> fetchPodcasts() async {
    if (_server == null || _apiKey == null) {
      return [];
    }

    // This endpoint would need to be implemented in your PinePods backend
    final url = Uri.parse('$_server/api/data/podcasts');

    try {
      final response = await _get(url, headers: {'Api-Key': _apiKey!});

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body) as List;
        return data.cast<Map<String, dynamic>>();
      }
      return [];
    } catch (e) {
      _devLog('Error fetching podcasts: $e');
      return [];
    }
  }

  // Get user's subscribed podcasts using return_pods endpoint
  Future<List<Podcast>> getUserPodcasts(int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/return_pods/$userId');
    _devLog('Making API call to: $url');

    try {
      final response = await _get(url, headers: {'Api-Key': _apiKey!});

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        final List<dynamic> podsData = data['pods'] ?? [];

        List<Podcast> podcasts = [];
        for (var podData in podsData) {
          // Store episode count in the podcast for display purposes
          // Don't create placeholder episodes - that's wasteful and causes memory issues
          final episodeCount = podData['episodecount'] ?? 0;

          podcasts.add(
            Podcast(
              id: podData['podcastid'],
              title: podData['podcastname'] ?? '',
              description: podData['description'] ?? '',
              imageUrl: podData['artworkurl'] ?? '',
              thumbImageUrl: podData['artworkurl'] ?? '',
              url: podData['feedurl'] ?? '',
              link: podData['websiteurl'] ?? '',
              copyright: podData['author'] ?? '',
              guid: podData['feedurl'] ?? '',
              isFavorite: podData['is_favorite'] ?? false,
              // Empty episodes list - episodes are loaded separately when needed
              episodes: [],
              // Store episode count for display (if Podcast model supports it)
              // Otherwise the count can be fetched when viewing the podcast
            ),
          );
        }

        return podcasts;
      } else {
        throw Exception('Failed to get user podcasts: ${response.statusCode}');
      }
    } catch (e) {
      _devLog('Error getting user podcasts: $e');
      rethrow;
    }
  }

  // Get user's subscribed podcasts with categories for filtering
  Future<Map<String, dynamic>> getUserPodcastsWithCategories(int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/return_pods/$userId');

    try {
      final response = await _get(url, headers: {'Api-Key': _apiKey!});

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        final List<dynamic> podsData = data['pods'] ?? [];

        List<Podcast> podcasts = [];
        Map<int, List<String>> podcastCategories = {};

        for (var podData in podsData) {
          final podcastId = podData['podcastid'] as int;

          // Parse categories
          List<String> categories = [];
          if (podData['categories'] != null) {
            if (podData['categories'] is Map) {
              categories = (podData['categories'] as Map)
                  .values
                  .map((v) => v.toString().trim())
                  .where((c) => c.isNotEmpty)
                  .toList();
            } else if (podData['categories'] is String) {
              categories = (podData['categories'] as String)
                  .split(',')
                  .map((c) => c.trim())
                  .where((c) => c.isNotEmpty)
                  .toList();
            }
          }
          podcastCategories[podcastId] = categories;

          podcasts.add(
            Podcast(
              id: podcastId,
              title: podData['podcastname'] ?? '',
              description: podData['description'] ?? '',
              imageUrl: podData['artworkurl'] ?? '',
              thumbImageUrl: podData['artworkurl'] ?? '',
              url: podData['feedurl'] ?? '',
              link: podData['websiteurl'] ?? '',
              copyright: podData['author'] ?? '',
              guid: podData['feedurl'] ?? '',
              isFavorite: podData['is_favorite'] ?? false,
              episodes: [],
            ),
          );
        }

        return {
          'podcasts': podcasts,
          'categories': podcastCategories,
        };
      } else {
        throw Exception('Failed to get user podcasts: ${response.statusCode}');
      }
    } catch (e) {
      _devLog('Error getting user podcasts with categories: $e');
      rethrow;
    }
  }

  // Get recent episodes (last 30 days)
  Future<EpisodePage> getRecentEpisodes(int userId, {int limit = 50, int offset = 0}) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated - server or API key missing');
    }

    final url = Uri.parse('$_server/api/data/return_episodes/$userId').replace(
      queryParameters: {
        'limit': limit.toString(),
        'offset': offset.toString(),
      },
    );

    try {
      final response = await _get(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);

        if (data is Map && data['episodes'] != null) {
          final episodesList = data['episodes'] as List;
          final episodes = episodesList
              .map((episode) => PinepodsEpisode.fromJson(episode))
              .toList();
          final total = (data['total'] as num?)?.toInt() ?? episodes.length;
          return EpisodePage(episodes: episodes, total: total);
        } else if (data is List) {
          final episodes = data
              .map((episode) => PinepodsEpisode.fromJson(episode))
              .toList();
          return EpisodePage(episodes: episodes, total: episodes.length);
        } else {
          return EpisodePage(episodes: [], total: 0);
        }
      } else {
        throw Exception(
          'Failed to fetch recent episodes: ${response.statusCode} ${response.reasonPhrase}',
        );
      }
    } catch (e) {
      _devLog('Error fetching recent episodes: $e');
      throw Exception('Error fetching recent episodes: $e');
    }
  }

  // Set credentials (used when user logs in)
  void setCredentials(String server, String apiKey) {
    _server = server.trim().replaceAll(RegExp(r'/$'), '');
    _apiKey = apiKey;
  }

  // Check if user is authenticated
  bool get isAuthenticated => _server != null && _apiKey != null;

  // Get server URL
  String? get server => _server;

  // Check if episode exists in database
  Future<bool> checkEpisodeInDb(
    int userId,
    String episodeTitle,
    String episodeUrl,
  ) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/check_episode_in_db');

    try {
      final requestBody = jsonEncode({
        'user_id': userId,
        'episode_title': episodeTitle,
        'episode_url': episodeUrl,
      });

      final response = await _post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data['exists'] == true;
      }
      return false;
    } catch (e) {
      _devLog('Error checking episode in DB: $e');
      return false;
    }
  }

  // Get episode ID from title and URL
  Future<int> getEpisodeId(
    int userId,
    String episodeTitle,
    String episodeUrl,
    bool isYoutube,
  ) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse(
      '$_server/api/data/get_episode_id_ep_name?user_id=$userId&episode_url=${Uri.encodeComponent(episodeUrl)}&episode_title=${Uri.encodeComponent(episodeTitle)}&is_youtube=$isYoutube',
    );

    try {
      final response = await _get(url, headers: {'Api-Key': _apiKey!});

      if (response.statusCode == 200) {
        // Parse the response as a plain integer
        final episodeId = int.tryParse(response.body.trim()) ?? 0;
        return episodeId;
      }
      return 0;
    } catch (e) {
      _devLog('Error getting episode ID: $e');
      return 0;
    }
  }

  // Add episode to history
  Future<bool> addHistory(
    int episodeId,
    double episodePos,
    int userId,
    bool isYoutube,
  ) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/record_podcast_history');
    _devLog('Making API call to: $url');

    try {
      final requestBody = jsonEncode({
        'episode_id': episodeId,
        'episode_pos': episodePos,
        'user_id': userId,
        'is_youtube': isYoutube,
      });

      final response = await _post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );

      // History API response received
      return response.statusCode == 200;
    } catch (e) {
      _devLog('Error adding history: $e');
      return false;
    }
  }

  // Queue episode
  Future<bool> queueEpisode(int episodeId, int userId, bool isYoutube) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/queue_pod');
    _devLog('Making API call to: $url');

    try {
      final requestBody = jsonEncode({
        'episode_id': episodeId,
        'user_id': userId,
        'is_youtube': isYoutube,
      });

      final response = await _post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );

      // Queue API response received
      return response.statusCode == 200;
    } catch (e) {
      _devLog('Error queueing episode: $e');
      return false;
    }
  }

  // Increment played count
  Future<bool> incrementPlayed(int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/increment_played/$userId');
    _devLog('Making API call to: $url');

    try {
      final response = await _put(url, headers: {'Api-Key': _apiKey!});

      _devLog(
        'Increment played response: ${response.statusCode} - ${response.body}',
      );
      return response.statusCode == 200;
    } catch (e) {
      _devLog('Error incrementing played: $e');
      return false;
    }
  }

  // Toggle a podcast's favorite status (podcast-level favorite, matching web).
  Future<bool> togglePodcastFavorite(
    int podcastId,
    int userId,
    bool isFavorite,
  ) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/podcast/toggle_favorite');
    final requestBody = jsonEncode({
      'user_id': userId,
      'podcast_id': podcastId,
      'is_favorite': isFavorite,
    });

    try {
      final response = await http.put(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );
      return response.statusCode == 200;
    } catch (e) {
      _devLog('Error toggling podcast favorite: $e');
      return false;
    }
  }

  // Get a podcast's favorite status.
  Future<bool> getPodcastFavoriteStatus(int podcastId, int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/podcast/favorite_status');
    final requestBody = jsonEncode({
      'user_id': userId,
      'podcast_id': podcastId,
    });

    try {
      final response = await http.post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );
      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data['is_favorite'] ?? false;
      }
      return false;
    } catch (e) {
      _devLog('Error getting podcast favorite status: $e');
      return false;
    }
  }

  // Get podcast ID from episode
  Future<int> getPodcastIdFromEpisode(
    int episodeId,
    int userId,
    bool isYoutube,
  ) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse(
      '$_server/api/data/get_podcast_id_from_ep/$episodeId',
    );

    try {
      final response = await _get(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data['podcast_id'] ?? 0;
      }
      return 0;
    } catch (e) {
      _devLog('Error getting podcast ID: $e');
      return 0;
    }
  }

  // Get play episode details (playback speed, skip times)
  Future<PlayEpisodeDetails> getPlayEpisodeDetails(
    int userId,
    int podcastId,
    bool isYoutube,
  ) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/get_play_episode_details');
    _devLog('Making API call to: $url');

    try {
      final requestBody = jsonEncode({
        'user_id': userId,
        'podcast_id': podcastId,
        'is_youtube': isYoutube,
      });

      final response = await _post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );

      _devLog(
        'Play episode details response: ${response.statusCode} - ${response.body}',
      );
      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return PlayEpisodeDetails(
          playbackSpeed: (data['playback_speed'] as num?)?.toDouble() ?? 1.0,
          startSkip: data['start_skip'] ?? 0,
          endSkip: data['end_skip'] ?? 0,
        );
      }
      return PlayEpisodeDetails(playbackSpeed: 1.0, startSkip: 0, endSkip: 0);
    } catch (e) {
      _devLog('Error getting play episode details: $e');
      return PlayEpisodeDetails(playbackSpeed: 1.0, startSkip: 0, endSkip: 0);
    }
  }

  // Get per-podcast silence-trim settings (#727). Returns (enabled, threshold).
  Future<SilenceTrimSettings> getSilenceTrim(int userId, int podcastId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }
    final url = Uri.parse(
      '$_server/api/data/get_silence_trim?podcast_id=$podcastId&user_id=$userId',
    );
    try {
      final response = await _get(url, headers: {'Api-Key': _apiKey!});
      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return SilenceTrimSettings(
          enabled: data['enabled'] == true,
          threshold: data['threshold'] ?? 2,
        );
      }
    } catch (e) {
      _devLog('Error getting silence trim: $e');
    }
    return const SilenceTrimSettings(enabled: false, threshold: 2);
  }

  // Get auto-skip segments (silence #727) for an episode, in seconds.
  Future<List<SkipSegment>> getEpisodeSkipSegments(int episodeId, int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }
    final url = Uri.parse(
      '$_server/api/data/episode_skip_segments?episode_id=$episodeId&user_id=$userId',
    );
    try {
      final response = await _get(url, headers: {'Api-Key': _apiKey!});
      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        final segments = (data['segments'] as List?) ?? [];
        return segments
            .map((s) => SkipSegment(
                  segmentId: (s['segment_id'] as num?)?.toInt() ?? 0,
                  kind: s['kind'] ?? '',
                  startTime: (s['start_time'] as num?)?.toDouble() ?? 0.0,
                  endTime: (s['end_time'] as num?)?.toDouble() ?? 0.0,
                  source: s['source'] as String?,
                  status: s['status'] as String?,
                ))
            .toList();
      }
    } catch (e) {
      _devLog('Error getting skip segments: $e');
    }
    return const [];
  }

  // --- AI features (#726 transcripts / #790 ad-block) --------------------
  // All endpoints live under /api/data and use the Api-Key header. The server
  // resolves per-user ad status; mobile just trusts it (never re-derives it).

  // GET /api/data/ai_status — gate all AI UI on `available`.
  Future<AiStatus> getAiStatus() async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }
    final url = Uri.parse('$_server/api/data/ai_status');
    try {
      final response = await http.get(url, headers: {'Api-Key': _apiKey!});
      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return AiStatus(
          available: data['available'] == true,
          transcriptionReady: data['transcription_ready'] == true,
          adRemovalReady: data['ad_removal_ready'] == true,
        );
      }
    } catch (e) {
      _devLog('Error getting AI status: $e');
    }
    return const AiStatus();
  }

  // POST /api/data/detect_ads — trigger ad detection (async background job).
  // Returns true if the request was accepted (503 => AI unavailable).
  Future<bool> detectAds(int episodeId, int userId, {bool force = false}) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }
    final url = Uri.parse('$_server/api/data/detect_ads');
    try {
      final response = await http.post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: jsonEncode({
          'episode_id': episodeId,
          'user_id': userId,
          'force': force,
        }),
      );
      return response.statusCode == 200;
    } catch (e) {
      _devLog('Error detecting ads: $e');
      return false;
    }
  }

  // POST /api/data/adjust_ad_segment_review — per-user confirm/deny of an ad.
  // `status` is "confirmed" or "rejected".
  Future<bool> adjustAdSegmentReview(
      int segmentId, int userId, String status) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }
    final url = Uri.parse('$_server/api/data/adjust_ad_segment_review');
    try {
      final response = await http.post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: jsonEncode({
          'segment_id': segmentId,
          'user_id': userId,
          'status': status,
        }),
      );
      return response.statusCode == 200;
    } catch (e) {
      _devLog('Error adjusting ad segment review: $e');
      return false;
    }
  }

  // GET /api/data/episode_transcript — stored AI transcript (with cue segments).
  Future<StoredTranscript?> getEpisodeTranscript(
      int episodeId, int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }
    final url = Uri.parse(
      '$_server/api/data/episode_transcript?episode_id=$episodeId&user_id=$userId',
    );
    try {
      final response = await http.get(url, headers: {'Api-Key': _apiKey!});
      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        final t = data['transcript'];
        if (t == null) return null;
        return StoredTranscript(
          source: t['source'] ?? '',
          language: t['language'] as String?,
          model: t['model'] as String?,
          status: t['status'] ?? '',
          fullText: t['full_text'] as String?,
          segments: t['segments'] as String?,
        );
      }
    } catch (e) {
      _devLog('Error getting episode transcript: $e');
    }
    return null;
  }

  // POST /api/data/transcribe_episode — trigger transcription (async).
  Future<bool> transcribeEpisode(int episodeId, int userId,
      {bool force = false}) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }
    final url = Uri.parse('$_server/api/data/transcribe_episode');
    try {
      final response = await http.post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: jsonEncode({
          'episode_id': episodeId,
          'user_id': userId,
          'force': force,
        }),
      );
      return response.statusCode == 200;
    } catch (e) {
      _devLog('Error transcribing episode: $e');
      return false;
    }
  }

  // --- Per-podcast AI toggles -------------------------------------------
  // Shared GET helper: /api/data/<path>?podcast_id=&user_id= => {enabled}.
  Future<bool> _getPodcastEnabled(
      String path, int userId, int podcastId, bool fallback) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }
    final url = Uri.parse(
      '$_server/api/data/$path?podcast_id=$podcastId&user_id=$userId',
    );
    try {
      final response = await http.get(url, headers: {'Api-Key': _apiKey!});
      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data['enabled'] == true;
      }
    } catch (e) {
      _devLog('Error getting $path: $e');
    }
    return fallback;
  }

  // Shared POST helper: /api/data/<path> with {podcast_id,user_id,enabled}.
  Future<bool> _setPodcastEnabled(
      String path, int userId, int podcastId, bool enabled) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }
    final url = Uri.parse('$_server/api/data/$path');
    try {
      final response = await http.post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: jsonEncode({
          'podcast_id': podcastId,
          'user_id': userId,
          'enabled': enabled,
        }),
      );
      return response.statusCode == 200;
    } catch (e) {
      _devLog('Error setting $path: $e');
      return false;
    }
  }

  Future<bool> getAutoTranscribe(int userId, int podcastId) =>
      _getPodcastEnabled('get_auto_transcribe', userId, podcastId, false);
  Future<bool> adjustAutoTranscribe(int userId, int podcastId, bool enabled) =>
      _setPodcastEnabled('adjust_auto_transcribe', userId, podcastId, enabled);

  Future<bool> getAutoAdDetect(int userId, int podcastId) =>
      _getPodcastEnabled('get_auto_ad_detect', userId, podcastId, false);
  Future<bool> adjustAutoAdDetect(int userId, int podcastId, bool enabled) =>
      _setPodcastEnabled('adjust_auto_ad_detect', userId, podcastId, enabled);

  // Server default is TRUE (auto-activate ad-skip).
  Future<bool> getAdSkipAutoActivate(int userId, int podcastId) =>
      _getPodcastEnabled('get_ad_skip_auto_activate', userId, podcastId, true);
  Future<bool> adjustAdSkipAutoActivate(
          int userId, int podcastId, bool enabled) =>
      _setPodcastEnabled(
          'adjust_ad_skip_auto_activate', userId, podcastId, enabled);

  // Record listen duration for episode
  Future<bool> recordListenDuration(
    int episodeId,
    int userId,
    double listenDuration,
    bool isYoutube,
  ) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/record_listen_duration');
    _devLog('Making API call to: $url');

    try {
      final requestBody = jsonEncode({
        'episode_id': episodeId,
        'user_id': userId,
        'listen_duration': listenDuration,
        'is_youtube': isYoutube,
      });

      final response = await _post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );

      _devLog(
        'Record listen duration response: ${response.statusCode} - ${response.body}',
      );
      return response.statusCode == 200;
    } catch (e) {
      _devLog('Error recording listen duration: $e');
      return false;
    }
  }

  // Update the stored duration for an episode. Mirrors the web frontend, which
  // corrects the episode duration to the real decoded length the first time an
  // episode is played (feeds frequently ship missing/zero itunes:duration).
  Future<bool> updateEpisodeDuration(
    int episodeId,
    int newDuration,
    bool isYoutube,
  ) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/update_episode_duration');
    _devLog('Making API call to: $url');

    try {
      final requestBody = jsonEncode({
        'episode_id': episodeId,
        'new_duration': newDuration,
        'is_youtube': isYoutube,
      });

      final response = await _post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );

      _devLog(
        'Update episode duration response: ${response.statusCode} - ${response.body}',
      );
      return response.statusCode == 200;
    } catch (e) {
      _devLog('Error updating episode duration: $e');
      return false;
    }
  }

  // Increment listen time for user stats
  Future<bool> incrementListenTime(int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/increment_listen_time/$userId');
    _devLog('Making API call to: $url');

    try {
      final response = await _put(url, headers: {'Api-Key': _apiKey!});

      _devLog(
        'Increment listen time response: ${response.statusCode} - ${response.body}',
      );
      return response.statusCode == 200;
    } catch (e) {
      _devLog('Error incrementing listen time: $e');
      return false;
    }
  }

  // Save episode
  Future<bool> saveEpisode(int episodeId, int userId, bool isYoutube) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/save_episode');
    _devLog('Making API call to: $url');

    try {
      final requestBody = jsonEncode({
        'episode_id': episodeId,
        'user_id': userId,
        'is_youtube': isYoutube,
      });

      final response = await _post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );

      // Save episode API response received
      return response.statusCode == 200;
    } catch (e) {
      _devLog('Error saving episode: $e');
      return false;
    }
  }

  // Remove saved episode
  Future<bool> removeSavedEpisode(
    int episodeId,
    int userId,
    bool isYoutube,
  ) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/remove_saved_episode');
    _devLog('Making API call to: $url');

    try {
      final requestBody = jsonEncode({
        'episode_id': episodeId,
        'user_id': userId,
        'is_youtube': isYoutube,
      });

      final response = await _post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );

      _devLog(
        'Remove saved episode response: ${response.statusCode} - ${response.body}',
      );
      return response.statusCode == 200;
    } catch (e) {
      _devLog('Error removing saved episode: $e');
      return false;
    }
  }

  // Download episode to server
  Future<bool> downloadEpisode(
    int episodeId,
    int userId,
    bool isYoutube,
  ) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/download_podcast');
    _devLog('Making API call to: $url');

    try {
      final requestBody = jsonEncode({
        'episode_id': episodeId,
        'user_id': userId,
        'is_youtube': isYoutube,
      });

      final response = await _post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );

      _devLog(
        'Download episode response: ${response.statusCode} - ${response.body}',
      );
      return response.statusCode == 200;
    } catch (e) {
      _devLog('Error downloading episode: $e');
      return false;
    }
  }

  /// Create a public share link for an episode. Returns the share `url_key`
  /// (append to `<server>/shared_episode/<url_key>` for the shareable URL).
  Future<String> createShareLink(int episodeId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/share_episode/$episodeId');
    _devLog('Making API call to: $url');

    final response = await http.post(
      url,
      headers: {'Api-Key': _apiKey!},
    );

    if (response.statusCode == 200) {
      final data = jsonDecode(response.body);
      final urlKey = data['url_key'];
      if (urlKey == null || (urlKey is String && urlKey.isEmpty)) {
        throw Exception('Server did not return a share link');
      }
      return urlKey.toString();
    } else {
      throw Exception('Failed to create share link: ${response.statusCode}');
    }
  }

  Future<List<DownloadTask>> getDownloadActivity(int userId) async {
    if (_server == null || _apiKey == null) return [];
    final url = Uri.parse('$_server/api/tasks/user/$userId');
    try {
      final response = await _get(url, headers: {'Api-Key': _apiKey!});
      if (response.statusCode == 200) {
        final data = jsonDecode(response.body) as List;
        return data
            .cast<Map<String, dynamic>>()
            .map(DownloadTask.fromJson)
            .where((t) => t.taskType == 'download_episode' || t.taskType == 'podcast_download' || t.taskType == 'download_all_episodes')
            .toList();
      }
      return [];
    } catch (e) {
      return [];
    }
  }

  // Delete downloaded episode from server
  Future<bool> deleteEpisode(int episodeId, int userId, bool isYoutube) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/delete_episode');
    _devLog('Making API call to: $url');

    try {
      final requestBody = jsonEncode({
        'episode_id': episodeId,
        'user_id': userId,
        'is_youtube': isYoutube,
      });

      final response = await _post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );

      _devLog(
        'Delete episode response: ${response.statusCode} - ${response.body}',
      );
      return response.statusCode == 200;
    } catch (e) {
      _devLog('Error deleting episode: $e');
      return false;
    }
  }

  // Mark episode as completed
  Future<bool> markEpisodeCompleted(
    int episodeId,
    int userId,
    bool isYoutube,
  ) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/mark_episode_completed');
    _devLog('Making API call to: $url');

    try {
      final requestBody = jsonEncode({
        'episode_id': episodeId,
        'user_id': userId,
        'is_youtube': isYoutube,
      });

      final response = await _post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );

      _devLog(
        'Mark completed response: ${response.statusCode} - ${response.body}',
      );
      return response.statusCode == 200;
    } catch (e) {
      _devLog('Error marking episode completed: $e');
      return false;
    }
  }

  // Mark episode as uncompleted
  Future<bool> markEpisodeUncompleted(
    int episodeId,
    int userId,
    bool isYoutube,
  ) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/mark_episode_uncompleted');
    _devLog('Making API call to: $url');

    try {
      final requestBody = jsonEncode({
        'episode_id': episodeId,
        'user_id': userId,
        'is_youtube': isYoutube,
      });

      final response = await _post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );

      _devLog(
        'Mark uncompleted response: ${response.statusCode} - ${response.body}',
      );
      return response.statusCode == 200;
    } catch (e) {
      _devLog('Error marking episode uncompleted: $e');
      return false;
    }
  }

  // Remove episode from queue
  Future<bool> removeQueuedEpisode(
    int episodeId,
    int userId,
    bool isYoutube,
  ) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/remove_queued_pod');
    _devLog('Making API call to: $url');

    try {
      final requestBody = jsonEncode({
        'episode_id': episodeId,
        'user_id': userId,
        'is_youtube': isYoutube,
      });

      final response = await _post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );

      _devLog(
        'Remove queued response: ${response.statusCode} - ${response.body}',
      );
      return response.statusCode == 200;
    } catch (e) {
      _devLog('Error removing queued episode: $e');
      return false;
    }
  }

  // Get user history
  Future<List<PinepodsEpisode>> getUserHistory(int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/user_history/$userId');
    _devLog('Making API call to: $url');

    try {
      final response = await _get(url, headers: {'Api-Key': _apiKey!});

      // User history API response received

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        final episodesList = data['data'] as List<dynamic>? ?? [];

        return episodesList.map((episodeData) {
          return PinepodsEpisode(
            podcastName: episodeData['podcastname'] ?? '',
            episodeTitle: episodeData['episodetitle'] ?? '',
            episodePubDate: episodeData['episodepubdate'] ?? '',
            episodeDescription: episodeData['episodedescription'] ?? '',
            episodeArtwork: episodeData['episodeartwork'] ?? '',
            episodeUrl: episodeData['episodeurl'] ?? '',
            episodeDuration: episodeData['episodeduration'] ?? 0,
            listenDuration: episodeData['listenduration'] ?? 0,
            episodeId: episodeData['episodeid'] ?? 0,
            completed: episodeData['completed'] ?? false,
            saved: episodeData['saved'] ?? false,
            queued: episodeData['queued'] ?? false,
            downloaded: episodeData['downloaded'] ?? false,
            isYoutube: episodeData['is_youtube'] ?? false,
            listenDate: episodeData['listendate'],
          );
        }).toList();
      } else {
        throw Exception('Failed to load user history: ${response.statusCode}');
      }
    } catch (e) {
      _devLog('Error getting user history: $e');
      rethrow;
    }
  }

  // Get user history with pagination support
  Future<EpisodePage> getUserHistoryPaged(
    int userId, {
    int limit = 50,
    int offset = 0,
    String sortBy = 'date',
    String sortOrder = 'desc',
    String filter = 'all',
  }) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/user_history/$userId').replace(
      queryParameters: {
        'limit': limit.toString(),
        'offset': offset.toString(),
        'sort_by': sortBy,
        'sort_order': sortOrder,
        'filter': filter,
      },
    );

    try {
      final response = await _get(url, headers: {'Api-Key': _apiKey!});

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        final total = (data['total'] as num?)?.toInt() ?? 0;
        final episodesList = data['data'] as List<dynamic>? ?? [];

        final episodes = episodesList.map((episodeData) {
          return PinepodsEpisode(
            podcastName: episodeData['podcastname'] ?? '',
            episodeTitle: episodeData['episodetitle'] ?? '',
            episodePubDate: episodeData['episodepubdate'] ?? '',
            episodeDescription: episodeData['episodedescription'] ?? '',
            episodeArtwork: episodeData['episodeartwork'] ?? '',
            episodeUrl: episodeData['episodeurl'] ?? '',
            episodeDuration: episodeData['episodeduration'] ?? 0,
            listenDuration: episodeData['listenduration'] ?? 0,
            episodeId: episodeData['episodeid'] ?? 0,
            completed: episodeData['completed'] ?? false,
            saved: episodeData['saved'] ?? false,
            queued: episodeData['queued'] ?? false,
            downloaded: episodeData['downloaded'] ?? false,
            isYoutube: episodeData['is_youtube'] ?? false,
            listenDate: episodeData['listendate'],
          );
        }).toList();

        return EpisodePage(episodes: episodes, total: total);
      } else {
        throw Exception('Failed to load user history: ${response.statusCode}');
      }
    } catch (e) {
      _devLog('Error getting paged user history: $e');
      rethrow;
    }
  }

  // Get queued episodes
  Future<List<PinepodsEpisode>> getQueuedEpisodes(int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse(
      '$_server/api/data/get_queued_episodes?user_id=$userId',
    );
    _devLog('Making API call to: $url');

    try {
      final response = await _get(url, headers: {'Api-Key': _apiKey!});

      _devLog(
        'Queued episodes response: ${response.statusCode} - ${response.body}',
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        final episodesList = data['data'] as List<dynamic>? ?? [];

        return episodesList.map((episodeData) {
          return PinepodsEpisode(
            podcastName: episodeData['podcastname'] ?? '',
            episodeTitle: episodeData['episodetitle'] ?? '',
            episodePubDate: episodeData['episodepubdate'] ?? '',
            episodeDescription: episodeData['episodedescription'] ?? '',
            episodeArtwork: episodeData['episodeartwork'] ?? '',
            episodeUrl: episodeData['episodeurl'] ?? '',
            episodeDuration: episodeData['episodeduration'] ?? 0,
            listenDuration: episodeData['listenduration'] ?? 0,
            episodeId: episodeData['episodeid'] ?? 0,
            completed: episodeData['completed'] ?? false,
            saved: episodeData['saved'] ?? false,
            queued:
                episodeData['queued'] ??
                true, // Should always be true for queued episodes
            downloaded: episodeData['downloaded'] ?? false,
            isYoutube: episodeData['is_youtube'] ?? false,
          );
        }).toList();
      } else {
        throw Exception(
          'Failed to load queued episodes: ${response.statusCode}',
        );
      }
    } catch (e) {
      _devLog('Error getting queued episodes: $e');
      rethrow;
    }
  }

  // Get saved episodes
  Future<List<PinepodsEpisode>> getSavedEpisodes(int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/saved_episode_list/$userId');
    _devLog('Making API call to: $url');

    try {
      final response = await _get(url, headers: {'Api-Key': _apiKey!});

      // Saved episodes API response received

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        final episodesList = data['saved_episodes'] as List<dynamic>? ?? [];

        return episodesList.map((episodeData) {
          return PinepodsEpisode(
            podcastName: episodeData['podcastname'] ?? '',
            episodeTitle: episodeData['episodetitle'] ?? '',
            episodePubDate: episodeData['episodepubdate'] ?? '',
            episodeDescription: episodeData['episodedescription'] ?? '',
            episodeArtwork: episodeData['episodeartwork'] ?? '',
            episodeUrl: episodeData['episodeurl'] ?? '',
            episodeDuration: episodeData['episodeduration'] ?? 0,
            listenDuration: episodeData['listenduration'] ?? 0,
            episodeId: episodeData['episodeid'] ?? 0,
            completed: episodeData['completed'] ?? false,
            saved:
                episodeData['saved'] ??
                true, // Should always be true for saved episodes
            queued: episodeData['queued'] ?? false,
            downloaded: episodeData['downloaded'] ?? false,
            isYoutube: episodeData['is_youtube'] ?? false,
            saveDate: episodeData['savedate'],
          );
        }).toList();
      } else {
        throw Exception(
          'Failed to load saved episodes: ${response.statusCode}',
        );
      }
    } catch (e) {
      _devLog('Error getting saved episodes: $e');
      rethrow;
    }
  }

  // Get saved episodes with pagination support
  Future<EpisodePage> getSavedEpisodesPaged(
    int userId, {
    int limit = 50,
    int offset = 0,
    String sortBy = 'date',
    String sortOrder = 'desc',
    String filter = 'all',
  }) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/saved_episode_list/$userId').replace(
      queryParameters: {
        'limit': limit.toString(),
        'offset': offset.toString(),
        'sort_by': sortBy,
        'sort_order': sortOrder,
        'filter': filter,
      },
    );

    try {
      final response = await _get(url, headers: {'Api-Key': _apiKey!});

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        final total = (data['total'] as num?)?.toInt() ?? 0;
        final episodesList = data['saved_episodes'] as List<dynamic>? ?? [];

        final episodes = episodesList.map((episodeData) {
          return PinepodsEpisode(
            podcastName: episodeData['podcastname'] ?? '',
            episodeTitle: episodeData['episodetitle'] ?? '',
            episodePubDate: episodeData['episodepubdate'] ?? '',
            episodeDescription: episodeData['episodedescription'] ?? '',
            episodeArtwork: episodeData['episodeartwork'] ?? '',
            episodeUrl: episodeData['episodeurl'] ?? '',
            episodeDuration: episodeData['episodeduration'] ?? 0,
            listenDuration: episodeData['listenduration'] ?? 0,
            episodeId: episodeData['episodeid'] ?? 0,
            completed: episodeData['completed'] ?? false,
            saved: episodeData['saved'] ?? true,
            queued: episodeData['queued'] ?? false,
            downloaded: episodeData['downloaded'] ?? false,
            isYoutube: episodeData['is_youtube'] ?? false,
            saveDate: episodeData['savedate'],
          );
        }).toList();

        return EpisodePage(episodes: episodes, total: total);
      } else {
        throw Exception('Failed to load saved episodes: ${response.statusCode}');
      }
    } catch (e) {
      _devLog('Error getting paged saved episodes: $e');
      rethrow;
    }
  }

  // Get episode metadata
  Future<PinepodsEpisode?> getEpisodeMetadata(
    int episodeId,
    int userId, {
    bool isYoutube = false,
    bool personEpisode = false,
  }) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/get_episode_metadata');

    try {
      final requestBody = jsonEncode({
        'episode_id': episodeId,
        'user_id': userId,
        'person_episode': personEpisode,
        'is_youtube': isYoutube,
      });

      final response = await _post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        final episodeData = data['episode'];

        return PinepodsEpisode(
          podcastName: episodeData['podcastname'] ?? '',
          episodeTitle: episodeData['episodetitle'] ?? '',
          episodePubDate: episodeData['episodepubdate'] ?? '',
          episodeDescription: episodeData['episodedescription'] ?? '',
          episodeArtwork: episodeData['episodeartwork'] ?? '',
          episodeUrl: episodeData['episodeurl'] ?? '',
          episodeDuration: episodeData['episodeduration'] ?? 0,
          listenDuration: episodeData['listenduration'] ?? 0,
          episodeId: episodeData['episodeid'] ?? episodeId,
          completed: episodeData['completed'] ?? false,
          saved: episodeData['is_saved'] ?? false,
          queued: episodeData['is_queued'] ?? false,
          downloaded: episodeData['is_downloaded'] ?? false,
          isYoutube: episodeData['is_youtube'] ?? isYoutube,
          podcastId: episodeData['podcastid'],
        );
      }
      return null;
    } catch (e) {
      _devLog('Error getting episode metadata: $e');
      return null;
    }
  }

  // Get downloaded episodes from server
  Future<List<PinepodsEpisode>> getServerDownloads(int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse(
      '$_server/api/data/download_episode_list?user_id=$userId',
    );
    _devLog('Making API call to: $url');

    try {
      final response = await _get(url, headers: {'Api-Key': _apiKey!});

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        final episodesList =
            data['downloaded_episodes'] as List<dynamic>? ?? [];

        return episodesList.map((episodeData) {
          return PinepodsEpisode(
            podcastName: episodeData['podcastname'] ?? '',
            episodeTitle: episodeData['episodetitle'] ?? '',
            episodePubDate: episodeData['episodepubdate'] ?? '',
            episodeDescription: episodeData['episodedescription'] ?? '',
            episodeArtwork: episodeData['episodeartwork'] ?? '',
            episodeUrl: episodeData['episodeurl'] ?? '',
            episodeDuration: episodeData['episodeduration'] ?? 0,
            listenDuration: episodeData['listenduration'] ?? 0,
            episodeId: episodeData['episodeid'] ?? 0,
            completed: episodeData['completed'] ?? false,
            saved: episodeData['saved'] ?? false,
            queued: episodeData['queued'] ?? false,
            downloaded:
                episodeData['downloaded'] ??
                true, // Should always be true for downloaded episodes
            isYoutube: episodeData['is_youtube'] ?? false,
          );
        }).toList();
      } else {
        throw Exception(
          'Failed to load server downloads: ${response.statusCode}',
        );
      }
    } catch (e) {
      _devLog('Error getting server downloads: $e');
      rethrow;
    }
  }

  Future<List<PodcastDownloadSummary>> getPodcastDownloadSummary(int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/podcast_download_summary/$userId');

    try {
      final response = await _get(url, headers: {'Api-Key': _apiKey!});

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        final podcastsList = data['podcasts'] as List<dynamic>? ?? [];

        return podcastsList.map((p) {
          return PodcastDownloadSummary(
            podcastId: (p['podcastid'] as num?)?.toInt() ?? 0,
            podcastName: p['podcastname'] ?? '',
            artworkUrl: p['artworkurl'],
            episodeCount: (p['episode_count'] as num?)?.toInt() ?? 0,
          );
        }).toList();
      } else {
        throw Exception('Failed to load podcast download summary: ${response.statusCode}');
      }
    } catch (e) {
      _devLog('Error getting podcast download summary: $e');
      rethrow;
    }
  }

  Future<EpisodePage> getPodcastDownloadsPaged(
    int userId,
    int podcastId, {
    int limit = 50,
    int offset = 0,
  }) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse(
      '$_server/api/data/podcast_downloads_paged/$userId/$podcastId',
    ).replace(queryParameters: {
      'limit': limit.toString(),
      'offset': offset.toString(),
    });

    try {
      final response = await _get(url, headers: {'Api-Key': _apiKey!});

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        final total = (data['total'] as num?)?.toInt() ?? 0;
        final episodesList = data['episodes'] as List<dynamic>? ?? [];

        final episodes = episodesList.map((episodeData) {
          return PinepodsEpisode(
            podcastName: episodeData['podcastname'] ?? '',
            episodeTitle: episodeData['episodetitle'] ?? '',
            episodePubDate: episodeData['episodepubdate'] ?? '',
            episodeDescription: episodeData['episodedescription'] ?? '',
            episodeArtwork: episodeData['episodeartwork'] ?? '',
            episodeUrl: episodeData['episodeurl'] ?? '',
            episodeDuration: episodeData['episodeduration'] ?? 0,
            listenDuration: episodeData['listenduration'] ?? 0,
            episodeId: episodeData['episodeid'] ?? 0,
            completed: episodeData['completed'] ?? false,
            saved: episodeData['saved'] ?? false,
            queued: episodeData['queued'] ?? false,
            downloaded: episodeData['downloaded'] ?? true,
            isYoutube: episodeData['is_youtube'] ?? false,
          );
        }).toList();

        return EpisodePage(episodes: episodes, total: total);
      } else {
        throw Exception('Failed to load podcast downloads: ${response.statusCode}');
      }
    } catch (e) {
      _devLog('Error getting podcast downloads paged: $e');
      rethrow;
    }
  }

  // Get stream URL for episode
  String getStreamUrl(
    int episodeId,
    int userId, {
    bool isYoutube = false,
    bool isLocal = false,
  }) {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    if (isYoutube) {
      return '$_server/api/data/stream/$episodeId?api_key=$_apiKey&user_id=$userId&type=youtube';
    } else if (isLocal) {
      return '$_server/api/data/stream/$episodeId?api_key=$_apiKey&user_id=$userId';
    } else {
      // For external episodes, return the original URL
      return '';
    }
  }

  // Search for podcasts using PinePods search API
  Future<PinepodsSearchResult> searchPodcasts(
    String query,
    SearchProvider provider,
  ) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Server and API key must be set');
    }

    // Route through the backend search proxy so the configured (possibly
    // internal-only) SEARCH_API_URL is honored instead of the public default.
    final url = Uri.parse(
      '$_server/api/data/proxy_search?query=${Uri.encodeComponent(query)}&index=${provider.value}',
    );

    try {
      _devLog('Making search request to: $url');
      final response = await _get(url, headers: {'Api-Key': _apiKey!});

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        // Search API response received
        return PinepodsSearchResult.fromJson(data);
      } else {
        throw Exception('Failed to search podcasts: ${response.statusCode}');
      }
    } catch (e) {
      _devLog('Error searching podcasts: $e');
      rethrow;
    }
  }

  // Check if a podcast is already added to the server
  Future<bool> checkPodcastExists(
    String podcastTitle,
    String podcastUrl,
    int userId,
  ) async {
    if (_server == null || _apiKey == null) {
      return false;
    }

    final url = Uri.parse('$_server/api/data/check_podcast').replace(
      queryParameters: {
        'user_id': userId.toString(),
        'podcast_name': podcastTitle,
        'podcast_url': podcastUrl,
      },
    );

    try {
      final response = await _get(url, headers: {'Api-Key': _apiKey!});

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data['exists'] == true;
      }
      return false;
    } catch (e) {
      _devLog('Error checking podcast exists: $e');
      return false;
    }
  }

  // Add a podcast to the server
  Future<bool> addPodcast(UnifiedPinepodsPodcast podcast, int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/add_podcast');
    final body = {
      'podcast_values': {
        'pod_title': podcast.title,
        'pod_artwork': podcast.artwork,
        'pod_author': podcast.author,
        'categories': podcast.categories ?? {},
        'pod_description': podcast.description,
        'pod_episode_count': podcast.episodeCount,
        'pod_feed_url': podcast.url,
        'pod_website': podcast.link,
        'pod_explicit': podcast.explicit,
        'user_id': userId,
      },
      'podcast_index_id': podcast.indexId,
    };

    try {
      final response = await _post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: jsonEncode(body),
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data['success'] == true;
      }
      return false;
    } catch (e) {
      _devLog('Error adding podcast: $e');
      rethrow;
    }
  }

  // Remove a podcast from the server
  Future<bool> removePodcast(
    String podcastTitle,
    String podcastUrl,
    int userId,
  ) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/remove_podcast');
    final body = {
      'podcast_name': podcastTitle,
      'podcast_url': podcastUrl,
      'user_id': userId,
    };

    try {
      final response = await _post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: jsonEncode(body),
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data['success'] == true;
      }
      return false;
    } catch (e) {
      _devLog('Error removing podcast: $e');
      rethrow;
    }
  }

  // Get podcast details dynamically (whether added or not)
  Future<PodcastDetailsData> getPodcastDetailsDynamic({
    required int userId,
    required String podcastTitle,
    required String podcastUrl,
    required int podcastIndexId,
    required bool added,
    bool displayOnly = false,
  }) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/get_podcast_details_dynamic')
        .replace(
          queryParameters: {
            'user_id': userId.toString(),
            'podcast_title': podcastTitle,
            'podcast_url': podcastUrl,
            'podcast_index_id': podcastIndexId.toString(),
            'added': added.toString(),
            'display_only': displayOnly.toString(),
          },
        );

    try {
      final response = await _get(url, headers: {'Api-Key': _apiKey!});

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        // Podcast details API response received
        return PodcastDetailsData.fromJson(data);
      } else {
        throw Exception(
          'Failed to get podcast details: ${response.statusCode}',
        );
      }
    } catch (e) {
      _devLog('Error getting podcast details: $e');
      rethrow;
    }
  }

  // Get podcast details by podcast ID (for subscribed podcasts)
  Future<Map<String, dynamic>?> getPodcastDetailsById(
    int podcastId,
    int userId,
  ) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/get_podcast_details').replace(
      queryParameters: {
        'podcast_id': podcastId.toString(),
        'user_id': userId.toString(),
      },
    );

    try {
      _devLog('Getting podcast details by ID from: $url');
      final response = await _get(url, headers: {'Api-Key': _apiKey!});

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        // Podcast details by ID API response received
        return data['details'];
      } else {
        throw Exception(
          'Failed to get podcast details: ${response.statusCode}',
        );
      }
    } catch (e) {
      _devLog('Error getting podcast details by ID: $e');
      rethrow;
    }
  }

  // Get podcast ID by feed URL and title
  Future<int?> getPodcastId(
    int userId,
    String podcastFeed,
    String podcastTitle,
  ) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/get_podcast_id').replace(
      queryParameters: {
        'user_id': userId.toString(),
        'podcast_feed': podcastFeed,
        'podcast_title': podcastTitle,
      },
    );

    try {
      _devLog('Getting podcast ID from: $url');
      final response = await _get(url, headers: {'Api-Key': _apiKey!});

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        // Podcast ID API response received
        final podcastId = data['podcast_id'];
        if (podcastId is int) {
          return podcastId;
        }
        return null;
      } else {
        throw Exception('Failed to get podcast ID: ${response.statusCode}');
      }
    } catch (e) {
      _devLog('Error getting podcast ID: $e');
      return null;
    }
  }

  // Get episodes for an added podcast
  Future<List<PinepodsEpisode>> getPodcastEpisodes(
    int userId,
    int podcastId,
  ) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/podcast_episodes').replace(
      queryParameters: {
        'user_id': userId.toString(),
        'podcast_id': podcastId.toString(),
      },
    );

    try {
      final response = await _get(url, headers: {'Api-Key': _apiKey!});

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        final episodes = data['episodes'] as List;
        return episodes.map((episodeData) {
          // Add default values only for fields not provided by this endpoint
          final episodeWithDefaults = Map<String, dynamic>.from(episodeData);
          
          // Only add defaults if these fields are not present in the API response
          episodeWithDefaults['saved'] ??= false;
          episodeWithDefaults['queued'] ??= false;
          episodeWithDefaults['downloaded'] ??= false;
          episodeWithDefaults['is_youtube'] ??= false;

          return PinepodsEpisode.fromJson(episodeWithDefaults);
        }).toList();
      } else {
        throw Exception(
          'Failed to get podcast episodes: ${response.statusCode}',
        );
      }
    } catch (e) {
      _devLog('Error getting podcast episodes: $e');
      rethrow;
    }
  }

  // Get user statistics
  Future<UserStats> getUserStats(int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse(
      '$_server/api/data/get_stats',
    ).replace(queryParameters: {'user_id': userId.toString()});

    try {
      final response = await _get(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return UserStats.fromJson(data);
      } else {
        throw Exception('Failed to get user stats: ${response.statusCode}');
      }
    } catch (e) {
      _devLog('Error getting user stats: $e');
      rethrow;
    }
  }

  // Get PinePods version
  Future<String> getPinepodsVersion() async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/get_pinepods_version');

    try {
      final response = await _get(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data['data'] ?? 'Unknown';
      } else {
        throw Exception(
          'Failed to get PinePods version: ${response.statusCode}',
        );
      }
    } catch (e) {
      _devLog('Error getting PinePods version: $e');
      return 'Unknown';
    }
  }

  // Get user details by user ID
  Future<Map<String, dynamic>?> getUserDetails(int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/user_details_id/$userId');

    try {
      final response = await _get(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data;
      } else {
        throw Exception('Failed to get user details: ${response.statusCode}');
      }
    } catch (e) {
      _devLog('Error getting user details: $e');
      return null;
    }
  }

  // Get user ID from API key
  Future<int?> getUserIdFromApiKey() async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/id_from_api_key');

    try {
      final response = await _get(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
      );

      if (response.statusCode == 200) {
        final userId = int.tryParse(response.body.trim());
        return userId;
      } else {
        throw Exception('Failed to get user ID: ${response.statusCode}');
      }
    } catch (e) {
      _devLog('Error getting user ID: $e');
      return null;
    }
  }

  // Get home overview data
  Future<HomeOverview> getHomeOverview(int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/home_overview?user_id=$userId');
    _devLog('Making API call to: $url');

    try {
      final response = await _get(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return HomeOverview.fromJson(data);
      } else {
        throw Exception('Failed to load home overview: ${response.statusCode}');
      }
    } catch (e) {
      _devLog('Error getting home overview: $e');
      rethrow;
    }
  }

  // Get playlists
  Future<PlaylistResponse> getPlaylists(int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/get_playlists?user_id=$userId');
    _devLog('Making API call to: $url');

    try {
      final response = await _get(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
      );

      // Playlists API response received

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return PlaylistResponse.fromJson(data);
      } else {
        throw Exception('Failed to load playlists: ${response.statusCode}');
      }
    } catch (e) {
      _devLog('Error getting playlists: $e');
      rethrow;
    }
  }

  // Get user theme
  Future<String?> getUserTheme(int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/get_theme/$userId');
    _devLog('Making API call to: $url');

    try {
      final response = await _get(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
      );

      // Theme API response received

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data['theme'] as String?;
      } else {
        throw Exception('Failed to get user theme: ${response.statusCode}');
      }
    } catch (e) {
      _devLog('Error getting user theme: $e');
      return null;
    }
  }

  // Set user theme
  Future<bool> setUserTheme(int userId, String theme) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/user/set_theme');
    _devLog('Making API call to: $url');

    try {
      final requestBody = jsonEncode({'user_id': userId, 'new_theme': theme});

      final response = await _put(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );

      // Set theme API response received

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data['message'] != null;
      } else {
        throw Exception('Failed to set user theme: ${response.statusCode}');
      }
    } catch (e) {
      _devLog('Error setting user theme: $e');
      return false;
    }
  }

  // Get user playlists
  Future<List<PlaylistData>> getUserPlaylists(int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/get_playlists?user_id=$userId');
    _devLog('Making API call to: $url');

    try {
      final response = await _get(url, headers: {'Api-Key': _apiKey!});

      _devLog(
        'Get playlists response: ${response.statusCode} - ${response.body}',
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        final List<dynamic> playlistsData = data['playlists'] ?? [];

        List<PlaylistData> playlists = [];
        for (var playlistData in playlistsData) {
          playlists.add(PlaylistData.fromJson(playlistData));
        }

        return playlists;
      } else {
        throw Exception('Failed to get playlists: ${response.statusCode}');
      }
    } catch (e) {
      _devLog('Error getting playlists: $e');
      rethrow;
    }
  }

  // Create playlist
  Future<bool> createPlaylist(CreatePlaylistRequest request) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/create_playlist');
    _devLog('Making API call to: $url');

    try {
      final response = await _post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: jsonEncode(request.toJson()),
      );

      _devLog(
        'Create playlist response: ${response.statusCode} - ${response.body}',
      );
      return response.statusCode == 200;
    } catch (e) {
      _devLog('Error creating playlist: $e');
      return false;
    }
  }

  // Delete playlist
  Future<bool> deletePlaylist(int userId, int playlistId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/delete_playlist');
    _devLog('Making API call to: $url');

    try {
      final response = await _post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: jsonEncode({'user_id': userId, 'playlist_id': playlistId}),
      );

      _devLog(
        'Delete playlist response: ${response.statusCode} - ${response.body}',
      );
      return response.statusCode == 200;
    } catch (e) {
      _devLog('Error deleting playlist: $e');
      return false;
    }
  }

  // Get playlist episodes
  Future<PlaylistEpisodesResponse> getPlaylistEpisodes(
    int userId,
    int playlistId, {
    int limit = 50,
    int offset = 0,
  }) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse(
      '$_server/api/data/get_playlist_episodes?user_id=$userId&playlist_id=$playlistId&limit=$limit&offset=$offset',
    );
    _devLog('Making API call to: $url');

    try {
      final response = await _get(url, headers: {'Api-Key': _apiKey!});

      _devLog(
        'Get playlist episodes response: ${response.statusCode} - ${response.body}',
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return PlaylistEpisodesResponse.fromJson(data);
      } else {
        throw Exception(
          'Failed to get playlist episodes: ${response.statusCode}',
        );
      }
    } catch (e) {
      _devLog('Error getting playlist episodes: $e');
      rethrow;
    }
  }

  // Reorder queue episodes
  Future<bool> reorderQueue(int userId, List<int> episodeIds) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/reorder_queue?user_id=$userId');
    _devLog('Making API call to: $url');

    try {
      final response = await _post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: jsonEncode({'episode_ids': episodeIds}),
      );

      _devLog(
        'Reorder queue response: ${response.statusCode} - ${response.body}',
      );
      return response.statusCode == 200;
    } catch (e) {
      _devLog('Error reordering queue: $e');
      return false;
    }
  }

  // Search episodes in user's subscriptions
  Future<SearchResultPage> searchEpisodes(
    int userId,
    String searchTerm, {
    int limit = 50,
    int offset = 0,
    List<String>? categories,
  }) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/search_data').replace(
      queryParameters: {
        'limit': limit.toString(),
        'offset': offset.toString(),
      },
    );
    _devLog('Making API call to: $url');

    try {
      final body = <String, dynamic>{
        'search_term': searchTerm,
        'user_id': userId,
      };
      if (categories != null && categories.isNotEmpty) {
        body['categories'] = categories;
      }

      final response = await _post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: jsonEncode(body),
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        final List<dynamic> episodesData = data['data'] ?? [];
        final total = (data['total'] as num?)?.toInt() ?? episodesData.length;

        List<SearchEpisodeResult> results = [];
        for (var episodeData in episodesData) {
          results.add(SearchEpisodeResult.fromJson(episodeData));
        }

        return SearchResultPage(results: results, total: total);
      } else {
        throw Exception('Failed to search episodes: ${response.statusCode}');
      }
    } catch (e) {
      _devLog('Error searching episodes: $e');
      rethrow;
    }
  }

  // Fetch podcast 2.0 data for a specific episode
  Future<Map<String, dynamic>?> fetchPodcasting2Data(
    int episodeId,
    int userId,
  ) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/fetch_podcasting_2_data').replace(
      queryParameters: {
        'episode_id': episodeId.toString(),
        'user_id': userId.toString(),
      },
    );

    _devLog('Making API call to: $url');

    try {
      final response = await _get(url, headers: {'Api-Key': _apiKey!});

      _devLog(
        'Podcast 2.0 data response: ${response.statusCode} - ${response.body}',
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data;
      } else {
        _devLog('Failed to fetch podcast 2.0 data: ${response.statusCode}');
        return null;
      }
    } catch (e) {
      _devLog('Error fetching podcast 2.0 data: $e');
      return null;
    }
  }

  // Fetch podcast 2.0 data for a specific podcast
  Future<Map<String, dynamic>?> fetchPodcasting2PodData(
    int podcastId,
    int userId,
  ) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/fetch_podcasting_2_pod_data')
        .replace(
          queryParameters: {
            'podcast_id': podcastId.toString(),
            'user_id': userId.toString(),
          },
        );

    _devLog('Making API call to: $url');

    try {
      final response = await _get(url, headers: {'Api-Key': _apiKey!});

      _devLog(
        'Podcast 2.0 pod data response: ${response.statusCode} - ${response.body}',
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data;
      } else {
        _devLog('Failed to fetch podcast 2.0 pod data: ${response.statusCode}');
        return null;
      }
    } catch (e) {
      _devLog('Error fetching podcast 2.0 pod data: $e');
      return null;
    }
  }

  Future<bool> getAutoPlayNextStatus(int podcastId, int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/get_auto_play_next_status');

    try {
      final response = await _post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: jsonEncode({'podcast_id': podcastId, 'user_id': userId}),
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data['auto_play_next'] ?? false;
      } else {
        return false;
      }
    } catch (e) {
      _devLog('Error getting auto-play-next status: $e');
      return false;
    }
  }

  Future<PinepodsEpisode?> getNextPodcastEpisode(int episodeId, int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/get_next_podcast_episode');

    try {
      final response = await _post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: jsonEncode({'episode_id': episodeId, 'user_id': userId}),
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        if (data == null) return null;

        return PinepodsEpisode(
          podcastName: data['podcastname'] ?? '',
          episodeTitle: data['episodetitle'] ?? '',
          episodePubDate: data['episodepubdate'] ?? '',
          episodeDescription: data['episodedescription'] ?? '',
          episodeArtwork: data['episodeartwork'] ?? '',
          episodeUrl: data['episodeurl'] ?? '',
          episodeDuration: data['episodeduration'] ?? 0,
          listenDuration: data['listenduration'] ?? 0,
          episodeId: data['episodeid'] ?? 0,
          completed: data['completed'] ?? false,
          saved: data['saved'] ?? false,
          queued: data['queued'] ?? false,
          downloaded: data['downloaded'] ?? false,
          isYoutube: data['is_youtube'] ?? false,
        );
      } else {
        return null;
      }
    } catch (e) {
      _devLog('Error getting next podcast episode: $e');
      return null;
    }
  }

  Future<PinepodsEpisode?> getNextPlaylistEpisode(
      int episodeId, int playlistId, int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/get_next_playlist_episode');

    try {
      final response = await _post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: jsonEncode(
            {'episode_id': episodeId, 'playlist_id': playlistId, 'user_id': userId}),
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        if (data == null) return null;

        return PinepodsEpisode(
          podcastName: data['podcastname'] ?? '',
          episodeTitle: data['episodetitle'] ?? '',
          episodePubDate: data['episodepubdate'] ?? '',
          episodeDescription: data['episodedescription'] ?? '',
          episodeArtwork: data['episodeartwork'] ?? '',
          episodeUrl: data['episodeurl'] ?? '',
          episodeDuration: data['episodeduration'] ?? 0,
          listenDuration: data['listenduration'] ?? 0,
          episodeId: data['episodeid'] ?? 0,
          completed: data['completed'] ?? false,
          saved: data['saved'] ?? false,
          queued: data['queued'] ?? false,
          downloaded: data['downloaded'] ?? false,
          isYoutube: data['is_youtube'] ?? false,
        );
      } else {
        return null;
      }
    } catch (e) {
      _devLog('Error getting next playlist episode: $e');
      return null;
    }
  }

  Future<int?> getPodcastIdFromEpisodeId(int episodeId, int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse(
      '$_server/api/data/get_podcast_id_from_ep_id?episode_id=$episodeId&user_id=$userId',
    );

    try {
      final response = await _get(url, headers: {'Api-Key': _apiKey!});

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data['podcast_id'] as int?;
      } else {
        return null;
      }
    } catch (e) {
      _devLog('Error getting podcast ID from episode: $e');
      return null;
    }
  }
}

class PodcastDetailsData {
  final int podcastId;
  final String podcastName;
  final String feedUrl;
  final String description;
  final String author;
  final String artworkUrl;
  final bool explicit;
  final int episodeCount;
  final Map<String, String>? categories;
  final String websiteUrl;
  final int podcastIndexId;
  final bool isYoutube;

  PodcastDetailsData({
    required this.podcastId,
    required this.podcastName,
    required this.feedUrl,
    required this.description,
    required this.author,
    required this.artworkUrl,
    required this.explicit,
    required this.episodeCount,
    this.categories,
    required this.websiteUrl,
    required this.podcastIndexId,
    required this.isYoutube,
  });

  factory PodcastDetailsData.fromJson(Map<String, dynamic> json) {
    return PodcastDetailsData(
      podcastId: json['podcastid'] ?? 0,
      podcastName: json['podcastname'] ?? '',
      feedUrl: json['feedurl'] ?? '',
      description: json['description'] ?? '',
      author: json['author'] ?? '',
      artworkUrl: json['artworkurl'] ?? '',
      explicit: json['explicit'] ?? false,
      episodeCount: json['episodecount'] ?? 0,
      categories: json['categories'] != null
          ? Map<String, String>.from(json['categories'] as Map)
          : null,
      websiteUrl: json['websiteurl'] ?? '',
      podcastIndexId: json['podcastindexid'] ?? 0,
      isYoutube: json['is_youtube'] ?? false,
    );
  }
}

class PlayEpisodeDetails {
  final double playbackSpeed;
  final int startSkip;
  final int endSkip;

  PlayEpisodeDetails({
    required this.playbackSpeed,
    required this.startSkip,
    required this.endSkip,
  });
}

// Per-podcast silence-trim settings (#727)
class SilenceTrimSettings {
  final bool enabled;
  final int threshold;

  const SilenceTrimSettings({required this.enabled, required this.threshold});
}

// A single auto-skip range (silence #727; ads #790), times in seconds.
// `status` is the requesting user's effective state for ad segments:
// "active"/"confirmed" => skip, "pending"/"rejected" => don't skip; null for silence.
class SkipSegment {
  final int segmentId;
  final String kind;
  final double startTime;
  final double endTime;
  final String? source;
  final String? status;

  const SkipSegment({
    this.segmentId = 0,
    required this.kind,
    required this.startTime,
    required this.endTime,
    this.source,
    this.status,
  });

  // True only for ad segments the server has resolved as skippable for this user.
  bool get isActiveAd =>
      kind == 'ad' && (status == 'active' || status == 'confirmed');
}

// AI capability status from GET /api/data/ai_status; gates all AI UI.
class AiStatus {
  final bool available;
  final bool transcriptionReady;
  final bool adRemovalReady;

  const AiStatus({
    this.available = false,
    this.transcriptionReady = false,
    this.adRemovalReady = false,
  });
}

// Stored AI transcript from GET /api/data/episode_transcript.
// `segments` is the raw JSON string of [{start,end,text}] (seconds), or null.
class StoredTranscript {
  final String source;
  final String? language;
  final String? model;
  final String status;
  final String? fullText;
  final String? segments;

  const StoredTranscript({
    required this.source,
    this.language,
    this.model,
    required this.status,
    this.fullText,
    this.segments,
  });
}

// Playlist Data Classes
class PlaylistData {
  final int playlistId;
  final int userId;
  final String name;
  final String? description;
  final bool isSystemPlaylist;
  final List<int>? podcastIds;
  final bool includeUnplayed;
  final bool includePartiallyPlayed;
  final bool includePlayed;
  final int? minDuration;
  final int? maxDuration;
  final String sortOrder;
  final bool groupByPodcast;
  final int? maxEpisodes;
  final String lastUpdated;
  final String created;
  final int? episodeCount;
  final String? iconName;

  PlaylistData({
    required this.playlistId,
    required this.userId,
    required this.name,
    this.description,
    required this.isSystemPlaylist,
    this.podcastIds,
    required this.includeUnplayed,
    required this.includePartiallyPlayed,
    required this.includePlayed,
    this.minDuration,
    this.maxDuration,
    required this.sortOrder,
    required this.groupByPodcast,
    this.maxEpisodes,
    required this.lastUpdated,
    required this.created,
    this.episodeCount,
    this.iconName,
  });

  factory PlaylistData.fromJson(Map<String, dynamic> json) {
    return PlaylistData(
      playlistId: json['playlist_id'] ?? 0,
      userId: json['user_id'] ?? 0,
      name: json['name'] ?? '',
      description: json['description'],
      isSystemPlaylist: json['is_system_playlist'] ?? false,
      podcastIds: json['podcast_ids'] != null
          ? List<int>.from(json['podcast_ids'])
          : null,
      includeUnplayed: json['include_unplayed'] ?? true,
      includePartiallyPlayed: json['include_partially_played'] ?? true,
      includePlayed: json['include_played'] ?? false,
      minDuration: json['min_duration'],
      maxDuration: json['max_duration'],
      sortOrder: json['sort_order'] ?? 'date_desc',
      groupByPodcast: json['group_by_podcast'] ?? false,
      maxEpisodes: json['max_episodes'],
      lastUpdated: json['last_updated'] ?? '',
      created: json['created'] ?? '',
      episodeCount: json['episode_count'],
      iconName: json['icon_name'],
    );
  }
}

class CreatePlaylistRequest {
  final int userId;
  final String name;
  final String? description;
  final List<int>? podcastIds;
  final bool includeUnplayed;
  final bool includePartiallyPlayed;
  final bool includePlayed;
  final int? minDuration;
  final int? maxDuration;
  final String sortOrder;
  final bool groupByPodcast;
  final int? maxEpisodes;
  final String iconName;
  final double? playProgressMin;
  final double? playProgressMax;
  final int? timeFilterHours;

  CreatePlaylistRequest({
    required this.userId,
    required this.name,
    this.description,
    this.podcastIds,
    required this.includeUnplayed,
    required this.includePartiallyPlayed,
    required this.includePlayed,
    this.minDuration,
    this.maxDuration,
    required this.sortOrder,
    required this.groupByPodcast,
    this.maxEpisodes,
    required this.iconName,
    this.playProgressMin,
    this.playProgressMax,
    this.timeFilterHours,
  });

  Map<String, dynamic> toJson() {
    return {
      'user_id': userId,
      'name': name,
      'description': description,
      'podcast_ids': podcastIds,
      'include_unplayed': includeUnplayed,
      'include_partially_played': includePartiallyPlayed,
      'include_played': includePlayed,
      'min_duration': minDuration,
      'max_duration': maxDuration,
      'sort_order': sortOrder,
      'group_by_podcast': groupByPodcast,
      'max_episodes': maxEpisodes,
      'icon_name': iconName,
      'play_progress_min': playProgressMin,
      'play_progress_max': playProgressMax,
      'time_filter_hours': timeFilterHours,
    };
  }
}

class PlaylistEpisodesResponse {
  final List<PinepodsEpisode> episodes;
  final PlaylistInfo playlistInfo;
  final int total;

  PlaylistEpisodesResponse({
    required this.episodes,
    required this.playlistInfo,
    required this.total,
  });

  factory PlaylistEpisodesResponse.fromJson(Map<String, dynamic> json) {
    return PlaylistEpisodesResponse(
      episodes: (json['episodes'] as List<dynamic>? ?? [])
          .map((e) => PinepodsEpisode.fromJson(e))
          .toList(),
      playlistInfo: PlaylistInfo.fromJson(json['playlist_info'] ?? {}),
      total: (json['total'] as num?)?.toInt() ?? 0,
    );
  }
}

class PlaylistInfo {
  final String name;
  final String? description;
  final int? episodeCount;
  final String? iconName;

  PlaylistInfo({
    required this.name,
    this.description,
    this.episodeCount,
    this.iconName,
  });

  factory PlaylistInfo.fromJson(Map<String, dynamic> json) {
    return PlaylistInfo(
      name: json['name'] ?? '',
      description: json['description'],
      episodeCount: json['episode_count'],
      iconName: json['icon_name'],
    );
  }
}

class SearchEpisodeResult {
  final int podcastId;
  final String podcastName;
  final String artworkUrl;
  final String author;
  final String categories;
  final String description;
  final int? episodeCount;
  final String feedUrl;
  final String websiteUrl;
  final bool explicit;
  final int userId;
  final int episodeId;
  final String episodeTitle;
  final String episodeDescription;
  final String episodePubDate;
  final String episodeArtwork;
  final String episodeUrl;
  final int episodeDuration;
  final bool completed;
  final bool saved;
  final bool queued;
  final bool downloaded;
  final bool isYoutube;
  final int? listenDuration;

  SearchEpisodeResult({
    required this.podcastId,
    required this.podcastName,
    required this.artworkUrl,
    required this.author,
    required this.categories,
    required this.description,
    this.episodeCount,
    required this.feedUrl,
    required this.websiteUrl,
    required this.explicit,
    required this.userId,
    required this.episodeId,
    required this.episodeTitle,
    required this.episodeDescription,
    required this.episodePubDate,
    required this.episodeArtwork,
    required this.episodeUrl,
    required this.episodeDuration,
    required this.completed,
    required this.saved,
    required this.queued,
    required this.downloaded,
    required this.isYoutube,
    this.listenDuration,
  });

  factory SearchEpisodeResult.fromJson(Map<String, dynamic> json) {
    return SearchEpisodeResult(
      podcastId: json['podcastid'] ?? 0,
      podcastName: json['podcastname'] ?? '',
      artworkUrl: json['artworkurl'] ?? '',
      author: json['author'] ?? '',
      categories: _parseCategories(json['categories']),
      description: json['description'] ?? '',
      episodeCount: json['episodecount'],
      feedUrl: json['feedurl'] ?? '',
      websiteUrl: json['websiteurl'] ?? '',
      explicit: (json['explicit'] ?? 0) == 1,
      userId: json['userid'] ?? 0,
      episodeId: json['episodeid'] ?? 0,
      episodeTitle: json['episodetitle'] ?? '',
      episodeDescription: json['episodedescription'] ?? '',
      episodePubDate: json['episodepubdate'] ?? '',
      episodeArtwork: json['episodeartwork'] ?? '',
      episodeUrl: json['episodeurl'] ?? '',
      episodeDuration: json['episodeduration'] ?? 0,
      completed: json['completed'] ?? false,
      saved: json['saved'] ?? false,
      queued: json['queued'] ?? false,
      downloaded: json['downloaded'] ?? false,
      isYoutube: json['is_youtube'] ?? false,
      listenDuration: json['listenduration'],
    );
  }

  // Convert to PinepodsEpisode for compatibility with existing widgets
  PinepodsEpisode toPinepodsEpisode() {
    return PinepodsEpisode(
      podcastName: podcastName,
      episodeTitle: episodeTitle,
      episodePubDate: episodePubDate,
      episodeDescription: episodeDescription,
      episodeArtwork: episodeArtwork.isNotEmpty ? episodeArtwork : artworkUrl,
      episodeUrl: episodeUrl,
      episodeDuration: episodeDuration,
      listenDuration: listenDuration,
      episodeId: episodeId,
      completed: completed,
      saved: saved,
      queued: queued,
      downloaded: downloaded,
      isYoutube: isYoutube,
      podcastId: podcastId,
    );
  }

  /// Parse categories from either string or Map format
  static String _parseCategories(dynamic categories) {
    if (categories == null) return '';

    if (categories is String) {
      // Old format - return as is
      return categories;
    } else if (categories is Map<String, dynamic>) {
      // New format - convert map values to comma-separated string
      if (categories.isEmpty) return '';
      return categories.values.join(', ');
    }

    return '';
  }
}

class PodcastDownloadSummary {
  final int podcastId;
  final String podcastName;
  final String? artworkUrl;
  final int episodeCount;

  PodcastDownloadSummary({
    required this.podcastId,
    required this.podcastName,
    this.artworkUrl,
    required this.episodeCount,
  });
}

class EpisodePage {
  final List<PinepodsEpisode> episodes;
  final int total;

  EpisodePage({required this.episodes, required this.total});
}

class SearchResultPage {
  final List<SearchEpisodeResult> results;
  final int total;

  SearchResultPage({required this.results, required this.total});
}

class DownloadTask {
  final String id;
  final String taskType;
  final String status;
  final double progress;
  final String? message;
  final DateTime createdAt;
  final DateTime updatedAt;
  final Map<String, dynamic>? result;
  final String? episodeTitle;
  final String? podcastName;

  DownloadTask({
    required this.id,
    required this.taskType,
    required this.status,
    required this.progress,
    this.message,
    required this.createdAt,
    required this.updatedAt,
    this.result,
    this.episodeTitle,
    this.podcastName,
  });

  factory DownloadTask.fromJson(Map<String, dynamic> json) {
    return DownloadTask(
      id: json['id'] as String,
      taskType: json['task_type'] as String,
      status: json['status'] as String,
      progress: (json['progress'] as num).toDouble(),
      message: json['message'] as String?,
      createdAt: DateTime.parse(json['created_at'] as String),
      updatedAt: DateTime.parse(json['updated_at'] as String),
      result: json['result'] as Map<String, dynamic>?,
      episodeTitle: json['episode_title'] as String?,
      podcastName: json['podcast_name'] as String?,
    );
  }

  int? get episodeId {
    final id = result?['episode_id'];
    if (id is int) return id;
    if (id is String) return int.tryParse(id);
    return null;
  }

  bool get isCompleted => status == 'SUCCESS';
  bool get isFailed => status == 'FAILED';
  bool get isActive => status == 'PENDING' || status == 'DOWNLOADING';
}
