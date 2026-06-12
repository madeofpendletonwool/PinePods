// lib/services/pinepods/pinepods_audio_service.dart

import 'dart:async';
import 'package:pinepods_mobile/entities/downloadable.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/entities/chapter.dart';
import 'package:pinepods_mobile/entities/person.dart';
import 'package:pinepods_mobile/entities/transcript.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/services/offline/offline_action_queue.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:logging/logging.dart';

class PinepodsAudioService {
  final log = Logger('PinepodsAudioService');
  final AudioPlayerService _audioPlayerService;
  final PinepodsService _pinepodsService;
  final SettingsBloc _settingsBloc;

  /// Offline outbox. When set, interaction recording (progress/history) is
  /// routed through it so it syncs even when the device was offline.
  OfflineActionQueue? _actionQueue;

  Timer? _episodeUpdateTimer;
  Timer? _userStatsTimer;
  int? _currentEpisodeId;
  int? _currentUserId;
  bool _isYoutube = false;
  double _lastRecordedPosition = 0;
  bool _isSyncingPosition = false;
  bool _hasPendingPositionSync = false;
  
  /// Callbacks for pause/stop events
  Function()? _onPauseCallback;
  Function()? _onStopCallback;

  PinepodsAudioService(
    this._audioPlayerService,
    this._pinepodsService,
    this._settingsBloc, {
    Function()? onPauseCallback,
    Function()? onStopCallback,
  }) : _onPauseCallback = onPauseCallback,
       _onStopCallback = onStopCallback;

  /// Wire up the offline outbox (called once during app start-up).
  void setActionQueue(OfflineActionQueue queue) {
    _actionQueue = queue;
  }

  void setPlaylistContext(int? playlistId) {
    _audioPlayerService.setPlaylistContext(playlistId);
  }

  /// Record a playback position, routing through the offline outbox when one is
  /// configured so the update survives being offline. Falls back to a direct
  /// (error-isolated) call otherwise.
  Future<void> _recordPosition(int episodeId, int userId, double positionSeconds) async {
    final queue = _actionQueue;
    if (queue != null) {
      await queue.enqueuePosition(episodeId, userId, positionSeconds, _isYoutube);
      return;
    }
    try {
      await _pinepodsService.recordListenDuration(episodeId, userId, positionSeconds, _isYoutube);
    } catch (e) {
      log.fine('Could not record position (continuing): $e');
    }
  }

