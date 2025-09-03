// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'dart:io';

/// The key required when searching via PodcastIndex.org.
const podcastIndexKey = String.fromEnvironment('PINDEX_KEY', defaultValue: '');

/// The secret required when searching via PodcastIndex.org.
const podcastIndexSecret = String.fromEnvironment(
  'PINDEX_SECRET',
  defaultValue: '',
);

/// Allows a user to override the default user agent string.
const userAgentAppString = String.fromEnvironment(
  'USER_AGENT',
  defaultValue: '',
);

/// Link to a feedback form. This will be shown in the main overflow menu if set
const feedbackUrl = String.fromEnvironment('FEEDBACK_URL', defaultValue: '');

/// This class stores version information for PinePods, including project version and
/// build number. This is then used for user agent strings when interacting with
/// APIs and RSS feeds.
///
/// The user agent string can be overridden by passing in the USER_AGENT variable
/// using dart-define.
class Environment {
  static const _applicationName = 'Pinepods';
  static const _applicationUrl =
      'https://github.com/madeofpendletonwool/pinepods';
  static const _projectVersion = '0.8.0';
  static const _build = '20252164';

  static var _agentString = userAgentAppString;

  static String userAgent() {
    if (_agentString.isEmpty) {
      var platform =
          '${Platform.operatingSystem} ${Platform.operatingSystemVersion}'
              .trim();

      _agentString =
          '$_applicationName/$_projectVersion b$_build (phone;$platform) $_applicationUrl';
    }

    return _agentString;
  }

  static String get projectVersion => '$_projectVersion b$_build';
}
