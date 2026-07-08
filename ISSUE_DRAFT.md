## Title
Mobile: the "Downloaded" badge doesn't notice when a local file is actually missing

## Status upstream
Not currently reported as far as I could find searching open/closed issues on this repo. Follow-up to a companion fix for playback of a locally-downloaded episode whose file is missing (`analyze/local-download-playback`), which self-heals a stale download record the moment you try to play it - this addresses the same underlying issue proactively, for episodes you haven't tried to play yet.

## Description

Episodes can keep showing a "Downloaded" badge/icon (in Episode Details, Home, Queue, Saved, Feed, History, podcast details, etc.) even after the local file backing that download is gone - cleared by the OS under storage pressure, an SD card swap/unmount, a changed storage root after an app update, or an interrupted download that never got marked as failed.

## Root cause

`LocalDownloadUtils.isEpisodeDownloadedLocally()` / `.loadLocalDownloadStatuses()` (`mobile/lib/ui/utils/local_download_utils.dart`) - the shared source of truth the whole app checks for this badge - only ever looked at the repository's `downloadState`/`downloaded` fields, never whether the file is actually still on disk. Both are also cached (`_localDownloadStatusCache`) for the lifetime of a page view, so even a manual pull-to-refresh wouldn't have caught this.

## Fix

Both functions now route through a shared `_resolvePresenceAndHeal()` helper: for every matching repository row marked downloaded, it checks the file actually exists (and is non-empty) via `File.exists()`/`.length()`, and resets any row whose file is missing back to "not downloaded" - so the badge becomes accurate and the record won't be checked again as if it were still a valid download. This runs once per episode per cache lifetime, same cost profile as the existing DB-only check (a single cheap file-metadata stat, not a content read), just more accurate.

The actual decision logic - "given a set of possibly-duplicate rows for one episode and which of them were found on disk, is this episode really downloaded, and which rows need resetting" - is extracted into a pure `resolveDownloadPresence()` function (`mobile/lib/core/download_presence.dart`) so it's unit-testable without any file-system I/O; the file check itself is a thin, untested async wrapper around it.

## Tests

Added `mobile/test/core/download_presence_test.dart` covering: presence via a single matching row, marking a missing row as stale, counting the episode as downloaded when *any* duplicate/legacy-guid row is present even while healing a dead duplicate alongside it, ignoring non-downloaded rows, and defensively treating a downloaded row missing from the presence map as stale rather than assuming it's fine.

I couldn't run `flutter test` in the environment these changes were written in (no Flutter/Dart SDK available there) - please run it before merging.
