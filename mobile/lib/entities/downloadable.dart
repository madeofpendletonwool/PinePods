// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

enum DownloadState { none, queued, downloading, failed, cancelled, paused, downloaded }

/// A Downloadble is an object that holds information about a podcast episode
/// and its download status.
///
/// Downloadables can be used to determine if a download has been successful and
/// if an episode can be played from the filesystem.
class Downloadable {
  /// Database ID
  int? id;

  /// Unique identifier for the download
  final String guid;

  /// URL of the file to download
  final String url;

  /// Destination directory
  String directory;

  /// Name of file
  String filename;

  /// Current task ID for the download
  String taskId;

  /// Current state of the download
  DownloadState state;

  /// Percentage of MP3 downloaded
  int? percentage;

  Downloadable({
    required this.guid,
    required this.url,
    required this.directory,
    required this.filename,
    required this.taskId,
    required this.state,
    this.percentage,
  });

  Map<String, dynamic> toMap() {
    return <String, dynamic>{
      'guid': guid,
      'url': url,
      'filename': filename,
      'directory': directory,
      'taskId': taskId,
      'state': state.index,
      'percentage': percentage.toString(),
    };
  }

  static Downloadable fromMap(Map<String, dynamic> downloadable) {
    return Downloadable(
      guid: downloadable['guid'] as String,
      url: downloadable['url'] as String,
      directory: downloadable['directory'] as String,
      filename: downloadable['filename'] as String,
      taskId: downloadable['taskId'] as String,
      state: _determineState(downloadable['state'] as int?),
      percentage: int.parse(downloadable['percentage'] as String),
    );
  }

  static DownloadState _determineState(int? index) {
    switch (index) {
      case 0:
        return DownloadState.none;
      case 1:
        return DownloadState.queued;
      case 2:
        return DownloadState.downloading;
      case 3:
        return DownloadState.failed;
      case 4:
        return DownloadState.cancelled;
      case 5:
        return DownloadState.paused;
      case 6:
        return DownloadState.downloaded;
    }

    return DownloadState.none;
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) || other is Downloadable && runtimeType == other.runtimeType && guid == other.guid;

  @override
  int get hashCode => guid.hashCode;
}
