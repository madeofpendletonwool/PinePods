// Native audio player service using platform channels instead of just_audio/audio_service

import 'dart:async';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:logging/logging.dart';
import 'package:pinepods_mobile/core/utils.dart';
import 'package:pinepods_mobile/entities/chapter.dart';
import 'package:pinepods_mobile/entities/downloadable.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/sleep.dart';
import 'package:pinepods_mobile/entities/transcript.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/entities/home_data.dart';
import 'package:pinepods_mobile/repository/repository.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_audio_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/global_services.dart';
import 'package:pinepods_mobile/services/settings/settings_service.dart';
import 'package:pinepods_mobile/state/queue_event_state.dart';
import 'package:pinepods_mobile/state/transcript_state_event.dart';
import 'package:rxdart/rxdart.dart';

class NativeAudioPlayerService extends AudioPlayerService {
  static const platform = MethodChannel('com.pinepods/audio_player');
  static const eventChannel = EventChannel('com.pinepods/audio_events');

  final log = Logger('NativeAudioPlayerService');
  final Repository repository;
  final SettingsService settingsService;
  PinepodsAudioService? _pinepodsAudioService;

  var _playbackSpeed = 1.0;
  var _trimSilence = false;
  var _volumeBoost = false;
  var _queue = <Episode>[];
  var _sleep = Sleep(type: SleepType.none);
  var _sleepEpisodesRemaining = 0;

  Episode? _currentEpisode;
  Transcript? _currentTranscript;

  DateTime? _episodeStartTime;
  Timer? _localPositionTimer;

  StreamSubscription<int>? _positionSubscription;
  StreamSubscription<int>? _sleepSubscription;
  StreamSubscription? _nativeEventSubscription;

  final BehaviorSubject<AudioState> _playingState = BehaviorSubject<AudioState>.seeded(AudioState.none);
  final _durationTicker = Stream<int>.periodic(const Duration(milliseconds: 500), (count) => count).asBroadcastStream();
  final _sleepTicker = Stream<int>.periodic(const Duration(milliseconds: 500), (count) => count).asBroadcastStream();
  final _playPosition = BehaviorSubject<PositionState>();
  final _episodeEvent = BehaviorSubject<Episode?>(sync: true);
  final _transcriptEvent = BehaviorSubject<TranscriptState>(sync: true);
  final _playbackError = PublishSubject<int>();
  final _queueState = BehaviorSubject<QueueListState>();
  final _sleepState = BehaviorSubject<Sleep>();

  NativeAudioPlayerService({
    required this.repository,
    required this.settingsService,
  }) {
    _init();
  }

  void _init() {
    log.info('Initializing NativeAudioPlayerService');

    // Set up method call handler for Android Auto / CarPlay browsing
    platform.setMethodCallHandler(_handleMethodCall);

    // Set up native log handler to capture Android/iOS logs
    const nativeLogChannel = MethodChannel('com.pinepods/native_logs');
    nativeLogChannel.setMethodCallHandler(_handleNativeLog);

    // Defer subscription — on Android, configureFlutterEngine (which registers the
    // plugin) runs on the native thread and may not finish before Dart starts.
    // Retry with increasing delays until the plugin is ready (max ~5s total).
    _subscribeToEventChannelWithRetry();

    _loadQueue();
  }

  void _subscribeToEventChannelWithRetry([int attempt = 0]) {
    // Delays: 0, 200, 400, 600, 800, 1000ms... capped at 1000ms per attempt (max 10 tries ~6s)
    final delay = attempt == 0
        ? Duration.zero
        : Duration(milliseconds: (200 * attempt).clamp(200, 1000));

    Future.delayed(delay, () {
      try {
        _nativeEventSubscription = eventChannel.receiveBroadcastStream().listen(
          _handleNativeEvent,
          onError: (error) => log.severe('Native event stream error: $error'),
        );
        log.info('Subscribed to native event channel (attempt ${attempt + 1})');
      } catch (e) {
        if (attempt < 10) {
          log.fine('Event channel not ready yet (attempt $attempt), retrying...');
          _subscribeToEventChannelWithRetry(attempt + 1);
        } else {
          log.severe('Failed to subscribe to native event channel after ${attempt + 1} attempts: $e');
        }
      }
    });
  }

  /// Handle native logs from Android/iOS and forward to Flutter logger
  Future<void> _handleNativeLog(MethodCall call) async {
    if (call.method == 'log') {
      final level = call.arguments['level'] as String?;
      final tag = call.arguments['tag'] as String?;
      final message = call.arguments['message'] as String?;

      if (level != null && tag != null && message != null) {
        // Forward to the standard logger with [NATIVE] prefix
        final logMessage = '[$tag] $message';
        switch (level) {
          case 'DEBUG':
            log.fine(logMessage);
            break;
          case 'INFO':
            log.info(logMessage);
            break;
          case 'WARN':
            log.warning(logMessage);
            break;
          case 'ERROR':
            log.severe(logMessage);
            break;
          default:
            log.info(logMessage);
        }
      }
    }
  }

