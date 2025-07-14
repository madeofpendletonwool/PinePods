// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/bloc/bloc.dart';
import 'package:pinepods_mobile/core/environment.dart';
import 'package:pinepods_mobile/entities/app_settings.dart';
import 'package:pinepods_mobile/entities/search_providers.dart';
import 'package:pinepods_mobile/services/settings/settings_service.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:logging/logging.dart';
import 'package:rxdart/rxdart.dart';

class SettingsBloc extends Bloc {
  final log = Logger('SettingsBloc');
  final SettingsService _settingsService;
  final BehaviorSubject<AppSettings> _settings = BehaviorSubject<AppSettings>.seeded(AppSettings.sensibleDefaults());
  final BehaviorSubject<bool> _darkMode = BehaviorSubject<bool>();
  final BehaviorSubject<String> _theme = BehaviorSubject<String>();
  final BehaviorSubject<bool> _markDeletedAsPlayed = BehaviorSubject<bool>();
  final BehaviorSubject<bool> _deleteDownloadedPlayedEpisodes = BehaviorSubject<bool>();
  final BehaviorSubject<bool> _storeDownloadOnSDCard = BehaviorSubject<bool>();
  final BehaviorSubject<double> _playbackSpeed = BehaviorSubject<double>();
  final BehaviorSubject<String> _searchProvider = BehaviorSubject<String>();
  final BehaviorSubject<bool> _externalLinkConsent = BehaviorSubject<bool>();
  final BehaviorSubject<bool> _autoOpenNowPlaying = BehaviorSubject<bool>();
  final BehaviorSubject<bool> _showFunding = BehaviorSubject<bool>();
  final BehaviorSubject<bool> _trimSilence = BehaviorSubject<bool>();
  final BehaviorSubject<bool> _volumeBoost = BehaviorSubject<bool>();
  final BehaviorSubject<int> _autoUpdatePeriod = BehaviorSubject<int>();
  final BehaviorSubject<int> _layoutMode = BehaviorSubject<int>();
  final BehaviorSubject<String?> _pinepodsServer = BehaviorSubject<String?>();
  final BehaviorSubject<String?> _pinepodsApiKey = BehaviorSubject<String?>();
  final BehaviorSubject<int?> _pinepodsUserId = BehaviorSubject<int?>();
  final BehaviorSubject<String?> _pinepodsUsername = BehaviorSubject<String?>();
  final BehaviorSubject<String?> _pinepodsEmail = BehaviorSubject<String?>();
  final BehaviorSubject<List<String>> _bottomBarOrder = BehaviorSubject<List<String>>();
  var _currentSettings = AppSettings.sensibleDefaults();

  SettingsBloc(this._settingsService) {
    _init();
    // Check if we need to fetch user details for existing login
    _fetchUserDetailsIfNeeded();
  }

  Future<void> _fetchUserDetailsIfNeeded() async {
    // Only fetch if we have server/api key but no username
    if (_currentSettings.pinepodsServer != null && 
        _currentSettings.pinepodsApiKey != null &&
        (_currentSettings.pinepodsUsername == null || _currentSettings.pinepodsUsername!.isEmpty)) {
      
      try {
        final pinepodsService = PinepodsService();
        pinepodsService.setCredentials(_currentSettings.pinepodsServer!, _currentSettings.pinepodsApiKey!);
        
        // Use stored user ID if available, otherwise we need to get it somehow
        final userId = _currentSettings.pinepodsUserId;
        print('DEBUG: User ID from settings: $userId');
        if (userId != null) {
          final userDetails = await pinepodsService.getUserDetails(userId);
          print('DEBUG: User details response: $userDetails');
          if (userDetails != null) {
            // Update settings with user details
            final username = userDetails['Username'] ?? userDetails['username'] ?? '';
            final email = userDetails['Email'] ?? userDetails['email'] ?? '';
            print('DEBUG: Parsed username: "$username", email: "$email"');
            setPinepodsUsername(username);
            setPinepodsEmail(email);
          }
        }
      } catch (e) {
        // Silently fail - don't break the app if this fails
        print('Failed to fetch user details on startup: $e');
      }
    }
  }

