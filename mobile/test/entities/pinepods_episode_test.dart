import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';

PinepodsEpisode _episode({
  int episodeDuration = 600,
  int? listenDuration,
  String episodePubDate = '2024-01-15T14:30:00',
}) {
  return PinepodsEpisode(
    podcastName: 'Some Podcast',
    episodeTitle: 'Some Episode',
    episodePubDate: episodePubDate,
    episodeDescription: 'Description',
    episodeArtwork: 'https://example.com/art.png',
    episodeUrl: 'https://example.com/episode.mp3',
    episodeDuration: episodeDuration,
    listenDuration: listenDuration,
    episodeId: 101,
    completed: false,
    saved: false,
    queued: false,
    downloaded: false,
    isYoutube: false,
  );
}

void main() {
  group('fromJson', () {
    test('reads capitalized keys (as returned by the PinePods API)', () {
      final episode = PinepodsEpisode.fromJson({
        'Podcastname': 'Some Podcast',
        'Episodetitle': 'Some Episode',
        'Episodepubdate': '2024-01-15T14:30:00',
        'Episodedescription': 'Description',
        'Episodeartwork': 'https://example.com/art.png',
        'Episodeurl': 'https://example.com/episode.mp3',
        'Episodeduration': 600,
        'Listenduration': 120,
        'Episodeid': 101,
        'Completed': true,
        'Saved': true,
        'Queued': true,
        'Downloaded': true,
        'Is_youtube': true,
        'Podcastid': 7,
      });

      expect(episode.podcastName, 'Some Podcast');
      expect(episode.episodeTitle, 'Some Episode');
      expect(episode.episodeDuration, 600);
      expect(episode.listenDuration, 120);
      expect(episode.episodeId, 101);
      expect(episode.completed, isTrue);
      expect(episode.saved, isTrue);
      expect(episode.queued, isTrue);
      expect(episode.downloaded, isTrue);
      expect(episode.isYoutube, isTrue);
      expect(episode.podcastId, 7);
    });

    test('falls back to lowercase keys', () {
      final episode = PinepodsEpisode.fromJson({
        'podcastname': 'Some Podcast',
        'episodetitle': 'Some Episode',
        'episodepubdate': '2024-01-15T14:30:00',
        'episodedescription': 'Description',
        'episodeartwork': 'https://example.com/art.png',
        'episodeurl': 'https://example.com/episode.mp3',
        'episodeduration': 600,
        'listenduration': 120,
        'episodeid': 101,
        'completed': true,
        'saved': false,
        'queued': false,
        'downloaded': false,
        'is_youtube': false,
      });

      expect(episode.podcastName, 'Some Podcast');
      expect(episode.listenDuration, 120);
      expect(episode.completed, isTrue);
    });

    test('defaults missing fields to empty/zero/false rather than throwing', () {
      final episode = PinepodsEpisode.fromJson(const {});

      expect(episode.podcastName, '');
      expect(episode.episodeTitle, '');
      expect(episode.episodeDuration, 0);
      expect(episode.listenDuration, isNull);
      expect(episode.episodeId, 0);
      expect(episode.completed, isFalse);
      expect(episode.podcastId, isNull);
    });
  });

  group('toJson', () {
    test('serializes back to the lowercase key format', () {
      final episode = _episode(listenDuration: 30);

      final json = episode.toJson();

      expect(json['podcastname'], 'Some Podcast');
      expect(json['listenduration'], 30);
      expect(json['is_youtube'], isFalse);
    });
  });

  group('formattedDuration', () {
    test('is 0:00 when duration is zero or negative', () {
      expect(_episode(episodeDuration: 0).formattedDuration, '0:00');
    });

    test('formats under an hour as M:SS', () {
      expect(_episode(episodeDuration: 125).formattedDuration, '2:05');
    });

    test('formats an hour or more as H:MM:SS', () {
      expect(_episode(episodeDuration: 3725).formattedDuration, '1:02:05');
    });
  });

  group('progressPercentage', () {
    test('is 0 when there is no listen duration yet', () {
      expect(_episode(episodeDuration: 600, listenDuration: null).progressPercentage, 0.0);
    });

    test('computes the percentage listened', () {
      expect(_episode(episodeDuration: 200, listenDuration: 50).progressPercentage, 25.0);
    });

    test('clamps at 100 even if listenDuration exceeds the episode duration', () {
      expect(_episode(episodeDuration: 100, listenDuration: 500).progressPercentage, 100.0);
    });
  });

  group('isStarted', () {
    test('is false with no listen duration', () {
      expect(_episode(listenDuration: null).isStarted, isFalse);
    });

    test('is false with a zero listen duration', () {
      expect(_episode(listenDuration: 0).isStarted, isFalse);
    });

    test('is true once there is some listen duration', () {
      expect(_episode(listenDuration: 1).isStarted, isTrue);
    });
  });

  group('formattedListenDuration', () {
    test('is 0:00 with no listen duration', () {
      expect(_episode(listenDuration: null).formattedListenDuration, '0:00');
    });

    test('formats under an hour as M:SS', () {
      expect(_episode(listenDuration: 65).formattedListenDuration, '1:05');
    });

    test('formats an hour or more as H:MM:SS', () {
      expect(_episode(listenDuration: 3665).formattedListenDuration, '1:01:05');
    });
  });

  group('formattedPubDate', () {
    test('delegates to relativePubDateLabel and returns the raw string when unparsable', () {
      expect(_episode(episodePubDate: 'not a date').formattedPubDate, 'not a date');
    });
  });
}