  /// Play a PinePods episode with full server integration
  Future<void> playPinepodsEpisode({
    required PinepodsEpisode pinepodsEpisode,
    bool resume = true,
    bool skipQueue = false,
    int? playlistId,
  }) async {
    _audioPlayerService.setPlaylistContext(playlistId);
    try {
      final settings = _settingsBloc.currentSettings;
      final userId = settings.pinepodsUserId;

      if (userId == null) {
        log.warning('No user ID found - cannot play episode with server tracking');
        return;
      }

      _currentUserId = userId;
      _isYoutube = pinepodsEpisode.isYoutube;

      // Use the episode ID that's already available from the PinepodsEpisode
      final episodeId = pinepodsEpisode.episodeId;

      if (episodeId == 0) {
        log.warning('Episode ID is 0 - cannot track playback');
        return;
      }

      _currentEpisodeId = episodeId;

      // Is there a local download for this episode? If so we play the on-disk
      // file and tolerate the server being unreachable for everything else.
      final localDownload = await _audioPlayerService.findDownloadedEpisode(episodeId);
      final hasLocalDownload = localDownload != null;

      // Get podcast ID for settings (tolerate offline / failures)
      int podcastId = 0;
      try {
        podcastId = await _pinepodsService.getPodcastIdFromEpisode(
          episodeId,
          userId,
          pinepodsEpisode.isYoutube,
        );
      } catch (e) {
        log.fine('Could not fetch podcast id (continuing): $e');
      }

      // Get playback settings (speed, skip times). This already returns sane
      // defaults on failure.
      final playDetails = await _pinepodsService.getPlayEpisodeDetails(
        userId,
        podcastId,
        pinepodsEpisode.isYoutube,
      );

      // Fetch podcast 2.0 data including chapters (tolerate offline / failures)
      Map<String, dynamic>? podcast2Data;
      try {
        podcast2Data = await _pinepodsService.fetchPodcasting2Data(episodeId, userId);
      } catch (e) {
        log.fine('Could not fetch podcast 2.0 data (continuing): $e');
      }

      // Convert PinepodsEpisode to Episode for the audio player
      final episode = _convertToEpisode(pinepodsEpisode, playDetails, podcast2Data);

      // Prefer the locally-downloaded file when one exists so playback works
      // offline and avoids needless streaming.
      //
      // We set only downloadState (which is what selects the on-disk file) plus
      // the file location. We deliberately do NOT set downloadPercentage = 100:
      // this is a transient playback record whose guid is the content URL, not
      // 'pinepods_<id>'. Marking it as a complete download would make it show up
      // as a SECOND entry in the downloads list (with a bogus duration, since
      // this record's duration is in milliseconds). The real 'pinepods_<id>'
      // record stays the single source of truth for the downloads list.
      if (hasLocalDownload) {
        episode.downloadState = DownloadState.downloaded;
        episode.filepath = localDownload.filepath;
        episode.filename = localDownload.filename;
        log.info('Playing local download for episode $episodeId');
      }

      // Start playing with the existing audio service
      await _audioPlayerService.playEpisode(episode: episode, resume: resume);

      // Apply server-side speed after episode starts — overrides any locally stored speed
      await _audioPlayerService.setPlaybackSpeed(playDetails.playbackSpeed);

      // Handle skip intro if enabled and episode just started
      if (playDetails.startSkip > 0 && !resume) {
        await Future.delayed(const Duration(milliseconds: 500)); // Wait for player to initialize
        await _audioPlayerService.seek(position: playDetails.startSkip);
      }

      // Add to history. Routes through the offline outbox so it is not lost when
      // the device is offline (e.g. playing a local download on a plane).
      final initialPosition = resume ? (pinepodsEpisode.listenDuration ?? 0).toDouble() : 0.0;
      await _recordPosition(episodeId, userId, initialPosition);

      // Queue episode for tracking (skip if auto-play-next is enabled or explicitly skipped)
      bool shouldQueue = !skipQueue;
      if (shouldQueue) {
        try {
          final autoPlayNext = await _pinepodsService.getAutoPlayNextStatus(podcastId, userId);
          if (autoPlayNext) {
            shouldQueue = false;
          }
        } catch (e) {
          log.fine('Could not check auto-play-next status: $e');
        }
      }
      if (shouldQueue) {
        try {
          await _pinepodsService.queueEpisode(
            episodeId,
            userId,
            pinepodsEpisode.isYoutube,
          );
        } catch (e) {
          log.fine('Could not queue episode (continuing): $e');
        }
      }

      // Increment played count (tolerate offline / failures)
      try {
        await _pinepodsService.incrementPlayed(userId);
      } catch (e) {
        log.fine('Could not increment played count (continuing): $e');
      }

      // Start periodic updates
      _startPeriodicUpdates();
    } catch (e) {
      log.severe('Error playing PinePods episode: $e');
      rethrow;
    }
  }

  /// Start periodic updates to server
  void _startPeriodicUpdates() {
    _stopPeriodicUpdates(); // Clean up any existing timers

    // Episode position updates every 15 seconds (more frequent for reliability)
    _episodeUpdateTimer = Timer.periodic(
      const Duration(seconds: 15),
      (_) => _safeUpdateEpisodePosition(),
    );

    // User listen time updates every 60 seconds
    _userStatsTimer = Timer.periodic(
      const Duration(seconds: 60),
      (_) => _safeUpdateUserListenTime(),
    );
  }

  /// Safely update episode position without affecting playback
  void _safeUpdateEpisodePosition() async {
    // If already syncing, mark that we have a pending sync and return
    // The current sync will check this flag when done and re-sync with latest position
    if (_isSyncingPosition) {
      _hasPendingPositionSync = true;
      log.fine('Position sync in progress - marked for re-sync with latest position');
      return;
    }

    _isSyncingPosition = true;
    try {
      await _updateEpisodePosition();

      // Check if another sync was requested while we were syncing
      if (_hasPendingPositionSync) {
        _hasPendingPositionSync = false;
        log.fine('Re-syncing with latest position after pending request');
        await _updateEpisodePosition(); // Sync again with the LATEST position
      }
    } catch (e) {
      log.warning('Periodic sync completely failed but playback continues: $e');
      // Completely isolate any network failures from affecting playback
    } finally {
      _isSyncingPosition = false;
    }
  }

  /// Update episode position on server
  Future<void> _updateEpisodePosition() async {
    // Updating episode position
    if (_currentEpisodeId == null || _currentUserId == null) {
      log.warning('Skipping scheduled sync - missing episode ID ($_currentEpisodeId) or user ID ($_currentUserId)');
      return;
    }

    try {
      final positionState = _audioPlayerService.playPosition?.value;
      if (positionState == null) return;

      final currentPosition = positionState.position.inSeconds.toDouble();
      
      // Only update if position has changed by more than 2 seconds (more responsive)
      if ((currentPosition - _lastRecordedPosition).abs() > 2) {
        await _recordPosition(_currentEpisodeId!, _currentUserId!, currentPosition);
        _lastRecordedPosition = currentPosition;
      }
    } catch (e) {
      log.warning('Failed to update episode position: $e');
    }
  }