  void _init() {
    /// Load all settings
    // Add our available search providers.
    var providers = <SearchProvider>[SearchProvider(key: 'itunes', name: 'iTunes')];

    if (podcastIndexKey.isNotEmpty) {
      providers.add(SearchProvider(key: 'podcastindex', name: 'PodcastIndex'));
    }

    _currentSettings = AppSettings(
      theme: _settingsService.theme,
      markDeletedEpisodesAsPlayed: _settingsService.markDeletedEpisodesAsPlayed,
      deleteDownloadedPlayedEpisodes: _settingsService.deleteDownloadedPlayedEpisodes,
      storeDownloadsSDCard: _settingsService.storeDownloadsSDCard,
      playbackSpeed: _settingsService.playbackSpeed,
      searchProvider: _settingsService.searchProvider,
      searchProviders: providers,
      externalLinkConsent: _settingsService.externalLinkConsent,
      autoOpenNowPlaying: _settingsService.autoOpenNowPlaying,
      showFunding: _settingsService.showFunding,
      autoUpdateEpisodePeriod: _settingsService.autoUpdateEpisodePeriod,
      trimSilence: _settingsService.trimSilence,
      volumeBoost: _settingsService.volumeBoost,
      layout: _settingsService.layoutMode,
      pinepodsServer: _settingsService.pinepodsServer,
      pinepodsApiKey: _settingsService.pinepodsApiKey,
      pinepodsUserId: _settingsService.pinepodsUserId,
      pinepodsUsername: _settingsService.pinepodsUsername,
      pinepodsEmail: _settingsService.pinepodsEmail,
      bottomBarOrder: _settingsService.bottomBarOrder,
    );

    _settings.add(_currentSettings);

    _darkMode.listen((bool darkMode) {
      _currentSettings = _currentSettings.copyWith(theme: darkMode ? 'Dark' : 'Light');
      _settings.add(_currentSettings);
      _settingsService.themeDarkMode = darkMode;
    });

    _theme.listen((String theme) {
      _currentSettings = _currentSettings.copyWith(theme: theme);
      _settings.add(_currentSettings);
      _settingsService.theme = theme;
      
      // Sync with server if authenticated
      _syncThemeToServer(theme);
    });

    _markDeletedAsPlayed.listen((bool mark) {
      _currentSettings = _currentSettings.copyWith(markDeletedEpisodesAsPlayed: mark);
      _settings.add(_currentSettings);
      _settingsService.markDeletedEpisodesAsPlayed = mark;
    });

    _deleteDownloadedPlayedEpisodes.listen((bool delete) {
      _currentSettings = _currentSettings.copyWith(deleteDownloadedPlayedEpisodes: delete);
      _settings.add(_currentSettings);
      _settingsService.deleteDownloadedPlayedEpisodes = delete;
    });

    _storeDownloadOnSDCard.listen((bool sdcard) {
      _currentSettings = _currentSettings.copyWith(storeDownloadsSDCard: sdcard);
      _settings.add(_currentSettings);
      _settingsService.storeDownloadsSDCard = sdcard;
    });

    _playbackSpeed.listen((double speed) {
      _currentSettings = _currentSettings.copyWith(playbackSpeed: speed);
      _settings.add(_currentSettings);
      _settingsService.playbackSpeed = speed;
    });

    _autoOpenNowPlaying.listen((bool autoOpen) {
      _currentSettings = _currentSettings.copyWith(autoOpenNowPlaying: autoOpen);
      _settings.add(_currentSettings);
      _settingsService.autoOpenNowPlaying = autoOpen;
    });

    _showFunding.listen((show) {
      // If the setting has not changed, don't bother updating it
      if (show != _currentSettings.showFunding) {
        _currentSettings = _currentSettings.copyWith(showFunding: show);
        _settingsService.showFunding = show;
      }

      _settings.add(_currentSettings);
    });

    _searchProvider.listen((search) {
      _currentSettings = _currentSettings.copyWith(searchProvider: search);
      _settings.add(_currentSettings);
      _settingsService.searchProvider = search;
    });

    _externalLinkConsent.listen((consent) {
      // If the setting has not changed, don't bother updating it
      if (consent != _settingsService.externalLinkConsent) {
        _currentSettings = _currentSettings.copyWith(externalLinkConsent: consent);
        _settingsService.externalLinkConsent = consent;
      }

      _settings.add(_currentSettings);
    });

    _autoUpdatePeriod.listen((period) {
      _currentSettings = _currentSettings.copyWith(autoUpdateEpisodePeriod: period);
      _settings.add(_currentSettings);
      _settingsService.autoUpdateEpisodePeriod = period;
    });

    _trimSilence.listen((trim) {
      _currentSettings = _currentSettings.copyWith(trimSilence: trim);
      _settings.add(_currentSettings);
      _settingsService.trimSilence = trim;
    });

    _volumeBoost.listen((boost) {
      _currentSettings = _currentSettings.copyWith(volumeBoost: boost);
      _settings.add(_currentSettings);
      _settingsService.volumeBoost = boost;
    });

    _pinepodsServer.listen((server) {
      _currentSettings = _currentSettings.copyWith(pinepodsServer: server);
      _settings.add(_currentSettings);
      _settingsService.pinepodsServer = server;
    });

    _pinepodsApiKey.listen((apiKey) {
      _currentSettings = _currentSettings.copyWith(pinepodsApiKey: apiKey);
      _settings.add(_currentSettings);
      _settingsService.pinepodsApiKey = apiKey;
    });

    _pinepodsUserId.listen((userId) {
      _currentSettings = _currentSettings.copyWith(pinepodsUserId: userId);
      _settings.add(_currentSettings);
      _settingsService.pinepodsUserId = userId;
    });

    _pinepodsUsername.listen((username) {
      _currentSettings = _currentSettings.copyWith(pinepodsUsername: username);
      _settings.add(_currentSettings);
      _settingsService.pinepodsUsername = username;
    });

    _pinepodsEmail.listen((email) {
      _currentSettings = _currentSettings.copyWith(pinepodsEmail: email);
      _settings.add(_currentSettings);
      _settingsService.pinepodsEmail = email;
    });

    _layoutMode.listen((mode) {
      _currentSettings = _currentSettings.copyWith(layout: mode);
      _settings.add(_currentSettings);
      _settingsService.layoutMode = mode;
    });

    _bottomBarOrder.listen((order) {
      _currentSettings = _currentSettings.copyWith(bottomBarOrder: order);
      _settings.add(_currentSettings);
      _settingsService.bottomBarOrder = order;
    });
  }

