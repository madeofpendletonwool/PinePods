// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/core/extensions.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/podcast.dart';
import 'package:pinepods_mobile/entities/queue.dart';
import 'package:pinepods_mobile/entities/transcript.dart';
import 'package:pinepods_mobile/repository/repository.dart';
import 'package:pinepods_mobile/repository/sembast/sembast_database_service.dart';
import 'package:pinepods_mobile/state/episode_state.dart';
import 'package:flutter/foundation.dart';
import 'package:logging/logging.dart';
import 'package:rxdart/rxdart.dart';
import 'package:sembast/sembast.dart';

/// An implementation of [Repository] that is backed by
/// [Sembast](https://github.com/tekartik/sembast.dart/tree/master/sembast)
class SembastRepository extends Repository {
  final log = Logger('SembastRepository');

  final _podcastSubject = BehaviorSubject<Podcast>();
  final _episodeSubject = BehaviorSubject<EpisodeState>();

  final _podcastStore = intMapStoreFactory.store('podcast');
  final _episodeStore = intMapStoreFactory.store('episode');
  final _queueStore = intMapStoreFactory.store('queue');
  final _transcriptStore = intMapStoreFactory.store('transcript');

  final _queueGuids = <String>[];

  late DatabaseService _databaseService;

  Future<Database> get _db async => _databaseService.database;

  SembastRepository({
    bool cleanup = true,
    String databaseName = 'anytime.db',
  }) {
    _databaseService = DatabaseService(databaseName, version: 2, upgraderCallback: dbUpgrader);

    if (cleanup) {
      _cleanupEpisodes().then((value) {
        log.fine('Orphan episodes cleanup complete');
      });
    }
  }

  /// Saves the [Podcast] instance and associated [Episode]s. Podcasts are
  /// only stored when we subscribe to them, so at the point we store a
  /// new podcast we store the current [DateTime] to mark the
  /// subscription date.
  @override
  Future<Podcast> savePodcast(Podcast podcast, {bool withEpisodes = true}) async {
    log.fine('Saving podcast (${podcast.id ?? -1}) ${podcast.url}');

    final finder = podcast.id == null
        ? Finder(filter: Filter.equals('guid', podcast.guid))
        : Finder(filter: Filter.byKey(podcast.id));
    final RecordSnapshot<int, Map<String, Object?>>? snapshot =
        await _podcastStore.findFirst(await _db, finder: finder);

    podcast.lastUpdated = DateTime.now();

    if (snapshot == null) {
      podcast.subscribedDate = DateTime.now();
      podcast.id = await _podcastStore.add(await _db, podcast.toMap());
    } else {
      await _podcastStore.update(await _db, podcast.toMap(), finder: finder);
    }

    if (withEpisodes) {
      await _saveEpisodes(podcast.episodes);
    }

    _podcastSubject.add(podcast);

    return podcast;
  }

  @override
  Future<List<Podcast>> subscriptions() async {
    // Custom sort order to ignore title case.
    final titleSortOrder = SortOrder<String>.custom('title', (title1, title2) {
      return title1.toLowerCase().compareTo(title2.toLowerCase());
    });

    final finder = Finder(sortOrders: [
      titleSortOrder,
    ]);

    final List<RecordSnapshot<int, Map<String, Object?>>> subscriptionSnapshot = await _podcastStore.find(
      await _db,
      finder: finder,
    );

    final subs = subscriptionSnapshot.map((snapshot) {
      final subscription = Podcast.fromMap(snapshot.key, snapshot.value);

      return subscription;
    }).toList();

    return subs;
  }

  @override
  Future<void> deletePodcast(Podcast podcast) async {
    final db = await _db;

    await db.transaction((txn) async {
      final podcastFinder = Finder(filter: Filter.byKey(podcast.id));
      final episodeFinder = Finder(filter: Filter.equals('pguid', podcast.guid));

      await _podcastStore.delete(
        txn,
        finder: podcastFinder,
      );

      await _episodeStore.delete(
        txn,
        finder: episodeFinder,
      );
    });
  }

  @override
  Future<Podcast?> findPodcastById(num id) async {
    final finder = Finder(filter: Filter.byKey(id));

    final RecordSnapshot<int, Map<String, Object?>>? snapshot =
        await _podcastStore.findFirst(await _db, finder: finder);

    if (snapshot != null) {
      var p = Podcast.fromMap(snapshot.key, snapshot.value);

      // Now attach all episodes for this podcast
      p.episodes = await findEpisodesByPodcastGuid(
        p.guid,
        filter: p.filter,
        sort: p.sort,
      );

      return p;
    }

    return null;
  }

