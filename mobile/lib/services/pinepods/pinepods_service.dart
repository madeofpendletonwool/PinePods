// Create this file at lib/services/pinepods/pinepods_service.dart

import 'dart:convert';
import 'package:http/http.dart' as http;
import 'package:pinepods_mobile/entities/pinepods_episode.dart';

class PinepodsService {
  String? _server;
  String? _apiKey;

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
      final response = await http.get(
        url,
        headers: {'Api-Key': _apiKey!},
      );

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
      final response = await http.get(
        url,
        headers: {'Api-Key': _apiKey!},
      );

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

  // Get recent episodes (last 30 days)
  Future<List<PinepodsEpisode>> getRecentEpisodes(int userId) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated - server or API key missing');
    }

    final url = Uri.parse('$_server/api/data/return_episodes/$userId');

    try {
      final response = await http.get(
        url,
        headers: {
          'Api-Key': _apiKey!,
          'Content-Type': 'application/json',
        },
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
        throw Exception('Failed to fetch recent episodes: ${response.statusCode} ${response.reasonPhrase}');
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
  Future<bool> checkEpisodeInDb(int userId, String episodeTitle, String episodeUrl) async {
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
        headers: {
          'Api-Key': _apiKey!,
          'Content-Type': 'application/json',
        },
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
  Future<int> getEpisodeId(int userId, String episodeTitle, String episodeUrl, bool isYoutube) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/get_episode_id_ep_name?user_id=$userId&episode_url=${Uri.encodeComponent(episodeUrl)}&episode_title=${Uri.encodeComponent(episodeTitle)}&is_youtube=$isYoutube');
    
    try {
      final response = await http.get(
        url,
        headers: {
          'Api-Key': _apiKey!,
        },
      );

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
  Future<bool> addHistory(int episodeId, double episodePos, int userId, bool isYoutube) async {
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
        headers: {
          'Api-Key': _apiKey!,
          'Content-Type': 'application/json',
        },
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
        headers: {
          'Api-Key': _apiKey!,
          'Content-Type': 'application/json',
        },
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
      final response = await http.put(
        url,
        headers: {
          'Api-Key': _apiKey!,
        },
      );

      print('Increment played response: ${response.statusCode} - ${response.body}');
      return response.statusCode == 200;
    } catch (e) {
      print('Error incrementing played: $e');
      return false;
    }
  }

  // Get podcast ID from episode
  Future<int> getPodcastIdFromEpisode(int episodeId, int userId, bool isYoutube) async {
    if (_server == null || _apiKey == null) {
      throw Exception('Not authenticated');
    }

    final url = Uri.parse('$_server/api/data/get_podcast_id_from_ep/$episodeId');
    
    try {
      final response = await http.get(
        url,
        headers: {
          'Api-Key': _apiKey!,
          'Content-Type': 'application/json',
        },
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
  Future<PlayEpisodeDetails> getPlayEpisodeDetails(int userId, int podcastId, bool isYoutube) async {
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
        headers: {
          'Api-Key': _apiKey!,
          'Content-Type': 'application/json',
        },
        body: requestBody,
      );

      print('Play episode details response: ${response.statusCode} - ${response.body}');
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
  Future<bool> recordListenDuration(int episodeId, int userId, double listenDuration, bool isYoutube) async {
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
        headers: {
          'Api-Key': _apiKey!,
          'Content-Type': 'application/json',
        },
        body: requestBody,
      );

      print('Record listen duration response: ${response.statusCode} - ${response.body}');
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
      final response = await http.put(
        url,
        headers: {
          'Api-Key': _apiKey!,
        },
      );

      print('Increment listen time response: ${response.statusCode} - ${response.body}');
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
        headers: {
          'Api-Key': _apiKey!,
          'Content-Type': 'application/json',
        },
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
  Future<bool> removeSavedEpisode(int episodeId, int userId, bool isYoutube) async {
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
        headers: {
          'Api-Key': _apiKey!,
          'Content-Type': 'application/json',
        },
        body: requestBody,
      );

      print('Remove saved episode response: ${response.statusCode} - ${response.body}');
      return response.statusCode == 200;
    } catch (e) {
      print('Error removing saved episode: $e');
      return false;
    }
  }

  // Get stream URL for episode
  String getStreamUrl(int episodeId, int userId, {bool isYoutube = false, bool isLocal = false}) {
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