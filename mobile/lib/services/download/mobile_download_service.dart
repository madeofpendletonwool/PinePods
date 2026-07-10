// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'dart:async';
import 'dart:io';

import 'package:pinepods_mobile/core/utils.dart';
import 'package:pinepods_mobile/entities/downloadable.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/transcript.dart';
import 'package:pinepods_mobile/repository/repository.dart';
import 'package:pinepods_mobile/services/download/download_manager.dart';
import 'package:pinepods_mobile/services/download/download_service.dart';
import 'package:pinepods_mobile/services/podcast/podcast_service.dart';
import 'package:collection/collection.dart' show IterableExtension;
import 'package:logging/logging.dart';
import 'package:mp3_info/mp3_info.dart';
import 'package:rxdart/rxdart.dart';

/// An implementation of a [DownloadService] that handles downloading
/// of episodes on mobile.
class MobileDownloadService extends DownloadService {
  static BehaviorSubject<DownloadProgress> downloadProgress = BehaviorSubject<DownloadProgress>();

  final log = Logger('MobileDownloadService');
  final Repository repository;
  final DownloadManager downloadManager;
  final PodcastService podcastService;

  MobileDownloadService({required this.repository, required this.downloadManager, required this.podcastService}) {
    downloadManager.downloadProgress.pipe(downloadProgress);
    downloadProgress.listen((progress) {
      _updateDownloadProgress(progress);
    });
  }

  @override
  void dispose() {
    downloadManager.dispose();
  }

  @override
  Future<bool> downloadEpisode(Episode episode) async {
    try {
      final season = episode.season > 0 ? episode.season.toString() : '';
      final epno = episode.episode > 0 ? episode.episode.toString() : '';
      var dirty = false;

      if (await hasStoragePermission()) {
        // If this episode contains chapter, fetch them first.
        if (episode.hasChapters && episode.chaptersUrl != null) {
          var chapters = await podcastService.loadChaptersByUrl(url: episode.chaptersUrl!);

          episode.chapters = chapters;

          dirty = true;
        }

        // Next, if the episode supports transcripts download that next
        if (episode.hasTranscripts) {
          var sub = episode.transcriptUrls.firstWhereOrNull((element) => element.type == TranscriptFormat.json);

          sub ??= episode.transcriptUrls.firstWhereOrNull((element) => element.type == TranscriptFormat.subrip);
          
          sub ??= episode.transcriptUrls.firstWhereOrNull((element) => element.type == TranscriptFormat.html);

          if (sub != null) {
            var transcript = await podcastService.loadTranscriptByUrl(transcriptUrl: sub);

            transcript = await podcastService.saveTranscript(transcript);

            episode.transcript = transcript;
            episode.transcriptId = transcript.id;

            dirty = true;
          }
        }

        if (dirty) {
          await podcastService.saveEpisode(episode);
        }

        final episodePath = await resolveDirectory(episode: episode);
        final downloadPath = await resolveDirectory(episode: episode, full: true);
        var uri = Uri.parse(episode.contentUrl!);

        // Ensure the download directory exists
        await createDownloadDirectory(episode);

        // Filename should be last segment of URI.
        var filename = safeFile(uri.pathSegments.lastWhereOrNull((e) => e.toLowerCase().endsWith('.mp3')));

        filename ??= safeFile(uri.pathSegments.lastWhereOrNull((e) => e.toLowerCase().endsWith('.m4a')));

        if (filename == null) {
          //TODO: Handle unsupported format.
        } else {
          // The last segment could also be a full URL. Take a second pass.
          if (filename.contains('/')) {
            try {
              uri = Uri.parse(filename);
              filename = uri.pathSegments.last;
            } on FormatException {
              // It wasn't a URL...
            }
          }

          // Build a human-readable filename from the episode title + pub date so
          // that OS download notifications (which use the filename as their title)
          // show something meaningful instead of the raw URL segment.
          final ext = filename!.toLowerCase().endsWith('.m4a') ? '.m4a' : '.mp3';

          var safeTitle = (episode.title ?? '')
              .toLowerCase()
              .replaceAll(RegExp(r'[^a-z0-9\s-]'), '')
              .trim()
              .replaceAll(RegExp(r'\s+'), '-')
              .replaceAll(RegExp(r'-{2,}'), '-');
          if (safeTitle.length > 60) safeTitle = safeTitle.substring(0, 60);
          safeTitle = safeTitle.replaceAll(RegExp(r'-+$'), '');
          if (safeTitle.isEmpty) safeTitle = 'episode';

          var pubDatePrefix = '';
          if (episode.publicationDate != null) {
            final d = episode.publicationDate!;
            pubDatePrefix =
                '${d.year}-${d.month.toString().padLeft(2, '0')}-${d.day.toString().padLeft(2, '0')}-';
          }

          filename = '$season$epno$pubDatePrefix$safeTitle$ext';

          log.fine('Download episode (${episode.title}) $filename to $downloadPath/$filename');

          String url;
          if (episode.downloadUrl != null && episode.downloadUrl!.isNotEmpty) {
            // A server-copy download URL is already fully resolved (no feed
            // redirects) and may legitimately be http on a LAN server, so use
            // it as-is rather than resolving/forcing https.
            url = episode.downloadUrl!;
          } else {
            /// If we get a redirect to an http endpoint the download will fail. Let's fully resolve
            /// the URL before calling download and ensure it is https.
            url = await resolveUrl(episode.contentUrl!, forceHttps: true);
          }

          final taskId = await downloadManager.enqueueTask(url, downloadPath, filename);

          // Update the episode with download data
          episode.filepath = episodePath;
          episode.filename = filename;
          episode.downloadTaskId = taskId;
          episode.downloadState = DownloadState.downloading;
          episode.downloadPercentage = 0;

          await repository.saveEpisode(episode);

          return true;
        }
      }

      return false;
    } catch (e, stack) {
      log.warning('Episode download failed (${episode.title})', e, stack);
      return false;
    }
  }

  @override
  Future<Episode?> findEpisodeByTaskId(String taskId) {
    return repository.findEpisodeByTaskId(taskId);
  }

  Future<void> _updateDownloadProgress(DownloadProgress progress) async {
    var episode = await repository.findEpisodeByTaskId(progress.id);

    if (episode != null) {
      // We might be called during the cleanup routine during startup.
      // Do not bother updating if nothing has changed.
      if (episode.downloadPercentage != progress.percentage || episode.downloadState != progress.status) {
        episode.downloadPercentage = progress.percentage;
        episode.downloadState = progress.status;

        if (progress.percentage == 100) {
          if (await hasStoragePermission()) {
            final filename = await resolvePath(episode);

            // If we do not have a duration for this file - let's calculate it
            if (episode.duration == 0) {
              var mp3Info = MP3Processor.fromFile(File(filename));

              episode.duration = mp3Info.duration.inSeconds;
            }
          }
        }

        await repository.saveEpisode(episode);
      }
    }
  }
}