  @override
  Future<Podcast?> findPodcastByGuid(String guid) async {
    final finder = Finder(filter: Filter.equals('guid', guid));

    final RecordSnapshot<int, Map<String, Object?>>? snapshot =
        await _podcastStore.findFirst(await _db, finder: finder);

    if (snapshot != null) {
      var p = Podcast.fromMap(snapshot.key, snapshot.value);

      // Now attach all episodes for this podcast
      p.episodes = await findEpisodesByPodcastGuid(
        p.guid,
        filter: p.filter,
        sort: p.sort,
      );

      return p;
    }

    return null;
  }

  @override
  Future<List<Episode>> findAllEpisodes() async {
    final finder = Finder(
      sortOrders: [SortOrder('publicationDate', false)],
    );

    final List<RecordSnapshot<int, Map<String, Object?>>> recordSnapshots =
        await _episodeStore.find(await _db, finder: finder);

    final results = recordSnapshots.map((snapshot) {
      final episode = Episode.fromMap(snapshot.key, snapshot.value);

      return episode;
    }).toList();

    return results;
  }

  @override
  Future<Episode?> findEpisodeById(int? id) async {
    final finder = Finder(filter: Filter.byKey(id));
    final RecordSnapshot<int, Map<String, Object?>> snapshot =
        (await _episodeStore.findFirst(await _db, finder: finder))!;

    return await _loadEpisodeSnapshot(snapshot.key, snapshot.value);
  }

  @override
  Future<Episode?> findEpisodeByGuid(String guid) async {
    final finder = Finder(filter: Filter.equals('guid', guid));

    final RecordSnapshot<int, Map<String, Object?>>? snapshot =
        await _episodeStore.findFirst(await _db, finder: finder);

    if (snapshot == null) {
      return null;
    }

    return await _loadEpisodeSnapshot(snapshot.key, snapshot.value);
  }

  // TODO: Remove nullable on pguid as this does not make sense.
  @override
  Future<List<Episode>> findEpisodesByPodcastGuid(
    String? pguid, {
    PodcastEpisodeFilter filter = PodcastEpisodeFilter.none,
    PodcastEpisodeSort sort = PodcastEpisodeSort.none,
  }) async {
    var episodeFilter = Filter.equals('pguid', pguid);
    var sortOrder = SortOrder('publicationDate', false);

    // If we have an additional episode filter and/or sort, apply it.
    episodeFilter = _applyEpisodeFilter(filter, episodeFilter, pguid);
    sortOrder = _applyEpisodeSort(sort, sortOrder);

    final finder = Finder(
      filter: episodeFilter,
      sortOrders: [sortOrder],
    );

    final List<RecordSnapshot<int, Map<String, Object?>>> recordSnapshots =
        await _episodeStore.find(await _db, finder: finder);

    final results = recordSnapshots.map((snapshot) async {
      return await _loadEpisodeSnapshot(snapshot.key, snapshot.value);
    }).toList();

    final episodeList = Future.wait(results);

    return episodeList;
  }

  @override
  Future<List<Episode>> findDownloadsByPodcastGuid(String pguid) async {
    final finder = Finder(
      filter: Filter.and([
        Filter.equals('pguid', pguid),
        Filter.equals('downloadPercentage', '100'),
      ]),
      sortOrders: [SortOrder('publicationDate', false)],
    );

    final List<RecordSnapshot<int, Map<String, Object?>>> recordSnapshots =
        await _episodeStore.find(await _db, finder: finder);

    final results = recordSnapshots.map((snapshot) {
      final episode = Episode.fromMap(snapshot.key, snapshot.value);

      return episode;
    }).toList();

    return results;
  }

  @override
  Future<List<Episode>> findDownloads() async {
    final finder =
        Finder(filter: Filter.equals('downloadPercentage', '100'), sortOrders: [SortOrder('publicationDate', false)]);

    final List<RecordSnapshot<int, Map<String, Object?>>> recordSnapshots =
        await _episodeStore.find(await _db, finder: finder);

    final results = recordSnapshots.map((snapshot) {
      final episode = Episode.fromMap(snapshot.key, snapshot.value);

      return episode;
    }).toList();

    return results;
  }

