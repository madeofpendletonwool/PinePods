// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/bloc/bloc.dart';
import 'package:pinepods_mobile/entities/downloadable.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/feed.dart';
import 'package:pinepods_mobile/entities/podcast.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/services/download/download_service.dart';
import 'package:pinepods_mobile/services/download/mobile_download_service.dart';
import 'package:pinepods_mobile/services/podcast/podcast_service.dart';
import 'package:pinepods_mobile/services/settings/settings_service.dart';
import 'package:pinepods_mobile/state/bloc_state.dart';
import 'package:collection/collection.dart' show IterableExtension;
import 'package:logging/logging.dart';
import 'package:rxdart/rxdart.dart';

enum PodcastEvent {
  subscribe,
  unsubscribe,
  markAllPlayed,
  clearAllPlayed,
  reloadSubscriptions,
  refresh,
  // Filter
  episodeFilterNone,
  episodeFilterStarted,
  episodeFilterNotFinished,
  episodeFilterFinished,
  // Sort
  episodeSortDefault,
  episodeSortLatest,
  episodeSortEarliest,
  episodeSortAlphabeticalAscending,
  episodeSortAlphabeticalDescending,
}

/// This BLoC provides access to the details of a given Podcast.
///
/// It takes a feed URL and creates a [Podcast] instance. There are several listeners that
/// handle actions on a podcast such as requesting an episode download, following/unfollowing
/// a podcast and marking/un-marking all episodes as played.
class PodcastBloc extends Bloc {
  final log = Logger('PodcastBloc');
  final PodcastService podcastService;
  final AudioPlayerService audioPlayerService;
  final DownloadService downloadService;
  final SettingsService settingsService;
  final BehaviorSubject<Feed> _podcastFeed = BehaviorSubject<Feed>(sync: true);

  /// Add to sink to start an Episode download
  final PublishSubject<Episode?> _downloadEpisode = PublishSubject<Episode?>();

  /// Listen to this subject's stream to obtain list of current subscriptions.
  late PublishSubject<List<Podcast>> _subscriptions;

  /// Stream containing details of the current podcast.
  final BehaviorSubject<BlocState<Podcast>> _podcastStream = BehaviorSubject<BlocState<Podcast>>(sync: true);

  /// A separate stream that allows us to listen to changes in the podcast's episodes.
  final BehaviorSubject<List<Episode?>?> _episodesStream = BehaviorSubject<List<Episode?>?>();

  /// Receives subscription and mark/clear as played events.
  final PublishSubject<PodcastEvent> _podcastEvent = PublishSubject<PodcastEvent>();

  final BehaviorSubject<String> _podcastSearchEvent = BehaviorSubject<String>();

  final BehaviorSubject<BlocState<void>> _backgroundLoadStream = BehaviorSubject<BlocState<void>>();

  Podcast? _podcast;
  List<Episode> _episodes = <Episode>[];
  String _searchTerm = '';
  late Feed lastFeed;
  bool first = true;

  PodcastBloc({
    required this.podcastService,
    required this.audioPlayerService,
    required this.downloadService,
    required this.settingsService,
  }) {
    _init();
  }

  void _init() {
    /// When someone starts listening for subscriptions, load them.
    _subscriptions = PublishSubject<List<Podcast>>(onListen: _loadSubscriptions);

    /// When we receive a load podcast request, send back a BlocState.
    _listenPodcastLoad();

    /// Listen to an Episode download request
    _listenDownloadRequest();

    /// Listen to active downloads
    _listenDownloads();

    /// Listen to episode change events sent by the [Repository]
    _listenEpisodeRepositoryEvents();

    /// Listen to Podcast subscription, mark/cleared played events
    _listenPodcastStateEvents();

    /// Listen for episode search requests
    _listenPodcastSearchEvents();
  }

  void _loadSubscriptions() async {
    _subscriptions.add(await podcastService.subscriptions());
  }

