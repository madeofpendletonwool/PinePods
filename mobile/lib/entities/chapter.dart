// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/core/extensions.dart';

/// A class that represents an individual chapter within an [Episode].
///
/// Chapters may, or may not, exist for an episode.
///
/// Part of the [podcast namespace](https://github.com/Podcastindex-org/podcast-namespace)
class Chapter {
  /// Title of this chapter.
  final String title;

  /// URL for the chapter image if one is available.
  final String? imageUrl;

  /// URL of an external link for this chapter if available.
  final String? url;

  /// Table of contents flag. If this is false the chapter should be treated as
  /// meta data only and not be displayed.
  final bool toc;

  /// The start time of the chapter in seconds.
  final double startTime;

  /// The optional end time of the chapter in seconds.
  final double? endTime;

  Chapter({
    required this.title,
    required String? imageUrl,
    required this.startTime,
    String? url,
    this.toc = true,
    this.endTime,
  })  : imageUrl = imageUrl?.forceHttps,
        url = url?.forceHttps;

  Map<String, dynamic> toMap() {
    return <String, dynamic>{
      'title': title,
      'imageUrl': imageUrl,
      'url': url,
      'toc': toc ? 'true' : 'false',
      'startTime': startTime.toString(),
      'endTime': endTime.toString(),
    };
  }

  static Chapter fromMap(Map<String, dynamic> chapter) {
    return Chapter(
      title: chapter['title'] as String,
      imageUrl: chapter['imageUrl'] as String?,
      url: chapter['url'] as String?,
      toc: chapter['toc'] == 'false' ? false : true,
      startTime: double.parse(chapter['startTime'] as String),
      endTime: double.parse(chapter['endTime'] as String),
    );
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is Chapter && runtimeType == other.runtimeType && title == other.title && startTime == other.startTime;

  @override
  int get hashCode => title.hashCode ^ startTime.hashCode;
}
