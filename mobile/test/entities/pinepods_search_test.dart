import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/entities/pinepods_search.dart';

PinepodsPodcast _pinepodsPodcast({int id = 1, String title = 'Some Podcast'}) {
  return PinepodsPodcast(
    id: id,
    title: title,
    url: 'https://example.com/feed.xml',
    originalUrl: 'https://example.com/original.xml',
    link: 'https://example.com',
    description: 'A podcast',
    author: 'Author',
    ownerName: 'Owner',
    image: 'https://example.com/art.png',
    artwork: 'https://example.com/artwork.png',
    lastUpdateTime: 1700000000,
    explicit: false,
    episodeCount: 42,
  );
}

PinepodsITunesPodcast _itunesPodcast({
  int trackId = 55,
  List<String> genres = const ['Comedy', 'Tech'],
  String releaseDate = '2024-01-15T00:00:00Z',
  String collectionExplicitness = 'notExplicit',
  int? trackCount = 10,
}) {
  return PinepodsITunesPodcast(
    wrapperType: 'track',
    kind: 'podcast',
    collectionId: 999,
    trackId: trackId,
    artistName: 'Some Artist',
    trackName: 'Some Show',
    collectionViewUrl: 'https://podcasts.apple.com/show',
    feedUrl: 'https://example.com/feed.xml',
    artworkUrl100: 'https://example.com/art100.png',
    releaseDate: releaseDate,
    genres: genres,
    collectionExplicitness: collectionExplicitness,
    trackCount: trackCount,
  );
}

