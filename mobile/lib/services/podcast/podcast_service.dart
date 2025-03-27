// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/api/podcast/podcast_api.dart';
import 'package:pinepods_mobile/entities/chapter.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/podcast.dart';
import 'package:pinepods_mobile/entities/transcript.dart';
import 'package:pinepods_mobile/repository/repository.dart';
import 'package:pinepods_mobile/services/settings/settings_service.dart';
import 'package:pinepods_mobile/state/episode_state.dart';
import 'package:podcast_search/podcast_search.dart' as pcast;

/// The [PodcastService] handles interactions around podcasts including searching, fetching
/// the trending/charts podcasts, loading the podcast RSS feed and anciallary items such as
/// chapters and transcripts.
abstract class PodcastService {
  final PodcastApi api;
  final Repository repository;
  final SettingsService settingsService;

  static const itunesGenres = [
    '<All>',
    'Arts',
    'Business',
    'Comedy',
    'Education',
    'Fiction',
    'Government',
    'Health & Fitness',
    'History',
    'Kids & Family',
    'Leisure',
    'Music',
    'News',
    'Religion & Spirituality',
    'Science',
    'Society & Culture',
    'Sports',
    'TV & Film',
    'Technology',
    'True Crime',
  ];

  static const podcastIndexGenres = <String>[
    '<All>',
    'After-Shows',
    'Alternative',
    'Animals',
    'Animation',
    'Arts',
    'Astronomy',
    'Automotive',
    'Aviation',
    'Baseball',
    'Basketball',
    'Beauty',
    'Books',
    'Buddhism',
    'Business',
    'Careers',
    'Chemistry',
    'Christianity',
    'Climate',
    'Comedy',
    'Commentary',
    'Courses',
    'Crafts',
    'Cricket',
    'Cryptocurrency',
    'Culture',
    'Daily',
    'Design',
    'Documentary',
    'Drama',
    'Earth',
    'Education',
    'Entertainment',
    'Entrepreneurship',
    'Family',
    'Fantasy',
    'Fashion',
    'Fiction',
    'Film',
    'Fitness',
    'Food',
    'Football',
    'Games',
    'Garden',
    'Golf',
    'Government',
    'Health',
    'Hinduism',
    'History',
    'Hobbies',
    'Hockey',
    'Home',
    'How-To',
    'Improv',
    'Interviews',
    'Investing',
    'Islam',
    'Journals',
    'Judaism',
    'Kids',
    'Language',
    'Learning',
    'Leisure',
    'Life',
    'Management',
    'Manga',
    'Marketing',
    'Mathematics',
    'Medicine',
    'Mental',
    'Music',
    'Natural',
    'Nature',
    'News',
    'Non-Profit',
    'Nutrition',
    'Parenting',
    'Performing',
    'Personal',
    'Pets',
    'Philosophy',
    'Physics',
    'Places',
    'Politics',
    'Relationships',
    'Religion',
    'Reviews',
    'Role-Playing',
    'Rugby',
    'Running',
    'Science',
    'Self-Improvement',
    'Sexuality',
    'Soccer',
    'Social',
    'Society',
    'Spirituality',
    'Sports',
    'Stand-Up',
    'Stories',
    'Swimming',
    'TV',
    'Tabletop',
    'Technology',
    'Tennis',
    'Travel',
    'True Crime',
    'Video-Games',
    'Visual',
    'Volleyball',
    'Weather',
    'Wilderness',
    'Wrestling',
  ];

  PodcastService({
    required this.api,
    required this.repository,
    required this.settingsService,
  });

  Future<pcast.SearchResult> search({
    required String term,
    String? country,
    String? attribute,
    int? limit,
    String? language,
    int version = 0,
    bool explicit = false,
  });

  Future<pcast.SearchResult> charts({
    required int size,
    String? genre,
    String? countryCode,
    String? languageCode,
  });

  List<String> genres();

  Future<Podcast?> loadPodcast({
    required Podcast podcast,
    bool highlightNewEpisodes = false,
    bool refresh = false,
  });

  Future<Podcast?> loadPodcastById({
    required int id,
  });

  Future<List<Episode>> loadDownloads();

  Future<List<Episode>> loadEpisodes();

  Future<List<Chapter>> loadChaptersByUrl({required String url});

  Future<Transcript> loadTranscriptByUrl({required TranscriptUrl transcriptUrl});

  Future<void> deleteDownload(Episode episode);

  Future<void> toggleEpisodePlayed(Episode episode);

  Future<List<Podcast>> subscriptions();

  Future<Podcast?> subscribe(Podcast podcast);

  Future<void> unsubscribe(Podcast podcast);

  Future<Podcast?> save(Podcast podcast, {bool withEpisodes = true});

  Future<Episode> saveEpisode(Episode episode);

  Future<List<Episode>> saveEpisodes(List<Episode> episodes);

  Future<Transcript> saveTranscript(Transcript transcript);

  Future<void> saveQueue(List<Episode> episodes);

  Future<List<Episode>> loadQueue();

  /// Event listeners
  Stream<Podcast?>? podcastListener;
  Stream<EpisodeState>? episodeListener;
}