  /// Handle method calls from native (Android Auto / CarPlay browsing)
  Future<dynamic> _handleMethodCall(MethodCall call) async {
    log.info('Received method call from native: ${call.method}');

    try {
      switch (call.method) {
        case 'getCurrent':
          return await _getCurrentForCar();
        case 'getQueue':
          return await _getQueueForCar();
        case 'getDownloads':
          return await _getDownloadsForCar();
        case 'getSaved':
          return await _getSavedForCar();
        case 'getHistory':
          return await _getHistoryForCar();
        case 'getPodcasts':
          return await _getPodcastsForCar();
        case 'getPodcastEpisodes':
          final podcastId = call.arguments['podcastId'] as String;
          return await _getPodcastEpisodesForCar(podcastId);
        case 'getPlaylists':
          return await _getPlaylistsForCar();
        case 'getPlaylistEpisodes':
          final playlistId = call.arguments['playlistId'] as String;
          return await _getPlaylistEpisodesForCar(playlistId);
        case 'playFromMediaId':
          final guid = call.arguments['guid'] as String;
          await _playFromMediaIdForCar(guid);
          return null;
        case 'search':
          final query = call.arguments['query'] as String;
          return await _searchForCar(query);
        default:
          log.warning('Unhandled method call: ${call.method}');
          return null;
      }
    } catch (e) {
      log.severe('Error handling method call ${call.method}: $e');
      rethrow;
    }
  }

  void _handleNativeEvent(dynamic event) {
    if (event is! Map) return;

    final type = event['type'] as String?;
    if (type == null) return;

    switch (type) {
      case 'playbackState':
        _handlePlaybackStateEvent(event);
        break;
      case 'error':
        _handleErrorEvent(event);
        break;
      case 'completed':
        _handleCompletedEvent();
        break;
      case 'mediaButtonAction':
        _handleMediaButtonAction(event);
        break;
    }
  }

  void _handlePlaybackStateEvent(Map<dynamic, dynamic> event) {
    final state = event['state'] as String?;
    final position = event['position'] as int? ?? 0;
    final bufferedPosition = event['bufferedPosition'] as int? ?? 0;
    final duration = event['duration'] as int? ?? 0;

    // Update playing state
    if (state != null) {
      final audioState = _parseState(state);
      _playingState.add(audioState);
    }

    // Update position
    if (_currentEpisode != null) {
      _playPosition.add(PositionState(
        position: Duration(milliseconds: position),
        length: Duration(milliseconds: duration),
        percentage: duration > 0 ? ((position / duration) * 100).toInt() : 0,
        episode: _currentEpisode,
        buffering: state == 'buffering',
      ));

      // Update chapter if needed
      _updateChapter(position ~/ 1000, duration ~/ 1000);
    }
  }

  AudioState _parseState(String state) {
    switch (state) {
      case 'playing':
        return AudioState.playing;
      case 'paused':
        return AudioState.pausing;
      case 'buffering':
        return AudioState.buffering;
      case 'stopped':
        return AudioState.stopped;
      case 'error':
        return AudioState.error;
      default:
        return AudioState.none;
    }
  }

  void _handleErrorEvent(Map<dynamic, dynamic> event) {
    final code = event['code'] as int? ?? -1;
    final message = event['message'] as String? ?? 'Unknown error';
    log.severe('Playback error $code: $message');
    _playbackError.add(code);
    _playingState.add(AudioState.error);
  }

  void _handleCompletedEvent() {
    log.info('Episode completed');
    if (_currentEpisode != null) {
      _currentEpisode!.played = true;
      _currentEpisode!.position = 0;
      repository.saveEpisode(_currentEpisode!);
    }

    // Check sleep timer
    if (_sleep.type == SleepType.episode) {
      _sleepEpisodesRemaining--;
      if (_sleepEpisodesRemaining <= 0) {
        log.info('Sleep timer triggered - episode count reached');
        stop();
        return;
      }
    }

    // Play next episode from queue
    if (_queue.isNotEmpty) {
      final nextEpisode = _queue.removeAt(0);
      _updateQueueState();
      playEpisode(episode: nextEpisode, resume: false);
    } else {
      _playingState.add(AudioState.stopped);
    }
  }

  void _handleMediaButtonAction(Map<dynamic, dynamic> event) {
    final action = event['action'] as String?;
    log.info('Media button action: $action');
    // Media button actions are already handled by native code
    // This is just for logging/tracking
  }

  @override
  Future<void> playEpisode({required Episode episode, bool resume = true}) async {
    log.info('playEpisode: ${episode.title}, resume: $resume');

    _currentEpisode = episode;
    _currentEpisode!.played = false;

    // Get URI (local file or stream)
    final uri = await _generateEpisodeUri(episode);
    if (uri == null) {
      log.severe('Failed to generate episode URI');
      _playingState.add(AudioState.error);
      return;
    }

    // Get best position (furthest of local vs server)
    final bestPosition = resume ? await _getBestEpisodePosition(episode) : 0;
    if (bestPosition > _currentEpisode!.position) {
      _currentEpisode!.position = bestPosition;
    }

    await repository.saveEpisode(_currentEpisode!);

    // Update state
    _playingState.add(AudioState.buffering);
    _updateQueueState();
    _updateEpisodeState();

    // Set playback settings
    _playbackSpeed = settingsService.playbackSpeed;
    _trimSilence = settingsService.trimSilence;
    _volumeBoost = settingsService.volumeBoost;

    // Load chapters and transcript
    await _loadChaptersAndTranscript();

    try {
      // Call native platform to play
      await platform.invokeMethod('playEpisode', {
        'url': uri,
        'startPosition': _currentEpisode!.position,
        'isLocal': episode.downloadState == DownloadState.downloaded,
        'metadata': {
          'title': episode.title ?? 'Unknown',
          'artist': episode.podcast ?? 'Unknown',
          'artwork': episode.imageUrl,
          'duration': episode.duration * 1000,
        },
      });

      // Apply settings
      await platform.invokeMethod('setPlaybackSpeed', {'speed': _playbackSpeed});
      if (Platform.isAndroid) {
        await platform.invokeMethod('setTrimSilence', {'enabled': _trimSilence});
        await platform.invokeMethod('setVolumeBoost', {'enabled': _volumeBoost});
      }

      // Start tracking
      _episodeStartTime = DateTime.now();
      _startLocalPositionSaver();

    } catch (e) {
      log.severe('Error playing episode: $e');
      _playingState.add(AudioState.error);
    }
  }

