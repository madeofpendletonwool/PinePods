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

  /// The playlist the current episode is being played from, if any. Used to
  /// continue playback through the playlist when an episode completes.
  /// (NativeAudioPlayerService.setPlaylistContext is a no-op, so we track it here.)
  int? _currentPlaylistId;

  /// Duration (seconds) the server currently has stored for the playing episode.
  /// Used to detect when the decoded length differs so we can correct it.
  int? _currentEpisodeDuration;
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
    _currentPlaylistId = playlistId;
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
    _currentPlaylistId = playlistId;
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
      _currentEpisodeDuration = pinepodsEpisode.episodeDuration;

      // Is there a local download for this episode? If so we play the on-disk
      // file and tolerate the server being unreachable for everything else.
      final localDownload = await _audioPlayerService.findDownloadedEpisode(episodeId);
      final hasLocalDownload = localDownload != null;

      // Get podcast ID for settings, and podcast 2.0 data (chapters/persons/
      // transcripts) in parallel - these two calls don't depend on each other,
      // and running them sequentially only adds latency before playback starts.
      // Both tolerate offline / failures.
      final results = await Future.wait<dynamic>([
        _pinepodsService.getPodcastIdFromEpisode(
          episodeId,
          userId,
          pinepodsEpisode.isYoutube,
        ).catchError((e) {
          log.fine('Could not fetch podcast id (continuing): $e');
          return 0;
        }),
        _pinepodsService.fetchPodcasting2Data(episodeId, userId).catchError((e) {
          log.fine('Could not fetch podcast 2.0 data (continuing): $e');
          return null;
        }),
      ]);
      final podcastId = results[0] as int;
      final podcast2Data = results[1] as Map<String, dynamic>?;

      // Get playback settings (speed, skip times). This already returns sane
      // defaults on failure.
      final playDetails = await _pinepodsService.getPlayEpisodeDetails(
        userId,
        podcastId,
        pinepodsEpisode.isYoutube,
      );

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

      // Silence (#727) + ad-skip (#790): fetch and apply the server-detected
      // skip ranges for this episode. Decoupled from the silence toggle so ads
      // skip even when silence-trim is off.
      await refreshSkipSegments(episodeId, userId, podcastId);

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

  /// Fetch the server-detected skip segments for an episode and push the active
  /// ranges to the native player. Silence (#727) honors the per-podcast toggle
  /// (Android ORs it with the user's global preference on the DSP path); ad
  /// ranges (#790) are gated on the server-resolved per-user status
  /// (active/confirmed => skip) and are independent of the silence toggle.
  ///
  /// Safe to call again mid-episode (e.g. after a Confirm/Deny/Detect in the UI)
  /// to re-supply the native layer — do NOT cache a one-time snapshot.
  Future<void> refreshSkipSegments(int episodeId, int userId, int podcastId) async {
    try {
      final trimSettings = await _pinepodsService.getSilenceTrim(userId, podcastId);
      final fetched = await _pinepodsService.getEpisodeSkipSegments(episodeId, userId);

      final silenceRanges = trimSettings.enabled
          ? fetched
              .where((s) => s.kind == 'silence')
              .map((s) => {'start': s.startTime, 'end': s.endTime})
              .toList()
          : <Map<String, double>>[];
      final adRanges = fetched
          .where((s) => s.isActiveAd)
          .map((s) => {'start': s.startTime, 'end': s.endTime})
          .toList();

      await _audioPlayerService.applyEpisodeSkipSegments(
        silenceEnabled: trimSettings.enabled,
        silenceRanges: silenceRanges,
        adRanges: adRanges,
      );
    } catch (e) {
      log.fine('Could not apply skip segments (continuing): $e');
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

  /// Handle natural end-of-episode: advance to the next thing to play.
  ///
  /// Mirrors the web frontend's `onended` handler
  /// (web/src/components/audio.rs) with a 3-priority advance:
  ///   1. Playlist continuation (if the current episode came from a playlist)
  ///   2. Per-podcast auto-play-next (next episode of the same podcast)
  ///   3. Server-side queue (next queued episode)
  ///
  /// Returns `true` if a next episode was started, `false` if there was nothing
  /// to play (so the caller can stop playback / hide the player). Every network
  /// call is guarded so a transient failure never leaves playback wedged.
  Future<bool> handleEpisodeCompleted() async {
    final episodeId = _currentEpisodeId;
    final userId = _currentUserId;
    if (episodeId == null || userId == null) {
      log.warning('Cannot advance on completion - missing episode/user id');
      return false;
    }

    // Ensure the shared service has credentials (it may have been used by
    // other screens since playback started).
    final settings = _settingsBloc.currentSettings;
    if (settings.pinepodsServer == null || settings.pinepodsApiKey == null) {
      log.warning('Cannot advance on completion - missing server credentials');
      return false;
    }
    _pinepodsService.setCredentials(
      settings.pinepodsServer!,
      settings.pinepodsApiKey!,
    );

    // Mark the finished episode completed on the server. On natural completion
    // nothing else records the final state (periodic sync can be up to 15s
    // stale), so do it explicitly to keep history/queue correct. Fire-and-forget
    // — it doesn't influence which episode plays next, so don't make the user
    // wait on it before playback resumes.
    unawaited(
      _pinepodsService.markEpisodeCompleted(episodeId, userId, _isYoutube).catchError((e) {
        log.fine('Could not mark episode completed (continuing): $e');
        return false;
      }),
    );

    // PRIORITY 1: Playlist continuation.
    final playlistId = _currentPlaylistId;
    if (playlistId != null) {
      try {
        final next = await _pinepodsService.getNextPlaylistEpisode(
          episodeId,
          playlistId,
          userId,
        );
        if (next != null) {
          log.info('Auto-advancing to next playlist episode: ${next.episodeTitle}');
          await playPinepodsEpisode(
            pinepodsEpisode: next,
            resume: false,
            playlistId: playlistId,
          );
          return true;
        }
        // Playlist exhausted - clear the context and fall through.
        log.info('Playlist exhausted, clearing playlist context');
        _currentPlaylistId = null;
      } catch (e) {
        log.fine('Could not get next playlist episode (continuing): $e');
      }
    }

    // PRIORITY 2: Per-podcast auto-play-next.
    try {
      final podcastId = await _pinepodsService.getPodcastIdFromEpisode(
        episodeId,
        userId,
        _isYoutube,
      );
      final autoPlayNext = await _pinepodsService.getAutoPlayNextStatus(
        podcastId,
        userId,
      );
      if (autoPlayNext) {
        final next = await _pinepodsService.getNextPodcastEpisode(episodeId, userId);
        if (next != null) {
          log.info('Auto-play-next enabled, playing next podcast episode: ${next.episodeTitle}');
          await playPinepodsEpisode(
            pinepodsEpisode: next,
            resume: false,
            // Auto-play-next episodes shouldn't be added to the queue.
            skipQueue: true,
          );
          return true;
        }
        log.info('No next episode found in podcast, falling through to queue');
      }
    } catch (e) {
      log.fine('Could not evaluate auto-play-next (continuing): $e');
    }

    // PRIORITY 3: Server-side queue.
    try {
      final queued = await _pinepodsService.getQueuedEpisodes(userId);
      log.info('Found ${queued.length} episodes in queue');

      // Backend returns episodes ORDER BY queueposition ASC, so the first entry
      // that isn't the just-finished episode (and isn't already completed) is
      // next. Pick it locally so we don't need a second round-trip.
      PinepodsEpisode? next;
      for (final ep in queued) {
        if (ep.episodeId != episodeId && !ep.completed) {
          next = ep;
          break;
        }
      }

      // Drop the finished episode and any completed leftovers from the queue in
      // the background so we don't block playback of the next episode on these
      // round-trips.
      for (final ep in queued) {
        if (ep.episodeId == episodeId || ep.completed) {
          unawaited(
            _pinepodsService.removeQueuedEpisode(ep.episodeId, userId, ep.isYoutube).catchError((e) {
              log.fine('Could not remove queued episode ${ep.episodeId}: $e');
              return false;
            }),
          );
        }
      }

      if (next == null) {
        log.info('Queue empty after cleanup - stopping playback');
        return false;
      }

      log.info('Auto-advancing to next queued episode: ${next.episodeTitle}');
      await playPinepodsEpisode(pinepodsEpisode: next, resume: false);
      return true;
    } catch (e) {
      log.warning('Failed to advance from queue on completion: $e');
      return false;
    }
  }

  /// Correct the server's stored episode duration to the real decoded length.
  ///
  /// Feeds frequently ship a missing or zero itunes:duration, which leaves the
  /// episode with episodeduration = 0 in the database. The web player already
  /// corrects this on play; without the equivalent here, playing such an episode
  /// on mobile records a listen position against a zero duration and leaves the
  /// row in a state that crashes the web frontend (divide-by-zero). Mirroring the
  /// web behaviour keeps durations accurate and stops the bad rows being created.
  ///
  /// [actualDurationSeconds] is the decoded length reported by the player.
  Future<void> updateEpisodeDurationIfNeeded(double actualDurationSeconds) async {
    final episodeId = _currentEpisodeId;
    if (episodeId == null) return;

    // Ignore bogus/unavailable durations from the decoder.
    if (!actualDurationSeconds.isFinite || actualDurationSeconds <= 0) return;

    final newDuration = actualDurationSeconds.round();

    // Only correct when it actually differs from what the server has, and avoid
    // resending on every play once corrected.
    if (_currentEpisodeDuration != null && _currentEpisodeDuration == newDuration) {
      return;
    }

    try {
      final ok = await _pinepodsService.updateEpisodeDuration(
        episodeId,
        newDuration,
        _isYoutube,
      );
      if (ok) {
        _currentEpisodeDuration = newDuration;
        log.info('Corrected episode $episodeId duration to ${newDuration}s');
      }
    } catch (e) {
      log.fine('Could not update episode duration (continuing): $e');
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
    _currentPlaylistId = null;
    _currentEpisodeDuration = null;
  }
}

