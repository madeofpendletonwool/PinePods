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

/// The [PodcastService] handles interactions around podcasts including searching, fetching
/// the trending/charts podcasts, loading the podcast RSS feed and anciallary items such as
/// chapters and transcripts.
abstract class PodcastService {
  final PodcastApi api;
  final Repository repository;
  final SettingsService settingsService;

  PodcastService({
    required this.api,
    required this.repository,
    required this.settingsService,
  });

  Future<Podcast?> loadPodcast({
    required Podcast podcast,
    bool highlightNewEpisodes = false,
    bool refresh = false,
  });

  Future<List<Episode>> loadDownloads();

  Future<List<Episode>> loadEpisodes();

  Future<List<Chapter>> loadChaptersByUrl({required String url});

  Future<Transcript> loadTranscriptByUrl({required TranscriptUrl transcriptUrl});

  Future<void> deleteDownload(Episode episode);

  Future<void> toggleEpisodePlayed(Episode episode);

  Future<Episode> saveEpisode(Episode episode);

  Future<List<Episode>> saveEpisodes(List<Episode> episodes);

  Future<Transcript> saveTranscript(Transcript transcript);



  /// Event listeners
  Stream<EpisodeState>? episodeListener;
}