  @override
  Future<void> play() async {
    log.info('play');
    _episodeStartTime = DateTime.now();
    _startLocalPositionSaver();

    try {
      await platform.invokeMethod('play');
    } catch (e) {
      log.severe('Error playing: $e');
    }
  }

  @override
  Future<void> pause() async {
    log.info('pause - starting');

    // Save position immediately
    try {
      await _saveLocalPosition();
      log.info('pause - position saved locally');
    } catch (e) {
      log.warning('Failed to save position before pause: $e');
    }

    // Pause immediately
    try {
      await platform.invokeMethod('pause');
      log.info('pause - native player paused');
    } catch (e) {
      log.severe('Error pausing: $e');
    }

    // Stop position saver
    _stopLocalPositionSaver();

    // Background sync (non-blocking)
    log.info('pause - starting background sync');
    _performBackgroundSync();
  }

  @override
  Future<void> stop() async {
    log.info('stop');

    try {
      await platform.invokeMethod('stop');
    } catch (e) {
      log.severe('Error stopping: $e');
    }

    _stopLocalPositionSaver();
    await _saveLocalPosition();
    await _recordListenDuration();

    if (_pinepodsAudioService != null) {
      try {
        await _pinepodsAudioService!.onPause();
      } catch (e) {
        log.warning('Failed to sync on stop: $e');
      }
    }

    _currentEpisode = null;
    _playingState.add(AudioState.stopped);
  }

  @override
  Future<void> rewind() async {
    log.info('rewind');
    try {
      await platform.invokeMethod('rewind', {'milliseconds': 10000});
    } catch (e) {
      log.severe('Error rewinding: $e');
    }
  }

  @override
  Future<void> fastForward() async {
    log.info('fastForward');
    try {
      await platform.invokeMethod('fastForward', {'milliseconds': 30000});
    } catch (e) {
      log.severe('Error fast forwarding: $e');
    }
  }

  @override
  Future<void> seek({required int position}) async {
    log.info('seek to $position seconds');
    try {
      // Convert seconds to milliseconds for native player
      final positionMs = position * 1000;
      await platform.invokeMethod('seek', {'position': positionMs});
    } catch (e) {
      log.severe('Error seeking: $e');
    }
  }

  @override
  Future<void> setPlaybackSpeed(double speed) async {
    log.info('setPlaybackSpeed: $speed');
    _playbackSpeed = speed;
    settingsService.playbackSpeed = speed;

    try {
      await platform.invokeMethod('setPlaybackSpeed', {'speed': speed});
    } catch (e) {
      log.severe('Error setting playback speed: $e');
    }
  }

  @override
  Future<void> trimSilence(bool trim) async {
    log.info('trimSilence: $trim');
    _trimSilence = trim;
    settingsService.trimSilence = trim;

    if (Platform.isAndroid) {
      try {
        await platform.invokeMethod('setTrimSilence', {'enabled': trim});
      } catch (e) {
        log.severe('Error setting trim silence: $e');
      }
    }
  }

  @override
  Future<void> volumeBoost(bool boost) async {
    log.info('volumeBoost: $boost');
    _volumeBoost = boost;
    settingsService.volumeBoost = boost;

    if (Platform.isAndroid) {
      try {
        await platform.invokeMethod('setVolumeBoost', {'enabled': boost});
      } catch (e) {
        log.severe('Error setting volume boost: $e');
      }
    }
  }

  @override
  Future<void> addUpNextEpisode(Episode episode) async {
    log.info('addUpNextEpisode: ${episode.title}');
    _queue.add(episode);
    await repository.saveQueue(_queue);
    _updateQueueState();
  }

  @override
  Future<bool> removeUpNextEpisode(Episode episode) async {
    log.info('removeUpNextEpisode: ${episode.title}');
    final initialLength = _queue.length;
    _queue.removeWhere((e) => e.guid == episode.guid);
    final removed = _queue.length < initialLength;
    if (removed) {
      await repository.saveQueue(_queue);
      _updateQueueState();
    }
    return removed;
  }

  @override
  Future<bool> moveUpNextEpisode(Episode episode, int oldIndex, int newIndex) async {
    log.info('moveUpNextEpisode from $oldIndex to $newIndex');
    if (oldIndex < 0 || oldIndex >= _queue.length) return false;
    if (newIndex < 0 || newIndex >= _queue.length) return false;

    final item = _queue.removeAt(oldIndex);
    _queue.insert(newIndex, item);

    await repository.saveQueue(_queue);
    _updateQueueState();
    return true;
  }

  @override
  Future<void> clearUpNext() async {
    log.info('clearUpNext');
    _queue.clear();
    await repository.saveQueue(_queue);
    _updateQueueState();
  }

