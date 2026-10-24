// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/bloc/bloc.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/services/download/download_service.dart';
import 'package:pinepods_mobile/services/podcast/podcast_service.dart';
import 'package:pinepods_mobile/services/settings/settings_service.dart';

/// Gives screens access to the on-device podcast and download services
/// (local downloads, feeds of podcasts that aren't followed).
class PodcastBloc extends Bloc {
  final PodcastService podcastService;
  final AudioPlayerService audioPlayerService;
  final DownloadService downloadService;
  final SettingsService settingsService;

  PodcastBloc({
    required this.podcastService,
    required this.audioPlayerService,
    required this.downloadService,
    required this.settingsService,
  });
}