  Stream<AppSettings> get settings => _settings.stream;

  void Function(bool) get darkMode => _darkMode.add;

  void Function(bool) get storeDownloadonSDCard => _storeDownloadOnSDCard.add;

  void Function(bool) get markDeletedAsPlayed => _markDeletedAsPlayed.add;

  void Function(bool) get deleteDownloadedPlayedEpisodes => _deleteDownloadedPlayedEpisodes.add;

  void Function(double) get setPlaybackSpeed => _playbackSpeed.add;

  void Function(bool) get setAutoOpenNowPlaying => _autoOpenNowPlaying.add;

  void Function(String) get setSearchProvider => _searchProvider.add;

  void Function(bool) get setExternalLinkConsent => _externalLinkConsent.add;

  void Function(bool) get setShowFunding => _showFunding.add;

  void Function(int) get autoUpdatePeriod => _autoUpdatePeriod.add;

  void Function(bool) get trimSilence => _trimSilence.add;

  void Function(bool) get volumeBoost => _volumeBoost.add;

  void Function(int) get layoutMode => _layoutMode.add;

  void Function(String?) get setPinepodsServer => _pinepodsServer.add;

  void Function(String?) get setPinepodsApiKey => _pinepodsApiKey.add;

  void Function(int?) get setPinepodsUserId => _pinepodsUserId.add;

  void Function(String?) get setPinepodsUsername => _pinepodsUsername.add;

  void Function(String?) get setPinepodsEmail => _pinepodsEmail.add;

  void Function(List<String>) get setBottomBarOrder => _bottomBarOrder.add;

  void Function(String) get setTheme => _theme.add;

  AppSettings get currentSettings => _settings.value;

  Future<void> _syncThemeToServer(String theme) async {
    try {
      // Only sync if we have PinePods credentials
      if (_currentSettings.pinepodsServer != null &&
          _currentSettings.pinepodsApiKey != null &&
          _currentSettings.pinepodsUserId != null) {
        
        final pinepodsService = PinepodsService();
        pinepodsService.setCredentials(
          _currentSettings.pinepodsServer!,
          _currentSettings.pinepodsApiKey!,
        );
        
        await pinepodsService.setUserTheme(_currentSettings.pinepodsUserId!, theme);
        log.info('Theme synced to server: $theme');
      }
    } catch (e) {
      log.warning('Failed to sync theme to server: $e');
      // Don't throw - theme should still work locally
    }
  }

  Future<void> fetchThemeFromServer() async {
    try {
      // Only fetch if we have PinePods credentials
      if (_currentSettings.pinepodsServer != null &&
          _currentSettings.pinepodsApiKey != null &&
          _currentSettings.pinepodsUserId != null) {
        
        final pinepodsService = PinepodsService();
        pinepodsService.setCredentials(
          _currentSettings.pinepodsServer!,
          _currentSettings.pinepodsApiKey!,
        );
        
        final serverTheme = await pinepodsService.getUserTheme(_currentSettings.pinepodsUserId!);
        if (serverTheme != null && serverTheme.isNotEmpty) {
          // Update local theme without syncing back to server
          _settingsService.theme = serverTheme;
          _currentSettings = _currentSettings.copyWith(theme: serverTheme);
          _settings.add(_currentSettings);
          log.info('Theme fetched from server: $serverTheme');
        }
      }
    } catch (e) {
      log.warning('Failed to fetch theme from server: $e');
      // Don't throw - continue with local theme
    }
  }

  @override
  void dispose() {
    _darkMode.close();
    _theme.close();
    _markDeletedAsPlayed.close();
    _deleteDownloadedPlayedEpisodes.close();
    _storeDownloadOnSDCard.close();
    _playbackSpeed.close();
    _searchProvider.close();
    _externalLinkConsent.close();
    _autoOpenNowPlaying.close();
    _showFunding.close();
    _trimSilence.close();
    _volumeBoost.close();
    _autoUpdatePeriod.close();
    _layoutMode.close();
    _pinepodsServer.close();
    _pinepodsApiKey.close();
    _pinepodsUserId.close();
    _pinepodsUsername.close();
    _pinepodsEmail.close();
    _bottomBarOrder.close();
    _settings.close();
  }
}