  @override
  Future<Episode?> resume() async {
    log.info('resume');
    // Resume current episode if exists
    if (_currentEpisode != null) {
      _updateEpisodeState();
    }
    return _currentEpisode;
  }

  @override
  Future<void> suspend() async {
    log.info('suspend');
    await _saveLocalPosition();
    _stopLocalPositionSaver();
  }

  @override
  void sleep(Sleep sleep) {
    log.info('sleep: ${sleep.type}');
    _sleep = sleep;
    _sleepState.add(sleep);

    // Cancel existing timer
    _sleepSubscription?.cancel();
    _sleepSubscription = null;

    if (sleep.type == SleepType.time) {
      // Time-based sleep
      _sleepSubscription = _sleepTicker.listen((_) {
        if (DateTime.now().isAfter(sleep.endTime)) {
          log.info('Sleep timer triggered - time limit reached');
          pause();
          _sleepSubscription?.cancel();
        }
      });
    } else if (sleep.type == SleepType.episode || sleep.type == SleepType.episodes) {
      // Episode-based sleep
      _sleepEpisodesRemaining = sleep.episodeCount;
    }
  }

  @override
  Future<void> searchTranscript(String search) async {
    // Transcript search logic - just update the transcript
    // The search filtering is handled by the UI/bloc layer
    if (_currentTranscript == null) return;

    _transcriptEvent.add(TranscriptUpdateState(
      transcript: _currentTranscript!,
    ));
  }

  @override
  Future<void> clearTranscript() async {
    _transcriptEvent.add(TranscriptUnavailableState());
  }

  void setPinepodsAudioService(PinepodsAudioService? service) {
    _pinepodsAudioService = service;
  }

  // Private helper methods

  Future<String?> _generateEpisodeUri(Episode episode) async {
    if (episode.downloadState == DownloadState.downloaded) {
      if (await hasStoragePermission()) {
        return await resolvePath(episode);
      }
    }
    return episode.contentUrl;
  }

  Future<int> _getBestEpisodePosition(Episode episode) async {
    final localPosition = episode.position;
    log.info('Local position: ${localPosition}ms');

    int serverPosition = 0;
    if (_pinepodsAudioService != null && episode.guid.startsWith('pinepods_')) {
      try {
        final episodeIdStr = episode.guid.replaceFirst('pinepods_', '').split('_').first;
        final episodeId = int.tryParse(episodeIdStr);

        if (episodeId != null) {
          final serverPos = await _pinepodsAudioService!.getServerPositionForEpisode(
            episodeId,
            settingsService.pinepodsUserId ?? 0,
            episode.pguid?.contains('youtube') ?? false,
          );

          if (serverPos != null) {
            serverPosition = (serverPos * 1000).round();
            log.info('Server position: ${serverPosition}ms');
          }
        }
      } catch (e) {
        log.warning('Failed to get server position: $e');
      }
    }

    final bestPosition = localPosition > serverPosition ? localPosition : serverPosition;
    log.info('Using position: ${bestPosition}ms');
    return bestPosition;
  }

  void _startLocalPositionSaver() {
    _localPositionTimer?.cancel();
    _localPositionTimer = Timer.periodic(const Duration(seconds: 3), (_) async {
      try {
        await _saveLocalPosition();
      } catch (e) {
        log.warning('Failed to save local position: $e');
      }
    });
  }

  void _stopLocalPositionSaver() {
    _localPositionTimer?.cancel();
    _localPositionTimer = null;
  }

  Future<void> _saveLocalPosition() async {
    if (_currentEpisode == null) return;

    try {
      final position = await platform.invokeMethod<int>('getPosition') ?? 0;
      _currentEpisode!.position = position;
      await repository.saveEpisode(_currentEpisode!);

      // Also update the position stream to keep it in sync
      final duration = await platform.invokeMethod<int>('getDuration') ?? 0;
      _playPosition.add(PositionState(
        position: Duration(milliseconds: position),
        length: Duration(milliseconds: duration),
        percentage: duration > 0 ? ((position / duration) * 100).toInt() : 0,
        episode: _currentEpisode,
        buffering: false,
      ));

      log.fine('Local position saved: ${position}ms');
    } catch (e) {
      log.severe('Failed to save local position: $e');
    }
  }

  void _performBackgroundSync() async {
    try {
      log.info('performBackgroundSync - recording listen duration');
      await _recordListenDuration();

      if (_pinepodsAudioService != null) {
        log.info('performBackgroundSync - syncing to server via pinepodsAudioService');
        await _pinepodsAudioService!.onPause();
        log.info('performBackgroundSync - server sync completed');
      } else {
        log.warning('performBackgroundSync - pinepodsAudioService is null, cannot sync to server');
      }
    } catch (e) {
      log.warning('Background sync failed: $e');
    }
  }

  Future<void> _recordListenDuration() async {
    if (_episodeStartTime == null || _pinepodsAudioService == null) return;

    final now = DateTime.now();
    final sessionDuration = now.difference(_episodeStartTime!);

    if (sessionDuration.inSeconds >= 5) {
      await _pinepodsAudioService!.recordListenDuration(sessionDuration.inSeconds.toDouble());
    }
  }

  Future<void> _loadQueue() async {
    _queue = await repository.loadQueue();
    _updateQueueState();
  }

