import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/entities/podcast.dart';

void main() {
  group('construction', () {
    test('upgrades http feed/artwork URLs to https', () {
      final podcast = Podcast(
        guid: 'guid-1',
        url: 'http://example.com/feed.xml',
        link: 'https://example.com',
        title: 'Some Podcast',
        imageUrl: 'http://example.com/art.png',
      );

      expect(podcast.url, 'https://example.com/feed.xml');
      expect(podcast.imageUrl, 'https://example.com/art.png');
    });
  });

  group('subscribed', () {
    test('is false before the podcast has a database id', () {
      final podcast = Podcast(guid: 'g', url: 'https://example.com/feed.xml', link: null, title: 'T');
      expect(podcast.subscribed, isFalse);
    });

    test('is true once a database id is assigned', () {
      final podcast = Podcast(guid: 'g', url: 'https://example.com/feed.xml', link: null, title: 'T')..id = 5;
      expect(podcast.subscribed, isTrue);
    });
  });

  group('lastUpdated', () {
    test('defaults to the epoch-adjacent 1970-01-01 when never set', () {
      final podcast = Podcast(guid: 'g', url: 'https://example.com/feed.xml', link: null, title: 'T');
      expect(podcast.lastUpdated, DateTime(1970, 1, 1));
    });

    test('returns whatever was set via the setter', () {
      final podcast = Podcast(guid: 'g', url: 'https://example.com/feed.xml', link: null, title: 'T');
      final when = DateTime(2024, 1, 1);

      podcast.lastUpdated = when;

      expect(podcast.lastUpdated, when);
    });
  });

  group('toMap/fromMap', () {
    test('round-trips guid, title, url, and filter/sort selections', () {
      final original = Podcast(
        guid: 'guid-1',
        url: 'https://example.com/feed.xml',
        link: 'https://example.com',
        title: 'Some Podcast',
        description: 'A podcast',
        copyright: '2024',
        filter: PodcastEpisodeFilter.notPlayed,
        sort: PodcastEpisodeSort.alphabeticalDescending,
        lastUpdated: DateTime(2024, 3, 1),
      );

      final restored = Podcast.fromMap(7, original.toMap());

      expect(restored.id, 7);
      expect(restored.guid, original.guid);
      expect(restored.title, original.title);
      expect(restored.url, original.url);
      expect(restored.description, original.description);
      expect(restored.copyright, original.copyright);
      expect(restored.filter, PodcastEpisodeFilter.notPlayed);
      expect(restored.sort, PodcastEpisodeSort.alphabeticalDescending);
      expect(restored.lastUpdated, DateTime(2024, 3, 1));
    });

    test('every filter enum value round-trips through its stored id', () {
      for (final filter in PodcastEpisodeFilter.values) {
        final original = Podcast(guid: 'g', url: 'https://example.com/feed.xml', link: null, title: 'T', filter: filter);
        final restored = Podcast.fromMap(1, original.toMap());

        expect(restored.filter, filter, reason: 'failed for $filter');
      }
    });

    test('every sort enum value round-trips through its stored id', () {
      for (final sort in PodcastEpisodeSort.values) {
        final original = Podcast(guid: 'g', url: 'https://example.com/feed.xml', link: null, title: 'T', sort: sort);
        final restored = Podcast.fromMap(1, original.toMap());

        expect(restored.sort, sort, reason: 'failed for $sort');
      }
    });
  });

  group('equality', () {
    test('two podcasts with the same guid and url are equal', () {
      final a = Podcast(guid: 'g', url: 'https://example.com/feed.xml', link: null, title: 'A');
      final b = Podcast(guid: 'g', url: 'https://example.com/feed.xml', link: null, title: 'B');

      expect(a, equals(b));
      expect(a.hashCode, b.hashCode);
    });

    test('podcasts with a different url are not equal', () {
      final a = Podcast(guid: 'g', url: 'https://example.com/a.xml', link: null, title: 'T');
      final b = Podcast(guid: 'g', url: 'https://example.com/b.xml', link: null, title: 'T');

      expect(a, isNot(equals(b)));
    });
  });
}