  @override
  Future<void> deleteEpisode(Episode episode) async {
    final finder = Finder(filter: Filter.byKey(episode.id));

    final RecordSnapshot<int, Map<String, Object?>>? snapshot =
        await _episodeStore.findFirst(await _db, finder: finder);

    if (snapshot == null) {
      // Oops!
    } else {
      await _episodeStore.delete(await _db, finder: finder);
      _episodeSubject.add(EpisodeDeleteState(episode));
    }
  }

  @override
  Future<void> deleteEpisodes(List<Episode> episodes) async {
    var d = await _db;

    if (episodes.isNotEmpty) {
      for (var chunk in episodes.chunk(100)) {
        await d.transaction((txn) async {
          var futures = <Future<int>>[];

          for (var episode in chunk) {
            final finder = Finder(filter: Filter.byKey(episode.id));

            futures.add(_episodeStore.delete(txn, finder: finder));
          }

          if (futures.isNotEmpty) {
            await Future.wait(futures);
          }
        });
      }
    }
  }

  @override
  Future<Episode> saveEpisode(Episode episode, [bool updateIfSame = false]) async {
    var e = await _saveEpisode(episode, updateIfSame);

    _episodeSubject.add(EpisodeUpdateState(e));

    return e;
  }

  @override
  Future<List<Episode>> saveEpisodes(List<Episode> episodes, [bool updateIfSame = false]) async {
    final updatedEpisodes = <Episode>[];

    for (var es in episodes) {
      var e = await _saveEpisode(es, updateIfSame);

      updatedEpisodes.add(e);

      _episodeSubject.add(EpisodeUpdateState(e));
    }

    return updatedEpisodes;
  }

  @override
  Future<List<Episode>> loadQueue() async {
    var episodes = <Episode>[];

    final RecordSnapshot<int, Map<String, Object?>>? snapshot = await _queueStore.record(1).getSnapshot(await _db);

    if (snapshot != null) {
      var queue = Queue.fromMap(snapshot.key, snapshot.value);

      var episodeFinder = Finder(filter: Filter.inList('guid', queue.guids));

      final List<RecordSnapshot<int, Map<String, Object?>>> recordSnapshots =
          await _episodeStore.find(await _db, finder: episodeFinder);

      episodes = recordSnapshots.map((snapshot) {
        final episode = Episode.fromMap(snapshot.key, snapshot.value);

        return episode;
      }).toList();
    }

    return episodes;
  }

  @override
  Future<void> saveQueue(List<Episode> episodes) async {
    /// Check to see if we have any ad-hoc episodes and save them first
    for (var e in episodes) {
      if (e.pguid == null || e.pguid!.isEmpty) {
        _saveEpisode(e, false);
      }
    }

    var guids = episodes.map((e) => e.guid).toList();

    /// Only bother saving if the queue has changed
    if (!listEquals(guids, _queueGuids)) {
      final queue = Queue(guids: guids);

      await _queueStore.record(1).put(await _db, queue.toMap());

      _queueGuids.clear();
      _queueGuids.addAll(guids);
    }
  }

  @override
  Future<Transcript?> findTranscriptById(int? id) async {
    final finder = Finder(filter: Filter.byKey(id));
    final RecordSnapshot<int, Map<String, Object?>>? snapshot =
        await _transcriptStore.findFirst(await _db, finder: finder);

    return snapshot == null ? null : Transcript.fromMap(snapshot.key, snapshot.value);
  }

  @override
  Future<void> deleteTranscriptById(int id) async {
    final finder = Finder(filter: Filter.byKey(id));

    final RecordSnapshot<int, Map<String, Object?>>? snapshot =
        await _transcriptStore.findFirst(await _db, finder: finder);

    if (snapshot == null) {
      // Oops!
    } else {
      await _transcriptStore.delete(await _db, finder: finder);
    }
  }

  @override
  Future<void> deleteTranscriptsById(List<int> id) async {
    var d = await _db;

    if (id.isNotEmpty) {
      for (var chunk in id.chunk(100)) {
        await d.transaction((txn) async {
          var futures = <Future<int>>[];

          for (var id in chunk) {
            final finder = Finder(filter: Filter.byKey(id));

            futures.add(_transcriptStore.delete(txn, finder: finder));
          }

          if (futures.isNotEmpty) {
            await Future.wait(futures);
          }
        });
      }
    }
  }

