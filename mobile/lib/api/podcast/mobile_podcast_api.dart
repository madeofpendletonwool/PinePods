// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'dart:io';

import 'package:pinepods_mobile/api/podcast/podcast_api.dart';
import 'package:pinepods_mobile/core/environment.dart';
import 'package:pinepods_mobile/entities/transcript.dart';
import 'package:flutter/foundation.dart';
import 'package:podcast_search/podcast_search.dart' as podcast_search;

/// An implementation of the [PodcastApi].
///
/// A simple wrapper class that interacts with the iTunes/PodcastIndex search API
/// via the podcast_search package.
class MobilePodcastApi extends PodcastApi {
  /// Set when using a custom certificate authority.
  SecurityContext? _defaultSecurityContext;

  /// Bytes containing a custom certificate authority.
  List<int> _certificateAuthorityBytes = [];

  @override
  Future<podcast_search.SearchResult> search(
    String term, {
    String? country,
    String? attribute,
    int? limit,
    String? language,
    int version = 0,
    bool explicit = false,
    String? searchProvider,
  }) async {
    var searchParams = {
      'term': term,
      'searchProvider': searchProvider,
    };

    return compute(_search, searchParams);
  }

  @override
  Future<podcast_search.SearchResult> charts({
    int? size = 20,
    String? genre,
    String? searchProvider,
    String? countryCode = '',
    String? languageCode = '',
  }) async {
    var searchParams = {
      'size': size.toString(),
      'genre': genre,
      'searchProvider': searchProvider,
      'countryCode': countryCode,
      'languageCode': languageCode,
    };

    return compute(_charts, searchParams);
  }

  @override
  List<String> genres(String searchProvider) {
    var provider = searchProvider == 'itunes'
        ? const podcast_search.ITunesProvider()
        : podcast_search.PodcastIndexProvider(
            key: podcastIndexKey,
            secret: podcastIndexSecret,
          );

    return podcast_search.Search(
      userAgent: Environment.userAgent(),
      searchProvider: provider,
    ).genres();
  }

  @override
  Future<podcast_search.Podcast> loadFeed(String url) async {
    return _loadFeed(url);
  }

  @override
  Future<podcast_search.Chapters> loadChapters(String url) async {
    // In podcast_search 0.7.11, load chapters using Feed.loadChaptersByUrl
    try {
      return await podcast_search.Feed.loadChaptersByUrl(url: url);
    } catch (e) {
      // Fallback: create empty chapters if loading fails
      return podcast_search.Chapters(url: url);
    }
  }

  @override
  Future<podcast_search.Transcript> loadTranscript(TranscriptUrl transcriptUrl) async {
    late podcast_search.TranscriptFormat format;

    switch (transcriptUrl.type) {
      case TranscriptFormat.subrip:
        format = podcast_search.TranscriptFormat.subrip;
        break;
      case TranscriptFormat.json:
        format = podcast_search.TranscriptFormat.json;
        break;
      case TranscriptFormat.unsupported:
        format = podcast_search.TranscriptFormat.unsupported;
        break;
    }

    // In podcast_search 0.7.11, load transcript using Feed.loadTranscriptByUrl
    try {
      // Create a podcast_search.TranscriptUrl from our local TranscriptUrl
      final searchTranscriptUrl = podcast_search.TranscriptUrl(
        url: transcriptUrl.url,
        type: format,
        language: transcriptUrl.language ?? '',
        rel: transcriptUrl.rel ?? '',
      );
      
      return await podcast_search.Feed.loadTranscriptByUrl(
        transcriptUrl: searchTranscriptUrl
      );
    } catch (e) {
      // Fallback: create empty transcript if loading fails
      return podcast_search.Transcript();
    }
  }

  static Future<podcast_search.SearchResult> _search(Map<String, String?> searchParams) {
    var term = searchParams['term']!;
    var provider = searchParams['searchProvider'] == 'itunes'
        ? const podcast_search.ITunesProvider()
        : podcast_search.PodcastIndexProvider(
            key: podcastIndexKey,
            secret: podcastIndexSecret,
          );

    return podcast_search.Search(
      userAgent: Environment.userAgent(),
      searchProvider: provider,
    ).search(term).timeout(const Duration(seconds: 30));
  }

  static Future<podcast_search.SearchResult> _charts(Map<String, String?> searchParams) {
    var provider = searchParams['searchProvider'] == 'itunes'
        ? const podcast_search.ITunesProvider()
        : podcast_search.PodcastIndexProvider(
            key: podcastIndexKey,
            secret: podcastIndexSecret,
          );

    var countryCode = searchParams['countryCode'];
    var languageCode = searchParams['languageCode'] ?? '';
    var country = podcast_search.Country.none;

    if (countryCode != null && countryCode.isNotEmpty) {
      country = podcast_search.Country.values.where((element) => element.code == countryCode).first;
    }

    return podcast_search.Search(userAgent: Environment.userAgent(), searchProvider: provider)
        .charts(genre: searchParams['genre']!, country: country, language: languageCode, limit: 50)
        .timeout(const Duration(seconds: 30));
  }

  Future<podcast_search.Podcast> _loadFeed(String url) {
    _setupSecurityContext();
    // In podcast_search 0.7.11, use Feed.loadFeed or create a Feed instance
    return podcast_search.Feed.loadFeed(url: url, userAgent: Environment.userAgent());
  }

  void _setupSecurityContext() {
    if (_certificateAuthorityBytes.isNotEmpty && _defaultSecurityContext == null) {
      SecurityContext.defaultContext.setTrustedCertificatesBytes(_certificateAuthorityBytes);
      _defaultSecurityContext = SecurityContext.defaultContext;
    }
  }

  @override
  void addClientAuthorityBytes(List<int> certificateAuthorityBytes) {
    _certificateAuthorityBytes = certificateAuthorityBytes;
  }
}
