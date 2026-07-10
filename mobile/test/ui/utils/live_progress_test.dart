// Unit tests for LiveProgressResolver/formatDuration, extracted from
// home.dart's _EpisodeCard so the live-vs-static progress display logic can
// be tested without any widget/Provider/audio stream scaffolding.

import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/ui/utils/live_progress.dart';

void main() {
  group('formatDuration', () {
    test('formats under an hour as MM:SS', () {
      expect(formatDuration(const Duration(minutes: 5, seconds: 3)), '05:03');
    });

    test('formats an hour or more as HH:MM:SS', () {
      expect(formatDuration(const Duration(hours: 1, minutes: 2, seconds: 3)), '01:02:03');
    });

    test('formats zero as 00:00', () {
      expect(formatDuration(Duration.zero), '00:00');
    });
  });

  group('LiveProgressResolver.percentage', () {
    test('uses the live percentage when this is the currently-playing episode', () {
      final result = LiveProgressResolver.percentage(
        isCurrentEpisode: true,
        staticPercentage: 10,
        livePercentage: 42,
      );
      expect(result, 42.0);
    });

    test('falls back to the static percentage when not the current episode', () {
      final result = LiveProgressResolver.percentage(
        isCurrentEpisode: false,
        staticPercentage: 10,
        livePercentage: 42,
      );
      expect(result, 10.0);
    });

    test('falls back to the static percentage when there is no live value yet', () {
      final result = LiveProgressResolver.percentage(
        isCurrentEpisode: true,
        staticPercentage: 10,
        livePercentage: null,
      );
      expect(result, 10.0);
    });
  });

  group('LiveProgressResolver.elapsedText', () {
    test('formats the live position when this is the currently-playing episode', () {
      final result = LiveProgressResolver.elapsedText(
        isCurrentEpisode: true,
        staticText: '00:10',
        livePosition: const Duration(minutes: 1, seconds: 30),
      );
      expect(result, '01:30');
    });

    test('falls back to the static text when not the current episode', () {
      final result = LiveProgressResolver.elapsedText(
        isCurrentEpisode: false,
        staticText: '00:10',
        livePosition: const Duration(minutes: 1, seconds: 30),
      );
      expect(result, '00:10');
    });

    test('falls back to the static text when there is no live position yet', () {
      final result = LiveProgressResolver.elapsedText(
        isCurrentEpisode: true,
        staticText: '00:10',
        livePosition: null,
      );
      expect(result, '00:10');
    });

    test('returns null when there is neither a live position nor static text', () {
      final result = LiveProgressResolver.elapsedText(
        isCurrentEpisode: false,
        staticText: null,
        livePosition: null,
      );
      expect(result, isNull);
    });
  });

  group('LiveProgressResolver.shouldShowProgress', () {
    test('shows progress when the static snapshot already has some', () {
      expect(
        LiveProgressResolver.shouldShowProgress(isCurrentEpisode: false, hasStaticProgress: true),
        isTrue,
      );
    });

    test('shows progress while actively playing even with no static progress yet', () {
      expect(
        LiveProgressResolver.shouldShowProgress(isCurrentEpisode: true, hasStaticProgress: false),
        isTrue,
      );
    });

    test('hides progress when neither playing nor already started', () {
      expect(
        LiveProgressResolver.shouldShowProgress(isCurrentEpisode: false, hasStaticProgress: false),
        isFalse,
      );
    });
  });
}
