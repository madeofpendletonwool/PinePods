// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:flutter/material.dart';
import 'package:pinepods_mobile/entities/app_settings.dart';
import 'package:pinepods_mobile/ui/podcast/now_playing.dart';
import 'package:pinepods_mobile/bloc/settings/settings_bloc.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_audio_service.dart';
import 'package:provider/provider.dart';

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

/// Helper function to play a PinePods episode and automatically show the full screen player if enabled
Future<void> playPinepodsEpisodeWithOptionalFullScreen(
  BuildContext context,
  PinepodsAudioService audioService,
  PinepodsEpisode episode, {
  bool resume = true,
}) async {
  await audioService.playPinepodsEpisode(
    pinepodsEpisode: episode,
    resume: resume,
  );
  
  // Show full screen player if setting is enabled
  final settingsBloc = Provider.of<SettingsBloc>(context, listen: false);
  optionalShowNowPlaying(context, settingsBloc.currentSettings);
}