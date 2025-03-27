// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'dart:async';

import 'package:pinepods_mobile/entities/downloadable.dart';

class DownloadProgress {
  final String id;
  final int percentage;
  final DownloadState status;

  DownloadProgress(
    this.id,
    this.percentage,
    this.status,
  );
}

abstract class DownloadManager {
  Future<String?> enqueueTask(String url, String downloadPath, String fileName);

  Stream<DownloadProgress> get downloadProgress;

  void dispose();
}
