// lib/services/pinepods/pinepods_audio_service.dart

import 'dart:async';
import 'package:flutter/foundation.dart';
import 'package:pinepods_mobile/entities/downloadable.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/entities/chapter.dart';
import 'package:pinepods_mobile/entities/person.dart';
import 'package:pinepods_mobile/entities/transcript.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/services/nowplaying/nowplaying_service.dart';
import 'package:pinepods_mobile/services/offline/offline_action_queue.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:logging/logging.dart';

/// An episode to play next, and how to play it.
typedef _NextEpisode = ({PinepodsEpisode episode, int? playlistId, bool skipQueue});

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

  /// Title/artwork of the current episode, cached so now-playing reports can
  /// describe what this device is playing without another lookup.
  String? _currentTitle;
  String? _currentArtwork;

  /// Latest playback state, tracked so now-playing reports carry an accurate
  /// playing/paused flag (the audio service's playingState is a plain Stream).
  bool _isPlaying = false;
  StreamSubscription<AudioState>? _playingStateSub;

  /// Best-effort now-playing reporter (cross-device awareness + remote control).
  /// Optional: a null service, or one that isn't connected, never affects playback.
  NowPlayingService? _nowPlayingService;

  /// AI transcript lines for the playing episode; empty when it has none.
  final ValueNotifier<List<TranscriptCue>> transcriptCues = ValueNotifier(const []);

  /// Ad ranges (seconds) being skipped in the playing episode.
  final ValueNotifier<List<({double start, double end})>> skippedAds = ValueNotifier(const []);

  /// Public getters used by the now-playing reporter.
  int? get currentEpisodeId => _currentEpisodeId;
  int? get currentPodcastId => _currentPodcastId;
  bool get isYoutube => _isYoutube;

  /// The playlist the current episode is being played from, if any. Used to
  /// continue playback through the playlist when an episode completes.
  /// (NativeAudioPlayerService.setPlaylistContext is a no-op, so we track it here.)
  int? _currentPlaylistId;

  /// Podcast id and auto-play-next flag of the playing episode, once known,
  /// so the end-of-episode advance doesn't have to fetch them again.
  int? _currentPodcastId;
  bool? _currentAutoPlayNext;

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
       _onStopCallback = onStopCallback {
    // Track playback state so now-playing reports carry an accurate flag.
    _playingStateSub = _audioPlayerService.playingState?.listen((state) {
      _isPlaying =
          state == AudioState.playing || state == AudioState.buffering;
    });
  }

  /// Wire up the offline outbox (called once during app start-up).
  void setActionQueue(OfflineActionQueue queue) {
    _actionQueue = queue;
  }

  /// Wire up the now-playing reporter (called once during app start-up).
  void setNowPlayingService(NowPlayingService service) {
    _nowPlayingService = service;
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

  /// Where to resume [episode], in seconds: the furthest of the position its
  /// (possibly stale) list data carries, any position still waiting in the
  /// offline outbox, and [serverPosition] when one was fetched.
  Future<double> _resolveResumePosition(PinepodsEpisode episode, {double? serverPosition}) async {
    if (episode.completed) return 0;
    var best = (episode.listenDuration ?? 0).toDouble();
    final pending = await _actionQueue?.pendingPosition(episode.episodeId, episode.isYoutube);
    for (final p in [pending, serverPosition]) {
      if (p != null && p > best) best = p;
    }
    return best;
  }

  /// How long a streamed start waits for the server's resume position and
  /// playback settings before starting from what it already knows.
  static const startWait = Duration(milliseconds: 1500);

  /// Play a PinePods episode with full server integration
  Future<void> playPinepodsEpisode({
    required PinepodsEpisode pinepodsEpisode,
    bool resume = true,
    bool skipQueue = false,
    int? playlistId,
  }) async {
    _currentPlaylistId = playlistId;
    _audioPlayerService.setPlaylistContext(playlistId);
    final prepWatch = Stopwatch()..start();
    try {
      // Not on the server (an unfollowed podcast's feed): nothing to track.
      if (pinepodsEpisode.episodeId == 0) {
        await _playUntracked(pinepodsEpisode);
        return;
      }

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

      _currentEpisodeId = episodeId;
      _currentEpisodeDuration = pinepodsEpisode.episodeDuration;
      _currentTitle = pinepodsEpisode.episodeTitle;
      _currentArtwork = pinepodsEpisode.episodeArtwork;
      final knownPodcastId = pinepodsEpisode.podcastId;
      _currentPodcastId = knownPodcastId != null && knownPodcastId > 0 ? knownPodcastId : null;
      _currentAutoPlayNext = null;
      transcriptCues.value = const [];
      skippedAds.value = const [];

      // Is there a local download for this episode? If so we play the on-disk
      // file and tolerate the server being unreachable for everything else.
      final localDownload = await _audioPlayerService.findDownloadedEpisode(episodeId);

      // Push any progress saved while offline before this playback adds more.
      unawaited(_actionQueue?.flush());

      // Offline-first fast path (#935): a local download plays entirely from
      // disk, so start it immediately and defer every server round-trip. This
      // means tapping a downloaded episode with the server unreachable begins
      // playback right away instead of stalling ~30s on API timeouts.
      if (localDownload != null) {
        final localEpisode = _convertToEpisode(
          pinepodsEpisode,
          PlayEpisodeDetails(playbackSpeed: 1.0, startSkip: 0, endSkip: 0),
          null,
        );
        // Select the on-disk file. As in the streaming path we set only
        // downloadState + the file location (NOT downloadPercentage) so this
        // transient playback record doesn't masquerade as a second entry in the
        // downloads list.
        localEpisode.downloadState = DownloadState.downloaded;
        localEpisode.filepath = localDownload.filepath;
        localEpisode.filename = localDownload.filename;
        // Reuse metadata already stored with the download (chapters/persons/
        // transcript) so the now-playing view is fully populated with no
        // network round-trip.
        localEpisode.chapters = localDownload.chapters;
        localEpisode.persons = localDownload.persons;
        localEpisode.chaptersUrl = localDownload.chaptersUrl;
        localEpisode.transcript = localDownload.transcript;
        localEpisode.transcriptId = localDownload.transcriptId;
        log.info('Playing local download for episode $episodeId (offline fast path)');

        final initialPosition = resume ? await _resolveResumePosition(pinepodsEpisode) : 0.0;
        if (resume) localEpisode.position = (initialPosition * 1000).round();

        await _audioPlayerService.playEpisode(episode: localEpisode, resume: resume, playbackSpeed: null);
        await _onStarted(
          pinepodsEpisode: pinepodsEpisode,
          episode: localEpisode,
          userId: userId,
          resume: resume,
          skipQueue: skipQueue,
          initialPosition: initialPosition,
          playDetails: null,
          lateDetails: null,
          podcast2Data: null,
        );
        return;
      }

      // Streaming: only the resume position and the playback settings (speed,
      // intro skip) matter before audio starts. Both are fetched in parallel
      // and waited on for at most [startWait]; chapters, people and transcript
      // links fill in once playback is underway.
      final podcast2Data = _pinepodsService.fetchPodcasting2Data(episodeId, userId).catchError((e) {
        log.fine('Could not fetch podcast 2.0 data (continuing): $e');
        return null;
      });
      final playDetailsFetch = _fetchPlayDetails(pinepodsEpisode, userId);
      final startData = await Future.wait<dynamic>([
        (resume
                ? getServerPositionForEpisode(episodeId, userId, pinepodsEpisode.isYoutube)
                : Future<double?>.value(null))
            .timeout(startWait, onTimeout: () {
          log.info('Server position slow for episode $episodeId; starting from the known position');
          return null;
        }),
        playDetailsFetch.timeout(startWait, onTimeout: () => null),
      ]);
      final initialPosition = resume
          ? await _resolveResumePosition(pinepodsEpisode, serverPosition: startData[0] as double?)
          : 0.0;
      final playDetails = startData[1] as PlayEpisodeDetails?;

      // Convert PinepodsEpisode to Episode for the audio player
      final episode = _convertToEpisode(
        pinepodsEpisode,
        playDetails ?? PlayEpisodeDetails(playbackSpeed: 1.0, startSkip: 0, endSkip: 0),
        null,
      );
      if (resume) episode.position = (initialPosition * 1000).round();

      // Skip the intro by starting past it rather than seeking once it plays.
      final introSkip = !resume ? playDetails?.startSkip ?? 0 : 0;
      if (introSkip > 0) episode.position = introSkip * 1000;

      log.info('Streamed start ready to play after ${prepWatch.elapsedMilliseconds}ms of server lookups');

      // Only apply the server speed if it's a real per-podcast override, not the echoed-back default (#882, #963).
      await _audioPlayerService.playEpisode(
        episode: episode,
        resume: resume || introSkip > 0,
        playbackSpeed: playDetails != null && playDetails.playbackSpeedCustomized ? playDetails.playbackSpeed : null,
      );

      await _onStarted(
        pinepodsEpisode: pinepodsEpisode,
        episode: episode,
        userId: userId,
        resume: resume,
        skipQueue: skipQueue,
        initialPosition: initialPosition,
        playDetails: playDetails,
        lateDetails: playDetails == null ? playDetailsFetch : null,
        podcast2Data: podcast2Data,
      );
    } catch (e) {
      log.severe('Error playing PinePods episode: $e');
      rethrow;
    }
  }

  /// The podcast's playback settings, looking up its id first when the list
  /// data didn't carry it. Null when the server can't be reached.
  Future<PlayEpisodeDetails?> _fetchPlayDetails(PinepodsEpisode episode, int userId) async {
    try {
      final podcastId = await _podcastIdFor(episode, userId);
      return await _pinepodsService.getPlayEpisodeDetails(userId, podcastId, episode.isYoutube);
    } catch (e) {
      log.fine('Could not fetch play details (continuing): $e');
      return null;
    }
  }

  /// The podcast id for [episode] (0 when unknown), cached for the episode
  /// now playing so completion handling doesn't look it up again.
  Future<int> _podcastIdFor(PinepodsEpisode episode, int userId) async {
    final known = episode.podcastId;
    if (known != null && known > 0) return known;
    if (_currentEpisodeId == episode.episodeId && _currentPodcastId != null) return _currentPodcastId!;
    final podcastId = await _pinepodsService
        .getPodcastIdFromEpisode(episode.episodeId, userId, episode.isYoutube)
        .catchError((e) {
      log.fine('Could not fetch podcast id (continuing): $e');
      return 0;
    });
    if (_currentEpisodeId == episode.episodeId && podcastId > 0) _currentPodcastId = podcastId;
    return podcastId;
  }

  /// Bookkeeping once native playback has been asked to start: record the
  /// play in history, start the periodic syncs, then apply the server-side
  /// extras in the background so they never hold up or fail playback.
  Future<void> _onStarted({
    required PinepodsEpisode pinepodsEpisode,
    required Episode episode,
    required int userId,
    required bool resume,
    required bool skipQueue,
    required double initialPosition,
    required PlayEpisodeDetails? playDetails,
    required Future<PlayEpisodeDetails?>? lateDetails,
    required Future<Map<String, dynamic>?>? podcast2Data,
  }) async {
    final episodeId = pinepodsEpisode.episodeId;

    // Add to history via the offline outbox (buffered locally when offline).
    // Records the resolved position: the outbox keeps one position per
    // episode, so a stale value here would replace newer unsynced progress.
    await _recordPosition(episodeId, userId, initialPosition);
    _lastRecordedPosition = initialPosition;

    _startPeriodicUpdates();
    _isPlaying = true;
    _reportNowPlaying();

    unawaited(_applyServerExtras(
      pinepodsEpisode: pinepodsEpisode,
      episode: episode,
      userId: userId,
      resume: resume,
      skipQueue: skipQueue,
      playDetails: playDetails,
      lateDetails: lateDetails,
      podcast2Data: podcast2Data,
    ));
  }

  /// Best-effort, background application of everything that doesn't need to
  /// be in place before audio starts: playback settings that missed the start
  /// window, skip ranges, chapters/people/transcript links, queue anchoring
  /// and the played count. The independent calls run concurrently, and the
  /// [_currentEpisodeId] guards drop results for an episode no longer playing.
  Future<void> _applyServerExtras({
    required PinepodsEpisode pinepodsEpisode,
    required Episode episode,
    required int userId,
    required bool resume,
    required bool skipQueue,
    required PlayEpisodeDetails? playDetails,
    required Future<PlayEpisodeDetails?>? lateDetails,
    required Future<Map<String, dynamic>?>? podcast2Data,
  }) async {
    final episodeId = pinepodsEpisode.episodeId;
    bool stillPlaying() => _currentEpisodeId == episodeId;

    try {
      final podcastId = await _podcastIdFor(pinepodsEpisode, userId);
      if (!stillPlaying()) return;

      Future<void> lateSettings() async {
        if (playDetails != null) return;
        // A streamed start that outwaited the settings keeps that request.
        final details = await (lateDetails ?? _fetchPlayDetails(pinepodsEpisode, userId));
        if (details == null || !stillPlaying()) return;
        if (details.playbackSpeedCustomized) {
          await _audioPlayerService.setPlaybackSpeed(details.playbackSpeed, persist: false);
        }
        final position = _audioPlayerService.playPosition?.valueOrNull?.position.inSeconds ?? 0;
        if (details.startSkip > 0 && !resume && position < details.startSkip) {
          await _audioPlayerService.seek(position: details.startSkip);
        }
      }

      Future<void> episodeDetails() async {
        final data = await podcast2Data;
        if (data == null || !stillPlaying()) return;
        final withDetails = _convertToEpisode(
          pinepodsEpisode,
          PlayEpisodeDetails(playbackSpeed: 1.0, startSkip: 0, endSkip: 0),
          data,
        );
        _audioPlayerService.updateNowPlayingDetails(episode.guid, (e) {
          e.chapters = withDetails.chapters;
          e.chaptersUrl = withDetails.chaptersUrl;
          e.persons = withDetails.persons;
          e.transcriptUrls = withDetails.transcriptUrls;
        });
      }

      Future<void> anchorInQueue() async {
        bool autoPlayNext = false;
        try {
          autoPlayNext = await _pinepodsService.getAutoPlayNextStatus(podcastId, userId);
          if (stillPlaying()) _currentAutoPlayNext = autoPlayNext;
        } catch (e) {
          log.fine('Could not check auto-play-next status: $e');
        }
        // Playing an episode makes it the queue anchor: move it to the top of
        // the queue (inserting it if not already queued). Skipped for
        // playlist/serial auto-play-next contexts, which stay out of the queue.
        if (skipQueue || autoPlayNext) return;
        try {
          await _pinepodsService.moveQueueEpisodeToTop(episodeId, userId, pinepodsEpisode.isYoutube);
        } catch (e) {
          log.fine('Could not move episode to top of queue (continuing): $e');
        }
      }

      Future<void> loadTranscript() async {
        try {
          final transcript = await _pinepodsService.getEpisodeTranscript(episodeId, userId);
          if (transcript != null && stillPlaying()) transcriptCues.value = transcript.cues;
        } catch (e) {
          log.fine('Could not load transcript (continuing): $e');
        }
      }

      Future<void> countPlay() async {
        try {
          await _pinepodsService.incrementPlayed(userId);
        } catch (e) {
          log.fine('Could not increment played count (continuing): $e');
        }
      }

      await Future.wait([
        lateSettings(),
        // Silence (#727) + ad-skip (#790): fetch and apply the server-detected
        // skip ranges for this episode. Decoupled from the silence toggle so
        // ads skip even when silence-trim is off.
        refreshSkipSegments(episodeId, userId, podcastId),
        episodeDetails(),
        anchorInQueue(),
        loadTranscript(),
        countPlay(),
      ]);
    } catch (e) {
      log.fine('Applying server extras failed (continuing): $e');
    }
  }

  /// Play an episode the server has no record of straight from its feed URL,
  /// with no position sync, history, queue or skip ranges. Clears the previous
  /// episode's tracking so its timers don't keep reporting under the old id.
  Future<void> _playUntracked(PinepodsEpisode pinepodsEpisode) async {
    if (pinepodsEpisode.episodeUrl.isEmpty) {
      throw Exception('Episode has no audio URL');
    }
    _stopPeriodicUpdates();
    _currentEpisodeId = null;
    _currentPodcastId = null;
    _currentAutoPlayNext = null;
    transcriptCues.value = const [];
    skippedAds.value = const [];
    _currentEpisodeDuration = null;
    _currentTitle = pinepodsEpisode.episodeTitle;
    _currentArtwork = pinepodsEpisode.episodeArtwork;
    _isYoutube = false;

    final episode = _convertToEpisode(
      pinepodsEpisode,
      PlayEpisodeDetails(playbackSpeed: 1.0, startSkip: 0, endSkip: 0),
      null,
    );
    log.info('Playing untracked episode: ${pinepodsEpisode.episodeTitle}');
    await _audioPlayerService.playEpisode(episode: episode, resume: false, playbackSpeed: null);
    // Drop skip ranges left over from the previous episode.
    await _audioPlayerService.applyEpisodeSkipSegments(
        silenceEnabled: false, silenceRanges: const [], adRanges: const []);
    _isPlaying = true;
  }

  /// Report this device's current playback to the now-playing socket. Purely
  /// best-effort: no-ops when no episode is loaded or no reporter is wired, and
  /// never touches local playback.
  void _reportNowPlaying({bool? playingOverride}) {
    final nps = _nowPlayingService;
    final episodeId = _currentEpisodeId;
    if (nps == null || episodeId == null) return;

    // valueOrNull (not .value): playPosition is an unseeded BehaviorSubject, so
    // .value throws until the first position event — and the play-start report
    // below runs before that, inside a try that would rethrow and break playback.
    final positionState = _audioPlayerService.playPosition?.valueOrNull;
    final positionSec =
        positionState?.position.inSeconds.toDouble() ?? _lastRecordedPosition;
    final durationSec = (positionState != null && positionState.length.inSeconds > 0)
        ? positionState.length.inSeconds.toDouble()
        : (_currentEpisodeDuration ?? 0).toDouble();

    nps.reportNowPlaying(
      episodeId: episodeId,
      isYoutube: _isYoutube,
      title: _currentTitle ?? '',
      artworkUrl: _currentArtwork ?? '',
      positionSec: positionSec,
      durationSec: durationSec,
      playing: playingOverride ?? _isPlaying,
      speed: _settingsBloc.currentSettings.playbackSpeed,
    );
  }

  /// Fetch the server-detected skip segments for an episode and push the active
  /// ranges to the native player. Silence (#727) honors the per-podcast toggle
  /// (Android ORs it with the user's global preference on the DSP path); ad
  /// ranges (#790) are gated on the server-resolved per-user status
  /// (active/confirmed => skip) and are independent of the silence toggle.
  ///
  /// Safe to call again mid-episode (e.g. after a Confirm/Deny/Detect in the UI)
  /// to re-supply the native layer — do NOT cache a one-time snapshot.
  ///
  /// Ad times only fit the copy of the audio they were found on (dynamically
  /// inserted ads differ between fetches), so ads are held back unless the
  /// playing copy is the user's matching server download or has the same length
  /// as the analyzed one.
  Future<void> refreshSkipSegments(int episodeId, int userId, int podcastId) async {
    try {
      final trimSettings = await _pinepodsService.getSilenceTrim(userId, podcastId);
      final info = await _pinepodsService.getEpisodeSkipInfo(episodeId, userId);
      final fetched = info.segments;

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
      final adAudio = info.adAudio;
      final url = _audioPlayerService.nowPlaying?.contentUrl ?? '';
      final playingServerCopy = url.contains('/api/data/stream/');
      final adsFitNow = adAudio == null ||
          adAudio.fits(0, playingServerCopy: playingServerCopy);

      Future<void> apply(List<Map<String, double>> ads) {
        skippedAds.value = [for (final a in ads) (start: a['start']!, end: a['end']!)];
        return _audioPlayerService.applyEpisodeSkipSegments(
          silenceEnabled: trimSettings.enabled,
          silenceRanges: silenceRanges,
          adRanges: ads,
        );
      }

      if (_currentEpisodeId != episodeId) return;
      await apply(adsFitNow ? adRanges : []);

      // The length decides whether the ads fit; it arrives once the player has
      // loaded the episode, so add the ads then without holding up playback.
      if (!adsFitNow && adRanges.isNotEmpty) {
        unawaited(_playbackLength(url).then((length) async {
          if (_currentEpisodeId != episodeId) return;
          if (length != null &&
              adAudio.fits(length, playingServerCopy: playingServerCopy)) {
            await apply(adRanges);
          } else {
            log.info('Holding back ad skips for episode $episodeId: the playing '
                "copy doesn't match the one ads were detected on "
                '(${adAudio.download}, length $length vs ${adAudio.analyzedDuration})');
          }
        }));
      }
    } catch (e) {
      log.fine('Could not apply skip segments (continuing): $e');
    }
  }

  /// The length (seconds) the player reports for [contentUrl], or null if it
  /// doesn't within 30s.
  Future<double?> _playbackLength(String contentUrl) async {
    final positions = _audioPlayerService.playPosition;
    if (positions == null) return null;
    try {
      final state = await positions
          .where((p) =>
              p.length.inMilliseconds > 0 && p.episode?.contentUrl == contentUrl)
          .first
          .timeout(const Duration(seconds: 30));
      return state.length.inMilliseconds / 1000.0;
    } catch (_) {
      return null;
    }
  }

  /// Start periodic updates to server
  void _startPeriodicUpdates() {
    _stopPeriodicUpdates(); // Clean up any existing timers

    // Episode position updates every 15 seconds (more frequent for reliability).
    // Also drives the best-effort now-playing report, which doubles as the
    // snapshot-TTL heartbeat while paused (the timer keeps running until dispose).
    _episodeUpdateTimer = Timer.periodic(
      const Duration(seconds: 15),
      (_) {
        _safeUpdateEpisodePosition();
        _reportNowPlaying();
        prefetchNextIfNearEnd();
      },
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

  /// Handle natural end-of-episode: advance to the next thing to play.
  ///
  /// Mirrors the web frontend's `onended` handler
  /// (web/src/components/audio.rs) with a 3-priority advance:
  ///   1. Playlist continuation (if the current episode came from a playlist)
  ///   2. Per-podcast auto-play-next (next episode of the same podcast)
  ///   3. Server-side queue (next queued episode)
  ///
  /// The next episode is normally worked out shortly before the end (see
  /// [prefetchNextIfNearEnd]), so the advance doesn't wait on those lookups.
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

    // Mark the finished episode completed on the server (which also drops it
    // from the queue). On natural completion nothing else records the final
    // state (periodic sync can be up to 15s stale), so do it explicitly.
    // Fire-and-forget — it doesn't influence which episode plays next.
    unawaited(
      _pinepodsService.markEpisodeCompleted(episodeId, userId, _isYoutube).catchError((e) {
        log.fine('Could not mark episode completed (continuing): $e');
        return false;
      }),
    );

    final prefetch = _prefetch;
    _prefetch = null;
    final fresh = prefetch != null &&
        prefetch.episodeId == episodeId &&
        DateTime.now().difference(prefetch.at) < const Duration(minutes: 5);
    final next = await (fresh ? prefetch.next : _resolveNext(episodeId, userId));

    if (next == null) {
      log.info('Nothing left to play - stopping playback');
      return false;
    }

    try {
      log.info('Auto-advancing to ${next.episode.episodeTitle}');
      await playPinepodsEpisode(
        pinepodsEpisode: next.episode,
        resume: false,
        playlistId: next.playlistId,
        // Auto-play-next episodes shouldn't be added to the queue.
        skipQueue: next.skipQueue,
      );
      return true;
    } catch (e) {
      log.warning('Failed to start the next episode: $e');
      return false;
    }
  }

  /// The next episode decided ahead of the end of [episodeId], with when.
  ({int episodeId, DateTime at, Future<_NextEpisode?> next})? _prefetch;

  /// Once the playing episode is within [prefetchLead] of its end, work out
  /// what plays next so completion can start it straight away.
  static const prefetchLead = Duration(seconds: 90);

  @visibleForTesting
  void prefetchNextIfNearEnd() {
    final episodeId = _currentEpisodeId;
    final userId = _currentUserId;
    if (episodeId == null || userId == null || _prefetch?.episodeId == episodeId) return;
    final position = _audioPlayerService.playPosition?.valueOrNull;
    if (position == null || position.length <= Duration.zero) return;
    if (position.length - position.position > prefetchLead) return;

    log.fine('Resolving the episode after $episodeId ahead of its end');
    _prefetch = (episodeId: episodeId, at: DateTime.now(), next: _resolveNext(episodeId, userId));
  }

  /// What plays after [episodeId]: playlist, then auto-play-next, then queue.
  Future<_NextEpisode?> _resolveNext(int episodeId, int userId) async {
    // PRIORITY 1: Playlist continuation.
    final playlistId = _currentPlaylistId;
    if (playlistId != null) {
      try {
        final next = await _pinepodsService.getNextPlaylistEpisode(episodeId, playlistId, userId);
        if (next != null) return (episode: next, playlistId: playlistId, skipQueue: false);
        log.info('Playlist exhausted, continuing outside it');
      } catch (e) {
        log.fine('Could not get next playlist episode (continuing): $e');
      }
    }

    // PRIORITY 2: Per-podcast auto-play-next. Uses what was learned when the
    // episode started where possible.
    try {
      final podcastId = _currentPodcastId ?? await _pinepodsService.getPodcastIdFromEpisode(episodeId, userId, _isYoutube);
      final autoPlayNext = _currentAutoPlayNext ?? await _pinepodsService.getAutoPlayNextStatus(podcastId, userId);
      if (autoPlayNext) {
        final next = await _pinepodsService.getNextPodcastEpisode(episodeId, userId);
        if (next != null) return (episode: next, playlistId: null, skipQueue: true);
        log.info('No next episode found in podcast, falling through to queue');
      }
    } catch (e) {
      log.fine('Could not evaluate auto-play-next (continuing): $e');
    }

    // PRIORITY 3: Server-side queue. The backend returns it ORDER BY
    // queueposition ASC, so the first entry that isn't the finishing episode
    // (and isn't already completed) is next.
    try {
      final queued = await _pinepodsService.getQueuedEpisodes(userId);
      for (final ep in queued) {
        if (ep.episodeId != episodeId && !ep.completed) {
          return (episode: ep, playlistId: null, skipQueue: false);
        }
      }
    } catch (e) {
      log.warning('Could not read the queue for the next episode: $e');
    }
    return null;
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
    _isPlaying = false;
    // Best-effort: let other devices see the pause promptly.
    _reportNowPlaying(playingOverride: false);
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
              // Servers with the people endpoints send the person id (0 = unknown).
              personId: (personData['id'] as int?) ?? 0,
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
    _playingStateSub?.cancel();
    _playingStateSub = null;
    _currentEpisodeId = null;
    _currentUserId = null;
    _currentPlaylistId = null;
    _currentPodcastId = null;
    _currentAutoPlayNext = null;
    _currentEpisodeDuration = null;
    _currentTitle = null;
    _currentArtwork = null;
  }
}

