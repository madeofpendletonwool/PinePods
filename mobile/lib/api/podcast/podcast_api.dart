// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/entities/transcript.dart';
import 'package:podcast_search/podcast_search.dart' as pslib;

/// A simple wrapper class that interacts with the search API via
/// the podcast_search package.
///
/// TODO: Make this more generic so it's not tied to podcast_search
abstract class PodcastApi {
  /// URL representing the RSS feed for a podcast.
  Future<pslib.Podcast> loadFeed(String url);

  /// Load episode chapters via JSON file.
  Future<pslib.Chapters> loadChapters(String url);

  /// Load episode transcript via SRT or JSON file.
  Future<pslib.Transcript> loadTranscript(TranscriptUrl transcriptUrl);

  /// Allow adding of custom certificates. Required as default context
  /// does not apply when running in separate Isolate.
  void addClientAuthorityBytes(List<int> certificateAuthorityBytes);
}
