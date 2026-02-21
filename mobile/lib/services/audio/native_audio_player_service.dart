// Native audio player service using platform channels for iOS
// This replaces just_audio/audio_service with native AVPlayer on iOS

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:logging/logging.dart';
import 'package:pinepods_mobile/core/utils.dart';
import 'package:pinepods_mobile/entities/downloadable.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/sleep.dart';
import 'package:pinepods_mobile/entities/transcript.dart';
import 'package:pinepods_mobile/repository/repository.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_audio_service.dart';
import 'package:pinepods_mobile/services/settings/settings_service.dart';
import 'package:pinepods_mobile/state/queue_event_state.dart';
import 'package:pinepods_mobile/state/transcript_state_event.dart';
import 'package:rxdart/rxdart.dart';

/// Native audio player service that uses platform channels to communicate
/// with iOS native AVPlayer for audio playback.
///
/// This provides better stability and iOS integration compared to Flutter
/// audio packages, with proper background playback, lock screen controls,
/// and audio session management.
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

  StreamSubscription<int>? _sleepSubscription;
  StreamSubscription? _nativeEventSubscription;

  final BehaviorSubject<AudioState> _playingState = BehaviorSubject<AudioState>.seeded(AudioState.none);
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
    log.info('Initializing NativeAudioPlayerService for iOS');

    // CRITICAL: Defer event channel subscription to avoid blocking iOS during app startup.
    // This is essential to prevent black screen issues on iOS.
    Future.delayed(const Duration(milliseconds: 100), () {
      log.info('Subscribing to native event channel');
      try {
        _nativeEventSubscription = eventChannel.receiveBroadcastStream().listen(
          _handleNativeEvent,
          onError: (error) {
            log.severe('Native event stream error: $error');
          },
        );
        log.info('Native event channel subscription successful');
      } catch (e) {
        log.severe('Failed to subscribe to native event channel: $e');
      }
    });

    _loadQueue();
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
      case 'log':
        // Forward native logs to Flutter logging
        final message = event['message'] as String? ?? 'Unknown';
        log.info('[NATIVE] $message');
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
      // Prepare metadata for native player
      // Note: episode.duration is already in milliseconds according to the Episode entity
      // For PinePods episodes, duration is converted to ms in pinepods_audio_service
      // For other episodes, duration might be in seconds - we need to handle both cases
      // Check if duration seems reasonable (if < 1 hour in seconds, likely needs conversion)
      int durationMs = episode.duration;
      if (durationMs < 86400) { // Less than 24 hours in seconds = likely stored as seconds
        durationMs = episode.duration * 1000;
        log.fine('Duration appears to be in seconds, converting: ${episode.duration}s -> ${durationMs}ms');
      } else {
        log.fine('Duration appears to be in milliseconds: ${durationMs}ms');
      }

      final metadata = {
        'title': episode.title ?? 'Unknown',
        'artist': episode.podcast ?? 'Unknown',
        'artwork': episode.imageUrl ?? '',
        'duration': durationMs,
      };

      log.info('Calling native playEpisode with metadata: title=${metadata['title']}, artist=${metadata['artist']}, artwork=${episode.imageUrl != null ? 'present' : 'null'}, duration=${durationMs}ms (${(durationMs / 1000 / 60).toStringAsFixed(1)} min)');

      // Call native platform to play
      await platform.invokeMethod('playEpisode', {
        'url': uri,
        'startPosition': _currentEpisode!.position,
        'isLocal': episode.downloadState == DownloadState.downloaded,
        'metadata': metadata,
      });

      log.info('Native playEpisode call completed');

      // Apply playback speed
      await platform.invokeMethod('setPlaybackSpeed', {'speed': _playbackSpeed});

      // Start tracking
      _episodeStartTime = DateTime.now();
      _startLocalPositionSaver();

      log.info('Episode playback started successfully');
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
        await _pinepodsAudioService!.onStop();
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
    log.info('trimSilence: $trim (not supported on iOS native player)');
    _trimSilence = trim;
    settingsService.trimSilence = trim;
    // Note: Trim silence is not supported on iOS native AVPlayer
    // This would require audio processing which AVPlayer doesn't provide
  }

  @override
  Future<void> volumeBoost(bool boost) async {
    log.info('volumeBoost: $boost (not supported on iOS native player)');
    _volumeBoost = boost;
    settingsService.volumeBoost = boost;
    // Note: Volume boost is not supported on iOS native AVPlayer
    // iOS handles audio normalization at the system level
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
    } else if (sleep.type == SleepType.episode) {
      // Episode-based sleep - sleep at end of current episode
      _sleepEpisodesRemaining = 1;
    }
  }

  @override
  Future<void> searchTranscript(String search) async {
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
    log.info('PinepodsAudioService reference set for server sync');
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

    if (_currentEpisode!.guid.startsWith('pinepods_')) {
      log.fine('PinePods episode detected - chapters/transcript can be loaded separately');
    }
  }

  void _updateChapter(int seconds, int duration) {
    if (_currentEpisode == null) return;

    final chapters = _currentEpisode!.chapters;
    if (chapters.isEmpty) return;

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

  void dispose() {
    _nativeEventSubscription?.cancel();
    _sleepSubscription?.cancel();
    _localPositionTimer?.cancel();
    _playingState.close();
    _playPosition.close();
    _episodeEvent.close();
    _transcriptEvent.close();
    _playbackError.close();
    _queueState.close();
    _sleepState.close();
  }
}