  void _updateQueueState() {
    _queueState.add(QueueListState(
      playing: _currentEpisode,
      queue: List.from(_queue),
    ));
  }

  void _updateEpisodeState() {
    _episodeEvent.add(_currentEpisode);
  }

  Future<void> _loadChaptersAndTranscript() async {
    if (_currentEpisode == null) return;

    // Chapters and transcript loading will be handled by PinepodsAudioService
    // if it's a PinePods episode. For now, just log that we're skipping this.
    if (_currentEpisode!.guid.startsWith('pinepods_')) {
      log.fine('PinePods episode detected - chapters/transcript can be loaded separately');
    }
  }

  void _updateChapter(int seconds, int duration) {
    if (_currentEpisode == null) return;

    final chapters = _currentEpisode!.chapters;
    if (chapters == null || chapters.isEmpty) return;

    for (var chapterPtr = 0; chapterPtr < chapters.length; chapterPtr++) {
      final startTime = chapters[chapterPtr].startTime;
      final endTime = chapterPtr == (chapters.length - 1) ? duration : chapters[chapterPtr + 1].startTime;

      if (seconds >= startTime && seconds < endTime) {
        if (chapters[chapterPtr] != _currentEpisode!.currentChapter) {
          _currentEpisode!.currentChapter = chapters[chapterPtr];
          _episodeEvent.add(_currentEpisode!);
          break;
        }
      }
    }
  }

  // Android Auto / CarPlay browsing methods

  Future<List<Map<String, dynamic>>> _getSubscriptionsForCar() async {
    try {
      var podcasts = await repository.subscriptions();

      // If no subscriptions, wait a bit and retry once
      if (podcasts.isEmpty) {
        log.info('No subscriptions found, waiting 2s and retrying...');
        await Future.delayed(const Duration(seconds: 2));
        podcasts = await repository.subscriptions();

        if (podcasts.isEmpty) {
          log.warning('Still no subscriptions after retry - user may have no subscriptions or data not synced');
          return [];
        }
      }

      log.info('Returning ${podcasts.length} subscriptions');
      return podcasts.map((podcast) {
        return {
          'id': podcast.guid ?? '',
          'title': podcast.title,
          'imageUrl': podcast.imageUrl ?? podcast.thumbImageUrl,
          'episodeCount': podcast.episodes.length,
        };
      }).toList();
    } catch (e) {
      log.severe('Error getting subscriptions for car: $e');
      return [];
    }
  }

  Future<List<Map<String, dynamic>>> _getDownloadsForCar() async {
    try {
      // Get PinePods service and credentials
      final pinepodsService = GlobalServices.pinepodsService;
      if (pinepodsService == null) {
        log.warning('PinepodsService not available for Downloads');
        return [];
      }

      if (settingsService.pinepodsServer == null ||
          settingsService.pinepodsApiKey == null ||
          settingsService.pinepodsUserId == null) {
        log.warning('Not connected to PinePods server for Downloads');
        return [];
      }

      // Set credentials and call SAME API as Downloads page
      pinepodsService.setCredentials(settingsService.pinepodsServer!, settingsService.pinepodsApiKey!);
      log.info('Calling PinePods API: getServerDownloads for Downloads tab');

      final episodes = await pinepodsService.getServerDownloads(settingsService.pinepodsUserId!);

      log.info('PinePods API returned ${episodes.length} downloaded episodes');
      return episodes.map((episode) => _pinepodsEpisodeToCarMap(episode)).toList();
    } catch (e) {
      log.severe('Error getting downloads for car: $e');
      return [];
    }
  }

  Future<List<Map<String, dynamic>>> _getQueueForCar() async {
    try {
      // Get PinePods service and credentials
      final pinepodsService = GlobalServices.pinepodsService;
      if (pinepodsService == null) {
        log.warning('PinepodsService not available for Queue');
        return [];
      }

      if (settingsService.pinepodsServer == null ||
          settingsService.pinepodsApiKey == null ||
          settingsService.pinepodsUserId == null) {
        log.warning('Not connected to PinePods server for Queue');
        return [];
      }

      // Set credentials and call SAME API as Queue page
      pinepodsService.setCredentials(settingsService.pinepodsServer!, settingsService.pinepodsApiKey!);
      log.info('Calling PinePods API: getQueuedEpisodes for Queue tab');

      final episodes = await pinepodsService.getQueuedEpisodes(settingsService.pinepodsUserId!);

      log.info('PinePods API returned ${episodes.length} queued episodes');
      return episodes.map((episode) => _pinepodsEpisodeToCarMap(episode)).toList();
    } catch (e) {
      log.severe('Error getting queue for car: $e');
      return [];
    }
  }

  Future<List<Map<String, dynamic>>> _getRecentForCar() async {
    try {
      // Use findAllEpisodes to get episodes directly
      var allEpisodes = await repository.findAllEpisodes();

      // If no episodes, wait a bit and retry once
      if (allEpisodes.isEmpty) {
        log.info('No episodes found for recent, waiting 2s and retrying...');
        await Future.delayed(const Duration(seconds: 2));
        allEpisodes = await repository.findAllEpisodes();

        if (allEpisodes.isEmpty) {
          log.warning('Still no episodes for recent after retry');
          return [];
        }
      }

      // Get episodes with progress
      final recentEpisodes = allEpisodes.where((e) => e.position > 0).toList();

      // Sort by most recently played (highest position percentage)
      recentEpisodes.sort((a, b) {
        final aPercent = a.duration > 0 ? (a.position / (a.duration * 1000)) : 0;
        final bPercent = b.duration > 0 ? (b.position / (b.duration * 1000)) : 0;
        return bPercent.compareTo(aPercent);
      });

      log.info('Returning ${recentEpisodes.length} recent episodes from ${allEpisodes.length} total (showing top 20)');
      // Take top 20
      return recentEpisodes.take(20).map((episode) => _episodeToCarMap(episode)).toList();
    } catch (e) {
      log.severe('Error getting recent for car: $e');
      return [];
    }
  }