  /// Safely update user listen time without affecting playback
  void _safeUpdateUserListenTime() async {
    try {
      await _updateUserListenTime();
    } catch (e) {
      log.warning('User stats sync completely failed but playback continues: $e');
      // Completely isolate any network failures from affecting playback
    }
  }

  /// Update user listen time statistics
  Future<void> _updateUserListenTime() async {
    if (_currentUserId == null) return;

    try {
      await _pinepodsService.incrementListenTime(_currentUserId!);
      // User listen time updated
    } catch (e) {
      log.warning('Failed to update user listen time: $e');
    }
  }

  /// Sync current position to server immediately (for pause/stop events)
  Future<void> syncCurrentPositionToServer() async {
    // Syncing current position to server
    
    if (_currentEpisodeId == null || _currentUserId == null) {
      log.warning('Cannot sync - missing episode ID ($_currentEpisodeId) or user ID ($_currentUserId)');
      return;
    }

    try {
      final positionState = _audioPlayerService.playPosition?.value;
      if (positionState == null) {
        log.warning('Cannot sync - positionState is null');
        return;
      }

      final currentPosition = positionState.position.inSeconds.toDouble();

      log.info('Syncing position to server: ${currentPosition}s for episode $_currentEpisodeId');

      await _recordPosition(_currentEpisodeId!, _currentUserId!, currentPosition);

      _lastRecordedPosition = currentPosition;
      log.info('Successfully synced position (queued if offline): ${currentPosition}s');
    } catch (e) {
      log.warning('Failed to sync position to server: $e');
      log.warning('Stack trace: ${StackTrace.current}');
    }
  }

  /// Get server position for current episode
  Future<double?> getServerPosition() async {
    if (_currentEpisodeId == null || _currentUserId == null) return null;

    try {
      final episodeMetadata = await _pinepodsService.getEpisodeMetadata(
        _currentEpisodeId!,
        _currentUserId!,
        isYoutube: _isYoutube,
      );
      
      return episodeMetadata?.listenDuration?.toDouble();
    } catch (e) {
      log.warning('Failed to get server position: $e');
      return null;
    }
  }

  /// Get server position for any episode
  Future<double?> getServerPositionForEpisode(int episodeId, int userId, bool isYoutube) async {
    try {
      final episodeMetadata = await _pinepodsService.getEpisodeMetadata(
        episodeId,
        userId,
        isYoutube: isYoutube,
      );
      
      return episodeMetadata?.listenDuration?.toDouble();
    } catch (e) {
      log.warning('Failed to get server position for episode $episodeId: $e');
      return null;
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
        _isYoutube,
      );
      log.info('Recorded listen duration: ${listenDuration}s');
    } catch (e) {
      log.warning('Failed to record listen duration: $e');
    }
  }

  /// Handle pause event - sync position to server
  Future<void> onPause() async {
    try {
      await syncCurrentPositionToServer();
      log.info('Pause event handled - position synced to server');
    } catch (e) {
      log.warning('Pause sync failed but pause succeeded: $e');
    }
    _onPauseCallback?.call();
  }

  /// Handle stop event - sync position to server
  Future<void> onStop() async {
    try {
      await syncCurrentPositionToServer();
      log.info('Stop event handled - position synced to server');
    } catch (e) {
      log.warning('Stop sync failed but stop succeeded: $e');
    }
    _onStopCallback?.call();
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
            
            // Processing transcript
            
            if (url.toLowerCase().contains('.json') || 
                mimeType.toLowerCase().contains('json') || 
                type.toLowerCase().contains('json')) {
              format = TranscriptFormat.json;
              // Detected JSON transcript
            } else if (url.toLowerCase().contains('.srt') || 
                       mimeType.toLowerCase().contains('srt') || 
                       type.toLowerCase().contains('srt') || 
                       type.toLowerCase().contains('subrip') ||
                       url.toLowerCase().contains('subrip')) {
              format = TranscriptFormat.subrip;
              // Detected SubRip transcript
            } else if (url.toLowerCase().contains('transcript') || 
                       mimeType.toLowerCase().contains('html') || 
                       type.toLowerCase().contains('html')) {
              format = TranscriptFormat.html;
              // Detected HTML transcript
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

