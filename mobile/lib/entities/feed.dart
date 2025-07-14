// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/entities/podcast.dart';

/// This class is used when loading a [Podcast] feed.
///
/// The key information is contained within the [Podcast] instance, but as the
/// iTunes API also returns large and thumbnail artwork within its search results
/// this class also contains properties to represent those.
class Feed {
  /// The podcast to load
  final Podcast podcast;

  /// The full-size artwork for the podcast.
  String? imageUrl;

  /// The thumbnail artwork for the podcast,
  String? thumbImageUrl;

  /// If true the podcast is loaded regardless of if it's currently cached.
  bool refresh;

  /// If true, will also perform an additional background refresh.
  bool backgroundFresh;

  /// If true any error can be ignored.
  bool silently;

  Feed({
    required this.podcast,
    this.imageUrl,
    this.thumbImageUrl,
    this.refresh = false,
    this.backgroundFresh = false,
    this.silently = false,
  });
}
