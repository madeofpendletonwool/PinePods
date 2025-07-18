// lib/services/pinepods/pinepods_audio_service.dart

import 'dart:async';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/entities/chapter.dart';
import 'package:pinepods_mobile/entities/person.dart';
import 'package:pinepods_mobile/entities/transcript.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:logging/logging.dart';

class PinepodsAudioService {
  final log = Logger('PinepodsAudioService');
  final AudioPlayerService _audioPlayerService;
  final PinepodsService _pinepodsService;
  final SettingsBloc _settingsBloc;

  Timer? _episodeUpdateTimer;
  Timer? _userStatsTimer;
  int? _currentEpisodeId;
  int? _currentUserId;
  double _lastRecordedPosition = 0;

  PinepodsAudioService(
    this._audioPlayerService,
    this._pinepodsService,
    this._settingsBloc,
  );

  /// Play a PinePods episode with full server integration
  Future<void> playPinepodsEpisode({
    required PinepodsEpisode pinepodsEpisode,
    bool resume = true,
  }) async {
    try {
      final settings = _settingsBloc.currentSettings;
      final userId = settings.pinepodsUserId;

      if (userId == null) {
        log.warning('No user ID found - cannot play episode with server tracking');
        return;
      }

      _currentUserId = userId;

      log.info('Starting PinePods episode playback: ${pinepodsEpisode.episodeTitle}');

      // Use the episode ID that's already available from the PinepodsEpisode
      final episodeId = pinepodsEpisode.episodeId;

      if (episodeId == 0) {
        log.warning('Episode ID is 0 - cannot track playback');
        return;
      }

      _currentEpisodeId = episodeId;

      // Get podcast ID for settings
      final podcastId = await _pinepodsService.getPodcastIdFromEpisode(
        episodeId,
        userId,
        pinepodsEpisode.isYoutube,
      );

      // Get playback settings (speed, skip times)
      final playDetails = await _pinepodsService.getPlayEpisodeDetails(
        userId,
        podcastId,
        pinepodsEpisode.isYoutube,
      );

      // Fetch podcast 2.0 data including chapters
      final podcast2Data = await _pinepodsService.fetchPodcasting2Data(episodeId, userId);
      
      // Convert PinepodsEpisode to Episode for the audio player
      final episode = _convertToEpisode(pinepodsEpisode, playDetails, podcast2Data);

      // Set playback speed
      await _audioPlayerService.setPlaybackSpeed(playDetails.playbackSpeed);

      // Start playing with the existing audio service
      await _audioPlayerService.playEpisode(episode: episode, resume: resume);

      // Handle skip intro if enabled and episode just started
      if (playDetails.startSkip > 0 && !resume) {
        await Future.delayed(const Duration(milliseconds: 500)); // Wait for player to initialize
        await _audioPlayerService.seek(position: playDetails.startSkip);
      }

      // Add to history
      log.info('Adding episode $episodeId to history for user $userId');
      await _pinepodsService.addHistory(
        episodeId,
        resume ? (pinepodsEpisode.listenDuration ?? 0).toDouble() : 0,
        userId,
        pinepodsEpisode.isYoutube,
      );

      // Queue episode for tracking
      log.info('Queueing episode $episodeId for user $userId');
      await _pinepodsService.queueEpisode(
        episodeId,
        userId,
        pinepodsEpisode.isYoutube,
      );

      // Increment played count
      log.info('Incrementing played count for user $userId');
      await _pinepodsService.incrementPlayed(userId);

      // Start periodic updates
      _startPeriodicUpdates();

      log.info('PinePods episode playback started successfully');
    } catch (e) {
      log.severe('Error playing PinePods episode: $e');
      rethrow;
    }
  }

  /// Start periodic updates to server
  void _startPeriodicUpdates() {
    _stopPeriodicUpdates(); // Clean up any existing timers

    // Episode position updates every 30 seconds
    _episodeUpdateTimer = Timer.periodic(
      const Duration(seconds: 30),
      (_) => _updateEpisodePosition(),
    );

    // User listen time updates every 60 seconds
    _userStatsTimer = Timer.periodic(
      const Duration(seconds: 60),
      (_) => _updateUserListenTime(),
    );
  }