  /// Sets up a listener to handle Podcast load requests. We first push a [BlocLoadingState] to
  /// indicate that the Podcast is being loaded, before calling the [PodcastService] to handle
  /// the loading. Once loaded, we extract the episodes from the Podcast and push them out via
  /// the episode stream before pushing a [BlocPopulatedState] containing the Podcast.
  void _listenPodcastLoad() async {
    _podcastFeed.listen((feed) async {
      var silent = false;
      lastFeed = feed;

      _episodes = [];
      _refresh();

      _podcastStream.sink.add(BlocLoadingState<Podcast>(feed.podcast));

      try {
        await _loadEpisodes(feed, feed.refresh);

        /// Do we also need to perform a background refresh?
        if (feed.podcast.id != null && feed.backgroundFresh && _shouldAutoRefresh()) {
          silent = feed.silently;
          log.fine('Performing background refresh of ${feed.podcast.url}');
          _backgroundLoadStream.sink.add(BlocLoadingState<void>());

          await _loadNewEpisodes(feed);
        }

        _backgroundLoadStream.sink.add(BlocSuccessfulState<void>());
      } catch (e) {
        _backgroundLoadStream.sink.add(BlocDefaultState<void>());

        // For now we'll assume a network error as this is the most likely.
        if ((_podcast == null || lastFeed.podcast.url == _podcast!.url) && !silent) {
          _podcastStream.sink.add(BlocErrorState<Podcast>());
          log.fine('Error loading podcast', e);
          log.fine(e);
        }
      }
    });
  }

  /// Determines if the current feed should be updated in the background.
  ///
  /// If the autoUpdatePeriod is -1 this means never; 0 means always and any other
  /// value is the time in minutes.
  bool _shouldAutoRefresh() {
    /// If we are currently following this podcast it will have an id. At
    /// this point we can compare the last updated time to the update
    /// after setting time.
    if (settingsService.autoUpdateEpisodePeriod == -1) {
      return false;
    } else if (_podcast == null || settingsService.autoUpdateEpisodePeriod == 0) {
      return true;
    } else if (_podcast != null && _podcast!.id != null) {
      var currentTime = DateTime.now().subtract(Duration(minutes: settingsService.autoUpdateEpisodePeriod));
      var lastUpdated = _podcast!.lastUpdated;

      return currentTime.isAfter(lastUpdated);
    }

    return false;
  }

  Future<void> _loadEpisodes(Feed feed, bool force) async {
    _podcast = await podcastService.loadPodcast(
      podcast: feed.podcast,
      refresh: force,
    );

    /// Only populate episodes if the ID we started the load with is the
    /// same as the one we have ended up with.
    if (_podcast != null && _podcast?.url != null) {
      if (lastFeed.podcast.url == _podcast!.url) {
        _episodes = _podcast!.episodes;
        _refresh();

        _podcastStream.sink.add(BlocPopulatedState<Podcast>(results: _podcast));
      }
    }
  }

  void _refresh() {
    applySearchFilter();
  }

  Future<void> _loadNewEpisodes(Feed feed) async {
    _podcast = await podcastService.loadPodcast(
      podcast: feed.podcast,
      highlightNewEpisodes: true,
      refresh: true,
    );

    /// Only populate episodes if the ID we started the load with is the
    /// same as the one we have ended up with.
    if (_podcast != null && lastFeed.podcast.url == _podcast!.url) {
      _episodes = _podcast!.episodes;

      if (_podcast!.newEpisodes) {
        log.fine('We have new episodes to display');
        _backgroundLoadStream.sink.add(BlocPopulatedState<void>());
        _podcastStream.sink.add(BlocPopulatedState<Podcast>(results: _podcast));
      } else if (_podcast!.updatedEpisodes) {
        log.fine('We have updated episodes to re-display');
        _refresh();
      }
    }

    log.fine('Background loading successful state');
    _backgroundLoadStream.sink.add(BlocSuccessfulState<void>());
  }

  Future<void> _loadFilteredEpisodes() async {
    if (_podcast != null) {
      _podcast = await podcastService.loadPodcast(
        podcast: _podcast!,
        highlightNewEpisodes: false,
        refresh: false,
      );

      _episodes = _podcast!.episodes;
      _podcastStream.add(BlocPopulatedState<Podcast>(results: _podcast));
      _refresh();
    }
  }

