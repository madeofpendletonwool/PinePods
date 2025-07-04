// lib/services/pinepods/pinepods_audio_service.dart

import 'dart:async';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/podcast.dart';
import 'package:pinepods_mobile/entities/funding.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
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

      // Convert PinepodsEpisode to Episode for the audio player
      final episode = _convertToEpisode(pinepodsEpisode, playDetails);

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
  Episode _convertToEpisode(PinepodsEpisode pinepodsEpisode, PlayEpisodeDetails playDetails) {
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
      position: ((pinepodsEpisode.listenDuration ?? 0) * 1000).round(), // Convert to milliseconds
      imageUrl: pinepodsEpisode.episodeArtwork,
      played: pinepodsEpisode.completed,
    );
  }

  /// Clean up resources
  void dispose() {
    _stopPeriodicUpdates();
    _currentEpisodeId = null;
    _currentUserId = null;
  }
}

