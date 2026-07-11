// Only covers the pure string-sanitizing helpers in core/utils.dart. The
// rest of that file (resolvePath, getStorageDirectory, hasStoragePermission,
// resolveUrl, etc.) talks to platform channels, the filesystem, or the
// network, so it's out of scope for plain unit tests.

import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/core/utils.dart';

void main() {
  group('safePath', () {
    test('strips characters invalid in file/directory names', () {
      expect(safePath('My Podcast: The Sequel!'), 'My Podcast The Sequel');
    });

    test('leaves alphanumeric and spaces untouched', () {
      expect(safePath('My Podcast 2'), 'My Podcast 2');
    });

    test('trims leading/trailing whitespace left behind by stripping', () {
      expect(safePath('  Podcast?  '), 'Podcast');
    });

    test('returns null for null input', () {
      expect(safePath(null), isNull);
    });
  });

  group('safeFile', () {
    test('strips invalid characters but keeps dots for file extensions', () {
      expect(safeFile('episode: title!.mp3'), 'episode title.mp3');
    });

    test('returns null for null input', () {
      expect(safeFile(null), isNull);
    });
  });
}
