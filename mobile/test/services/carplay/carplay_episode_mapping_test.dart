// CarPlay list items are plain Episodes; playback rebuilds the PinePods episode
// from them so it goes through the tracked play path. The server-download and
// YouTube flags decide the stream URL, so they must survive the round trip.

import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/services/carplay/carplay_service.dart';

import '../../support/factories.dart';

void main() {
  test('rebuilds id, position and source URL from a CarPlay item', () {
    final ep = CarPlayService.toPinepodsEpisode(buildEpisode(
      guid: 'pinepods_101',
      contentUrl: 'https://example.com/episode.mp3',
    )
      ..link = 'https://example.com/episode.mp3'
      ..position = 754000
      ..duration = 600000);

    expect(ep, isNotNull);
    expect(ep!.episodeId, 101);
    expect(ep.listenDuration, 754);
    expect(ep.episodeDuration, 600);
    expect(ep.episodeUrl, 'https://example.com/episode.mp3');
    expect(ep.downloaded, isFalse);
    expect(ep.isYoutube, isFalse);
  });

  test('recovers server-download and YouTube flags from the stream URL', () {
    final downloaded = CarPlayService.toPinepodsEpisode(buildEpisode(
      guid: 'pinepods_5',
      contentUrl: 'https://pp.example/api/data/stream/5?api_key=k&user_id=1',
    ));
    final youtube = CarPlayService.toPinepodsEpisode(buildEpisode(
      guid: 'pinepods_6',
      contentUrl: 'https://pp.example/api/data/stream/6?api_key=k&user_id=1&type=youtube',
    ));

    expect(downloaded!.downloaded, isTrue);
    expect(downloaded.isYoutube, isFalse);
    expect(youtube!.isYoutube, isTrue);
    expect(youtube.downloaded, isFalse);
  });

  test('items without a PinePods id are left to the direct play path', () {
    expect(CarPlayService.toPinepodsEpisode(buildEpisode(guid: 'https://feed/ep')), isNull);
    expect(CarPlayService.toPinepodsEpisode(buildEpisode(guid: 'pinepods_x')), isNull);
  });
}
