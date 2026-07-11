// Exercises the real SembastRepository persistence contracts against an
// in-memory sembast database (no disk, no platform channels). This is the layer
// most of the app's data flows through and was previously untested.

import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/entities/downloadable.dart';
import 'package:pinepods_mobile/entities/pending_action.dart';
import 'package:pinepods_mobile/repository/sembast/sembast_repository.dart';

import '../support/factories.dart';
import '../support/in_memory_repository.dart';

void main() {
  late SembastRepository repo;

  setUp(() {
    repo = newInMemoryRepository();
  });

  tearDown(() async {
    await repo.close();
  });

  group('podcasts', () {
    test('savePodcast assigns an id and a subscribed date, and is findable by guid', () async {
      final saved = await repo.savePodcast(buildPodcast(guid: 'p1', title: 'My Show'));

      expect(saved.id, isNotNull);
      expect(saved.subscribedDate, isNotNull);

      final found = await repo.findPodcastByGuid('p1');
      expect(found, isNotNull);
      expect(found!.id, saved.id);
      expect(found.title, 'My Show');
    });

    test('findPodcastById returns the stored podcast', () async {
      final saved = await repo.savePodcast(buildPodcast(guid: 'p1'));
      final found = await repo.findPodcastById(saved.id!);
      expect(found, isNotNull);
      expect(found!.guid, 'p1');
    });

    test('re-saving the same guid updates in place rather than duplicating', () async {
      await repo.savePodcast(buildPodcast(guid: 'p1', title: 'Original'));
      // Same guid, no id -> savePodcast matches on guid and updates.
      await repo.savePodcast(buildPodcast(guid: 'p1', title: 'Renamed'));

      final subs = await repo.subscriptions();
      expect(subs, hasLength(1));
      expect(subs.single.title, 'Renamed');
    });

    test('subscriptions lists saved podcasts sorted case-insensitively by title', () async {
      await repo.savePodcast(buildPodcast(guid: 'b', title: 'banana'));
      await repo.savePodcast(buildPodcast(guid: 'a', title: 'Apple'));

      final subs = await repo.subscriptions();
      expect(subs.map((p) => p.title), ['Apple', 'banana']);
    });

    test('deletePodcast removes it from subscriptions', () async {
      final saved = await repo.savePodcast(buildPodcast(guid: 'p1'));
      await repo.deletePodcast(saved);

      expect(await repo.subscriptions(), isEmpty);
      expect(await repo.findPodcastByGuid('p1'), isNull);
    });
  });

  group('episodes', () {
    test('saveEpisode round-trips and is findable by guid and id', () async {
      final saved = await repo.saveEpisode(buildEpisode(guid: 'e1', title: 'Ep 1'));

      expect(saved.id, isNotNull);

      final byGuid = await repo.findEpisodeByGuid('e1');
      expect(byGuid, isNotNull);
      expect(byGuid!.title, 'Ep 1');

      final byId = await repo.findEpisodeById(saved.id!);
      expect(byId!.guid, 'e1');
    });

    test('findDownloads returns only fully-downloaded (100%) episodes', () async {
      await repo.saveEpisode(buildEpisode(
        guid: 'e1',
        downloadState: DownloadState.downloaded,
        downloadPercentage: 100,
      ));
      await repo.saveEpisode(buildEpisode(guid: 'e2', downloadPercentage: 0));

      final downloads = await repo.findDownloads();
      expect(downloads.map((e) => e.guid), ['e1']);
    });

    test('deleteEpisode removes it', () async {
      final saved = await repo.saveEpisode(buildEpisode(guid: 'e1'));
      await repo.deleteEpisode(saved);
      expect(await repo.findEpisodeByGuid('e1'), isNull);
    });
  });

  group('queue', () {
    test('saveQueue then loadQueue round-trips already-saved episodes', () async {
      // Regular (non-ad-hoc) episodes are persisted independently, then queued.
      final a = await repo.saveEpisode(buildEpisode(guid: 'q1'));
      final b = await repo.saveEpisode(buildEpisode(guid: 'q2'));

      await repo.saveQueue([a, b]);

      final loaded = await repo.loadQueue();
      expect(loaded.map((e) => e.guid), containsAll(['q1', 'q2']));
    });

    test('saveQueue persists ad-hoc (empty-pguid) episodes so loadQueue finds them', () async {
      // Ad-hoc episodes are saved by saveQueue itself. This used to be
      // fire-and-forget, so the queue record could reference episodes that
      // weren't persisted yet; loadQueue must now find them.
      final a = buildEpisode(guid: 'adhoc1', pguid: '');
      final b = buildEpisode(guid: 'adhoc2', pguid: '');

      await repo.saveQueue([a, b]);

      final loaded = await repo.loadQueue();
      expect(loaded.map((e) => e.guid), containsAll(['adhoc1', 'adhoc2']));
    });

    test('loadQueue is empty when nothing has been queued', () async {
      expect(await repo.loadQueue(), isEmpty);
    });
  });

  group('pending actions (offline queue)', () {
    test('savePendingAction assigns an id', () async {
      final saved = await repo.savePendingAction(
        PendingAction(type: PendingActionType.markCompleted, episodeId: 1, userId: 1),
      );
      expect(saved.id, isNotNull);
    });

    test('getPendingActions returns them oldest-first', () async {
      await repo.savePendingAction(PendingAction(
        type: PendingActionType.saveEpisode,
        episodeId: 1,
        userId: 1,
        createdAt: DateTime(2024, 1, 1),
      ));
      await repo.savePendingAction(PendingAction(
        type: PendingActionType.queue,
        episodeId: 2,
        userId: 1,
        createdAt: DateTime(2024, 1, 2),
      ));

      final actions = await repo.getPendingActions();
      expect(actions.map((a) => a.episodeId), [1, 2]);
    });

    test('deletePendingAction removes the given action', () async {
      final saved = await repo.savePendingAction(
        PendingAction(type: PendingActionType.markCompleted, episodeId: 9, userId: 1),
      );

      await repo.deletePendingAction(saved.id!);

      expect(await repo.getPendingActions(), isEmpty);
    });
  });

  test('separate in-memory repositories do not share data', () async {
    await repo.savePodcast(buildPodcast(guid: 'p1'));

    final other = newInMemoryRepository();
    addTearDown(other.close);

    expect(await other.subscriptions(), isEmpty);
  });
}