  Future<List<Map<String, dynamic>>> _getCurrentForCar() async {
    try {
      // Get PinePods service and credentials
      final pinepodsService = GlobalServices.pinepodsService;
      if (pinepodsService == null) {
        log.warning('PinepodsService not available for Current');
        return [];
      }

      if (settingsService.pinepodsServer == null ||
          settingsService.pinepodsApiKey == null ||
          settingsService.pinepodsUserId == null) {
        log.warning('Not connected to PinePods server for Current');
        return [];
      }

      // Set credentials and call SAME API as Home page
      pinepodsService.setCredentials(settingsService.pinepodsServer!, settingsService.pinepodsApiKey!);
      log.info('Calling PinePods API: getHomeOverview for Current tab');

      final homeData = await pinepodsService.getHomeOverview(settingsService.pinepodsUserId!);

      // Only show in-progress episodes for "Current" tab
      final inProgressEpisodes = homeData.inProgressEpisodes;

      log.info('PinePods API returned ${inProgressEpisodes.length} in-progress episodes for Current');
      return inProgressEpisodes.map((episode) => _homeEpisodeToCarMap(episode)).toList();
    } catch (e) {
      log.severe('Error getting current for car: $e');
      return [];
    }
  }

  Future<List<Map<String, dynamic>>> _getFeedForCar() async {
    try {
      // Get PinePods service and credentials
      final pinepodsService = GlobalServices.pinepodsService;
      if (pinepodsService == null) {
        log.warning('PinepodsService not available for Feed');
        return [];
      }

      if (settingsService.pinepodsServer == null ||
          settingsService.pinepodsApiKey == null ||
          settingsService.pinepodsUserId == null) {
        log.warning('Not connected to PinePods server for Feed');
        return [];
      }

      // Set credentials and call SAME API as Feed page
      pinepodsService.setCredentials(settingsService.pinepodsServer!, settingsService.pinepodsApiKey!);
      log.info('Calling PinePods API: getRecentEpisodes for Feed tab');

      final episodes = await pinepodsService.getRecentEpisodes(settingsService.pinepodsUserId!);

      log.info('PinePods API returned ${episodes.length} recent episodes for Feed');
      // Return first 50 episodes
      return episodes.take(50).map((episode) => _pinepodsEpisodeToCarMap(episode)).toList();
    } catch (e) {
      log.severe('Error getting feed for car: $e');
      return [];
    }
  }

  Future<List<Map<String, dynamic>>> _getSavedForCar() async {
    try {
      // Get PinePods service and credentials
      final pinepodsService = GlobalServices.pinepodsService;
      if (pinepodsService == null) {
        log.warning('PinepodsService not available for Saved');
        return [];
      }

      if (settingsService.pinepodsServer == null ||
          settingsService.pinepodsApiKey == null ||
          settingsService.pinepodsUserId == null) {
        log.warning('Not connected to PinePods server for Saved');
        return [];
      }

      // Set credentials and call SAME API as Saved page
      pinepodsService.setCredentials(settingsService.pinepodsServer!, settingsService.pinepodsApiKey!);
      log.info('Calling PinePods API: getSavedEpisodes for Saved tab');

      final episodes = await pinepodsService.getSavedEpisodes(settingsService.pinepodsUserId!);

      log.info('PinePods API returned ${episodes.length} saved episodes');
      return episodes.map((episode) => _pinepodsEpisodeToCarMap(episode)).toList();
    } catch (e) {
      log.severe('Error getting saved for car: $e');
      return [];
    }
  }

  Future<List<Map<String, dynamic>>> _getHistoryForCar() async {
    try {
      // Get PinePods service and credentials
      final pinepodsService = GlobalServices.pinepodsService;
      if (pinepodsService == null) {
        log.warning('PinepodsService not available for History');
        return [];
      }

      if (settingsService.pinepodsServer == null ||
          settingsService.pinepodsApiKey == null ||
          settingsService.pinepodsUserId == null) {
        log.warning('Not connected to PinePods server for History');
        return [];
      }

      // Set credentials and call SAME API as History page
      pinepodsService.setCredentials(settingsService.pinepodsServer!, settingsService.pinepodsApiKey!);
      log.info('Calling PinePods API: getUserHistory for History tab');

      final episodes = await pinepodsService.getUserHistory(settingsService.pinepodsUserId!);

      log.info('PinePods API returned ${episodes.length} history episodes');
      return episodes.map((episode) => _pinepodsEpisodeToCarMap(episode)).toList();
    } catch (e) {
      log.severe('Error getting history for car: $e');
      return [];
    }
  }

