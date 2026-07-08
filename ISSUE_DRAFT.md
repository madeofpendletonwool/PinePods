## Title
Mobile: Home tab doesn't refresh after the queue changes or playback starts

## Status upstream
Not currently reported as far as I could find searching open/closed issues on this repo.

## Description

The Home tab's stats ("Queue" count), "Continue Listening", and "Up Next" (queue preview) sections show a stale snapshot. They don't update after queuing/dequeuing an episode, after starting playback, or after an episode auto-advances to the next one in the queue - even when you're looking directly at the Home tab while it happens.

## Root cause

`_PinepodsHomeState._loadHomeContent()` (`mobile/lib/ui/pinepods/home.dart`) fetches `_homeData` from the server exactly once, in `initState()`. Nothing invalidated it afterwards:

- All navigation off Home (to Episode Details, etc.) used plain `Navigator.push(...)` with no callback on return, so nothing refreshed when you came back.
- Actions taken directly on Home (e.g. `_toggleQueueEpisode`) only patched the tapped episode's own `queued` flag in the already-loaded list (so that one card's icon updates), but never touched `_homeData.queuePreview` or `.queueCount` - a separate part of the same snapshot.
- Nothing on the page subscribed to the audio service at all, so starting playback anywhere (mini player, Episode Details, Android Auto, or the server-queue auto-advance added in a companion fix) never signalled Home to update.

## Fix

- Added `_refreshHomeContentSilently()`: re-fetches the same data `_loadHomeContent()` does, but without flipping `_isLoading` (so the page doesn't flash back to a loading spinner - the old content stays visible/usable until the refreshed data replaces it, or is kept as-is if the background refresh fails).
- Subscribed to `AudioBloc.nowPlaying` in `didChangeDependencies`; whenever the now-playing episode's guid changes, trigger a silent refresh. This covers playback started from anywhere, including the server-queue auto-advance case, without needing to know which screen triggered it.
- Added `.then((_) => _refreshHomeContentSilently())` to the three `Navigator.push` calls that go to Episode Details from Home.
- Added a silent refresh after `_toggleQueueEpisode` succeeds, so the "Up Next" preview and Queue count catch up even for a queue toggle performed right there on Home.

## Tests

No automated test added for this one. The new logic is thin UI wiring (a guid-equality check, plus calling the same `getHomeOverview`/`getPlaylists` fetch `_loadHomeContent` already uses, from a few more trigger points) rather than a self-contained algorithm - the previous fixes in this series that gained unit tests (queue dequeue selection, HTTP timeout, button debounce) all had non-trivial logic worth isolating from Flutter's widget/Provider machinery. Here a meaningful test would mean a full widget test mocking `PinepodsService`'s network layer inside a `MultiProvider` tree, which felt disproportionate to what's actually new. Verified by tracing the code paths by hand instead - happy to add a widget test if you'd rather have one before merging.
