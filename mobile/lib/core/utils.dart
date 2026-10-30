// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'dart:io';

import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/services/settings/mobile_settings_service.dart';
import 'package:pinepods_mobile/services/settings/settings_service.dart';
import 'package:path/path.dart';
import 'package:path_provider/path_provider.dart';

/// Returns the storage directory for the current platform.
///
/// On iOS, the directory that the app has available to it for storing episodes may
/// change between updates, whereas on Android we are able to save the full path. To
/// ensure we can handle the directory name change on iOS without breaking existing
/// Android installations we have created the following three functions to help with
/// resolving the various paths correctly depending upon platform.
Future<String> resolvePath(Episode episode) async {
  if (Platform.isIOS) {
    return Future.value(join(await getStorageDirectory(), episode.filepath, episode.filename));
  }

  return Future.value(join(episode.filepath!, episode.filename));
}

Future<String> resolveDirectory({required Episode episode, bool full = false}) async {
  if (full || Platform.isAndroid) {
    return Future.value(join(await getStorageDirectory(), safePath(episode.podcast!)));
  }

  return Future.value(safePath(episode.podcast!));
}

Future<void> createDownloadDirectory(Episode episode) async {
  var path = join(await getStorageDirectory(), safePath(episode.podcast!));

  Directory(path).createSync(recursive: true);
}

/// Downloads only go to app-specific directories (including the SD card's
/// `Android/data/<package>` folder), which need no storage permission. Requesting
/// Permission.storage would always be denied on Android 13+ (#983).
Future<bool> hasStoragePermission() async => true;

Future<String> getStorageDirectory() async {
  SettingsService? settings = await MobileSettingsService.instance();
  Directory directory;

  if (Platform.isIOS) {
    directory = await getApplicationDocumentsDirectory();
  } else if (settings!.storeDownloadsSDCard) {
    directory = await _getSDCard();
  } else {
    directory = await getApplicationSupportDirectory();
  }

  return join(directory.path, 'PinePods');
}

Future<bool> hasExternalStorage() async {
  try {
    await _getSDCard();

    return Future.value(true);
  } catch (e) {
    return Future.value(false);
  }
}

Future<Directory> _getSDCard() async {
  final appDocumentDir = (await getExternalStorageDirectories(type: StorageDirectory.podcasts))!;

  Directory? path;

  // If the directory contains the word 'emulated' we are
  // probably looking at a mapped user partition rather than
  // an actual SD card - so skip those and find the first
  // non-emulated directory.
  if (appDocumentDir.isNotEmpty) {
    // See if we can find the last card without emulated
    for (var d in appDocumentDir) {
      if (!d.path.contains('emulated')) {
        path = d.absolute;
      }
    }
  }

  if (path == null) {
    throw ('No SD card found');
  }

  return path;
}

/// Strips characters that are invalid for file and directory names.
String? safePath(String? s) {
  return s?.replaceAll(RegExp(r'[^\w\s]+'), '').trim();
}

String? safeFile(String? s) {
  return s?.replaceAll(RegExp(r'[^\w\s\.]+'), '').trim();
}

/// The URL to hand the native downloader for [url].
///
/// When every hop of the redirect chain is https the starting URL is kept, so
/// the downloader follows the redirects itself: signed CDN URLs can be tied to
/// the network that requested them (#1035). An http URL is tried as https
/// first. A chain that still goes through plain http is resolved and upgraded
/// to https instead, since the native downloaders refuse an https -> http
/// redirect.
Future<String> resolveDownloadUrl(String url) async {
  final original = Uri.parse(url);
  if (original.scheme == 'http') {
    try {
      final secure = await redirectChain(httpsVariant(original).toString());
      if (secure.status < 400 && secure.chain.every((uri) => uri.scheme == 'https')) {
        return secure.chain.first.toString();
      }
    } catch (_) {
      // No https version; fall back to the http chain.
    }
  }

  return pickDownloadUrl((await redirectChain(url)).chain);
}

/// [uri] over https. BBC media selector URLs also name the protocol of the
/// URL they redirect to, which is switched to https too.
Uri httpsVariant(Uri uri) {
  var path = uri.path;
  if (uri.host.endsWith('bbc.co.uk') && path.contains('/mediaselector/')) {
    path = path.replaceFirst('/proto/http/', '/proto/https/');
  }

  return uri.replace(scheme: 'https', path: path);
}

/// See [resolveDownloadUrl]; [chain] is the original URL followed by each
/// redirect target.
String pickDownloadUrl(List<Uri> chain) {
  if (chain.every((uri) => uri.scheme == 'https')) {
    return chain.first.toString();
  }

  var last = chain.last;
  if (last.scheme == 'http') {
    last = last.replace(scheme: 'https');
  }

  return last.toString();
}

/// [url] followed by each redirect target (up to 10), and the status code of
/// the last response.
Future<({List<Uri> chain, int status})> redirectChain(String url) async {
  final client = HttpClient();

  try {
    var uri = Uri.parse(url);
    final chain = [uri];
    var request = await client.getUrl(uri);

    request.followRedirects = false;

    var response = await request.close();

    // Follow redirects manually up to a reasonable limit
    var redirectCount = 0;
    const maxRedirects = 10;

    while (response.isRedirect && redirectCount < maxRedirects) {
      response.drain(0);
      final location = response.headers.value(HttpHeaders.locationHeader);

      if (location == null) {
        // No location header, break out of the loop
        break;
      }

      uri = uri.resolve(location);
      chain.add(uri);
      request = await client.getUrl(uri);
      request.followRedirects = false;
      response = await request.close();
      redirectCount++;
    }

    return (chain: chain, status: response.statusCode);
  } finally {
    client.close(force: true);
  }
}