  /// Sets up a listener to handle requests to download an episode.
  void _listenDownloadRequest() {
    _downloadEpisode.listen((Episode? e) async {
      log.fine('Received download request for ${e!.title}');

      // To prevent a pause between the user tapping the download icon and
      // the UI showing some sort of progress, set it to queued now.
      var episode = _episodes.firstWhereOrNull((ep) => ep.guid == e.guid);

      if (episode != null) {
        episode.downloadState = e.downloadState = DownloadState.queued;

        _refresh();

        var result = await downloadService.downloadEpisode(e);

        // If there was an error downloading the episode, push an error state
        // and then restore to none.
        if (!result) {
          episode.downloadState = e.downloadState = DownloadState.failed;
          _refresh();
          episode.downloadState = e.downloadState = DownloadState.none;
          _refresh();
        }
      }
    });
  }

  /// Sets up a listener to listen for status updates from any currently downloading episode.
  ///
  /// If the ID of a current download matches that of an episode currently in
  /// use, we update the status of the episode and push it back into the episode stream.
  void _listenDownloads() {
    // Listen to download progress
    MobileDownloadService.downloadProgress.listen((downloadProgress) {
      downloadService.findEpisodeByTaskId(downloadProgress.id).then((downloadable) {
        if (downloadable != null) {
          // If the download matches a current episode push the update back into the stream.
          var episode = _episodes.firstWhereOrNull((e) => e.downloadTaskId == downloadProgress.id);

          if (episode != null) {
            // Update the stream.
            _refresh();
          }
        } else {
          log.severe('Downloadable not found with id ${downloadProgress.id}');
        }
      });
    });
  }

  /// Listen to episode change events sent by the [Repository]
  void _listenEpisodeRepositoryEvents() {
    podcastService.episodeListener!.listen((state) {
      // Do we have this episode?
      var eidx = _episodes.indexWhere((e) => e.guid == state.episode.guid && e.pguid == state.episode.pguid);

      if (eidx != -1) {
        _episodes[eidx] = state.episode;
        _refresh();
      }
    });
  }

