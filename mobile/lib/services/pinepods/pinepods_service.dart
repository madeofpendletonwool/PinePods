// Create this file at lib/services/pinepods/pinepods_service.dart

import 'dart:convert';
import 'package:http/http.dart' as http;

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
}