// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/entities/app_settings.dart';

abstract class SettingsService {
  AppSettings? get settings;

  set settings(AppSettings? settings);

  bool get themeDarkMode;

  set themeDarkMode(bool value);

  String get theme;

  set theme(String value);

  bool get markDeletedEpisodesAsPlayed;

  set markDeletedEpisodesAsPlayed(bool value);

  bool get deleteDownloadedPlayedEpisodes;

  set deleteDownloadedPlayedEpisodes(bool value);

  bool get storeDownloadsSDCard;

  set storeDownloadsSDCard(bool value);

  set playbackSpeed(double playbackSpeed);

  double get playbackSpeed;

  set searchProvider(String provider);

  String get searchProvider;

  set externalLinkConsent(bool consent);

  bool get externalLinkConsent;

  set autoOpenNowPlaying(bool autoOpenNowPlaying);

  bool get autoOpenNowPlaying;

  set showFunding(bool show);

  bool get showFunding;

  set autoUpdateEpisodePeriod(int period);

  int get autoUpdateEpisodePeriod;

  set trimSilence(bool trim);

  bool get trimSilence;

  set volumeBoost(bool boost);

  bool get volumeBoost;

  set fastForwardInterval(int seconds);

  int get fastForwardInterval;

  set rewindInterval(int seconds);

  int get rewindInterval;

  set layoutMode(int mode);

  int get layoutMode;

  /// Only run automatic downloads (per-podcast auto-download, queue auto-download
  /// and server mirror) while connected to WiFi. Manual downloads are unaffected.
  bool get autoDownloadWifiOnly;
  set autoDownloadWifiOnly(bool value);

  /// When an episode already exists as a server download, pull the bytes from the
  /// server's copy instead of the original feed URL. Falls back to the source URL
  /// when the server does not have it.
  bool get preferServerDownloadSource;
  set preferServerDownloadSource(bool value);

  /// Number of leading queue episodes to keep downloaded locally. 0 disables the
  /// feature.
  int get autoDownloadQueueCount;
  set autoDownloadQueueCount(int value);

  /// Keep the device's local downloads mirrored to the server's downloaded
  /// episodes (two-way: add new, prune removed).
  bool get mirrorServerDownloads;
  set mirrorServerDownloads(bool value);

  Stream<String> get settingsListener;

  String? get pinepodsServer;
  set pinepodsServer(String? value);

  String? get pinepodsApiKey;
  set pinepodsApiKey(String? value);

  int? get pinepodsUserId;
  set pinepodsUserId(int? value);

  String? get pinepodsUsername;
  set pinepodsUsername(String? value);

  String? get pinepodsEmail;
  set pinepodsEmail(String? value);

  List<String> get bottomBarOrder;
  set bottomBarOrder(List<String> value);
}
