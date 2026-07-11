import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/entities/home_data.dart';

HomeEpisode _homeEpisode({
  int episodeDuration = 600,
  int? listenDuration,
}) {
  return HomeEpisode(
    episodeId: 1,
    podcastId: 1,
    episodeTitle: 'Some Episode',
    episodeDescription: 'Description',
    episodeUrl: 'https://example.com/episode.mp3',
    episodeArtwork: 'https://example.com/art.png',
    episodePubDate: '2024-01-15T00:00:00Z',
    episodeDuration: episodeDuration,
    completed: false,
    podcastName: 'Some Podcast',
    isYoutube: false,
    listenDuration: listenDuration,
  );
}

void main() {
  group('HomePodcast.fromJson', () {
    test('parses categories given as the old string format', () {
      final podcast = HomePodcast.fromJson({
        'podcastid': 1,
        'podcastname': 'Some Podcast',
        'is_youtube': false,
        'play_count': 0,
        'categories': 'Comedy, News',
      });

      expect(podcast.categories, 'Comedy, News');
    });

    test('parses categories given as the newer map format into a comma-separated string', () {
      final podcast = HomePodcast.fromJson({
        'podcastid': 1,
        'podcastname': 'Some Podcast',
        'is_youtube': false,
        'play_count': 0,
        'categories': {'0': 'Comedy', '1': 'News'},
      });

      expect(podcast.categories, 'Comedy, News');
    });

    test('an empty categories map becomes null rather than an empty string', () {
      final podcast = HomePodcast.fromJson({
        'podcastid': 1,
        'podcastname': 'Some Podcast',
        'is_youtube': false,
        'play_count': 0,
        'categories': <String, dynamic>{},
      });

      expect(podcast.categories, isNull);
    });

    test('categories is null when the key is absent', () {
      final podcast = HomePodcast.fromJson({
        'podcastid': 1,
        'podcastname': 'Some Podcast',
        'is_youtube': false,
        'play_count': 0,
      });

      expect(podcast.categories, isNull);
    });

    test('defaults missing required-ish fields rather than throwing', () {
      final podcast = HomePodcast.fromJson(const {});

      expect(podcast.podcastId, 0);
      expect(podcast.podcastName, '');
      expect(podcast.isYoutube, isFalse);
      expect(podcast.playCount, 0);
    });
  });

  group('HomeEpisode.fromJson', () {
    test('reads every field', () {
      final episode = HomeEpisode.fromJson({
        'episodeid': 5,
        'podcastid': 1,
        'episodetitle': 'Title',
        'episodedescription': 'Description',
        'episodeurl': 'https://example.com/episode.mp3',
        'episodeartwork': 'https://example.com/art.png',
        'episodepubdate': '2024-01-15T00:00:00Z',
        'episodeduration': 600,
        'completed': true,
        'podcastname': 'Some Podcast',
        'is_youtube': false,
        'listenduration': 120,
        'saved': true,
        'queued': true,
        'downloaded': true,
      });

      expect(episode.episodeId, 5);
      expect(episode.completed, isTrue);
      expect(episode.saved, isTrue);
      expect(episode.queued, isTrue);
      expect(episode.downloaded, isTrue);
      expect(episode.listenDuration, 120);
    });
  });

  group('HomeEpisode.formattedDuration', () {
    test('is placeholder dashes when duration is zero', () {
      expect(_homeEpisode(episodeDuration: 0).formattedDuration, '--:--');
    });

    test('formats under an hour as MM:SS', () {
      expect(_homeEpisode(episodeDuration: 125).formattedDuration, '02:05');
    });

    test('formats an hour or more as HH:MM:SS', () {
      expect(_homeEpisode(episodeDuration: 3725).formattedDuration, '01:02:05');
    });
  });

  group('HomeEpisode.formattedListenDuration', () {
    test('is null with no listen duration', () {
      expect(_homeEpisode(listenDuration: null).formattedListenDuration, isNull);
    });

    test('formats a listen duration under an hour', () {
      expect(_homeEpisode(listenDuration: 65).formattedListenDuration, '01:05');
    });
  });

  group('HomeEpisode.progressPercentage', () {
    test('is 0 with no listen duration', () {
      expect(_homeEpisode(episodeDuration: 600, listenDuration: null).progressPercentage, 0.0);
    });

    test('computes the percentage listened', () {
      expect(_homeEpisode(episodeDuration: 200, listenDuration: 50).progressPercentage, 25.0);
    });
  });

  group('WeeklyStats', () {
    test('fromJson defaults missing fields to zero', () {
      final stats = WeeklyStats.fromJson(const {});
      expect(stats.secondsListened, 0);
      expect(stats.episodesCompleted, 0);
    });

    test('hasActivity is false with nothing listened or completed', () {
      expect(WeeklyStats().hasActivity, isFalse);
    });

    test('hasActivity is true with either seconds listened or episodes completed', () {
      expect(WeeklyStats(secondsListened: 1).hasActivity, isTrue);
      expect(WeeklyStats(episodesCompleted: 1).hasActivity, isTrue);
    });

    test('formattedListened shows minutes only under an hour', () {
      expect(WeeklyStats(secondsListened: 29 * 60).formattedListened, '29m');
    });

    test('formattedListened shows hours and minutes once over an hour', () {
      expect(WeeklyStats(secondsListened: 65 * 60).formattedListened, '1h 5m');
    });
  });

  group('HomeOverview.fromJson', () {
    test('parses nested episode and podcast lists, defaulting missing ones to empty', () {
      final overview = HomeOverview.fromJson({
        'recent_episodes': [
          {'episodeid': 1, 'podcastid': 1, 'episodetitle': 'A', 'podcastname': 'P', 'episodepubdate': '', 'episodeduration': 0},
        ],
        'saved_count': 3,
        'downloaded_count': 2,
        'queue_count': 1,
      });

      expect(overview.recentEpisodes, hasLength(1));
      expect(overview.inProgressEpisodes, isEmpty);
      expect(overview.queuePreview, isEmpty);
      expect(overview.topPodcasts, isEmpty);
      expect(overview.savedCount, 3);
      expect(overview.downloadedCount, 2);
      expect(overview.queueCount, 1);
      expect(overview.weeklyStats.hasActivity, isFalse);
    });
  });

  group('Playlist/PlaylistResponse.fromJson', () {
    test('Playlist.fromJson reads fields and defaults iconName', () {
      final playlist = Playlist.fromJson({'playlist_id': 1, 'name': 'My Playlist'});

      expect(playlist.playlistId, 1);
      expect(playlist.name, 'My Playlist');
      expect(playlist.iconName, 'ph-music-notes');
    });

    test('PlaylistResponse.fromJson parses a list of playlists', () {
      final response = PlaylistResponse.fromJson({
        'playlists': [
          {'playlist_id': 1, 'name': 'A'},
          {'playlist_id': 2, 'name': 'B'},
        ],
      });

      expect(response.playlists, hasLength(2));
      expect(response.playlists[1].name, 'B');
    });

    test('PlaylistResponse.fromJson defaults to an empty list when absent', () {
      final response = PlaylistResponse.fromJson(const {});
      expect(response.playlists, isEmpty);
    });
  });
}
