// lib/ui/utils/artwork_colors.dart
import 'dart:async';
import 'dart:io';
import 'dart:typed_data';
import 'dart:ui' as ui;

import 'package:extended_image/extended_image.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:material_color_utilities/material_color_utilities.dart';

/// Accent colours pulled from podcast artwork, cached per image and
/// brightness. Extraction runs one image at a time, and the quantization runs
/// on a background isolate so a list scrolling into view doesn't drop frames.
class ArtworkColors {
  static final Map<String, Color?> _cache = {};
  static final Map<String, Future<Color?>> _pending = {};
  static Future<void> _tail = Future.value();

  static const int _sampleSize = 64;

  static String _key(String source, Brightness brightness) => '${brightness.name}|$source';

  static String? _source(String imageUrl, String? localImagePath) {
    if (imageUrl.isNotEmpty) return imageUrl;
    if (localImagePath != null && localImagePath.isNotEmpty) return localImagePath;
    return null;
  }

  /// The colour if it has already been extracted, so a rebuilt card can show
  /// it on its first frame.
  static Color? cached(String imageUrl, String? localImagePath, Brightness brightness) {
    final source = _source(imageUrl, localImagePath);
    return source == null ? null : _cache[_key(source, brightness)];
  }

  /// Resolves to a colour tuned to contrast with a surface of [brightness],
  /// or null when there's no artwork or it can't be loaded.
  static Future<Color?> resolve(String imageUrl, String? localImagePath, Brightness brightness) {
    final source = _source(imageUrl, localImagePath);
    if (source == null) return Future.value(null);
    final key = _key(source, brightness);
    if (_cache.containsKey(key)) return Future.value(_cache[key]);
    return _pending[key] ??= _enqueue(key, source, imageUrl.isEmpty, brightness);
  }

  static Future<Color?> _enqueue(String key, String source, bool isFile, Brightness brightness) {
    final result = _tail.then((_) => _extract(source, isFile, brightness));
    _tail = result.then((_) {}, onError: (_) {});
    return result.then((color) {
      _cache[key] = color;
      _pending.remove(key);
      return color;
    });
  }

  static Future<Color?> _extract(String source, bool isFile, Brightness brightness) async {
    try {
      final ImageProvider base = isFile ? FileImage(File(source)) : ExtendedNetworkImageProvider(source, cache: true);
      final pixels = await _pixels(ResizeImage(base, width: _sampleSize, height: _sampleSize));
      if (pixels == null) return null;
      final argb = await compute(_primaryFromPixels, (pixels, brightness == Brightness.dark));
      return argb == null ? null : Color(argb);
    } catch (_) {
      return null;
    }
  }

  /// Decoded RGBA pixels of [provider], one per Uint32 (ABGR on little-endian).
  static Future<Uint32List?> _pixels(ImageProvider provider) async {
    final stream = provider.resolve(ImageConfiguration.empty);
    final completer = Completer<ui.Image>();
    late final ImageStreamListener listener;
    listener = ImageStreamListener(
      (info, _) {
        stream.removeListener(listener);
        if (!completer.isCompleted) completer.complete(info.image);
      },
      onError: (error, stackTrace) {
        stream.removeListener(listener);
        if (!completer.isCompleted) completer.completeError(error, stackTrace);
      },
    );
    stream.addListener(listener);

    final ui.Image image;
    try {
      image = await completer.future.timeout(const Duration(seconds: 10));
    } on TimeoutException {
      stream.removeListener(listener);
      return null;
    }
    try {
      final bytes = await image.toByteData(format: ui.ImageByteFormat.rawRgba);
      return bytes?.buffer.asUint32List();
    } finally {
      image.dispose();
    }
  }
}

/// Same pipeline as [ColorScheme.fromImageProvider] (Celebi quantize, score,
/// tonal-spot primary), minus the full scheme. Top-level so it can run on an
/// isolate.
Future<int?> _primaryFromPixels((Uint32List, bool) args) async {
  final (pixels, isDark) = args;
  final result = await QuantizerCelebi().quantize(pixels, 128);
  if (result.colorToCount.isEmpty) return null;
  final colorToCount = result.colorToCount.map((abgr, count) => MapEntry(_argbFromAbgr(abgr), count));
  final seed = Score.score(colorToCount, desired: 1).first;
  final scheme = SchemeTonalSpot(sourceColorHct: Hct.fromInt(seed), isDark: isDark, contrastLevel: 0);
  return MaterialDynamicColors.primary.getArgb(scheme);
}

int _argbFromAbgr(int abgr) {
  final r = (abgr >> 0) & 0xFF;
  final b = (abgr >> 16) & 0xFF;
  return (abgr & 0xFF00FF00) | (r << 16) | b;
}
