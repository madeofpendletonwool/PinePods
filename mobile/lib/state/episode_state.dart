// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/entities/episode.dart';

abstract class EpisodeState {
  final Episode episode;

  EpisodeState(this.episode);
}

class EpisodeUpdateState extends EpisodeState {
  EpisodeUpdateState(super.episode);
}

class EpisodeDeleteState extends EpisodeState {
  EpisodeDeleteState(super.episode);
}
