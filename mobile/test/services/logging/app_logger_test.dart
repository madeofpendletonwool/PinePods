import 'package:flutter/foundation.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/services/logging/app_logger.dart';

void main() {
  group('isImageLoadError', () {
    test('an image that failed to load is not treated as a crash', () {
      final details = FlutterErrorDetails(
        exception: StateError('Failed to load https://feeds.podcastindex.org/dave_avatar.jpg.'),
        library: 'image resource service',
      );
      expect(AppLogger.isImageLoadError(details), isTrue);
    });

    test('other Flutter errors are still crashes', () {
      final details = FlutterErrorDetails(exception: StateError('boom'), library: 'widgets library');
      expect(AppLogger.isImageLoadError(details), isFalse);
    });
  });
}