void main() {
  group('PinepodsPodcast.fromJson/toJson', () {
    test('round-trips every field, including a categories map', () {
      final json = {
        'id': 1,
        'title': 'Some Podcast',
        'url': 'https://example.com/feed.xml',
        'originalUrl': 'https://example.com/original.xml',
        'link': 'https://example.com',
        'description': 'A podcast',
        'author': 'Author',
        'ownerName': 'Owner',
        'image': 'https://example.com/art.png',
        'artwork': 'https://example.com/artwork.png',
        'lastUpdateTime': 1700000000,
        'categories': {'0': 'Comedy'},
        'explicit': false,
        'episodeCount': 42,
      };

      final podcast = PinepodsPodcast.fromJson(json);

      expect(podcast.id, 1);
      expect(podcast.title, 'Some Podcast');
      expect(podcast.categories, {'0': 'Comedy'});
      expect(podcast.episodeCount, 42);
      expect(podcast.toJson(), json);
    });

    test('categories defaults to null when absent', () {
      final podcast = _pinepodsPodcast();
      expect(podcast.categories, isNull);
    });
  });

  group('PinepodsITunesPodcast.fromJson/toJson', () {
    test('round-trips every field, including the genres list', () {
      final json = {
        'wrapperType': 'track',
        'kind': 'podcast',
        'collectionId': 999,
        'trackId': 55,
        'artistName': 'Some Artist',
        'trackName': 'Some Show',
        'collectionViewUrl': 'https://podcasts.apple.com/show',
        'feedUrl': 'https://example.com/feed.xml',
        'artworkUrl100': 'https://example.com/art100.png',
        'releaseDate': '2024-01-15T00:00:00Z',
        'genres': ['Comedy', 'Tech'],
        'collectionExplicitness': 'notExplicit',
        'trackCount': 10,
      };

      final podcast = PinepodsITunesPodcast.fromJson(json);

      expect(podcast.trackId, 55);
      expect(podcast.genres, ['Comedy', 'Tech']);
      expect(podcast.trackCount, 10);
      expect(podcast.toJson(), json);
    });

    test('genres defaults to an empty list and trackCount to null when absent', () {
      final podcast = PinepodsITunesPodcast.fromJson(const {});

      expect(podcast.genres, isEmpty);
      expect(podcast.trackCount, isNull);
    });
  });

  group('UnifiedPinepodsPodcast.fromPodcast', () {
    test('maps the podcast index id into indexId and zeroes the internal id', () {
      final podcast = _pinepodsPodcast(id: 7, title: 'Indexed Show');

      final unified = UnifiedPinepodsPodcast.fromPodcast(podcast);

      expect(unified.id, 0);
      expect(unified.indexId, 7);
      expect(unified.title, 'Indexed Show');
      expect(unified.url, podcast.url);
      expect(unified.episodeCount, podcast.episodeCount);
      expect(unified.categories, podcast.categories);
    });
  });

  group('UnifiedPinepodsPodcast.fromITunesPodcast', () {
    test('maps trackId to id, converts the genre list to an indexed map', () {
      final podcast = _itunesPodcast(trackId: 123, genres: const ['Comedy', 'News']);

      final unified = UnifiedPinepodsPodcast.fromITunesPodcast(podcast);

      expect(unified.id, 123);
      expect(unified.indexId, 0);
      expect(unified.title, podcast.trackName);
      expect(unified.categories, {'0': 'Comedy', '1': 'News'});
      expect(unified.description, 'Descriptions not provided by iTunes');
    });

    test('parses a valid releaseDate into a unix-seconds timestamp', () {
      final podcast = _itunesPodcast(releaseDate: '2024-01-01T00:00:00Z');

      final unified = UnifiedPinepodsPodcast.fromITunesPodcast(podcast);

      expect(unified.lastUpdateTime, DateTime.parse('2024-01-01T00:00:00Z').millisecondsSinceEpoch ~/ 1000);
    });

    test('falls back to a 0 timestamp when releaseDate cannot be parsed', () {
      final podcast = _itunesPodcast(releaseDate: 'not a date');

      final unified = UnifiedPinepodsPodcast.fromITunesPodcast(podcast);

      expect(unified.lastUpdateTime, 0);
    });

    test('explicit is true only when collectionExplicitness is exactly "explicit"', () {
      expect(UnifiedPinepodsPodcast.fromITunesPodcast(_itunesPodcast(collectionExplicitness: 'explicit')).explicit, isTrue);
      expect(UnifiedPinepodsPodcast.fromITunesPodcast(_itunesPodcast(collectionExplicitness: 'notExplicit')).explicit, isFalse);
      expect(UnifiedPinepodsPodcast.fromITunesPodcast(_itunesPodcast(collectionExplicitness: 'cleaned')).explicit, isFalse);
    });

    test('episodeCount falls back to 0 when trackCount is null', () {
      final podcast = _itunesPodcast(trackCount: null);
      expect(UnifiedPinepodsPodcast.fromITunesPodcast(podcast).episodeCount, 0);
    });
  });

  group('UnifiedPinepodsPodcast.fromJson/toJson', () {
    test('round-trips every field', () {
      final json = {
        'id': 1,
        'indexId': 2,
        'title': 'T',
        'url': 'https://example.com/feed.xml',
        'originalUrl': 'https://example.com/original.xml',
        'link': 'https://example.com',
        'description': 'D',
        'author': 'A',
        'ownerName': 'O',
        'image': 'https://example.com/art.png',
        'artwork': 'https://example.com/artwork.png',
        'lastUpdateTime': 1700000000,
        'categories': {'0': 'Comedy'},
        'explicit': true,
        'episodeCount': 10,
      };

      final unified = UnifiedPinepodsPodcast.fromJson(json);
      expect(unified.toJson(), json);
    });
  });

  group('PinepodsSearchResult', () {
    test('getUnifiedPodcasts merges PodcastIndex feeds and iTunes results', () {
      final result = PinepodsSearchResult(
        feeds: [_pinepodsPodcast(id: 1, title: 'From Index')],
        results: [_itunesPodcast(trackId: 2)],
      );

      final unified = result.getUnifiedPodcasts();

      expect(unified, hasLength(2));
      expect(unified[0].title, 'From Index');
      expect(unified[0].indexId, 1);
      expect(unified[1].id, 2);
    });

    test('getUnifiedPodcasts is empty when both feeds and results are null', () {
      final result = PinepodsSearchResult();
      expect(result.getUnifiedPodcasts(), isEmpty);
    });

    test('fromJson/toJson round-trips status, resultCount, feeds, and results', () {
      final json = {
        'status': 'true',
        'resultCount': 1,
        'feeds': [
          {
            'id': 1,
            'title': 'T',
            'url': 'https://example.com/feed.xml',
            'originalUrl': 'https://example.com/original.xml',
            'link': 'https://example.com',
            'description': 'D',
            'author': 'A',
            'ownerName': 'O',
            'image': 'https://example.com/art.png',
            'artwork': 'https://example.com/artwork.png',
            'lastUpdateTime': 0,
            'categories': null,
            'explicit': false,
            'episodeCount': 0,
          }
        ],
        'results': null,
      };

      final result = PinepodsSearchResult.fromJson(json);

      expect(result.status, 'true');
      expect(result.resultCount, 1);
      expect(result.feeds, hasLength(1));
      expect(result.results, isNull);
    });
  });

  group('SearchProviderExtension', () {
    test('has a display name and value for every provider', () {
      expect(SearchProvider.podcastIndex.name, 'Podcast Index');
      expect(SearchProvider.podcastIndex.value, 'podcast_index');
      expect(SearchProvider.itunes.name, 'iTunes');
      expect(SearchProvider.itunes.value, 'itunes');
    });
  });
}