  @override
  Future<Transcript> saveTranscript(Transcript transcript) async {
    final finder = Finder(filter: Filter.byKey(transcript.id));

    final RecordSnapshot<int, Map<String, Object?>>? snapshot =
        await _transcriptStore.findFirst(await _db, finder: finder);

    transcript.lastUpdated = DateTime.now();

    if (snapshot == null) {
      transcript.id = await _transcriptStore.add(await _db, transcript.toMap());
    } else {
      await _transcriptStore.update(await _db, transcript.toMap(), finder: finder);
    }

    return transcript;
  }

  Future<void> _cleanupEpisodes() async {
    final threshold = DateTime.now().subtract(const Duration(days: 60)).millisecondsSinceEpoch;

    /// Find all streamed episodes over the threshold.
    final filter = Filter.and([
      Filter.equals('downloadState', 0),
      Filter.lessThan('lastUpdated', threshold),
    ]);

    final orphaned = <Episode>[];
    final pguids = <String?>[];
    final List<RecordSnapshot<int, Map<String, Object?>>> episodes =
        await _episodeStore.find(await _db, finder: Finder(filter: filter));

    // First, find all podcasts
    for (var podcast in await _podcastStore.find(await _db)) {
      pguids.add(podcast.value['guid'] as String?);
    }

    for (var episode in episodes) {
      final pguid = episode.value['pguid'] as String?;
      final podcast = pguids.contains(pguid);

      if (!podcast) {
        orphaned.add(Episode.fromMap(episode.key, episode.value));
      }
    }

    await deleteEpisodes(orphaned);
  }

  SortOrder<Object?> _applyEpisodeSort(PodcastEpisodeSort sort, SortOrder<Object?> sortOrder) {
    switch (sort) {
      case PodcastEpisodeSort.none:
      case PodcastEpisodeSort.latestFirst:
        sortOrder = SortOrder('publicationDate', false);
        break;
      case PodcastEpisodeSort.earliestFirst:
        sortOrder = SortOrder('publicationDate', true);
        break;
      case PodcastEpisodeSort.alphabeticalDescending:
        sortOrder = SortOrder<String>.custom('title', (title1, title2) {
          return title2.toLowerCase().compareTo(title1.toLowerCase());
        });
        break;
      case PodcastEpisodeSort.alphabeticalAscending:
        sortOrder = SortOrder<String>.custom('title', (title1, title2) {
          return title1.toLowerCase().compareTo(title2.toLowerCase());
        });
        break;
    }
    return sortOrder;
  }

  Filter _applyEpisodeFilter(PodcastEpisodeFilter filter, Filter episodeFilter, String? pguid) {
    // If we have an additional episode filter, apply it.
    switch (filter) {
      case PodcastEpisodeFilter.none:
        episodeFilter = Filter.equals('pguid', pguid);
        break;
      case PodcastEpisodeFilter.started:
        episodeFilter = Filter.and([Filter.equals('pguid', pguid), Filter.notEquals('position', '0')]);
        break;
      case PodcastEpisodeFilter.played:
        episodeFilter = Filter.and([Filter.equals('pguid', pguid), Filter.equals('played', 'true')]);
        break;
      case PodcastEpisodeFilter.notPlayed:
        episodeFilter = Filter.and([Filter.equals('pguid', pguid), Filter.equals('played', 'false')]);
        break;
    }
    return episodeFilter;
  }

  /// Saves a list of episodes to the repository. To improve performance we
  /// split the episodes into chunks of 100 and save any that have been updated
  /// in that chunk in a single transaction.
  Future<void> _saveEpisodes(List<Episode?>? episodes) async {
    var d = await _db;
    var dateStamp = DateTime.now();

    if (episodes != null && episodes.isNotEmpty) {
      for (var chunk in episodes.chunk(100)) {
        await d.transaction((txn) async {
          var futures = <Future<int>>[];

          for (var episode in chunk) {
            episode!.lastUpdated = dateStamp;

            if (episode.id == null) {
              futures.add(_episodeStore.add(txn, episode.toMap()).then((id) => episode.id = id));
            } else {
              final finder = Finder(filter: Filter.byKey(episode.id));

              var existingEpisode = await findEpisodeById(episode.id);

              if (existingEpisode == null || existingEpisode != episode) {
                futures.add(_episodeStore.update(txn, episode.toMap(), finder: finder));
              }
            }
          }

          if (futures.isNotEmpty) {
            await Future.wait(futures);
          }
        });
      }
    }
  }

