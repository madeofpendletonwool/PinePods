## Title
Mobile: queue never advances to the next episode when one finishes

## Relates to
- #690 "Playback jumps back upon finish" (iOS + Android, open) — the "occasionally it correctly jumps to the next episode in the queue but not reliably" part of that report is this same bug.
- #809 "[Feature Request] Auto play next episode" (open) — filed as a feature request, but auto-advance is already intended to exist; it's just broken.

## Description

On the mobile app (Android/iOS), when a playing episode finishes, playback just stops instead of advancing to the next episode in the Queue — even though the Queue tab shows episodes waiting.

## Root cause

The mobile app has two separate, disconnected queue implementations:

1. **Server-side queue** (the real one) — used by the Queue tab (`ui/pinepods/queue.dart`) and the Up Next view (`ui/podcast/pinepods_up_next_view.dart`, whose own doc comment says *"This replaces the local queue functionality with server-based queue management"*). Episodes are added/removed via `PinepodsService.queueEpisode()` / `removeQueuedEpisode()` and fetched with `getQueuedEpisodes()`.
2. **Local/legacy queue** — an in-memory list (`_queue` in `services/audio/native_audio_player_service.dart`), only ever populated through `addUpNextEpisode()`, whose only callers are in `bloc/podcast/queue_bloc.dart` — a bloc nothing in the current PinePods UI dispatches to anymore.

The auto-advance logic in `_handleCompletedEvent()` (`native_audio_player_service.dart`) only ever checked the local/legacy `_queue`. Since nothing populates it anymore, `_queue.isNotEmpty` is always false in practice, so playback stops instead of pulling the next episode from the server queue that the Queue tab actually shows.

This looks like a leftover from migrating the queue feature to be server-backed — the UI and add/remove paths were migrated, but episode-completion handling wasn't updated to match.

## Fix

`_handleCompletedEvent()` now falls back to the PinePods server queue (via a new `PinepodsAudioService.playNextFromServerQueue()`, mirroring the existing manual "tap to play from queue" flow in `queue.dart`) when the local legacy queue is empty: it fetches the next queued episode, removes it from the server queue, and plays it.

## Tests

There was no existing unit test setup for the mobile app (no `test/` directory, `flutter_test`/`mockito` were unused dev dependencies), so this adds the first one: `mobile/test/services/pinepods/pinepods_audio_service_test.dart`, covering `peekAndDequeueNextServerEpisode()` and `playNextFromServerQueue()` with hand-written mocks (no `build_runner` in this project yet, so no `@GenerateMocks` codegen). I couldn't run `flutter test` in the environment these changes were written in (no Flutter/Dart SDK available there) - please run it before merging.
