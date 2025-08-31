// lib/services/search_history_service.dart

import 'dart:convert';
import 'package:logging/logging.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// Service for managing search history for different search types
/// Stores search terms separately for episode search and podcast search
class SearchHistoryService {
  final log = Logger('SearchHistoryService');
  
  static const int maxHistoryItems = 30;
  static const String episodeSearchKey = 'episode_search_history';
  static const String podcastSearchKey = 'podcast_search_history';
  
  SearchHistoryService();
  
  /// Adds a search term to episode search history
  /// Moves existing term to top if already exists, otherwise adds as new
  Future<void> addEpisodeSearchTerm(String searchTerm) async {
    print('SearchHistoryService.addEpisodeSearchTerm called with: "$searchTerm"');
    await _addSearchTerm(episodeSearchKey, searchTerm);
  }
  
  /// Adds a search term to podcast search history
  /// Moves existing term to top if already exists, otherwise adds as new
  Future<void> addPodcastSearchTerm(String searchTerm) async {
    print('SearchHistoryService.addPodcastSearchTerm called with: "$searchTerm"');
    await _addSearchTerm(podcastSearchKey, searchTerm);
  }
  
  /// Gets episode search history, most recent first
  Future<List<String>> getEpisodeSearchHistory() async {
    print('SearchHistoryService.getEpisodeSearchHistory called');
    return await _getSearchHistory(episodeSearchKey);
  }
  
  /// Gets podcast search history, most recent first
  Future<List<String>> getPodcastSearchHistory() async {
    print('SearchHistoryService.getPodcastSearchHistory called');
    return await _getSearchHistory(podcastSearchKey);
  }
  
  /// Clears episode search history
  Future<void> clearEpisodeSearchHistory() async {
    await _clearSearchHistory(episodeSearchKey);
  }
  
  /// Clears podcast search history
  Future<void> clearPodcastSearchHistory() async {
    await _clearSearchHistory(podcastSearchKey);
  }
  
  /// Removes a specific term from episode search history
  Future<void> removeEpisodeSearchTerm(String searchTerm) async {
    await _removeSearchTerm(episodeSearchKey, searchTerm);
  }
  
  /// Removes a specific term from podcast search history
  Future<void> removePodcastSearchTerm(String searchTerm) async {
    await _removeSearchTerm(podcastSearchKey, searchTerm);
  }
  
  /// Internal method to add a search term to specified history type
  Future<void> _addSearchTerm(String historyKey, String searchTerm) async {
    if (searchTerm.trim().isEmpty) return;
    
    final trimmedTerm = searchTerm.trim();
    print('SearchHistoryService: Adding search term "$trimmedTerm" to $historyKey');
    
    try {
      final prefs = await SharedPreferences.getInstance();
      
      // Get existing history
      final historyJson = prefs.getString(historyKey);
      List<String> history = [];
      
      if (historyJson != null) {
        final List<dynamic> decodedList = jsonDecode(historyJson);
        history = decodedList.cast<String>();
      }
      
      print('SearchHistoryService: Existing data for $historyKey: $history');
      
      // Remove if already exists (to avoid duplicates)
      history.remove(trimmedTerm);
      
      // Add to beginning (most recent first)
      history.insert(0, trimmedTerm);
      
      // Limit to max items
      if (history.length > maxHistoryItems) {
        history = history.take(maxHistoryItems).toList();
      }
      
      // Save updated history
      await prefs.setString(historyKey, jsonEncode(history));
      
      print('SearchHistoryService: Updated $historyKey with ${history.length} terms: $history');
    } catch (e) {
      print('SearchHistoryService: Failed to add search term to $historyKey: $e');
      log.warning('Failed to add search term to $historyKey: $e');
    }
  }
  
  /// Internal method to get search history for specified type
  Future<List<String>> _getSearchHistory(String historyKey) async {
    try {
      final prefs = await SharedPreferences.getInstance();
      final historyJson = prefs.getString(historyKey);
      
      print('SearchHistoryService: Getting history for $historyKey: $historyJson');
      
      if (historyJson != null) {
        final List<dynamic> decodedList = jsonDecode(historyJson);
        final history = decodedList.cast<String>();
        print('SearchHistoryService: Returning history for $historyKey: $history');
        return history;
      }
    } catch (e) {
      print('SearchHistoryService: Failed to get search history for $historyKey: $e');
    }
    
    print('SearchHistoryService: Returning empty history for $historyKey');
    return [];
  }
  
  /// Internal method to clear search history for specified type
  Future<void> _clearSearchHistory(String historyKey) async {
    try {
      final prefs = await SharedPreferences.getInstance();
      await prefs.remove(historyKey);
      print('SearchHistoryService: Cleared search history for $historyKey');
    } catch (e) {
      print('SearchHistoryService: Failed to clear search history for $historyKey: $e');
    }
  }
  
  /// Internal method to remove specific term from history
  Future<void> _removeSearchTerm(String historyKey, String searchTerm) async {
    try {
      final prefs = await SharedPreferences.getInstance();
      final historyJson = prefs.getString(historyKey);
      
      if (historyJson == null) return;
      
      final List<dynamic> decodedList = jsonDecode(historyJson);
      List<String> history = decodedList.cast<String>();
      history.remove(searchTerm);
      
      if (history.isEmpty) {
        await prefs.remove(historyKey);
      } else {
        await prefs.setString(historyKey, jsonEncode(history));
      }
      
      print('SearchHistoryService: Removed "$searchTerm" from $historyKey');
    } catch (e) {
      print('SearchHistoryService: Failed to remove search term from $historyKey: $e');
    }
  }
}