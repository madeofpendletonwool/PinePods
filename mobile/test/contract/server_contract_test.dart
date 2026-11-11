// The app's API client against a real server: catches the app and the server disagreeing
// about a route or a field, which tests with canned responses can't see.
//
// rust-api's integration suite boots the server, seeds it and runs this file:
//   rust-api/scripts/integration-tests.sh --db postgres -- -- --ignored mobile_contract
// Without its environment variables every test here is skipped.

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/ui/pinepods/create_playlist.dart';

final _env = Platform.environment;
final _server = _env['PINEPODS_CONTRACT_SERVER'];
final _username = _env['PINEPODS_CONTRACT_USERNAME'] ?? '';
final _password = _env['PINEPODS_CONTRACT_PASSWORD'] ?? '';
final _userId = int.tryParse(_env['PINEPODS_CONTRACT_USER_ID'] ?? '') ?? 0;
final _podcastId = int.tryParse(_env['PINEPODS_CONTRACT_PODCAST_ID'] ?? '') ?? 0;
final _showTitle = _env['PINEPODS_CONTRACT_SHOW_TITLE'] ?? '';

void main() {
  final skip = _server == null ? 'needs a server from rust-api\'s integration suite' : null;
  final service = PinepodsService(timeout: const Duration(seconds: 30));
  late List<int> episodes;

  setUpAll(() async {
    if (_server == null) return;
    expect(await service.login(_server!, _username, _password), isTrue);
    final list = await service.getPodcastEpisodes(_userId, _podcastId);
    episodes = list.map((e) => e.episodeId).toList();
  });

  group('signing in', () {
    test('the server is recognised', () async {
      expect(await service.verifyPinepodsInstance(_server!), isTrue);
    });

    test('a wrong password is refused', () async {
      final other = PinepodsService();
      expect(await other.login(_server!, _username, 'wrong-password'), isFalse);
    });

    test('the key identifies the user', () async {
      expect(await service.verifyApiKey(), isTrue);
      expect(await service.getUserIdFromApiKey(), _userId);
    });

    test('the server reports a version', () async {
      expect(await service.getPinepodsVersion(), isNotEmpty);
    });
  }, skip: skip);

  group('library', () {
    test('subscriptions list the show', () async {
      final podcasts = await service.getUserPodcasts(_userId);
      expect(podcasts.map((p) => p.title), contains(_showTitle));
    });

    test('episodes have everything the player needs', () async {
      final list = await service.getPodcastEpisodes(_userId, _podcastId);
      expect(list, hasLength(3));
      for (final episode in list) {
        expect(episode.episodeId, greaterThan(0));
        expect(episode.episodeTitle, startsWith('Episode '));
        expect(episode.episodeUrl, startsWith('http'));
        expect(episode.episodePubDate, isNotEmpty);
        expect(episode.episodeDuration, greaterThan(0));
        expect(episode.podcastName, _showTitle);
      }
    });

    test('the feed pages with a total', () async {
      final page = await service.getRecentEpisodes(_userId, limit: 2);
      expect(page.total, 3);
      expect(page.episodes, hasLength(2));
    });

    test('a podcast can be favorited', () async {
      expect(await service.togglePodcastFavorite(_podcastId, _userId, true), isTrue);
      expect(await service.getPodcastFavoriteStatus(_podcastId, _userId), isTrue);
      expect(await service.togglePodcastFavorite(_podcastId, _userId, false), isTrue);
      expect(await service.getPodcastFavoriteStatus(_podcastId, _userId), isFalse);
    });

    test('the home page loads', () async {
      await service.getHomeOverview(_userId);
    });

    test('stats load', () async {
      await service.getUserStats(_userId);
    });
  }, skip: skip);

  group('queue', () {
    test('episodes can be queued, reordered and removed', () async {
      for (final id in episodes.take(2)) {
        expect(await service.queueEpisode(id, _userId, false), isTrue);
      }
      var queue = await service.getQueuedEpisodes(_userId);
      expect(queue.map((e) => e.episodeId), containsAll(episodes.take(2)));
      expect(queue.every((e) => e.queued), isTrue);

      final reversed = queue.reversed.map((e) => (episodeId: e.episodeId, isYoutube: e.isYoutube)).toList();
      expect(await service.reorderQueue(_userId, reversed), isTrue);
      queue = await service.getQueuedEpisodes(_userId);
      expect(queue.map((e) => e.episodeId).toList(), reversed.map((e) => e.episodeId).toList());

      for (final id in episodes.take(2)) {
        expect(await service.removeQueuedEpisode(id, _userId, false), isTrue);
      }
      expect(await service.getQueuedEpisodes(_userId), isEmpty);
    });
  }, skip: skip);

  group('saved', () {
    test('an episode can be saved and removed', () async {
      expect(await service.saveEpisode(episodes.first, _userId, false), isTrue);
      final saved = await service.getSavedEpisodes(_userId);
      expect(saved.map((e) => e.episodeId), contains(episodes.first));
      expect(saved.firstWhere((e) => e.episodeId == episodes.first).saved, isTrue);

      final page = await service.getSavedEpisodesPaged(_userId);
      expect(page.total, 1);

      expect(await service.removeSavedEpisode(episodes.first, _userId, false), isTrue);
      expect(await service.getSavedEpisodes(_userId), isEmpty);
    });
  }, skip: skip);

  group('playback', () {
    test('progress is stored and shows up in history', () async {
      final id = episodes[1];
      expect(await service.recordListenDuration(id, _userId, 120, false), isTrue);

      final episode = await service.getEpisodeMetadata(id, _userId);
      expect(episode, isNotNull);
      expect(episode!.listenDuration, 120);

      final history = await service.getUserHistory(_userId);
      expect(history.map((e) => e.episodeId), contains(id));
      final page = await service.getUserHistoryPaged(_userId);
      expect(page.episodes.map((e) => e.episodeId), contains(id));
    });

    test('an episode can be marked finished and back', () async {
      final id = episodes[2];
      expect(await service.markEpisodeCompleted(id, _userId, false), isTrue);
      expect((await service.getEpisodeMetadata(id, _userId))!.completed, isTrue);
      expect(await service.markEpisodeUncompleted(id, _userId, false), isTrue);
      expect((await service.getEpisodeMetadata(id, _userId))!.completed, isFalse);
    });

    test('play settings and skip times load', () async {
      final details = await service.getPlayEpisodeDetails(_userId, _podcastId, false);
      expect(details.playbackSpeed, greaterThan(0));
      expect(await service.getEpisodeSkipSegments(episodes.first, _userId), isEmpty);
    });

    test('the podcast knows which episode comes next', () async {
      expect(await service.getAutoPlayNextStatus(_podcastId, _userId), isFalse);
      await service.getNextPodcastEpisode(episodes.first, _userId);
    });
  }, skip: skip);

  group('server downloads', () {
    test('an episode can be downloaded to the server and removed', () async {
      final id = episodes.first;
      expect(await service.downloadEpisode(id, _userId, false), isTrue);

      final deadline = DateTime.now().add(const Duration(seconds: 20));
      var found = false;
      while (!found && DateTime.now().isBefore(deadline)) {
        found = (await service.getServerDownloads(_userId)).any((e) => e.episodeId == id);
        if (!found) await Future<void>.delayed(const Duration(milliseconds: 200));
      }
      expect(found, isTrue, reason: 'the download should appear');

      expect(await service.deleteEpisode(id, _userId, false), isTrue);
      expect(await service.getServerDownloads(_userId), isEmpty);
    });
  }, skip: skip);

  group('playlists', () {
    test('the default playlists are listed and one can be created and deleted', () async {
      final before = await service.getUserPlaylists(_userId);
      expect(before.map((p) => p.name), contains('Fresh Releases'));

      CreatePlaylistRequest request(String name, String sortOrder) => CreatePlaylistRequest(
            userId: _userId,
            name: name,
            description: 'made by a test',
            includeUnplayed: true,
            includePartiallyPlayed: true,
            includePlayed: true,
            sortOrder: sortOrder,
            groupByPodcast: false,
            iconName: 'ph-playlist',
          );

      // Every choice the create-playlist screen offers has to be one the server takes.
      for (final sortOrder in playlistSortOptions) {
        expect(await service.createPlaylist(request('Contract $sortOrder', sortOrder)), isTrue, reason: sortOrder);
      }

      final after = await service.getUserPlaylists(_userId);
      for (final mine in after.where((p) => p.name.startsWith('Contract '))) {
        expect(mine.isSystemPlaylist, isFalse);
        expect(await service.deletePlaylist(_userId, mine.playlistId), isTrue);
      }
      final gone = await service.getUserPlaylists(_userId);
      expect(gone.where((p) => p.name.startsWith('Contract ')), isEmpty);
    });
  }, skip: skip);

  group('settings', () {
    test('the theme round-trips', () async {
      expect(await service.setUserTheme(_userId, 'Nordic'), isTrue);
      expect(await service.getUserTheme(_userId), 'Nordic');
    });

    test('AI status reads as off without a sidecar', () async {
      final status = await service.getAiStatus();
      expect(status.available, isFalse);
      expect(status.downloadsEnabled, isTrue);
    });

    test('a share link can be created', () async {
      expect(await service.createShareLink(episodes.first), isNotEmpty);
    });

    test('episode search finds the show', () async {
      final page = await service.searchEpisodes(_userId, 'Episode');
      expect(page.results, isNotEmpty);
    });
  }, skip: skip);
}
