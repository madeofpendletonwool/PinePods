## Title
Mobile: episode details and playback start are slow due to per-request HTTP connections with no timeout

## Status upstream
Not currently reported as far as I could find searching open/closed issues on this repo.

## Description

Loading an episode's detail page, and starting playback, both take noticeably longer than they should on the mobile app - sometimes several seconds before anything visible happens.

## Root cause

`PinepodsService` (`mobile/lib/services/pinepods/pinepods_service.dart`) never uses a persistent `http.Client`. Every one of its ~65 network calls used the package-level `http.get()` / `http.post()` / `http.put()` functions, each of which creates a brand-new `http.Client` and closes it immediately after the single request. That means every API call pays a full fresh TCP+TLS handshake instead of reusing a keep-alive connection - and **none of the calls had a timeout**, so a stalled request just hangs indefinitely with no feedback to the user.

On top of that, two hot paths made several of these calls **sequentially** when they didn't need to:

- `ui/pinepods/episode_details.dart` (`_loadEpisodeDetails`) awaited `getEpisodeMetadata` and then `fetchPodcasting2Data` one after another, even though neither depends on the other's result.
- `services/pinepods/pinepods_audio_service.dart` (`playPinepodsEpisode`, the method that runs every time Play is tapped) awaited `getPodcastIdFromEpisode` and `fetchPodcasting2Data` sequentially before the native player is even told to start - each paying full connection-setup cost on top of each other.

Combined, starting playback could involve 3-4+ sequential fresh-connection network calls before any audio starts, which is also a contributing factor to the "have to tap Play multiple times" experience (see the separate button-debounce issue).

## Fix

- `PinepodsService` now uses a single shared `http.Client` (reused across all instances) with a 15s timeout wrapped around every request.
- `_loadEpisodeDetails` and `playPinepodsEpisode` now fetch their independent data with `Future.wait` instead of sequential awaits.

## Tests

There was no existing unit test setup for the mobile app (no `test/` directory, `flutter_test`/`mockito` were unused dev dependencies). Added:

- `mobile/test/services/pinepods/pinepods_service_test.dart` - injects a fake `http.Client` (`package:http/testing.dart`) to verify a stalled request now times out instead of hanging forever, that normal responses still work, and that the client is reused across calls rather than recreated. Required making `PinepodsService`'s client/timeout constructor-injectable (defaults unchanged for all existing call sites).
- `mobile/test/services/pinepods/pinepods_audio_service_parallel_test.dart` - verifies `playPinepodsEpisode` fetches podcast id and podcast 2.0 data concurrently (bounded wall-clock time) rather than sequentially.

Hand-written mocks throughout, since this project has no `build_runner` setup for `@GenerateMocks` codegen. I couldn't run `flutter test` in the environment these changes were written in (no Flutter/Dart SDK available there) - please run it before merging.
