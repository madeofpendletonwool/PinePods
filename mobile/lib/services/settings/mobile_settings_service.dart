// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'dart:math';

import 'package:pinepods_mobile/core/environment.dart';
import 'package:pinepods_mobile/entities/app_settings.dart';
import 'package:pinepods_mobile/services/settings/settings_service.dart';
import 'package:rxdart/rxdart.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// An implementation [SettingsService] for mobile devices backed by
/// shared preferences.
class MobileSettingsService extends SettingsService {
  static late SharedPreferences _sharedPreferences;
  static MobileSettingsService? _instance;

  final settingsNotifier = PublishSubject<String>();

  MobileSettingsService._create();

  static Future<MobileSettingsService?> instance() async {
    if (_instance == null) {
      _instance = MobileSettingsService._create();

      _sharedPreferences = await SharedPreferences.getInstance();
    }

    return _instance;
  }

  @override
  bool get markDeletedEpisodesAsPlayed => _sharedPreferences.getBool('markplayedasdeleted') ?? false;

  @override
  set markDeletedEpisodesAsPlayed(bool value) {
    _sharedPreferences.setBool('markplayedasdeleted', value);
    settingsNotifier.sink.add('markplayedasdeleted');
  }

  @override
  String? get pinepodsServer => _sharedPreferences.getString('pinepods_server');

  @override
  set pinepodsServer(String? value) {
    if (value == null) {
      _sharedPreferences.remove('pinepods_server');
    } else {
      _sharedPreferences.setString('pinepods_server', value);
    }
    settingsNotifier.sink.add('pinepods_server');
  }

  @override
  String? get pinepodsApiKey => _sharedPreferences.getString('pinepods_api_key');

  @override
  set pinepodsApiKey(String? value) {
    if (value == null) {
      _sharedPreferences.remove('pinepods_api_key');
    } else {
      _sharedPreferences.setString('pinepods_api_key', value);
    }
    settingsNotifier.sink.add('pinepods_api_key');
  }

  @override
  int? get pinepodsUserId => _sharedPreferences.getInt('pinepods_user_id');

  @override
  set pinepodsUserId(int? value) {
    if (value == null) {
      _sharedPreferences.remove('pinepods_user_id');
    } else {
      _sharedPreferences.setInt('pinepods_user_id', value);
    }
    settingsNotifier.sink.add('pinepods_user_id');
  }

  @override
  bool get deleteDownloadedPlayedEpisodes => _sharedPreferences.getBool('deleteDownloadedPlayedEpisodes') ?? false;

  @override
  set deleteDownloadedPlayedEpisodes(bool value) {
    _sharedPreferences.setBool('deleteDownloadedPlayedEpisodes', value);
    settingsNotifier.sink.add('deleteDownloadedPlayedEpisodes');
  }

  @override
  bool get storeDownloadsSDCard => _sharedPreferences.getBool('savesdcard') ?? false;

  @override
  set storeDownloadsSDCard(bool value) {
    _sharedPreferences.setBool('savesdcard', value);
    settingsNotifier.sink.add('savesdcard');
  }

  @override
  bool get themeDarkMode {
    var theme = _sharedPreferences.getString('theme') ?? 'dark';

    return theme == 'dark';
  }

  @override
  set themeDarkMode(bool value) {
    _sharedPreferences.setString('theme', value ? 'dark' : 'light');
    settingsNotifier.sink.add('theme');
  }

  @override
  set playbackSpeed(double playbackSpeed) {
    _sharedPreferences.setDouble('speed', playbackSpeed);
    settingsNotifier.sink.add('speed');
  }

  @override
  double get playbackSpeed {
    var speed = _sharedPreferences.getDouble('speed') ?? 1.0;

    // We used to use 0.25 increments and now we use 0.1. Round
    // any setting that uses the old 0.25.
    var mod = pow(10.0, 1).toDouble();
    return ((speed * mod).round().toDouble() / mod);
  }

  @override
  set searchProvider(String provider) {
    _sharedPreferences.setString('search', provider);
    settingsNotifier.sink.add('search');
  }

  @override
  String get searchProvider {
    // If we do not have PodcastIndex key, fallback to iTunes
    if (podcastIndexKey.isEmpty) {
      return 'itunes';
    } else {
      return _sharedPreferences.getString('search') ?? 'itunes';
    }
  }

  @override
  set externalLinkConsent(bool consent) {
    _sharedPreferences.setBool('elconsent', consent);
    settingsNotifier.sink.add('elconsent');
  }

  @override
  bool get externalLinkConsent {
    return _sharedPreferences.getBool('elconsent') ?? false;
  }

  @override
  set autoOpenNowPlaying(bool autoOpenNowPlaying) {
    _sharedPreferences.setBool('autoopennowplaying', autoOpenNowPlaying);
    settingsNotifier.sink.add('autoopennowplaying');
  }

  @override
  bool get autoOpenNowPlaying {
    return _sharedPreferences.getBool('autoopennowplaying') ?? false;
  }

  @override
  set showFunding(bool show) {
    _sharedPreferences.setBool('showFunding', show);
    settingsNotifier.sink.add('showFunding');
  }

  @override
  bool get showFunding {
    return _sharedPreferences.getBool('showFunding') ?? true;
  }

  @override
  set autoUpdateEpisodePeriod(int period) {
    _sharedPreferences.setInt('autoUpdateEpisodePeriod', period);
    settingsNotifier.sink.add('autoUpdateEpisodePeriod');
  }

  @override
  int get autoUpdateEpisodePeriod {
    /// Default to 3 hours.
    return _sharedPreferences.getInt('autoUpdateEpisodePeriod') ?? 180;
  }

  @override
  set trimSilence(bool trim) {
    _sharedPreferences.setBool('trimSilence', trim);
    settingsNotifier.sink.add('trimSilence');
  }

  @override
  bool get trimSilence {
    return _sharedPreferences.getBool('trimSilence') ?? false;
  }

  @override
  set volumeBoost(bool boost) {
    _sharedPreferences.setBool('volumeBoost', boost);
    settingsNotifier.sink.add('volumeBoost');
  }

  @override
  bool get volumeBoost {
    return _sharedPreferences.getBool('volumeBoost') ?? false;
  }

  @override
  set layoutMode(int mode) {
    _sharedPreferences.setInt('layout', mode);
    settingsNotifier.sink.add('layout');
  }

  @override
  int get layoutMode {
    return _sharedPreferences.getInt('layout') ?? 0;
  }

  @override
  AppSettings? settings;

  @override
  Stream<String> get settingsListener => settingsNotifier.stream;
}