  Future<List<Map<String, dynamic>>> _getPodcastsForCar() async {
    try {
      // Get PinePods service and credentials
      final pinepodsService = GlobalServices.pinepodsService;
      if (pinepodsService == null) {
        log.warning('PinepodsService not available for Podcasts');
        return [];
      }

      if (settingsService.pinepodsServer == null ||
          settingsService.pinepodsApiKey == null ||
          settingsService.pinepodsUserId == null) {
        log.warning('Not connected to PinePods server for Podcasts');
        return [];
      }

      // Set credentials and call SAME API as Podcasts page
      pinepodsService.setCredentials(settingsService.pinepodsServer!, settingsService.pinepodsApiKey!);
      log.info('Calling PinePods API: getUserPodcasts for Podcasts tab');

      final podcasts = await pinepodsService.getUserPodcasts(settingsService.pinepodsUserId!);

      log.info('PinePods API returned ${podcasts.length} podcasts');
      return podcasts.map((podcast) {
        return {
          'id': podcast.id.toString(),
          'title': podcast.title,
          'imageUrl': podcast.imageUrl ?? podcast.thumbImageUrl,
          'episodeCount': podcast.episodes.length,
        };
      }).toList();
    } catch (e) {
      log.severe('Error getting podcasts for car: $e');
      return [];
    }
  }

  Future<List<Map<String, dynamic>>> _getPodcastEpisodesForCar(String podcastId) async {
    try {
      // Get PinePods service and credentials
      final pinepodsService = GlobalServices.pinepodsService;
      if (pinepodsService == null) {
        log.warning('PinepodsService not available for Podcast Episodes');
        return [];
      }

      if (settingsService.pinepodsServer == null ||
          settingsService.pinepodsApiKey == null ||
          settingsService.pinepodsUserId == null) {
        log.warning('Not connected to PinePods server for Podcast Episodes');
        return [];
      }

      final podcastIdInt = int.tryParse(podcastId);
      if (podcastIdInt == null) {
        log.warning('Invalid podcast ID: $podcastId');
        return [];
      }

      // Set credentials and call API
      pinepodsService.setCredentials(settingsService.pinepodsServer!, settingsService.pinepodsApiKey!);
      log.info('Calling PinePods API: getPodcastEpisodes for podcast $podcastId');

      final episodes = await pinepodsService.getPodcastEpisodes(
        settingsService.pinepodsUserId!,
        podcastIdInt,
      );

      log.info('PinePods API returned ${episodes.length} episodes for podcast $podcastId');
      return episodes.map((episode) => _pinepodsEpisodeToCarMap(episode)).toList();
    } catch (e) {
      log.severe('Error getting podcast episodes for car: $e');
      return [];
    }
  }

  Future<List<Map<String, dynamic>>> _getPlaylistsForCar() async {
    try {
      // Get PinePods service and credentials
      final pinepodsService = GlobalServices.pinepodsService;
      if (pinepodsService == null) {
        log.warning('PinepodsService not available for Playlists');
        return [];
      }

      if (settingsService.pinepodsServer == null ||
          settingsService.pinepodsApiKey == null ||
          settingsService.pinepodsUserId == null) {
        log.warning('Not connected to PinePods server for Playlists');
        return [];
      }

      // Set credentials and call SAME API as Playlists page
      pinepodsService.setCredentials(settingsService.pinepodsServer!, settingsService.pinepodsApiKey!);
      log.info('Calling PinePods API: getPlaylists for Playlists tab');

      final response = await pinepodsService.getPlaylists(settingsService.pinepodsUserId!);

      log.info('PinePods API returned ${response.playlists.length} playlists');
      return response.playlists.map((playlist) {
        return {
          'id': playlist.playlistId,
          'name': playlist.name,
          'description': playlist.description,
          'episodeCount': playlist.episodeCount ?? 0,
        };
      }).toList();
    } catch (e) {
      log.severe('Error getting playlists for car: $e');
      return [];
    }
  }

  Future<List<Map<String, dynamic>>> _getPlaylistEpisodesForCar(String playlistId) async {
    try {
      // Get PinePods service and credentials
      final pinepodsService = GlobalServices.pinepodsService;
      if (pinepodsService == null) {
        log.warning('PinepodsService not available for Playlist Episodes');
        return [];
      }

      if (settingsService.pinepodsServer == null ||
          settingsService.pinepodsApiKey == null ||
          settingsService.pinepodsUserId == null) {
        log.warning('Not connected to PinePods server for Playlist Episodes');
        return [];
      }

      final playlistIdInt = int.tryParse(playlistId);
      if (playlistIdInt == null) {
        log.warning('Invalid playlist ID: $playlistId');
        return [];
      }

      // Set credentials and call API
      pinepodsService.setCredentials(settingsService.pinepodsServer!, settingsService.pinepodsApiKey!);
      log.info('Calling PinePods API: getPlaylistEpisodes for playlist $playlistId');

      final response = await pinepodsService.getPlaylistEpisodes(
        settingsService.pinepodsUserId!,
        playlistIdInt,
      );

      log.info('PinePods API returned ${response.episodes.length} episodes for playlist $playlistId');
      return response.episodes.map((episode) => _pinepodsEpisodeToCarMap(episode)).toList();
    } catch (e) {
      log.severe('Error getting playlist episodes for car: $e');
      return [];
    }
  }

