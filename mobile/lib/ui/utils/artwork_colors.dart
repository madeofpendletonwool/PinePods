// lib/ui/utils/artwork_colors.dart
import 'dart:io';

import 'package:extended_image/extended_image.dart';
import 'package:flutter/material.dart';

/// Accent colours pulled from podcast artwork, cached per image and
/// brightness. Extraction runs one image at a time so a list scrolling into
/// view doesn't stall the UI thread with a burst of quantization work.
class ArtworkColors {
  static final Map<String, Color?> _cache = {};
  static final Map<String, Future<Color?>> _pending = {};
  static Future<void> _tail = Future.value();

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
      final scheme = await ColorScheme.fromImageProvider(
        provider: ResizeImage(base, width: 64, height: 64),
        brightness: brightness,
      );
      return scheme.primary;
    } catch (_) {
      return null;
    }
  }
}
