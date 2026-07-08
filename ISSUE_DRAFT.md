## Title
Mobile: playing a locally-downloaded episode with a missing file silently fails (mini player appears then disappears)

## Status upstream
Not currently reported as far as I could find searching open/closed issues on this repo.

## Description

Sometimes, tapping play on an episode that's downloaded locally makes the bottom mini player appear briefly and then vanish, with no error message. The workaround has been to delete the local download and try again (which streams instead).

## Root cause

`NativeAudioPlayerService._generateEpisodeUri()` (`mobile/lib/services/audio/native_audio_player_service.dart`) resolves a local file path for any episode whose `downloadState` is `DownloadState.downloaded`, but never checks that the file actually still exists there. The DB row can say "downloaded" while the file itself is gone - cleared by the OS under storage pressure, an SD card that's been swapped/unmounted, a changed storage root after an app update, or an interrupted download that never got marked as failed.

That dead path gets handed straight to the native player. On Android, `PinepodsMediaService.playEpisode()` builds a `MediaItem` from `Uri.fromFile(File(url))` and calls `player.prepare()`; ExoPlayer fails to open the missing file *asynchronously*, which fires `onPlayerError` → an `error` event over the method channel → `_handleErrorEvent()` sets `AudioState.error`. `MiniPlayer` (`mobile/lib/ui/podcast/mini_player.dart`) hides itself for `AudioState.stopped`, `.none`, **and `.error`** - so from the user's perspective the player just flashes and disappears, with nothing explaining why.

Sequence: Flutter sets `AudioState.buffering` (mini player appears) → invokes native `playEpisode` with the dead path → ExoPlayer fails asynchronously → `error` event → `AudioState.error` → mini player hides.

## Fix

`_generateEpisodeUri()` now checks the resolved path with `File(path).exists()` (and a non-zero length, to also catch an empty/corrupt file from an interrupted download) before handing it to the native player. If the file's missing/empty, it falls back to `episode.contentUrl` (always a valid streamable URL - already the case for episodes marked downloaded, verified via `PinepodsAudioService._convertToEpisode`) instead of failing, and repairs the stale download record so the episode goes back to showing as not-downloaded (matching what the user was already doing manually).

The episode object being played is sometimes a transient playback wrapper rather than the real repository row for its download (its guid is the stream URL, not `pinepods_<id>` - see the "transient playback record" comment already in `PinepodsAudioService._convertToEpisode`), so the repair matches by `filepath`/`filename` instead of guid to find and fix the actual underlying record(s) - this also self-heals any duplicate legacy-guid rows pointing at the same dead file, the same situation `LocalDownloadUtils.deleteLocalDownload` already handles for manual deletes.

## Tests

Extracted the record-matching/reset logic into pure functions (`mobile/lib/core/stale_download.dart`: `findStaleDownloadRecords`, `clearDownloadState`) so they're unit-testable without any file-system/repository/platform-channel scaffolding - `_generateEpisodeUri` itself is a private method on a class that talks to real platform channels and can't usefully be unit tested directly. `mobile/test/core/stale_download_test.dart` covers: matching by filepath+filename, catching duplicate/legacy-guid records for the same dead file, ignoring non-downloaded episodes even with a matching path, not mass-matching on null filepath/filename, and the reset itself.

## Not in scope here

`LocalDownloadUtils.isEpisodeDownloadedLocally()` (drives the "Downloaded" badge/icon across the UI) still only checks `downloadState`, not file presence, so other downloaded episodes with similarly dead files will keep showing as downloaded until you actually try to play them (at which point this fix self-heals that one). Proactively checking file existence for every downloaded episode (e.g. in the downloads list) is a separate, distinctly-scoped change with its own performance tradeoffs.

I couldn't run `flutter test` in the environment these changes were written in (no Flutter/Dart SDK available there) - please run it before merging.
