// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/state/opml_state.dart';

/// This service handles the import and export of Podcasts via
/// the OPML format.
abstract class OPMLService {
  Stream<OPMLState> loadOPMLFile(String file);

  Stream<OPMLState> saveOPMLFile();

  void cancel();
}
