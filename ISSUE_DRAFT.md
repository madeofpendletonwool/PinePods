## Title
Mobile: episode action buttons (play, save, queue, download, complete) require multiple taps

## Status upstream
Not currently reported as far as I could find searching open/closed issues on this repo.

## Description

On the episode details page, buttons like Play, Save, Queue, Download, and Mark Complete often need to be tapped more than once before the action visibly registers.

## Root cause

None of the action handlers in `mobile/lib/ui/pinepods/episode_details.dart` (`_togglePlayPause`, `_toggleQueue`, `_toggleDownload`, `_toggleComplete`, `_saveEpisode`/`_removeSavedEpisode`, `_localDownloadEpisode`/`_deleteLocalDownload`) guard against being invoked again while a previous call is still in flight, and none of the buttons show any loading/disabled state during the awaited network call. Combined with the per-request network latency described in the companion HTTP-client issue - starting playback in particular can involve several sequential network calls before anything visible happens - a slow response invites the user to tap again, firing a second, overlapping request. Since handlers read the episode's current state (e.g. `_episode!.saved`) at call time, two overlapping taps can both see the same stale state and both perform the same action, occasionally leaving the toggle in the wrong state.

## Fix

Added a single guard: the first tap on any of these buttons disables all of them (via `onPressed: null`, which also grays them out for visual feedback) until that action's `Future` completes, then re-enables them. This prevents re-entrant taps from firing duplicate/overlapping requests and gives the user visible confirmation that something is happening.

The guard logic itself is extracted into a small, plain-Dart `ActionGuard` class (`mobile/lib/ui/utils/action_guard.dart`) rather than inlined as a bool + setState directly in the widget's State, so it's unit-testable without any widget/Provider scaffolding.

## Tests

There was no existing unit test setup for the mobile app (no `test/` directory, `flutter_test`/`mockito` were unused dev dependencies). Added `mobile/test/ui/utils/action_guard_test.dart`, covering: it starts out not in progress; it flips to in-progress synchronously as soon as `run()` starts (before the action resolves); a second call while one is in flight is dropped without running its action; `inProgress` resets (and the error still propagates) if the action throws; and a new action can run once the previous one finishes.

I couldn't run `flutter test` in the environment these changes were written in (no Flutter/Dart SDK available there) - please run it before merging.