  /// Update episode position on server
  Future<void> _updateEpisodePosition() async {
    if (_currentEpisodeId == null || _currentUserId == null) return;

    try {
      final positionState = _audioPlayerService.playPosition?.value;
      if (positionState == null) return;

      final currentPosition = positionState.position.inSeconds.toDouble();
      
      // Only update if position has changed significantly
      if ((currentPosition - _lastRecordedPosition).abs() > 5) {
        await _pinepodsService.addHistory(
          _currentEpisodeId!,
          currentPosition,
          _currentUserId!,
          false, // Assume not YouTube for now
        );
        
        _lastRecordedPosition = currentPosition;
        log.fine('Updated episode position: ${currentPosition}s');
      }
    } catch (e) {
      log.warning('Failed to update episode position: $e');
    }
  }

  /// Update user listen time statistics
  Future<void> _updateUserListenTime() async {
    if (_currentUserId == null) return;

    try {
      await _pinepodsService.incrementListenTime(_currentUserId!);
      log.fine('Updated user listen time');
    } catch (e) {
      log.warning('Failed to update user listen time: $e');
    }
  }

  /// Record listen duration when episode ends or is stopped
  Future<void> recordListenDuration(double listenDuration) async {
    if (_currentEpisodeId == null || _currentUserId == null) return;

    try {
      await _pinepodsService.recordListenDuration(
        _currentEpisodeId!,
        _currentUserId!,
        listenDuration,
        false, // Assume not YouTube for now
      );
      log.info('Recorded listen duration: ${listenDuration}s');
    } catch (e) {
      log.warning('Failed to record listen duration: $e');
    }
  }

  /// Stop periodic updates
  void _stopPeriodicUpdates() {
    _episodeUpdateTimer?.cancel();
    _userStatsTimer?.cancel();
    _episodeUpdateTimer = null;
    _userStatsTimer = null;
  }

