// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/entities/search_providers.dart';

class AppSettings {
  /// The current theme name.
  final String theme;

  /// True if episodes are marked as played when deleted.
  final bool markDeletedEpisodesAsPlayed;

  /// True if downloaded played episodes must be deleted automatically.
  final bool deleteDownloadedPlayedEpisodes;

  /// True if downloads should be saved to the SD card.
  final bool storeDownloadsSDCard;

  /// The default playback speed.
  final double playbackSpeed;

  /// The search provider: itunes or podcastindex.
  final String? searchProvider;

  /// List of search providers: currently itunes or podcastindex.
  final List<SearchProvider> searchProviders;

  /// True if the user has confirmed dialog accepting funding links.
  final bool externalLinkConsent;

  /// If true the main player window will open as soon as an episode starts.
  final bool autoOpenNowPlaying;

  /// If true the funding link icon will appear (if the podcast supports it).
  final bool showFunding;

  /// If -1 never; 0 always; otherwise time in minutes.
  final int autoUpdateEpisodePeriod;

  /// If true, silence in audio playback is trimmed. Currently Android only.
  final bool trimSilence;

  /// If true, volume is boosted. Currently Android only.
  final bool volumeBoost;

  /// If 0, list view; else grid view
  final int layout;

  final String? pinepodsServer;

  final String? pinepodsApiKey;

  final int? pinepodsUserId;

  AppSettings({
    required this.theme,
    required this.markDeletedEpisodesAsPlayed,
    required this.deleteDownloadedPlayedEpisodes,
    required this.storeDownloadsSDCard,
    required this.playbackSpeed,
    required this.searchProvider,
    required this.searchProviders,
    required this.externalLinkConsent,
    required this.autoOpenNowPlaying,
    required this.showFunding,
    required this.autoUpdateEpisodePeriod,
    required this.trimSilence,
    required this.volumeBoost,
    required this.layout,
    this.pinepodsServer,
    this.pinepodsApiKey,
    this.pinepodsUserId,
  });

  AppSettings.sensibleDefaults()
      : theme = 'dark',
        markDeletedEpisodesAsPlayed = false,
        deleteDownloadedPlayedEpisodes = false,
        storeDownloadsSDCard = false,
        playbackSpeed = 1.0,
        searchProvider = 'itunes',
        searchProviders = <SearchProvider>[],
        externalLinkConsent = false,
        autoOpenNowPlaying = false,
        showFunding = true,
        autoUpdateEpisodePeriod = -1,
        trimSilence = false,
        volumeBoost = false,
        layout = 0,
        pinepodsServer = null,
        pinepodsApiKey = null,
        pinepodsUserId = null;

  AppSettings copyWith({
    String? theme,
    bool? markDeletedEpisodesAsPlayed,
    bool? deleteDownloadedPlayedEpisodes,
    bool? storeDownloadsSDCard,
    double? playbackSpeed,
    String? searchProvider,
    List<SearchProvider>? searchProviders,
    bool? externalLinkConsent,
    bool? autoOpenNowPlaying,
    bool? showFunding,
    int? autoUpdateEpisodePeriod,
    bool? trimSilence,
    bool? volumeBoost,
    int? layout,
    String? pinepodsServer,
    String? pinepodsApiKey,
    int? pinepodsUserId,
  }) =>
      AppSettings(
        theme: theme ?? this.theme,
        markDeletedEpisodesAsPlayed: markDeletedEpisodesAsPlayed ?? this.markDeletedEpisodesAsPlayed,
        deleteDownloadedPlayedEpisodes: deleteDownloadedPlayedEpisodes ?? this.deleteDownloadedPlayedEpisodes,
        storeDownloadsSDCard: storeDownloadsSDCard ?? this.storeDownloadsSDCard,
        playbackSpeed: playbackSpeed ?? this.playbackSpeed,
        searchProvider: searchProvider ?? this.searchProvider,
        searchProviders: searchProviders ?? this.searchProviders,
        externalLinkConsent: externalLinkConsent ?? this.externalLinkConsent,
        autoOpenNowPlaying: autoOpenNowPlaying ?? this.autoOpenNowPlaying,
        showFunding: showFunding ?? this.showFunding,
        autoUpdateEpisodePeriod: autoUpdateEpisodePeriod ?? this.autoUpdateEpisodePeriod,
        trimSilence: trimSilence ?? this.trimSilence,
        volumeBoost: volumeBoost ?? this.volumeBoost,
        layout: layout ?? this.layout,
        pinepodsServer: pinepodsServer ?? this.pinepodsServer,
        pinepodsApiKey: pinepodsApiKey ?? this.pinepodsApiKey,
        pinepodsUserId: pinepodsUserId ?? this.pinepodsUserId,
      );
}