  // TODO: This needs refactoring to simplify the long switch statement.
  void _listenPodcastStateEvents() async {
    _podcastEvent.listen((event) async {
      switch (event) {
        case PodcastEvent.subscribe:
          if (_podcast != null) {
            _podcast = await podcastService.subscribe(_podcast!);
            _podcastStream.add(BlocPopulatedState<Podcast>(results: _podcast));
            _loadSubscriptions();
            _episodesStream.add(_podcast?.episodes);
          }
          break;
        case PodcastEvent.unsubscribe:
          if (_podcast != null) {
            await podcastService.unsubscribe(_podcast!);
            _podcast!.id = null;
            _podcastStream.add(BlocPopulatedState<Podcast>(results: _podcast));
            _loadSubscriptions();
            _episodesStream.add(_podcast!.episodes);
          }
          break;
        case PodcastEvent.markAllPlayed:
          if (_podcast != null && _podcast?.episodes != null) {
            final changedEpisodes = <Episode>[];

            for (var e in _podcast!.episodes) {
              if (!e.played) {
                e.played = true;
                e.position = 0;

                changedEpisodes.add(e);
              }
            }

            await podcastService.saveEpisodes(changedEpisodes);
            _episodesStream.add(_podcast!.episodes);
          }
          break;
        case PodcastEvent.clearAllPlayed:
          if (_podcast != null && _podcast?.episodes != null) {
            final changedEpisodes = <Episode>[];

            for (var e in _podcast!.episodes) {
              if (e.played) {
                e.played = false;
                e.position = 0;

                changedEpisodes.add(e);
              }
            }

            await podcastService.saveEpisodes(changedEpisodes);
            _episodesStream.add(_podcast!.episodes);
          }
          break;
        case PodcastEvent.reloadSubscriptions:
          _loadSubscriptions();
          break;
        case PodcastEvent.refresh:
          _refresh();
          break;
        case PodcastEvent.episodeFilterNone:
          if (_podcast != null) {
            _podcast!.filter = PodcastEpisodeFilter.none;
            _podcast = await podcastService.save(_podcast!, withEpisodes: false);
            await _loadFilteredEpisodes();
          }
          break;
        case PodcastEvent.episodeFilterStarted:
          _podcast!.filter = PodcastEpisodeFilter.started;
          _podcast = await podcastService.save(_podcast!, withEpisodes: false);
          await _loadFilteredEpisodes();
          break;
        case PodcastEvent.episodeFilterFinished:
          _podcast!.filter = PodcastEpisodeFilter.played;
          _podcast = await podcastService.save(_podcast!, withEpisodes: false);
          await _loadFilteredEpisodes();
          break;
        case PodcastEvent.episodeFilterNotFinished:
          _podcast!.filter = PodcastEpisodeFilter.notPlayed;
          _podcast = await podcastService.save(_podcast!, withEpisodes: false);
          await _loadFilteredEpisodes();
          break;
        case PodcastEvent.episodeSortDefault:
          _podcast!.sort = PodcastEpisodeSort.none;
          _podcast = await podcastService.save(_podcast!, withEpisodes: false);
          await _loadFilteredEpisodes();
          break;
        case PodcastEvent.episodeSortLatest:
          _podcast!.sort = PodcastEpisodeSort.latestFirst;
          _podcast = await podcastService.save(_podcast!, withEpisodes: false);
          await _loadFilteredEpisodes();
          break;
        case PodcastEvent.episodeSortEarliest:
          _podcast!.sort = PodcastEpisodeSort.earliestFirst;
          _podcast = await podcastService.save(_podcast!, withEpisodes: false);
          await _loadFilteredEpisodes();
          break;
        case PodcastEvent.episodeSortAlphabeticalAscending:
          _podcast!.sort = PodcastEpisodeSort.alphabeticalAscending;
          _podcast = await podcastService.save(_podcast!, withEpisodes: false);
          await _loadFilteredEpisodes();
          break;
        case PodcastEvent.episodeSortAlphabeticalDescending:
          _podcast!.sort = PodcastEpisodeSort.alphabeticalDescending;
          _podcast = await podcastService.save(_podcast!, withEpisodes: false);
          await _loadFilteredEpisodes();
          break;
      }
    });
  }

  void _listenPodcastSearchEvents() {
    _podcastSearchEvent.debounceTime(const Duration(milliseconds: 200)).listen((search) {
      _searchTerm = search;
      applySearchFilter();
    });
  }

  void applySearchFilter() {
    if (_searchTerm.isEmpty) {
      _episodesStream.add(_episodes);
    } else {
      var searchFilteredEpisodes =
          _episodes.where((e) => e.title!.toLowerCase().contains(_searchTerm.trim().toLowerCase())).toList();
      _episodesStream.add(searchFilteredEpisodes);
    }
  }

  @override
  void detach() {
    downloadService.dispose();
  }

  @override
  void dispose() {
    _podcastFeed.close();
    _downloadEpisode.close();
    _subscriptions.close();
    _podcastStream.close();
    _episodesStream.close();
    _podcastEvent.close();
    MobileDownloadService.downloadProgress.close();
    downloadService.dispose();
    super.dispose();
  }

  /// Sink to load a podcast.
  void Function(Feed) get load => _podcastFeed.add;

  /// Sink to trigger an episode download.
  void Function(Episode?) get downloadEpisode => _downloadEpisode.add;

  void Function(PodcastEvent) get podcastEvent => _podcastEvent.add;

  void Function(String) get podcastSearchEvent => _podcastSearchEvent.add;

  /// Stream containing the current state of the podcast load.
  Stream<BlocState<Podcast>> get details => _podcastStream.stream;

  Stream<BlocState<void>> get backgroundLoading => _backgroundLoadStream.stream;

  /// Stream containing the current list of Podcast episodes.
  Stream<List<Episode?>?> get episodes => _episodesStream;

  /// Obtain a list of podcast currently subscribed to.
  Stream<List<Podcast>> get subscriptions => _subscriptions.stream;
}
