// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:flutter/material.dart';
import 'package:pinepods_mobile/entities/app_settings.dart';
import 'package:pinepods_mobile/ui/podcast/now_playing.dart';

/// If we have the 'show now playing upon play' option set to true, launch
/// the [NowPlaying] widget automatically.
void optionalShowNowPlaying(BuildContext context, AppSettings settings) {
  if (settings.autoOpenNowPlaying) {
    Navigator.push(
      context,
      MaterialPageRoute<void>(
        builder: (context) => const NowPlaying(),
        settings: const RouteSettings(name: 'nowplaying'),
        fullscreenDialog: false,
      ),
    );
  }
}