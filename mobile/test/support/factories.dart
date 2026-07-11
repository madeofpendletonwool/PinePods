// Shared entity builders for tests. Centralizes the "valid-by-default" object
// construction that individual test files would otherwise each re-declare as a
// local `_episode(...)`/`_podcast(...)` helper.
//
// Episodes default to a concrete [lastUpdated] on purpose: Episode.toMap stores
// a null lastUpdated as '' which Episode.fromMap can't round-trip (a known
// pre-existing bug), so leaving it null would break persistence round-trips
// unrelated to what a test is actually checking.

import 'package:pinepods_mobile/entities/downloadable.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/entities/podcast.dart';

Episode buildEpisode({
  String guid = 'episode-guid',
  String? pguid = 'podcast-guid',
  String? podcast = 'Some Podcast',
  String? title = 'Some Episode',
  String? description,
  String? contentUrl = 'https://example.com/episode.mp3',
  String? imageUrl,
  DownloadState downloadState = DownloadState.none,
  String? filepath,
  String? filename,
  int duration = 600,
  int position = 0,
  int? downloadPercentage = 0,
  bool played = false,
  DateTime? publicationDate,
  DateTime? lastUpdated,
}) {
  return Episode(
    guid: guid,
    pguid: pguid,
    podcast: podcast,
    title: title,
    description: description,
    contentUrl: contentUrl,
    imageUrl: imageUrl,
    downloadState: downloadState,
    filepath: filepath,
    filename: filename,
    duration: duration,
    position: position,
    downloadPercentage: downloadPercentage,
    played: played,
    publicationDate: publicationDate ?? DateTime(2024, 1, 1),
    lastUpdated: lastUpdated ?? DateTime(2024, 1, 1),
  );
}

Podcast buildPodcast({
  String guid = 'podcast-guid',
  String url = 'https://example.com/feed.xml',
  String? link = 'https://example.com',
  String title = 'Some Podcast',
  String? description = 'A podcast',
  List<Episode> episodes = const <Episode>[],
  DateTime? lastUpdated,
}) {
  return Podcast(
    guid: guid,
    url: url,
    link: link,
    title: title,
    description: description,
    episodes: episodes,
    lastUpdated: lastUpdated ?? DateTime(2024, 1, 1),
  );
}

PinepodsEpisode buildPinepodsEpisode({
  int episodeId = 101,
  String podcastName = 'Some Podcast',
  String episodeTitle = 'Some Episode',
  String episodePubDate = '2024-01-15T14:30:00',
  int episodeDuration = 600,
  int? listenDuration,
  bool completed = false,
  bool saved = false,
  bool queued = false,
  bool downloaded = false,
  bool isYoutube = false,
}) {
  return PinepodsEpisode(
    podcastName: podcastName,
    episodeTitle: episodeTitle,
    episodePubDate: episodePubDate,
    episodeDescription: 'Description',
    episodeArtwork: 'https://example.com/art.png',
    episodeUrl: 'https://example.com/episode.mp3',
    episodeDuration: episodeDuration,
    listenDuration: listenDuration,
    episodeId: episodeId,
    completed: completed,
    saved: saved,
    queued: queued,
    downloaded: downloaded,
    isYoutube: isYoutube,
  );
}
