// Create this file at lib/services/pinepods/pinepods_service.dart

import 'dart:convert';
import 'package:http/http.dart' as http;
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/entities/pinepods_search.dart';
import 'package:pinepods_mobile/entities/user_stats.dart';
import 'package:pinepods_mobile/entities/home_data.dart';
import 'package:pinepods_mobile/entities/podcast.dart';

class PinepodsService {
  String? _server;
  String? _apiKey;

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
      final response = await http.get(url);

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data['pinepods_instance'] == true;
      }
      return false;
    } catch (e) {
      print('Error verifying PinePods instance: $e');
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
      final response = await http.get(
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
      print('Login error: $e');
      return false;
    }
  }

  Future<bool> verifyApiKey() async {
    if (_server == null || _apiKey == null) {
      return false;
    }

    final url = Uri.parse('$_server/api/data/verify_key');

    try {
      final response = await http.get(url, headers: {'Api-Key': _apiKey!});

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data['status'] == 'success';
      }
      return false;
    } catch (e) {
      print('Error verifying API key: $e');
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
      final response = await http.get(url, headers: {'Api-Key': _apiKey!});

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body) as List;
        return data.cast<Map<String, dynamic>>();
      }
      return [];
    } catch (e) {
      print('Error fetching podcasts: $e');
      return [];
    }
  }

  // Get user's subscribed podcasts using return_pods endpoint
  Future<List<Podcast>> getUserPodcasts(int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/return_pods/$userId');
    print('Making API call to: $url');

    try {
      final response = await http.get(url, headers: {'Api-Key': _apiKey!});

      print(
        'Get user podcasts response: ${response.statusCode} - ${response.body}',
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        final List<dynamic> podsData = data['pods'] ?? [];

        List<Podcast> podcasts = [];
        for (var podData in podsData) {
          // Use episode count from server response
          final episodeCount = podData['episodecount'] ?? 0;

          // Create placeholder episodes to represent the count
          final placeholderEpisodes = List.generate(
            episodeCount,
            (index) => Episode(
              guid: 'placeholder_$index',
              podcast: podData['podcastname'] ?? '',
              title: 'Episode ${index + 1}',
            ),
          );

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
              episodes: placeholderEpisodes,
            ),
          );
        }

        return podcasts;
      } else {
        throw Exception('Failed to get user podcasts: ${response.statusCode}');
      }
    } catch (e) {
      print('Error getting user podcasts: $e');
      rethrow;
    }
  }

  // Get recent episodes (last 30 days)
  Future<List<PinepodsEpisode>> getRecentEpisodes(int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated - server or API key missing');
    }

    final url = Uri.parse('$_server/api/data/return_episodes/$userId');

    try {
      final response = await http.get(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
      );

      if (response.statusCode == 200) {
        final responseText = response.body;
        final data = jsonDecode(responseText);

        // Handle the response structure from the web implementation
        if (data is Map && data['episodes'] != null) {
          final episodesList = data['episodes'] as List;
          return episodesList
              .map((episode) => PinepodsEpisode.fromJson(episode))
              .toList();
        } else if (data is List) {
          // Handle direct list response
          return data
              .map((episode) => PinepodsEpisode.fromJson(episode))
              .toList();
        } else {
          return [];
        }
      } else {
        throw Exception(
          'Failed to fetch recent episodes: ${response.statusCode} ${response.reasonPhrase}',
        );
      }
    } catch (e) {
      print('Error fetching recent episodes: $e');
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

      final response = await http.post(
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
      print('Error checking episode in DB: $e');
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
      final response = await http.get(url, headers: {'Api-Key': _apiKey!});

      if (response.statusCode == 200) {
        // Parse the response as a plain integer
        final episodeId = int.tryParse(response.body.trim()) ?? 0;
        return episodeId;
      }
      return 0;
    } catch (e) {
      print('Error getting episode ID: $e');
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
    print('Making API call to: $url');

    try {
      final requestBody = jsonEncode({
        'episode_id': episodeId,
        'episode_pos': episodePos,
        'user_id': userId,
        'is_youtube': isYoutube,
      });

      final response = await http.post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );

      print('Add history response: ${response.statusCode} - ${response.body}');
      return response.statusCode == 200;
    } catch (e) {
      print('Error adding history: $e');
      return false;
    }
  }

  // Queue episode
  Future<bool> queueEpisode(int episodeId, int userId, bool isYoutube) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/queue_pod');
    print('Making API call to: $url');

    try {
      final requestBody = jsonEncode({
        'episode_id': episodeId,
        'user_id': userId,
        'is_youtube': isYoutube,
      });

      final response = await http.post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );

      print('Queue pod response: ${response.statusCode} - ${response.body}');
      return response.statusCode == 200;
    } catch (e) {
      print('Error queueing episode: $e');
      return false;
    }
  }

  // Increment played count
  Future<bool> incrementPlayed(int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/increment_played/$userId');
    print('Making API call to: $url');

    try {
      final response = await http.put(url, headers: {'Api-Key': _apiKey!});

      print(
        'Increment played response: ${response.statusCode} - ${response.body}',
      );
      return response.statusCode == 200;
    } catch (e) {
      print('Error incrementing played: $e');
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
      final response = await http.get(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data['podcast_id'] ?? 0;
      }
      return 0;
    } catch (e) {
      print('Error getting podcast ID: $e');
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
    print('Making API call to: $url');

    try {
      final requestBody = jsonEncode({
        'user_id': userId,
        'podcast_id': podcastId,
        'is_youtube': isYoutube,
      });

      final response = await http.post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );

      print(
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
      print('Error getting play episode details: $e');
      return PlayEpisodeDetails(playbackSpeed: 1.0, startSkip: 0, endSkip: 0);
    }
  }

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
    print('Making API call to: $url');

    try {
      final requestBody = jsonEncode({
        'episode_id': episodeId,
        'user_id': userId,
        'listen_duration': listenDuration,
        'is_youtube': isYoutube,
      });

      final response = await http.post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );

      print(
        'Record listen duration response: ${response.statusCode} - ${response.body}',
      );
      return response.statusCode == 200;
    } catch (e) {
      print('Error recording listen duration: $e');
      return false;
    }
  }

  // Increment listen time for user stats
  Future<bool> incrementListenTime(int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/increment_listen_time/$userId');
    print('Making API call to: $url');

    try {
      final response = await http.put(url, headers: {'Api-Key': _apiKey!});

      print(
        'Increment listen time response: ${response.statusCode} - ${response.body}',
      );
      return response.statusCode == 200;
    } catch (e) {
      print('Error incrementing listen time: $e');
      return false;
    }
  }

  // Save episode
  Future<bool> saveEpisode(int episodeId, int userId, bool isYoutube) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/save_episode');
    print('Making API call to: $url');

    try {
      final requestBody = jsonEncode({
        'episode_id': episodeId,
        'user_id': userId,
        'is_youtube': isYoutube,
      });

      final response = await http.post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );

      print('Save episode response: ${response.statusCode} - ${response.body}');
      return response.statusCode == 200;
    } catch (e) {
      print('Error saving episode: $e');
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
    print('Making API call to: $url');

    try {
      final requestBody = jsonEncode({
        'episode_id': episodeId,
        'user_id': userId,
        'is_youtube': isYoutube,
      });

      final response = await http.post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );

      print(
        'Remove saved episode response: ${response.statusCode} - ${response.body}',
      );
      return response.statusCode == 200;
    } catch (e) {
      print('Error removing saved episode: $e');
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
    print('Making API call to: $url');

    try {
      final requestBody = jsonEncode({
        'episode_id': episodeId,
        'user_id': userId,
        'is_youtube': isYoutube,
      });

      final response = await http.post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );

      print(
        'Download episode response: ${response.statusCode} - ${response.body}',
      );
      return response.statusCode == 200;
    } catch (e) {
      print('Error downloading episode: $e');
      return false;
    }
  }

  // Delete downloaded episode from server
  Future<bool> deleteEpisode(int episodeId, int userId, bool isYoutube) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/delete_episode');
    print('Making API call to: $url');

    try {
      final requestBody = jsonEncode({
        'episode_id': episodeId,
        'user_id': userId,
        'is_youtube': isYoutube,
      });

      final response = await http.post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );

      print(
        'Delete episode response: ${response.statusCode} - ${response.body}',
      );
      return response.statusCode == 200;
    } catch (e) {
      print('Error deleting episode: $e');
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
    print('Making API call to: $url');

    try {
      final requestBody = jsonEncode({
        'episode_id': episodeId,
        'user_id': userId,
        'is_youtube': isYoutube,
      });

      final response = await http.post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );

      print(
        'Mark completed response: ${response.statusCode} - ${response.body}',
      );
      return response.statusCode == 200;
    } catch (e) {
      print('Error marking episode completed: $e');
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
    print('Making API call to: $url');

    try {
      final requestBody = jsonEncode({
        'episode_id': episodeId,
        'user_id': userId,
        'is_youtube': isYoutube,
      });

      final response = await http.post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );

      print(
        'Mark uncompleted response: ${response.statusCode} - ${response.body}',
      );
      return response.statusCode == 200;
    } catch (e) {
      print('Error marking episode uncompleted: $e');
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
    print('Making API call to: $url');

    try {
      final requestBody = jsonEncode({
        'episode_id': episodeId,
        'user_id': userId,
        'is_youtube': isYoutube,
      });

      final response = await http.post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );

      print(
        'Remove queued response: ${response.statusCode} - ${response.body}',
      );
      return response.statusCode == 200;
    } catch (e) {
      print('Error removing queued episode: $e');
      return false;
    }
  }

  // Get user history
  Future<List<PinepodsEpisode>> getUserHistory(int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/user_history/$userId');
    print('Making API call to: $url');

    try {
      final response = await http.get(url, headers: {'Api-Key': _apiKey!});

      print('User history response: ${response.statusCode} - ${response.body}');

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
          );
        }).toList();
      } else {
        throw Exception('Failed to load user history: ${response.statusCode}');
      }
    } catch (e) {
      print('Error getting user history: $e');
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
    print('Making API call to: $url');

    try {
      final response = await http.get(url, headers: {'Api-Key': _apiKey!});

      print(
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
      print('Error getting queued episodes: $e');
      rethrow;
    }
  }

  // Get saved episodes
  Future<List<PinepodsEpisode>> getSavedEpisodes(int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/saved_episode_list/$userId');
    print('Making API call to: $url');

    try {
      final response = await http.get(url, headers: {'Api-Key': _apiKey!});

      print(
        'Saved episodes response: ${response.statusCode} - ${response.body}',
      );

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
          );
        }).toList();
      } else {
        throw Exception(
          'Failed to load saved episodes: ${response.statusCode}',
        );
      }
    } catch (e) {
      print('Error getting saved episodes: $e');
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

      final response = await http.post(
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
      print('Error getting episode metadata: $e');
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
    print('Making API call to: $url');

    try {
      final response = await http.get(url, headers: {'Api-Key': _apiKey!});

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
      print('Error getting server downloads: $e');
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
    const searchApiUrl = 'https://search.pinepods.online';
    final url = Uri.parse(
      '$searchApiUrl/api/search?query=${Uri.encodeComponent(query)}&index=${provider.value}',
    );

    try {
      print('Making search request to: $url');
      final response = await http.get(url);

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        print('Search response: ${response.body}');
        return PinepodsSearchResult.fromJson(data);
      } else {
        throw Exception('Failed to search podcasts: ${response.statusCode}');
      }
    } catch (e) {
      print('Error searching podcasts: $e');
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
      final response = await http.get(url, headers: {'Api-Key': _apiKey!});

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data['exists'] == true;
      }
      return false;
    } catch (e) {
      print('Error checking podcast exists: $e');
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
      final response = await http.post(
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
      print('Error adding podcast: $e');
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
      final response = await http.post(
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
      print('Error removing podcast: $e');
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
      final response = await http.get(url, headers: {'Api-Key': _apiKey!});

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        print('Podcast details response: ${response.body}');
        return PodcastDetailsData.fromJson(data);
      } else {
        throw Exception(
          'Failed to get podcast details: ${response.statusCode}',
        );
      }
    } catch (e) {
      print('Error getting podcast details: $e');
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
      print('Getting podcast details by ID from: $url');
      final response = await http.get(url, headers: {'Api-Key': _apiKey!});

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        print('Podcast details by ID response: ${response.body}');
        return data['details'];
      } else {
        throw Exception(
          'Failed to get podcast details: ${response.statusCode}',
        );
      }
    } catch (e) {
      print('Error getting podcast details by ID: $e');
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
      print('Getting podcast ID from: $url');
      final response = await http.get(url, headers: {'Api-Key': _apiKey!});

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        print('Podcast ID response: ${response.body}');
        final podcastId = data['podcast_id'];
        if (podcastId is int) {
          return podcastId;
        }
        return null;
      } else {
        throw Exception('Failed to get podcast ID: ${response.statusCode}');
      }
    } catch (e) {
      print('Error getting podcast ID: $e');
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
      final response = await http.get(url, headers: {'Api-Key': _apiKey!});

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
      print('Error getting podcast episodes: $e');
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
      final response = await http.get(
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
      print('Error getting user stats: $e');
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
      final response = await http.get(
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
      print('Error getting PinePods version: $e');
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
      final response = await http.get(
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
      print('Error getting user details: $e');
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
      final response = await http.get(
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
      print('Error getting user ID: $e');
      return null;
    }
  }

  // Get home overview data
  Future<HomeOverview> getHomeOverview(int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/home_overview?user_id=$userId');
    print('Making API call to: $url');

    try {
      final response = await http.get(
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
      print('Error getting home overview: $e');
      rethrow;
    }
  }

  // Get playlists
  Future<PlaylistResponse> getPlaylists(int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/get_playlists?user_id=$userId');
    print('Making API call to: $url');

    try {
      final response = await http.get(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
      );

      print('Playlists response: ${response.statusCode} - ${response.body}');

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return PlaylistResponse.fromJson(data);
      } else {
        throw Exception('Failed to load playlists: ${response.statusCode}');
      }
    } catch (e) {
      print('Error getting playlists: $e');
      rethrow;
    }
  }

  // Get user theme
  Future<String?> getUserTheme(int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/get_theme/$userId');
    print('Making API call to: $url');

    try {
      final response = await http.get(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
      );

      print('Get theme response: ${response.statusCode} - ${response.body}');

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data['theme'] as String?;
      } else {
        throw Exception('Failed to get user theme: ${response.statusCode}');
      }
    } catch (e) {
      print('Error getting user theme: $e');
      return null;
    }
  }

  // Set user theme
  Future<bool> setUserTheme(int userId, String theme) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/user/set_theme');
    print('Making API call to: $url');

    try {
      final requestBody = jsonEncode({'user_id': userId, 'new_theme': theme});

      final response = await http.put(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: requestBody,
      );

      print('Set theme response: ${response.statusCode} - ${response.body}');

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data['message'] != null;
      } else {
        throw Exception('Failed to set user theme: ${response.statusCode}');
      }
    } catch (e) {
      print('Error setting user theme: $e');
      return false;
    }
  }

  // Get user playlists
  Future<List<PlaylistData>> getUserPlaylists(int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/get_playlists?user_id=$userId');
    print('Making API call to: $url');

    try {
      final response = await http.get(url, headers: {'Api-Key': _apiKey!});

      print(
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
      print('Error getting playlists: $e');
      rethrow;
    }
  }

  // Create playlist
  Future<bool> createPlaylist(CreatePlaylistRequest request) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/create_playlist');
    print('Making API call to: $url');

    try {
      final response = await http.post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: jsonEncode(request.toJson()),
      );

      print(
        'Create playlist response: ${response.statusCode} - ${response.body}',
      );
      return response.statusCode == 200;
    } catch (e) {
      print('Error creating playlist: $e');
      return false;
    }
  }

  // Delete playlist
  Future<bool> deletePlaylist(int userId, int playlistId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/delete_playlist');
    print('Making API call to: $url');

    try {
      final response = await http.post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: jsonEncode({'user_id': userId, 'playlist_id': playlistId}),
      );

      print(
        'Delete playlist response: ${response.statusCode} - ${response.body}',
      );
      return response.statusCode == 200;
    } catch (e) {
      print('Error deleting playlist: $e');
      return false;
    }
  }

  // Get playlist episodes
  Future<PlaylistEpisodesResponse> getPlaylistEpisodes(
    int userId,
    int playlistId,
  ) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse(
      '$_server/api/data/get_playlist_episodes?user_id=$userId&playlist_id=$playlistId',
    );
    print('Making API call to: $url');

    try {
      final response = await http.get(url, headers: {'Api-Key': _apiKey!});

      print(
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
      print('Error getting playlist episodes: $e');
      rethrow;
    }
  }

  // Reorder queue episodes
  Future<bool> reorderQueue(int userId, List<int> episodeIds) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/reorder_queue?user_id=$userId');
    print('Making API call to: $url');

    try {
      final response = await http.post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: jsonEncode({'episode_ids': episodeIds}),
      );

      print(
        'Reorder queue response: ${response.statusCode} - ${response.body}',
      );
      return response.statusCode == 200;
    } catch (e) {
      print('Error reordering queue: $e');
      return false;
    }
  }

  // Search episodes in user's subscriptions
  Future<List<SearchEpisodeResult>> searchEpisodes(
    int userId,
    String searchTerm,
  ) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/search_data');
    print('Making API call to: $url');

    try {
      final response = await http.post(
        url,
        headers: {'Api-Key': _apiKey!, 'Content-Type': 'application/json'},
        body: jsonEncode({'search_term': searchTerm, 'user_id': userId}),
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        final List<dynamic> episodesData = data['data'] ?? [];

        List<SearchEpisodeResult> episodes = [];
        for (var episodeData in episodesData) {
          episodes.add(SearchEpisodeResult.fromJson(episodeData));
        }

        return episodes;
      } else {
        throw Exception('Failed to search episodes: ${response.statusCode}');
      }
    } catch (e) {
      print('Error searching episodes: $e');
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

    print('Making API call to: $url');

    try {
      final response = await http.get(url, headers: {'Api-Key': _apiKey!});

      print(
        'Podcast 2.0 data response: ${response.statusCode} - ${response.body}',
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data;
      } else {
        print('Failed to fetch podcast 2.0 data: ${response.statusCode}');
        return null;
      }
    } catch (e) {
      print('Error fetching podcast 2.0 data: $e');
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

    print('Making API call to: $url');

    try {
      final response = await http.get(url, headers: {'Api-Key': _apiKey!});

      print(
        'Podcast 2.0 pod data response: ${response.statusCode} - ${response.body}',
      );

      if (response.statusCode == 200) {
        final data = jsonDecode(response.body);
        return data;
      } else {
        print('Failed to fetch podcast 2.0 pod data: ${response.statusCode}');
        return null;
      }
    } catch (e) {
      print('Error fetching podcast 2.0 pod data: $e');
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

  PlaylistEpisodesResponse({
    required this.episodes,
    required this.playlistInfo,
  });

  factory PlaylistEpisodesResponse.fromJson(Map<String, dynamic> json) {
    return PlaylistEpisodesResponse(
      episodes: (json['episodes'] as List<dynamic>? ?? [])
          .map((e) => PinepodsEpisode.fromJson(e))
          .toList(),
      playlistInfo: PlaylistInfo.fromJson(json['playlist_info'] ?? {}),
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
