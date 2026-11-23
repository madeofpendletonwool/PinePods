import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/core/extensions.dart';

void main() {
  group('IterableExtensions.chunk', () {
    test('splits a list into equal-sized chunks', () {
      final result = [1, 2, 3, 4, 5, 6].chunk(2).toList();

      expect(result, [
        [1, 2],
        [3, 4],
        [5, 6],
      ]);
    });

    test('the last chunk can be smaller than the requested size', () {
      final result = [1, 2, 3, 4, 5].chunk(2).toList();

      expect(result, [
        [1, 2],
        [3, 4],
        [5],
      ]);
    });

    test('yields a single empty chunk for an empty iterable', () {
      final result = <int>[].chunk(3).toList();

      expect(result, [[]]);
    });

    test('a chunk size larger than the list returns everything as one chunk', () {
      final result = [1, 2, 3].chunk(10).toList();

      expect(result, [
        [1, 2, 3],
      ]);
    });
  });

  group('ExtString.forceHttps', () {
    test('upgrades a plain http URL to https', () {
      expect('http://example.com/feed.xml'.forceHttps, 'https://example.com/feed.xml');
    });

    test('leaves an already-https URL unchanged', () {
      expect('https://example.com/feed.xml'.forceHttps, 'https://example.com/feed.xml');
    });

    test('leaves localhost over http unchanged, to support self-hosted development', () {
      expect('http://localhost:8040/api'.forceHttps, 'http://localhost:8040/api');
    });

    test('leaves 127.0.0.1 over http unchanged', () {
      expect('http://127.0.0.1:8040/api'.forceHttps, 'http://127.0.0.1:8040/api');
    });

    test('leaves private 10.x addresses over http unchanged', () {
      expect('http://10.0.1.5:8040/api'.forceHttps, 'http://10.0.1.5:8040/api');
    });

    test('leaves private 192.168.x addresses over http unchanged', () {
      expect('http://192.168.1.50/api'.forceHttps, 'http://192.168.1.50/api');
    });

    test('leaves private 172.x addresses over http unchanged', () {
      expect('http://172.16.0.5/api'.forceHttps, 'http://172.16.0.5/api');
    });

    test('leaves .local hostnames over http unchanged', () {
      expect('http://pinepods.local/api'.forceHttps, 'http://pinepods.local/api');
    });

    test('is case-insensitive when matching the local-network exceptions', () {
      expect('http://PINEPODS.LOCAL/api'.forceHttps, 'http://PINEPODS.LOCAL/api');
    });

    test('leaves Tailscale (100.64.0.0/10) addresses over http unchanged', () {
      expect('http://100.64.0.1:13378/a.mp3'.forceHttps, 'http://100.64.0.1:13378/a.mp3');
      expect('http://100.127.255.9/a.mp3'.forceHttps, 'http://100.127.255.9/a.mp3');
    });

    test('still upgrades public addresses next to the private ranges', () {
      expect('http://100.63.0.1/a.mp3'.forceHttps, 'https://100.63.0.1/a.mp3');
      expect('http://100.128.0.1/a.mp3'.forceHttps, 'https://100.128.0.1/a.mp3');
      expect('http://172.32.0.1/a.mp3'.forceHttps, 'https://172.32.0.1/a.mp3');
      expect('http://172.15.0.1/a.mp3'.forceHttps, 'https://172.15.0.1/a.mp3');
      expect('http://10.example.com/a.mp3'.forceHttps, 'https://10.example.com/a.mp3');
    });

    test('leaves local-only hostnames over http unchanged', () {
      for (final host in ['nas', 'box.lan', 'nas.home.arpa', 'abs.internal', 'nas.tail1234.ts.net', 'app.localhost']) {
        expect('http://$host:8080/a.mp3'.forceHttps, 'http://$host:8080/a.mp3');
      }
    });

    test('leaves link-local and local IPv6 addresses over http unchanged', () {
      expect('http://169.254.10.2/a.mp3'.forceHttps, 'http://169.254.10.2/a.mp3');
      expect('http://[::1]:8040/a.mp3'.forceHttps, 'http://[::1]:8040/a.mp3');
      expect('http://[fd7a:115c:a1e0::1]/a.mp3'.forceHttps, 'http://[fd7a:115c:a1e0::1]/a.mp3');
      expect('http://[fe80::1]/a.mp3'.forceHttps, 'http://[fe80::1]/a.mp3');
      expect('http://[2001:db8::1]/a.mp3'.forceHttps, 'https://[2001:db8::1]/a.mp3');
    });

    test("leaves the user's own server over http unchanged", () {
      addTearDown(() => HttpsUpgrade.setServer(null));
      expect('http://pods.example.com/api/data/stream/1'.forceHttps, 'https://pods.example.com/api/data/stream/1');
      HttpsUpgrade.setServer('http://Pods.Example.com:8040');
      expect('http://pods.example.com/api/data/stream/1'.forceHttps, 'http://pods.example.com/api/data/stream/1');
      expect('http://other.example.com/a.mp3'.forceHttps, 'https://other.example.com/a.mp3');
    });

    test('returns an empty string for null', () {
      String? value;
      expect(value.forceHttps, '');
    });

    test('leaves a non-http scheme (e.g. a relative path) unchanged', () {
      expect('/relative/path'.forceHttps, '/relative/path');
    });
  });

  group('ExtDouble.toTenth', () {
    test('rounds to one decimal place', () {
      expect(1.23.toTenth, 1.2);
      expect(1.26.toTenth, 1.3);
    });

    test('leaves a value already at one decimal place unchanged', () {
      expect(1.5.toTenth, 1.5);
    });

    test('rounds a whole number to itself', () {
      expect(2.0.toTenth, 2.0);
    });
  });
}
