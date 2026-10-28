// Covers the pure helpers in core/utils.dart, plus redirectChain against a
// local server. The rest of that file (resolvePath, getStorageDirectory,
// hasStoragePermission, etc.) talks to platform channels or the filesystem,
// so it's out of scope for plain unit tests.

import 'dart:io';

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

  group('pickDownloadUrl', () {
    List<Uri> chain(List<String> urls) => urls.map(Uri.parse).toList();

    test('keeps the original URL when every hop is https', () {
      expect(
        pickDownloadUrl(chain(['https://feed.example/ep.mp3', 'https://cdn.example/signed/ep.mp3?token=1'])),
        'https://feed.example/ep.mp3',
      );
    });

    test('resolves and upgrades a chain through http', () {
      expect(
        pickDownloadUrl(chain(['https://feed.example/ep.mp3', 'http://cdn.example/ep.mp3'])),
        'https://cdn.example/ep.mp3',
      );
      expect(
        pickDownloadUrl(chain(['http://feed.example/ep.mp3', 'https://cdn.example/ep.mp3'])),
        'https://cdn.example/ep.mp3',
      );
    });

    test('upgrades an http URL without redirects', () {
      expect(pickDownloadUrl(chain(['http://feed.example/ep.mp3'])), 'https://feed.example/ep.mp3');
    });
  });

  group('redirectChain', () {
    test('lists the original URL and each redirect target', () async {
      final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
      server.listen((request) {
        final next = {'/a': '/b', '/b': '/c'}[request.uri.path];
        if (next != null) {
          request.response.redirect(Uri.parse(next));
        } else {
          request.response.write('audio');
          request.response.close();
        }
      });
      addTearDown(() => server.close(force: true));

      final base = 'http://127.0.0.1:${server.port}';
      final result = await redirectChain('$base/a');
      expect(result.chain.map((u) => u.toString()), ['$base/a', '$base/b', '$base/c']);
      expect(result.status, 200);
    });
  });

  group('httpsVariant', () {
    test('switches the scheme to https', () {
      expect(httpsVariant(Uri.parse('http://feed.example/ep.mp3?x=1')).toString(), 'https://feed.example/ep.mp3?x=1');
    });

    test('asks BBC media selector URLs for an https redirect', () {
      const path = '/mediaselector/6/redir/version/2.0/mediaset/audio-nondrm-download-rss/proto';
      expect(
        httpsVariant(Uri.parse('http://open.live.bbc.co.uk$path/http/vpid/p0pj8fyh.mp3')).toString(),
        'https://open.live.bbc.co.uk$path/https/vpid/p0pj8fyh.mp3',
      );
    });

    test('leaves proto segments on other hosts alone', () {
      expect(
        httpsVariant(Uri.parse('http://cdn.example/proto/http/ep.mp3')).toString(),
        'https://cdn.example/proto/http/ep.mp3',
      );
    });
  });
}
