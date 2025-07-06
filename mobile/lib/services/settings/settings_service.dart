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

  set layoutMode(int mode);

  int get layoutMode;

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
}
