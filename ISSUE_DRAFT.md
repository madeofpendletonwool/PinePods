## Title
Mobile: the now-playing episode's progress bar on Home never updates

## Status upstream
Not currently reported as far as I could find searching open/closed issues on this repo.

## Description

On the Home tab, the "Continue Listening"/"Recent Episodes" card for the episode that's actually playing right now shows a progress bar and elapsed-time text that never moves - it stays frozen at whatever position it was at when Home last loaded, even while the episode keeps playing.

## Root cause

`_EpisodeCardState` (`mobile/lib/ui/pinepods/home.dart`) already subscribes to `AudioBloc.nowPlaying`/`.playingState` so its play/pause icon correctly reflects live playback state, but it never subscribed to `AudioBloc.playPosition` - the `ValueStream<PositionState>` that carries live position ticks. The progress bar and duration text read only `widget.episode.progressPercentage`/`.formattedListenDuration`, both derived from the static snapshot Home fetched once from the server.

`mini_player.dart` already does this correctly with a `StreamBuilder<PositionState>` on `audioBloc.playPosition` - the pattern to copy already existed elsewhere in the codebase.

## Fix

- Subscribed `_EpisodeCardState` to `AudioBloc.playPosition`.
- Extracted the "live value vs. static snapshot" decision into a small, pure `LiveProgressResolver` class (`mobile/lib/ui/utils/live_progress.dart`) rather than inlining it in the widget, so it's unit-testable without any widget/Provider/stream scaffolding: it uses the live position while this card is the one actually playing, and falls back to the static snapshot otherwise (including before any live tick has arrived yet).
- The progress bar now also appears while a card is actively playing even if the static snapshot hadn't recorded a listen position yet (e.g. an episode just started from 0 via auto-advance, or via this same card's own play button) - previously the bar was hidden entirely in that case since it only checked the static snapshot's `listenDuration`.

## Tests

Added `mobile/test/ui/utils/live_progress_test.dart` covering `formatDuration` (MM:SS / HH:MM:SS formatting) and all three `LiveProgressResolver` methods: preferring live values for the current episode, falling back to the static snapshot otherwise (including when no live value has arrived yet), and the show/hide decision for the progress section.

I couldn't run `flutter test` in the environment these changes were written in (no Flutter/Dart SDK available there) - please run it before merging.
