// mocktail mocks for the collaborators that show up across service/bloc tests,
// plus fallback registration for the entity types used as `any()` arguments.
//
// mocktail needs no codegen: `class MockX extends Mock implements X {}` is the
// whole declaration. Call [registerCommonFallbacks] once in a test's setUpAll
// before using `any()` with Episode/Podcast arguments.

import 'package:mocktail/mocktail.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/entities/podcast.dart';
import 'package:pinepods_mobile/services/audio/audio_player_service.dart';
import 'package:pinepods_mobile/services/podcast/podcast_service.dart';

import 'factories.dart';

class MockPodcastService extends Mock implements PodcastService {}

class MockAudioPlayerService extends Mock implements AudioPlayerService {}

var _fallbacksRegistered = false;

/// Registers fallback values mocktail requires when matching non-primitive
/// arguments with `any()`. Safe to call more than once.
void registerCommonFallbacks() {
  if (_fallbacksRegistered) return;
  registerFallbackValue(buildEpisode());
  registerFallbackValue(buildPodcast());
  _fallbacksRegistered = true;
}

// Convenience aliases so tests read naturally regardless of the concrete types.
Episode fallbackEpisode() => buildEpisode();
Podcast fallbackPodcast() => buildPodcast();