  /// Convert PinepodsEpisode to Episode for the audio player
  Episode _convertToEpisode(PinepodsEpisode pinepodsEpisode, PlayEpisodeDetails playDetails, Map<String, dynamic>? podcast2Data) {
    // Determine the content URL
    String contentUrl;
    if (pinepodsEpisode.downloaded && _currentEpisodeId != null && _currentUserId != null) {
      // Use stream URL for local episodes
      contentUrl = _pinepodsService.getStreamUrl(
        _currentEpisodeId!,
        _currentUserId!,
        isYoutube: pinepodsEpisode.isYoutube,
        isLocal: true,
      );
    } else if (pinepodsEpisode.isYoutube && _currentEpisodeId != null && _currentUserId != null) {
      // Use stream URL for YouTube episodes
      contentUrl = _pinepodsService.getStreamUrl(
        _currentEpisodeId!,
        _currentUserId!,
        isYoutube: true,
        isLocal: false,
      );
    } else {
      // Use original URL for external episodes
      contentUrl = pinepodsEpisode.episodeUrl;
    }

    // Process podcast 2.0 data
    List<Chapter> chapters = [];
    List<Person> persons = [];
    List<TranscriptUrl> transcriptUrls = [];
    String? chaptersUrl;
    
    if (podcast2Data != null) {
      // Extract chapters data
      final chaptersData = podcast2Data['chapters'] as List<dynamic>?;
      if (chaptersData != null) {
        try {
          chapters = chaptersData.map((chapterData) {
            return Chapter(
              title: chapterData['title'] ?? '',
              startTime: _parseDouble(chapterData['startTime'] ?? chapterData['start_time'] ?? 0) ?? 0.0,
              endTime: _parseDouble(chapterData['endTime'] ?? chapterData['end_time']),
              imageUrl: chapterData['img'] ?? chapterData['image'],
              url: chapterData['url'],
              toc: chapterData['toc'] ?? true,
            );
          }).toList();
          
          log.info('Loaded ${chapters.length} chapters from podcast 2.0 data');
        } catch (e) {
          log.warning('Error parsing chapters from podcast 2.0 data: $e');
        }
      }
      
      // Extract chapters URL if available
      chaptersUrl = podcast2Data['chapters_url'];
      
      // Extract persons data
      final personsData = podcast2Data['people'] as List<dynamic>?;
      if (personsData != null) {
        try {
          persons = personsData.map((personData) {
            return Person(
              name: personData['name'] ?? '',
              role: personData['role'] ?? '',
              group: personData['group'] ?? '',
              image: personData['img'],
              link: personData['href'],
            );
          }).toList();
          
          log.info('Loaded ${persons.length} persons from podcast 2.0 data');
        } catch (e) {
          log.warning('Error parsing persons from podcast 2.0 data: $e');
        }
      }
      
      // Extract transcript data
      final transcriptsData = podcast2Data['transcripts'] as List<dynamic>?;
      if (transcriptsData != null) {
        try {
          transcriptUrls = transcriptsData.map((transcriptData) {
            TranscriptFormat format = TranscriptFormat.unsupported;
            
            // Determine format from URL, mime_type, or type field
            final url = transcriptData['url'] ?? '';
            final mimeType = transcriptData['mime_type'] ?? '';
            final type = transcriptData['type'] ?? '';
            
            log.info('Processing transcript: url=$url, mimeType=$mimeType, type=$type');
            
            if (url.toLowerCase().contains('.json') || 
                mimeType.toLowerCase().contains('json') || 
                type.toLowerCase().contains('json')) {
              format = TranscriptFormat.json;
              log.info('Detected JSON transcript format');
            } else if (url.toLowerCase().contains('.srt') || 
                       mimeType.toLowerCase().contains('srt') || 
                       type.toLowerCase().contains('srt') || 
                       type.toLowerCase().contains('subrip') ||
                       url.toLowerCase().contains('subrip')) {
              format = TranscriptFormat.subrip;
              log.info('Detected SubRip transcript format');
            } else {
              log.warning('Transcript format not recognized: mimeType=$mimeType, type=$type');
            }
            
            return TranscriptUrl(
              url: url,
              type: format,
              language: transcriptData['language'] ?? transcriptData['lang'] ?? 'en',
              rel: transcriptData['rel'],
            );
          }).toList();
          
          log.info('Loaded ${transcriptUrls.length} transcript URLs from podcast 2.0 data');
        } catch (e) {
          log.warning('Error parsing transcripts from podcast 2.0 data: $e');
        }
      }
    }

    return Episode(
      guid: pinepodsEpisode.episodeUrl,
      podcast: pinepodsEpisode.podcastName,
      title: pinepodsEpisode.episodeTitle,
      description: pinepodsEpisode.episodeDescription,
      link: pinepodsEpisode.episodeUrl,
      publicationDate: DateTime.tryParse(pinepodsEpisode.episodePubDate) ?? DateTime.now(),
      author: '',
      duration: (pinepodsEpisode.episodeDuration * 1000).round(), // Convert to milliseconds
      contentUrl: contentUrl,
      position: pinepodsEpisode.completed ? 0 : ((pinepodsEpisode.listenDuration ?? 0) * 1000).round(), // Convert to milliseconds, reset to 0 for completed episodes
      imageUrl: pinepodsEpisode.episodeArtwork,
      played: pinepodsEpisode.completed,
      chapters: chapters,
      chaptersUrl: chaptersUrl,
      persons: persons,
      transcriptUrls: transcriptUrls,
    );
  }

  /// Helper method to safely parse double values
  double? _parseDouble(dynamic value) {
    if (value == null) return null;
    if (value is double) return value;
    if (value is int) return value.toDouble();
    if (value is String) {
      try {
        return double.parse(value);
      } catch (e) {
        log.warning('Failed to parse double from string: $value');
        return null;
      }
    }
    return null;
  }

  /// Clean up resources
  void dispose() {
    _stopPeriodicUpdates();
    _currentEpisodeId = null;
    _currentUserId = null;
  }
}

