// Native audio player service using platform channels instead of just_audio/audio_service

import 'dart:async';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:logging/logging.dart';
import 'package:pinepods_mobile/core/download_presence.dart';
import 'package:pinepods_mobile/core/utils.dart';
import 'package:pinepods_mobile/entities/downloadable.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/sleep.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/entities/home_data.dart';
import 'package:pinepods_mobile/repository/repository.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_audio_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/services/global_services.dart';
import 'package:pinepods_mobile/services/settings/settings_service.dart';
import 'package:pinepods_mobile/state/queue_event_state.dart';
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

  /// True from the moment native reports the track ended until the next
  /// episode is actually underway (or we give up and stop). While set, the
  /// transient `stopped`/`none` states that ExoPlayer/AVPlayer emit around
  /// end-of-track and while switching media are suppressed, so the mini/floating
  /// player stays visible (as a loading state) during the hand-off instead of
  /// vanishing and popping back.
  bool _handlingCompletion = false;

  /// Re-entry guard so a double-fired native `completed` event can't launch two
  /// concurrent auto-advances.
  bool _advanceInProgress = false;

  Episode? _currentEpisode;

  Timer? _localPositionTimer;

  /// Serialises local episode saves so a background save can't race the next
  /// one (an unsaved episode has no id yet, so two concurrent saves add twice).
  Future<void> _persistChain = Future.value();

  /// Time from a play request to the first `playing` event, for start-latency logs.
  Stopwatch? _startWatch;
  bool _startIsLocal = false;

  /// Whether we've already reported the decoded duration for the current
  /// episode to the server (see PinepodsAudioService.updateEpisodeDurationIfNeeded).
  bool _actualDurationReported = false;

  StreamSubscription<int>? _sleepSubscription;
  StreamSubscription? _nativeEventSubscription;

  final BehaviorSubject<AudioState> _playingState = BehaviorSubject<AudioState>.seeded(AudioState.none);
  final BehaviorSubject<double> _speedState = BehaviorSubject<double>.seeded(1.0);
  final _sleepTicker = Stream<int>.periodic(const Duration(milliseconds: 500), (count) => count).asBroadcastStream();
  final _playPosition = BehaviorSubject<PositionState>();
  final _episodeEvent = BehaviorSubject<Episode?>(sync: true);
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

    // Re-push skip intervals to the native media session whenever the user
    // changes them, so car/notification rewind & fast-forward buttons stay in
    // sync without needing to restart playback.
    settingsService.settingsListener.listen((key) {
      if (key == 'fastForwardInterval' || key == 'rewindInterval') {
        _applySkipIntervals();
      }
    });

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
          // Older native builds report load failures as stream errors rather
          // than {type: error} events; either way the player must leave the
          // buffering state or the UI spins forever.
          onError: (error) {
            log.severe('Native event stream error: $error');
            final code = error is PlatformException ? int.tryParse(error.code) ?? -1 : -1;
            _handleErrorEvent({'code': code, 'message': '$error'});
          },
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
        unawaited(_handleCompletedEvent());
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
      // ExoPlayer reports STATE_ENDED as a 'completed' playback state (and keeps
      // re-reporting it via onIsPlayingChanged). Ignore it: the separate
      // {type:'completed'} event is what drives the advance and shows the
      // loading state. Emitting a real state for 'completed' maps to `none` and
      // would hide the player mid hand-off.
      if (state == 'completed') {
        return;
      }

      final audioState = _parseState(state);

      // The next episode is underway (or the user resumed) — end the hand-off.
      if (audioState == AudioState.playing || audioState == AudioState.buffering) {
        _handlingCompletion = false;
      }

      final watch = _startWatch;
      if (watch != null && audioState == AudioState.playing) {
        _startWatch = null;
        log.info('Playback started ${watch.elapsedMilliseconds}ms after request '
            '(${_startIsLocal ? 'local file' : 'stream'})');
      }

      // Suppress the brief stopped/idle states emitted while tearing down the
      // finished track and loading the next one.
      if (_handlingCompletion &&
          (audioState == AudioState.stopped || audioState == AudioState.none)) {
        return;
      }

      // A failed item still reports paused/stopped afterwards; keep the error
      // so the UI offers a retry rather than a resume that can't work.
      if (_playingState.valueOrNull == AudioState.error &&
          audioState != AudioState.playing &&
          audioState != AudioState.buffering) {
        return;
      }

      _setPlayingState(audioState);
    }

    // Update position
    if (_currentEpisode != null) {
      _playPosition.add(PositionState(
        position: Duration(milliseconds: position),
        length: Duration(milliseconds: duration),
        percentage: duration > 0 ? ((position / duration) * 100).toInt() : 0,
        episode: _currentEpisode,
        buffering: state == 'buffering',
        buffered: Duration(milliseconds: bufferedPosition),
      ));

      // Update chapter if needed
      _updateChapter(position ~/ 1000, duration ~/ 1000);

      // Once the decoder reports a real length, correct the server's stored
      // duration if it's wrong (e.g. feeds shipping a missing/zero duration).
      // Mirrors the web player; only fires once per episode.
      if (!_actualDurationReported && duration > 0) {
        _actualDurationReported = true;
        _pinepodsAudioService?.updateEpisodeDurationIfNeeded(duration / 1000.0);
      }
    }
  }

  /// Native re-reports the same state with every position tick; only pass on
  /// real changes so listeners don't rebuild every second.
  void _setPlayingState(AudioState state) {
    if (_playingState.valueOrNull != state) _playingState.add(state);
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
    _handlingCompletion = false;
    // Don't save here: a failed item reads position 0 and would wipe the
    // resume point. Just stop the periodic saver for the same reason.
    _stopLocalPositionSaver();
    _playbackError.add(code);
    _setPlayingState(AudioState.error);
  }

  Future<void> _handleCompletedEvent() async {
    log.info('Episode completed');
    if (_advanceInProgress) {
      log.info('Already handling completion - ignoring duplicate event');
      return;
    }
    _advanceInProgress = true;
    // Enter the hand-off: keep the player visible (loading) and suppress the
    // transient stopped/idle states native emits while switching episodes, until
    // the next one is actually underway (cleared in _handlePlaybackStateEvent).
    _handlingCompletion = true;
    try {
      final completedEpisode = _currentEpisode;
      if (completedEpisode != null) {
        completedEpisode.played = true;
        completedEpisode.position = 0;
        unawaited(_persist(completedEpisode));
      }

      // Check sleep timer
      if (_sleep.type == SleepType.episode || _sleep.type == SleepType.episodes) {
        _sleepEpisodesRemaining--;
        if (_sleepEpisodesRemaining <= 0) {
          log.info('Sleep timer triggered - episode count reached');
          _sleep = Sleep(type: SleepType.none);
          _sleepState.add(_sleep);
          // Don't suppress the stop that follows — we intend to hide the player.
          _handlingCompletion = false;
          stop();
          return;
        }
      }

      // Legacy Anytime "Up Next" queue (never populated by PinePods playback,
      // but kept for the non-PinePods path).
      if (_queue.isNotEmpty) {
        final nextEpisode = _queue.removeAt(0);
        _updateQueueState();
        await playEpisode(episode: nextEpisode, resume: false);
        return;
      }

      // PinePods auto-advance: continue the playlist / auto-play-next / server
      // queue. Keep the player on screen (buffering) during the network round
      // trip so it doesn't flicker to "stopped"; the next episode's playEpisode
      // emits its own buffering/playing, and if nothing is queued we stop.
      //
      // NOTE: we deliberately do NOT gate on the Episode.guid here — PinePods
      // episodes are played with guid == episode URL (see
      // PinepodsAudioService._convertToEpisode), not "pinepods_<id>". The
      // PinepodsAudioService knows whether it is tracking an episode and returns
      // false (→ stop) when it is not.
      if (_pinepodsAudioService != null) {
        _setPlayingState(AudioState.buffering);
        final advanced = await _pinepodsAudioService!.handleEpisodeCompleted();
        if (advanced) {
          // Leave _handlingCompletion set: the next episode's native
          // playing/buffering event clears it (see _handlePlaybackStateEvent),
          // which keeps the player visible across the media switch.
        } else {
          // Nothing left to play — allow the hide and stop.
          _handlingCompletion = false;
          _setPlayingState(AudioState.stopped);
        }
      } else {
        _handlingCompletion = false;
        _setPlayingState(AudioState.stopped);
      }
    } finally {
      _advanceInProgress = false;
    }
  }

  void _handleMediaButtonAction(Map<dynamic, dynamic> event) {
    final action = event['action'] as String?;
    log.info('Media button action: $action');
    // Media button actions are already handled by native code
    // This is just for logging/tracking
  }

  @override
  Future<Episode?> findDownloadedEpisode(int episodeId) async {
    final guid = 'pinepods_$episodeId';

    final direct = await repository.findEpisodeByGuid(guid);
    if (direct != null && direct.downloadState == DownloadState.downloaded) {
      return direct;
    }

    // Legacy 'pinepods_<id>_<timestamp>' guids.
    final legacy = await repository.findEpisodesByGuidOrLegacy(guid);
    for (final e in legacy) {
      if (e.downloadState == DownloadState.downloaded) return e;
    }
    return null;
  }

  @override
  Future<void> playEpisode({required Episode episode, bool resume = true, double? playbackSpeed}) async {
    log.info('playEpisode: ${episode.title}, resume: $resume');
    _startWatch = Stopwatch()..start();

    _currentEpisode = episode;
    _currentEpisode!.played = false;
    _actualDurationReported = false;

    // Get URI (local file or stream)
    final uri = await _generateEpisodeUri(episode);
    if (uri == null) {
      log.severe('Failed to generate episode URI');
      _startWatch = null;
      _setPlayingState(AudioState.error);
      return;
    }
    final isLocal = episode.downloadState == DownloadState.downloaded;
    _startIsLocal = isLocal;

    // The caller has already resolved where to resume (see
    // PinepodsAudioService._resolveResumePosition).
    if (!resume) _currentEpisode!.position = 0;

    // Update state
    _setPlayingState(AudioState.buffering);
    _updateQueueState();
    _updateEpisodeState();

    // Session speed (a per-podcast override) or the user's default.
    _playbackSpeed = playbackSpeed ?? settingsService.playbackSpeed;
    _speedState.add(_playbackSpeed);
    _trimSilence = settingsService.trimSilence;
    _volumeBoost = settingsService.volumeBoost;

    try {
      // One call: native starts with the right speed, silence trim, boost and
      // skip intervals instead of switching a moment after audio begins.
      await platform.invokeMethod('playEpisode', {
        'url': uri,
        'startPosition': _currentEpisode!.position,
        'isLocal': isLocal,
        'speed': _playbackSpeed,
        'trimSilence': _trimSilence,
        'volumeBoost': _volumeBoost,
        'forwardMs': settingsService.fastForwardInterval * 1000,
        'backwardMs': settingsService.rewindInterval * 1000,
        'metadata': {
          'title': episode.title ?? 'Unknown',
          'artist': episode.podcast ?? 'Unknown',
          'artwork': episode.imageUrl,
          'duration': episode.duration,
        },
      });

      _startLocalPositionSaver();
    } catch (e) {
      log.severe('Error playing episode: $e');
      _startWatch = null;
      _setPlayingState(AudioState.error);
    }

    unawaited(_persist(_currentEpisode!));
  }

  @override
  Future<void> play() async {
    log.info('play');

    // After a failure (or once the track has run out) native has nothing it
    // can resume, so a plain play() is a silent no-op. Reload instead.
    final current = _currentEpisode;
    final state = _playingState.valueOrNull;
    if (current != null &&
        (state == AudioState.error || state == AudioState.stopped || state == AudioState.none)) {
      log.info('play: no healthy item loaded (state $state), reloading current episode');
      await playEpisode(episode: current, resume: true);
      return;
    }

    _startLocalPositionSaver();

    try {
      await platform.invokeMethod('play');
    } catch (e) {
      log.severe('Error playing: $e');
    }
  }

  @override
  Future<void> pause() async {
    log.info('pause');

    // Pause first so the tap takes effect at once; the position read after
    // the pause is also the exact one to save.
    try {
      await platform.invokeMethod('pause');
    } catch (e) {
      log.severe('Error pausing: $e');
    }

    _stopLocalPositionSaver();
    await _saveLocalPosition();

    // Background sync (non-blocking)
    _performBackgroundSync();
  }

  @override
  Future<void> stop() async {
    log.info('stop');

    // Read the position before stopping: native clears the item on stop and
    // reports 0 afterwards, which would replace newer unsynced progress.
    int? positionMs;
    if (_currentEpisode != null && _playingState.valueOrNull != AudioState.error) {
      try {
        positionMs = await platform.invokeMethod<int>('getPosition');
      } catch (e) {
        log.warning('Could not read position before stop: $e');
      }
    }

    try {
      await platform.invokeMethod('stop');
    } catch (e) {
      log.severe('Error stopping: $e');
    }

    _stopLocalPositionSaver();
    if (positionMs != null) await _saveLocalPosition(positionMs: positionMs);

    if (_pinepodsAudioService != null) {
      try {
        await _pinepodsAudioService!.onPause();
      } catch (e) {
        log.warning('Failed to sync on stop: $e');
      }
    }

    _currentEpisode = null;
    _setPlayingState(AudioState.stopped);
  }

  @override
  Future<void> rewind() async {
    log.info('rewind');
    try {
      await platform.invokeMethod('rewind', {'milliseconds': settingsService.rewindInterval * 1000});
    } catch (e) {
      log.severe('Error rewinding: $e');
    }
  }

  @override
  Future<void> fastForward() async {
    log.info('fastForward');
    try {
      await platform.invokeMethod('fastForward', {'milliseconds': settingsService.fastForwardInterval * 1000});
    } catch (e) {
      log.severe('Error fast forwarding: $e');
    }
  }

  /// Push the user-configured skip intervals down to the native media
  /// session so the car/notification rewind & fast-forward buttons honor
  /// the same values as the in-app controls.
  Future<void> _applySkipIntervals() async {
    try {
      await platform.invokeMethod('setSkipIntervals', {
        'forwardMs': settingsService.fastForwardInterval * 1000,
        'backwardMs': settingsService.rewindInterval * 1000,
      });
    } catch (e) {
      log.warning('Error setting skip intervals: $e');
    }
  }

  @override
  Future<void> seek({required int position}) async {
    log.info('seek to $position seconds');
    try {
      // Convert seconds to milliseconds for native player
      final positionMs = position * 1000;
      await platform.invokeMethod('seek', {'position': positionMs});
      // Persist the new position immediately so a crash/quick-close right after
      // a scrub doesn't lose it (steady-state saver only runs every 15s).
      await _saveLocalPosition(positionMs: positionMs);
    } catch (e) {
      log.severe('Error seeking: $e');
    }
  }

  @override
  Future<void> setPlaybackSpeed(double speed, {bool persist = true}) async {
    log.info('setPlaybackSpeed: $speed');
    _playbackSpeed = speed;
    _speedState.add(speed);
    if (persist) settingsService.playbackSpeed = speed;

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
  Future<void> applyEpisodeSkipSegments({
    required bool silenceEnabled,
    required List<Map<String, double>> silenceRanges,
    required List<Map<String, double>> adRanges,
  }) async {
    log.info('applyEpisodeSkipSegments: silenceEnabled=$silenceEnabled, '
        'silence=${silenceRanges.length}, ads=${adRanges.length}');
    // Session-only: unlike trimSilence(), this does NOT persist to the global
    // settingsService, so a per-podcast value never overwrites the user default.
    if (Platform.isAndroid) {
      // Silence stays on ExoPlayer's real-time skip-silence DSP. Effective
      // enablement is the per-podcast flag OR the user's global preference.
      final effective = silenceEnabled || settingsService.trimSilence;
      try {
        await platform.invokeMethod('setTrimSilence', {'enabled': effective});
      } catch (e) {
        log.severe('Error applying episode trim silence (Android): $e');
      }
      // Ads (#790) can't ride the DSP (they're content, not silence): send the
      // ranges to the native position-poll seeker. Empty list clears them.
      try {
        await platform.invokeMethod('setAdSkipSegments', {
          'segments':
              adRanges.map((s) => {'start': s['start'], 'end': s['end']}).toList(),
        });
      } catch (e) {
        log.severe('Error applying episode ad-skip segments (Android): $e');
      }
    } else if (Platform.isIOS) {
      // iOS (AVPlayer has no DSP skip-silence): the native seek observer is
      // kind-agnostic, so send the union of silence + active-ad ranges and let
      // it seek past any of them. Enabled whenever there's anything to skip.
      final combined = [...silenceRanges, ...adRanges]
          .map((s) => {'start': s['start'], 'end': s['end']})
          .toList();
      try {
        await platform.invokeMethod('setSkipSegments', {
          'enabled': combined.isNotEmpty,
          'segments': combined,
        });
      } catch (e) {
        log.severe('Error applying episode skip segments (iOS): $e');
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

  /// How long a timed sleep fades the volume out before pausing.
  static const _sleepFade = Duration(seconds: 8);

  @override
  void sleep(Sleep sleep) {
    log.info('sleep: ${sleep.type}');
    _sleep = sleep;
    _sleepState.add(sleep);

    // Cancel existing timer, and undo any fade it had started.
    _sleepSubscription?.cancel();
    _sleepSubscription = null;
    unawaited(_setVolume(1.0));

    if (sleep.type == SleepType.time) {
      // Time-based sleep, fading out over its last few seconds.
      _sleepSubscription = _sleepTicker.listen((_) {
        final remaining = sleep.endTime.difference(DateTime.now());
        if (remaining <= Duration.zero) {
          log.info('Sleep timer triggered - time limit reached');
          unawaited(_endSleep());
        } else if (remaining < _sleepFade) {
          unawaited(_setVolume(remaining.inMilliseconds / _sleepFade.inMilliseconds));
        }
      });
    } else if (sleep.type == SleepType.episode || sleep.type == SleepType.episodes) {
      // Episode-based sleep: the current episode counts as the first.
      _sleepEpisodesRemaining = sleep.type == SleepType.episode ? 1 : sleep.episodeCount;
    }
  }

  /// The sleep timer ran out: pause, restore the volume for next time and
  /// clear the timer.
  Future<void> _endSleep() async {
    _sleepSubscription?.cancel();
    _sleepSubscription = null;
    _sleep = Sleep(type: SleepType.none);
    _sleepState.add(_sleep);
    await pause();
    await _setVolume(1.0);
  }

  Future<void> _setVolume(double volume) async {
    try {
      await platform.invokeMethod('setVolume', {'volume': volume.clamp(0.0, 1.0)});
    } catch (e) {
      log.fine('Could not set volume: $e');
    }
  }

  @override
  void setPlaylistContext(int? playlistId) {}

  @override
  void updateNowPlayingDetails(String guid, void Function(Episode episode) update) {
    final episode = _currentEpisode;
    if (episode == null || episode.guid != guid) return;
    update(episode);
    final position = _playPosition.valueOrNull;
    if (position != null) {
      _updateChapter(position.position.inSeconds, position.length.inSeconds);
    }
    _updateEpisodeState();
  }

  void setPinepodsAudioService(PinepodsAudioService? service) {
    _pinepodsAudioService = service;
  }

  // Private helper methods

  Future<String?> _generateEpisodeUri(Episode episode) async {
    if (episode.downloadState == DownloadState.downloaded) {
      if (await hasStoragePermission()) {
        final path = await resolvePath(episode);

        // The DB row can say "downloaded" while the file itself is gone -
        // cleared by the OS under storage pressure, an SD card that's been
        // swapped/unmounted, a changed storage root after an app update, or
        // an interrupted download that never got marked as failed. Handing
        // ExoPlayer/AVPlayer a path to a missing (or empty/corrupt) file
        // fails asynchronously with no useful error surfaced to the user -
        // the mini player just appears then immediately disappears. Detect
        // it up front instead: fall back to streaming, and repair the stale
        // record so the "Downloaded" badge stops lying and future attempts
        // don't hit the same dead path.
        final file = File(path);
        if (await file.exists() && (await file.length()) > 0) {
          return path;
        }

        log.warning('Downloaded file missing/empty for "${episode.title}" at $path - streaming instead');
        await _clearStaleDownloadRecord(episode);
      }
    }
    return episode.contentUrl;
  }

  /// Reset every download record pointing at a file that no longer exists on
  /// disk, so those episodes go back to showing as not-downloaded instead of
  /// continuing to point at a dead local path. See findStaleDownloadRecords
  /// for why matching happens by filepath/filename rather than guid.
  Future<void> _clearStaleDownloadRecord(Episode episode) async {
    try {
      final all = await repository.findAllEpisodes();
      final staleRecords = findStaleDownloadRecords(
        all,
        filepath: episode.filepath,
        filename: episode.filename,
      );

      for (final record in staleRecords) {
        clearDownloadState(record);
        await repository.saveEpisode(record);
      }

      // Keep the in-memory object used for *this* playback attempt in sync
      // too, so the 'isLocal' flag computed right after this call is
      // correct even if [episode] itself was never a real repository row.
      clearDownloadState(episode);
    } catch (e) {
      log.warning('Failed to clear stale download record for "${episode.title}": $e');
    }
  }

  void _startLocalPositionSaver() {
    _localPositionTimer?.cancel();
    // Steady-state local resume-position save. Kept infrequent for battery; the
    // important moments (pause/seek/stop/complete) save immediately via
    // _saveLocalPosition() so crash-resume stays accurate.
    _localPositionTimer = Timer.periodic(const Duration(seconds: 15), (_) {
      // The latest native position event is at most a second old; no need to
      // ask native again.
      final latest = _playPosition.valueOrNull?.position.inMilliseconds;
      unawaited(_saveLocalPosition(positionMs: latest));
    });
  }

  void _stopLocalPositionSaver() {
    _localPositionTimer?.cancel();
    _localPositionTimer = null;
  }

  /// Save the resume position locally and publish it on the position stream
  /// (server sync reads it from there). Asks native when [positionMs] isn't given.
  Future<void> _saveLocalPosition({int? positionMs}) async {
    final episode = _currentEpisode;
    if (episode == null) return;

    try {
      final position = positionMs ?? await platform.invokeMethod<int>('getPosition') ?? 0;
      episode.position = position;

      final duration = _playPosition.valueOrNull?.length.inMilliseconds ?? 0;
      _playPosition.add(PositionState(
        position: Duration(milliseconds: position),
        length: Duration(milliseconds: duration),
        percentage: duration > 0 ? ((position / duration) * 100).toInt() : 0,
        episode: episode,
        buffering: false,
        buffered: _playPosition.valueOrNull?.buffered ?? Duration.zero,
      ));

      await _persist(episode);
      log.fine('Local position saved: ${position}ms');
    } catch (e) {
      log.severe('Failed to save local position: $e');
    }
  }

  Future<void> _persist(Episode episode) {
    return _persistChain = _persistChain
        .then((_) => repository.saveEpisode(episode))
        .then((_) {}, onError: (Object e) => log.warning('Failed to save episode locally: $e'));
  }

  void _performBackgroundSync() async {
    try {
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

  void _updateChapter(int seconds, int duration) {
    if (_currentEpisode == null) return;

    final chapters = _currentEpisode!.chapters;
    if (chapters == null || chapters.isEmpty) return;

    for (var chapterPtr = 0; chapterPtr < chapters.length; chapterPtr++) {
      final startTime = chapters[chapterPtr].startTime;
      final endTime = chapterPtr == (chapters.length - 1) ? duration : chapters[chapterPtr + 1].startTime;

      if (seconds >= startTime && seconds < endTime) {
        if (chapters[chapterPtr] != _currentEpisode!.currentChapter) {
          final finishedChapter = _currentEpisode!.currentChapter != null;
          _currentEpisode!.currentChapter = chapters[chapterPtr];
          if (finishedChapter && _sleep.type == SleepType.chapter) {
            log.info('Sleep timer triggered - chapter ended');
            unawaited(_endSleep());
          }
          _episodeEvent.add(_currentEpisode!);
          break;
        }
      }
    }
  }

  // Android Auto / CarPlay browsing methods. Each reads the same server API as
  // the matching app screen.

  /// The shared PinePods client with credentials applied, and the user id, or
  /// null when not connected to a server.
  ({PinepodsService api, int userId})? _carApi(String what) {
    final api = GlobalServices.pinepodsService;
    final server = settingsService.pinepodsServer;
    final apiKey = settingsService.pinepodsApiKey;
    final userId = settingsService.pinepodsUserId;
    if (api == null || server == null || apiKey == null || userId == null) {
      log.warning('Not connected to a PinePods server for $what');
      return null;
    }
    api.setCredentials(server, apiKey);
    return (api: api, userId: userId);
  }

  Future<List<Map<String, dynamic>>> _carList(
    String what,
    Future<List<Map<String, dynamic>>> Function(PinepodsService api, int userId) load,
  ) async {
    final car = _carApi(what);
    if (car == null) return [];
    try {
      final items = await load(car.api, car.userId);
      log.info('$what for car: ${items.length} items');
      return items;
    } catch (e) {
      log.severe('Error getting $what for car: $e');
      return [];
    }
  }

  Future<List<Map<String, dynamic>>> _getCurrentForCar() => _carList('Current', (api, userId) async {
        // Only in-progress episodes, as on the Home screen.
        final home = await api.getHomeOverview(userId);
        return home.inProgressEpisodes.map(_homeEpisodeToCarMap).toList();
      });

  Future<List<Map<String, dynamic>>> _getDownloadsForCar() => _carList('Downloads',
      (api, userId) async => (await api.getServerDownloads(userId)).map(_pinepodsEpisodeToCarMap).toList());

  Future<List<Map<String, dynamic>>> _getQueueForCar() => _carList('Queue',
      (api, userId) async => (await api.getQueuedEpisodes(userId)).map(_pinepodsEpisodeToCarMap).toList());

  Future<List<Map<String, dynamic>>> _getSavedForCar() => _carList('Saved',
      (api, userId) async => (await api.getSavedEpisodes(userId)).map(_pinepodsEpisodeToCarMap).toList());

  Future<List<Map<String, dynamic>>> _getHistoryForCar() => _carList('History',
      (api, userId) async => (await api.getUserHistory(userId)).map(_pinepodsEpisodeToCarMap).toList());

  Future<List<Map<String, dynamic>>> _getPodcastsForCar() => _carList('Podcasts', (api, userId) async {
        final podcasts = await api.getUserPodcasts(userId);
        return podcasts
            .map((podcast) => <String, dynamic>{
                  'id': podcast.id.toString(),
                  'title': podcast.title,
                  'imageUrl': podcast.imageUrl ?? podcast.thumbImageUrl,
                  'episodeCount': podcast.episodes.length,
                })
            .toList();
      });

  Future<List<Map<String, dynamic>>> _getPodcastEpisodesForCar(String podcastId) =>
      _carList('Podcast $podcastId episodes', (api, userId) async {
        final id = int.tryParse(podcastId);
        if (id == null) return [];
        return (await api.getPodcastEpisodes(userId, id)).map(_pinepodsEpisodeToCarMap).toList();
      });

  Future<List<Map<String, dynamic>>> _getPlaylistsForCar() => _carList('Playlists', (api, userId) async {
        final response = await api.getPlaylists(userId);
        return response.playlists
            .map((playlist) => <String, dynamic>{
                  'id': playlist.playlistId,
                  'name': playlist.name,
                  'description': playlist.description,
                  'episodeCount': playlist.episodeCount ?? 0,
                })
            .toList();
      });

  Future<List<Map<String, dynamic>>> _getPlaylistEpisodesForCar(String playlistId) =>
      _carList('Playlist $playlistId episodes', (api, userId) async {
        final id = int.tryParse(playlistId);
        if (id == null) return [];
        return (await api.getPlaylistEpisodes(userId, id)).episodes.map(_pinepodsEpisodeToCarMap).toList();
      });

  Future<void> _playFromMediaIdForCar(String guid) async {
    // Car media ids are "pinepods_<episodeId>".
    final episodeId = guid.startsWith('pinepods_') ? int.tryParse(guid.replaceFirst('pinepods_', '')) : null;
    if (episodeId == null) {
      log.warning('Invalid car media id: $guid');
      return;
    }
    final car = _carApi('playback');
    final pinepodsAudioService = GlobalServices.pinepodsAudioService;
    if (car == null || pinepodsAudioService == null) return;

    try {
      final episode = await car.api.getEpisodeMetadata(episodeId, car.userId);
      if (episode == null) {
        log.warning('Episode metadata not found for ID: $episodeId');
        return;
      }
      // Play using the SAME method as the app UI
      await pinepodsAudioService.playPinepodsEpisode(pinepodsEpisode: episode, resume: true);
    } catch (e) {
      log.severe('Error playing from media ID: $e');
    }
  }

  /// Voice/text search from the car, against the server's episode search.
  Future<Map<String, dynamic>> _searchForCar(String query) async {
    final episodes = await _carList('Search', (api, userId) async {
      final page = await api.searchEpisodes(userId, query, limit: 50);
      return page.results.map((r) => _pinepodsEpisodeToCarMap(r.toPinepodsEpisode())).toList();
    });
    return {'episodes': episodes};
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
  ValueStream<AudioState> get playingState => _playingState.stream;

  @override
  ValueStream<PositionState> get playPosition => _playPosition.stream;

  @override
  ValueStream<double> get playbackSpeed => _speedState.stream;

  @override
  ValueStream<Episode?> get episodeEvent => _episodeEvent.stream;

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