  Future<Episode> _saveEpisode(Episode episode, bool updateIfSame) async {
    final finder = Finder(filter: Filter.byKey(episode.id));

    final RecordSnapshot<int, Map<String, Object?>>? snapshot =
        await _episodeStore.findFirst(await _db, finder: finder);

    if (snapshot == null) {
      episode.lastUpdated = DateTime.now();
      episode.id = await _episodeStore.add(await _db, episode.toMap());
    } else {
      var e = Episode.fromMap(episode.id, snapshot.value);
      episode.lastUpdated = DateTime.now();

      if (updateIfSame || episode != e) {
        await _episodeStore.update(await _db, episode.toMap(), finder: finder);
      }
    }

    return episode;
  }

  @override
  Future<Episode?> findEpisodeByTaskId(String taskId) async {
    final finder = Finder(filter: Filter.equals('downloadTaskId', taskId));
    final RecordSnapshot<int, Map<String, Object?>>? snapshot =
        await _episodeStore.findFirst(await _db, finder: finder);

    if (snapshot != null) {
      return await _loadEpisodeSnapshot(snapshot.key, snapshot.value);
    } else {
      return null;
    }
  }

  Future<Episode> _loadEpisodeSnapshot(int key, Map<String, Object?> snapshot) async {
    var episode = Episode.fromMap(key, snapshot);

    if (episode.transcriptId! > 0) {
      episode.transcript = await findTranscriptById(episode.transcriptId);
    }

    return episode;
  }

  @override
  Future<void> close() async {
    final d = await _db;

    await d.close();
  }

  Future<void> dbUpgrader(Database db, int oldVersion, int newVersion) async {
    if (oldVersion == 1) {
      await _upgradeV2(db);
    }
  }

  /// In v1 we allowed http requests, where as now we force to https. As we currently use the
  /// URL as the GUID we need to upgrade any followed podcasts that have a http base to https.
  /// We use the passed [Database] rather than _db to prevent deadlocking, hence the direct
  /// update to data within this routine rather than using the existing find/update methods.
  Future<void> _upgradeV2(Database db) async {
    List<RecordSnapshot<int, Map<String, Object?>>> data = await _podcastStore.find(db);
    final podcasts = data.map((e) => Podcast.fromMap(e.key, e.value)).toList();

    log.info('Upgrading Sembast store to V2');

    for (var podcast in podcasts) {
      if (podcast.guid!.startsWith('http:')) {
        final idFinder = Finder(filter: Filter.byKey(podcast.id));
        final guid = podcast.guid!.replaceFirst('http:', 'https:');
        final episodeFinder = Finder(
          filter: Filter.equals('pguid', podcast.guid),
        );

        log.fine('Upgrading GUID ${podcast.guid} - to $guid');

        var upgradedPodcast = Podcast(
          id: podcast.id,
          guid: guid,
          url: podcast.url,
          link: podcast.link,
          title: podcast.title,
          description: podcast.description,
          imageUrl: podcast.imageUrl,
          thumbImageUrl: podcast.thumbImageUrl,
          copyright: podcast.copyright,
          funding: podcast.funding,
          persons: podcast.persons,
          lastUpdated: DateTime.now(),
        );

        final List<RecordSnapshot<int, Map<String, Object?>>> episodeData =
            await _episodeStore.find(db, finder: episodeFinder);
        final episodes = episodeData.map((e) => Episode.fromMap(e.key, e.value)).toList();

        // Now upgrade episodes
        for (var e in episodes) {
          e.pguid = guid;
          log.fine('Updating episode guid for ${e.title} from ${e.pguid} to $guid');

          final epf = Finder(filter: Filter.byKey(e.id));
          await _episodeStore.update(db, e.toMap(), finder: epf);
        }

        upgradedPodcast.episodes = episodes;
        await _podcastStore.update(db, upgradedPodcast.toMap(), finder: idFinder);
      }
    }
  }

  @override
  Stream<EpisodeState> get episodeListener => _episodeSubject.stream;

  @override
  Stream<Podcast> get podcastListener => _podcastSubject.stream;
}
