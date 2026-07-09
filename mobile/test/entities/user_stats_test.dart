import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/entities/user_stats.dart';

UserStats _stats({
  int timeListened = 0,
  String userCreated = '2024-01-15T00:00:00.000Z',
  String podSyncType = 'none',
  String gpodderUrl = '',
}) {
  return UserStats(
    userCreated: userCreated,
    podcastsPlayed: 0,
    timeListened: timeListened,
    podcastsAdded: 0,
    episodesSaved: 0,
    episodesDownloaded: 0,
    gpodderUrl: gpodderUrl,
    podSyncType: podSyncType,
  );
}

void main() {
  group('fromJson/toJson', () {
    test('round-trips every field', () {
      final json = {
        'UserCreated': '2024-01-15T00:00:00.000Z',
        'PodcastsPlayed': 5,
        'TimeListened': 120,
        'PodcastsAdded': 3,
        'EpisodesSaved': 7,
        'EpisodesDownloaded': 2,
        'GpodderUrl': 'http://localhost:8042',
        'Pod_Sync_Type': 'gpodder',
      };

      final stats = UserStats.fromJson(json);

      expect(stats.podcastsPlayed, 5);
      expect(stats.timeListened, 120);
      expect(stats.podcastsAdded, 3);
      expect(stats.episodesSaved, 7);
      expect(stats.episodesDownloaded, 2);
      expect(stats.gpodderUrl, 'http://localhost:8042');
      expect(stats.podSyncType, 'gpodder');
      expect(stats.toJson(), json);
    });

    test('defaults missing fields to empty/zero rather than throwing', () {
      final stats = UserStats.fromJson(const {});

      expect(stats.userCreated, '');
      expect(stats.podcastsPlayed, 0);
      expect(stats.gpodderUrl, '');
    });
  });

  group('formattedTimeListened', () {
    test('is "0 minutes" when nothing has been listened to', () {
      expect(_stats(timeListened: 0).formattedTimeListened, '0 minutes');
    });

    test('pluralizes a single minute correctly', () {
      expect(_stats(timeListened: 1).formattedTimeListened, '1 minute');
    });

    test('shows minutes only under an hour', () {
      expect(_stats(timeListened: 45).formattedTimeListened, '45 minutes');
    });

    test('shows only hours when there are no leftover minutes', () {
      expect(_stats(timeListened: 120).formattedTimeListened, '2 hours');
    });

    test('shows a singular hour correctly', () {
      expect(_stats(timeListened: 60).formattedTimeListened, '1 hour');
    });

    test('shows hours and minutes together', () {
      expect(_stats(timeListened: 125).formattedTimeListened, '2 hours 5 minutes');
    });
  });

  group('formattedUserCreated', () {
    test('formats a valid ISO date as D/M/YYYY', () {
      expect(_stats(userCreated: '2024-03-05T00:00:00.000Z').formattedUserCreated, '5/3/2024');
    });

    test('returns the raw string unchanged if it cannot be parsed', () {
      expect(_stats(userCreated: 'not a date').formattedUserCreated, 'not a date');
    });
  });

  group('syncStatusDescription', () {
    test('describes "none" as Not Syncing', () {
      expect(_stats(podSyncType: 'none').syncStatusDescription, 'Not Syncing');
    });

    test('describes the internal gpodder server distinctly from an external one', () {
      expect(
        _stats(podSyncType: 'gpodder', gpodderUrl: 'http://localhost:8042').syncStatusDescription,
        'Internal gpodder',
      );
      expect(
        _stats(podSyncType: 'gpodder', gpodderUrl: 'https://gpodder.example.com').syncStatusDescription,
        'External gpodder',
      );
    });

    test('describes nextcloud sync', () {
      expect(_stats(podSyncType: 'nextcloud').syncStatusDescription, 'Nextcloud');
    });

    test('is case-insensitive when matching the sync type', () {
      expect(_stats(podSyncType: 'NONE').syncStatusDescription, 'Not Syncing');
    });

    test('falls back to Unknown for an unrecognized sync type', () {
      expect(_stats(podSyncType: 'something-else').syncStatusDescription, 'Unknown sync type');
    });
  });
}
