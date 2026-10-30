import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/entities/youtube_channel.dart';

void main() {
  group('YouTubeChannelResult.fromJson', () {
    test('reads the search service shape', () {
      final channel = YouTubeChannelResult.fromJson({
        'channelId': 'UCabc',
        'name': 'Chan',
        'description': 'About',
        'thumbnailUrl': 'https://img/chan.jpg',
        'subscriberCount': 1200,
        'videoCount': null,
        'url': 'https://www.youtube.com/channel/UCabc',
        'recentVideos': [
          {'id': 'vid1', 'title': 'One', 'duration': 61.0, 'url': 'https://www.youtube.com/watch?v=vid1'},
          {'id': '', 'title': 'No id'},
        ],
      });

      expect(channel.channelId, 'UCabc');
      expect(channel.url, 'https://www.youtube.com/channel/UCabc');
      expect(channel.subscriberCount, 1200);
      expect(channel.videoCount, isNull);
      expect(channel.recentVideos.map((v) => v.id), ['vid1']);
      expect(channel.recentVideos.single.durationSeconds, 61);
    });

    test("reads the server's own snake_case shape", () {
      final channel = YouTubeChannelResult.fromJson({
        'channel_id': 'UCxyz',
        'name': 'Other',
        'description': '',
        'thumbnail_url': 'https://img/other.jpg',
        'recent_videos': [
          {'id': 'v', 'title': 'T', 'duration': null},
        ],
      });

      expect(channel.channelId, 'UCxyz');
      expect(channel.thumbnailUrl, 'https://img/other.jpg');
      expect(channel.recentVideos.single.durationSeconds, isNull);
    });
  });

  test('channel feed URLs', () {
    expect(isYoutubeChannelUrl(youtubeChannelUrl('UCabc')), isTrue);
    expect(isYoutubeChannelUrl('https://feeds.example.com/pod.xml'), isFalse);
  });

  test('YouTubeVideoFilter.fromJson leaves unset limits null', () {
    final filter = YouTubeVideoFilter.fromJson({'min_minutes': 5, 'max_minutes': null});
    expect(filter.minMinutes, 5);
    expect(filter.maxMinutes, isNull);
    expect(filter.titleContains, isNull);
  });
}
