import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/entities/downloadable.dart';
import 'package:pinepods_mobile/entities/episode.dart';

Episode _episode({
  String guid = 'guid-1',
  String? podcast = 'Some Podcast',
  int position = 0,
  int duration = 0,
  int? downloadPercentage = 0,
  String? description,
  String? imageUrl,
  String? contentUrl,
}) {
  return Episode(
    guid: guid,
    podcast: podcast,
    position: position,
    duration: duration,
    downloadPercentage: downloadPercentage,
    description: description,
    imageUrl: imageUrl,
    contentUrl: contentUrl,
    // fromMap can't round-trip a null lastUpdated (toMap stores '' rather
    // than a value fromMap's null-check recognizes), so give every episode
    // built by this helper a concrete value to stay clear of that path. A
    // fixed value (rather than DateTime.now()) keeps two separately-built
    // episodes hashCode-equal, since hashCode (unlike ==) does factor in
    // lastUpdated.
    lastUpdated: DateTime(2024, 1, 1),
  );
}

void main() {
  group('construction', () {
    test('upgrades http image/content URLs to https on construction', () {
      final episode = _episode(
        imageUrl: 'http://example.com/art.png',
        contentUrl: 'http://example.com/episode.mp3',
      );

      expect(episode.imageUrl, 'https://example.com/art.png');
      expect(episode.contentUrl, 'https://example.com/episode.mp3');
    });

    test('leaves a local-network content URL over http unchanged', () {
      final episode = _episode(contentUrl: 'http://192.168.1.10:8040/episode.mp3');

      expect(episode.contentUrl, 'http://192.168.1.10:8040/episode.mp3');
    });
  });

  group('toMap/fromMap', () {
    test('round-trips every field', () {
      final original = Episode(
        guid: 'guid-1',
        pguid: 'podcast-guid',
        podcast: 'Some Podcast',
        downloadTaskId: 'task-1',
        filepath: '/storage/podcast',
        filename: 'episode.mp3',
        downloadState: DownloadState.downloaded,
        title: 'Episode Title',
        description: '<p>desc</p>',
        content: 'content',
        link: 'https://example.com',
        imageUrl: 'https://example.com/art.png',
        publicationDate: DateTime.fromMillisecondsSinceEpoch(1700000000000),
        contentUrl: 'https://example.com/episode.mp3',
        author: 'Author',
        season: 2,
        episode: 5,
        duration: 3600,
        position: 120,
        downloadPercentage: 100,
        played: true,
        lastUpdated: DateTime.fromMillisecondsSinceEpoch(1700000001000),
      );

      final restored = Episode.fromMap(42, original.toMap());

      expect(restored.id, 42);
      expect(restored.guid, original.guid);
      expect(restored.pguid, original.pguid);
      expect(restored.downloadTaskId, original.downloadTaskId);
      expect(restored.filepath, original.filepath);
      expect(restored.filename, original.filename);
      expect(restored.downloadState, original.downloadState);
      expect(restored.podcast, original.podcast);
      expect(restored.title, original.title);
      expect(restored.description, original.description);
      expect(restored.content, original.content);
      expect(restored.link, original.link);
      expect(restored.imageUrl, original.imageUrl);
      expect(restored.publicationDate, original.publicationDate);
      expect(restored.contentUrl, original.contentUrl);
      expect(restored.author, original.author);
      expect(restored.season, original.season);
      expect(restored.episode, original.episode);
      expect(restored.duration, original.duration);
      expect(restored.position, original.position);
      expect(restored.downloadPercentage, original.downloadPercentage);
      expect(restored.played, original.played);
      expect(restored.lastUpdated, original.lastUpdated);
    });

    test('defaults numeric fields to 0 and a fresh date when absent', () {
      final map = <String, dynamic>{
        'guid': 'guid-1',
        'podcast': 'Some Podcast',
      };

      final restored = Episode.fromMap(1, map);

      expect(restored.season, 0);
      expect(restored.episode, 0);
      expect(restored.duration, 0);
      expect(restored.position, 0);
      expect(restored.downloadPercentage, 0);
      expect(restored.played, isFalse);
      expect(restored.publicationDate, isNotNull);
    });

    test('maps every DownloadState index round-trip through toMap/fromMap', () {
      for (final state in DownloadState.values) {
        final original = _episode()..downloadState = state;
        final restored = Episode.fromMap(1, original.toMap());

        expect(restored.downloadState, state, reason: 'failed for $state');
      }
    });
  });

  group('equality', () {
    test('two episodes with the same field values are equal', () {
      final a = _episode(guid: 'guid-1');
      final b = _episode(guid: 'guid-1');

      expect(a, equals(b));
      expect(a.hashCode, b.hashCode);
    });

    test('episodes with a different guid are not equal', () {
      final a = _episode(guid: 'guid-1');
      final b = _episode(guid: 'guid-2');

      expect(a, isNot(equals(b)));
    });
  });

  group('downloaded', () {
    test('is true only at exactly 100%', () {
      expect(_episode(downloadPercentage: 100).downloaded, isTrue);
      expect(_episode(downloadPercentage: 99).downloaded, isFalse);
      expect(_episode(downloadPercentage: null).downloaded, isFalse);
    });
  });

  group('timeRemaining', () {
    test('is zero when there is no position yet', () {
      expect(_episode(position: 0, duration: 600).timeRemaining, Duration.zero);
    });

    test('is zero when duration is unknown', () {
      expect(_episode(position: 1000, duration: 0).timeRemaining, Duration.zero);
    });

    test('is duration (seconds) minus position (ms treated as seconds component)', () {
      // position is stored in milliseconds; timeRemaining takes the seconds
      // component of that as elapsed seconds against a duration in seconds.
      final episode = _episode(position: 30000, duration: 600);
      expect(episode.timeRemaining, const Duration(seconds: 570));
    });
  });

  group('percentagePlayed', () {
    test('is zero with no position or duration', () {
      expect(_episode(position: 0, duration: 600).percentagePlayed, 0.0);
    });

    test('computes position (ms) over duration (seconds) as a percentage', () {
      final episode = _episode(position: 30000, duration: 60);
      expect(episode.percentagePlayed, 50.0);
    });

    test('clamps at 100 even if position implies more than the full duration', () {
      final episode = _episode(position: 999999, duration: 10);
      expect(episode.percentagePlayed, 100.0);
    });
  });

  group('descriptionText', () {
    test('is empty when there is no description', () {
      expect(_episode(description: null).descriptionText, '');
    });

    test('strips HTML tags down to plain text', () {
      final episode = _episode(description: '<p>Hello <b>world</b></p>');
      expect(episode.descriptionText, 'Hello world');
    });

    test('replaces <br> tags with a space for readability', () {
      final episode = _episode(description: 'Line one<br/>Line two');
      expect(episode.descriptionText, 'Line one Line two');
    });
  });

  group('hasChapters / hasTranscripts', () {
    test('hasChapters is false with no chaptersUrl and no chapters', () {
      expect(_episode().hasChapters, isFalse);
    });

    test('hasChapters is true when chaptersUrl is set', () {
      final episode = Episode(guid: 'g', podcast: 'p', chaptersUrl: 'https://example.com/chapters.json');
      expect(episode.hasChapters, isTrue);
    });

    test('hasTranscripts is false with no transcript URLs', () {
      expect(_episode().hasTranscripts, isFalse);
    });
  });

  group('chaptersAreLoaded / chaptersAreNotLoaded', () {
    test('neither is true before loading has been attempted', () {
      final episode = _episode();
      expect(episode.chaptersAreLoaded, isFalse);
      expect(episode.chaptersAreNotLoaded, isFalse);
    });

    test('chaptersAreNotLoaded is true while loading with nothing loaded yet', () {
      final episode = _episode()..chaptersLoading = true;
      expect(episode.chaptersAreNotLoaded, isTrue);
      expect(episode.chaptersAreLoaded, isFalse);
    });
  });

  group('positionalImageUrl', () {
    test('falls back to the episode image when there is no current chapter', () {
      final episode = _episode(imageUrl: 'https://example.com/episode.png');
      expect(episode.positionalImageUrl, 'https://example.com/episode.png');
    });
  });
}