  Future<void> _playFromMediaIdForCar(String guid) async {
    try {
      log.info('Playing episode from car: $guid');

      // Extract episode ID from guid (format: "pinepods_123")
      if (!guid.startsWith('pinepods_')) {
        log.warning('Invalid guid format: $guid');
        return;
      }

      final episodeIdStr = guid.replaceFirst('pinepods_', '');
      final episodeId = int.tryParse(episodeIdStr);

      if (episodeId == null) {
        log.warning('Could not parse episode ID from guid: $guid');
        return;
      }

      // Get PinePods service and credentials
      final pinepodsService = GlobalServices.pinepodsService;
      final pinepodsAudioService = GlobalServices.pinepodsAudioService;

      if (pinepodsService == null || pinepodsAudioService == null) {
        log.warning('PinePods services not available');
        return;
      }

      if (settingsService.pinepodsServer == null ||
          settingsService.pinepodsApiKey == null ||
          settingsService.pinepodsUserId == null) {
        log.warning('Not connected to PinePods server');
        return;
      }

      // Set credentials
      pinepodsService.setCredentials(settingsService.pinepodsServer!, settingsService.pinepodsApiKey!);
      final userId = settingsService.pinepodsUserId!;

      // Fetch full episode metadata from API
      log.info('Fetching episode metadata for ID: $episodeId');
      final episode = await pinepodsService.getEpisodeMetadata(episodeId, userId);

      if (episode == null) {
        log.warning('Episode metadata not found for ID: $episodeId');
        return;
      }

      // Play using the SAME method as the app UI
      log.info('Playing PinePods episode: ${episode.episodeTitle}');
      await pinepodsAudioService.playPinepodsEpisode(
        pinepodsEpisode: episode,
        resume: true,
      );

      log.info('Episode started playing from Android Auto');
    } catch (e) {
      log.severe('Error playing from media ID: $e');
    }
  }

  Future<Map<String, dynamic>> _searchForCar(String query) async {
    try {
      final podcasts = await repository.subscriptions();
      final results = <Episode>[];
      final queryLower = query.toLowerCase();

      for (final podcast in podcasts) {
        results.addAll(
          podcast.episodes.where((episode) =>
              (episode.title ?? '').toLowerCase().contains(queryLower) ||
              (episode.description?.toLowerCase().contains(queryLower) ?? false)),
        );
      }

      // Limit to 50 results
      return {
        'episodes': results.take(50).map((episode) => _episodeToCarMap(episode)).toList(),
      };
    } catch (e) {
      log.severe('Error searching for car: $e');
      return {'episodes': []};
    }
  }

  Map<String, dynamic> _episodeToCarMap(Episode episode) {
    return {
      'guid': episode.guid ?? '',
      'title': episode.title ?? 'Unknown Episode',
      'podcast': episode.podcast ?? 'Unknown Podcast',
      'imageUrl': episode.imageUrl,
      'duration': episode.duration,
      'position': episode.position,  // Add progress information for display
    };
  }

  /// Convert PinepodsEpisode to car map format
  Map<String, dynamic> _pinepodsEpisodeToCarMap(PinepodsEpisode episode) {
    return {
      'guid': 'pinepods_${episode.episodeId}',
      'title': episode.episodeTitle,
      'podcast': episode.podcastName,
      'imageUrl': episode.episodeArtwork,
      'duration': episode.episodeDuration,
      'position': (episode.listenDuration ?? 0) * 1000,  // Convert seconds to milliseconds
      'pubDate': episode.episodePubDate,  // Add publication date
    };
  }

  /// Convert HomeEpisode to car map format
  Map<String, dynamic> _homeEpisodeToCarMap(HomeEpisode episode) {
    return {
      'guid': 'pinepods_${episode.episodeId}',
      'title': episode.episodeTitle,
      'podcast': episode.podcastName,
      'imageUrl': episode.episodeArtwork,
      'duration': episode.episodeDuration,
      'position': (episode.listenDuration ?? 0) * 1000,  // Convert seconds to milliseconds
      'pubDate': episode.episodePubDate,  // Add publication date
    };
  }

  @override
  Episode? get nowPlaying => _currentEpisode;

  @override
  Stream<AudioState> get playingState => _playingState.stream;

  @override
  ValueStream<PositionState> get playPosition => _playPosition.stream;

  @override
  ValueStream<Episode?> get episodeEvent => _episodeEvent.stream;

  @override
  Stream<TranscriptState> get transcriptEvent => _transcriptEvent.stream;

  @override
  Stream<int> get playbackError => _playbackError.stream;

  @override
  Stream<QueueListState> get queueState => _queueState.stream;

  @override
  Stream<Sleep> get sleepStream => _sleepState.stream;

  // MARK: - CarPlay Debug Methods

  /// Get the current Now Playing info from MPNowPlayingInfoCenter for debugging
  Future<Map<String, dynamic>> getNowPlayingInfo() async {
    try {
      final result = await platform.invokeMethod<Map>('getNowPlayingInfo');
      if (result != null) {
        final info = Map<String, dynamic>.from(result);
        log.info('getNowPlayingInfo: $info');
        return info;
      }
      return {'error': 'No info returned'};
    } catch (e) {
      log.severe('Failed to get now playing info: $e');
      return {'error': e.toString()};
    }
  }

  /// Configure the CarPlay Now Playing template (call before showing it)
  Future<void> configureCarPlayNowPlaying() async {
    try {
      await platform.invokeMethod('configureCarPlayNowPlaying');
      log.info('CarPlay Now Playing template configured');
    } catch (e) {
      log.severe('Failed to configure CarPlay Now Playing: $e');
    }
  }
}
