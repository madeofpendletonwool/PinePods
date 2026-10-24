// lib/ui/widgets/lazy_network_image.dart
import 'dart:io';

import 'package:extended_image/extended_image.dart';
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';

class LazyNetworkImage extends StatefulWidget {
  final String imageUrl;

  /// Absolute path to a locally-cached copy of the image. When set and the file
  /// exists on disk, it is rendered instead of [imageUrl] so artwork shows with
  /// no network (offline, #935). Falls back to [imageUrl] if the file is
  /// missing.
  final String? localImagePath;
  final double width;
  final double height;
  final BoxFit fit;
  final Widget? placeholder;
  final Widget? errorWidget;
  final BorderRadius? borderRadius;

  const LazyNetworkImage({
    super.key,
    required this.imageUrl,
    this.localImagePath,
    required this.width,
    required this.height,
    this.fit = BoxFit.cover,
    this.placeholder,
    this.errorWidget,
    this.borderRadius,
  });

  @override
  State<LazyNetworkImage> createState() => _LazyNetworkImageState();
}

class _LazyNetworkImageState extends State<LazyNetworkImage> {
  /// Resolved once per path, off the build path: a sync stat on every rebuild
  /// is too slow for list rows that rebuild on each playback state change.
  File? _localFile;
  String? _checkedPath;

  @override
  void initState() {
    super.initState();
    _checkLocalFile();
  }

  @override
  void didUpdateWidget(LazyNetworkImage oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.localImagePath != widget.localImagePath) _checkLocalFile();
  }

  Future<void> _checkLocalFile() async {
    final path = widget.localImagePath;
    _checkedPath = path;
    if (path == null || path.isEmpty) {
      _localFile = null;
      return;
    }
    final file = File(path);
    final exists = await file.exists();
    if (!mounted || _checkedPath != path) return;
    setState(() => _localFile = exists ? file : null);
  }

  Widget _box(IconData icon) {
    // hintColor rather than colorScheme.onSurface: some themes set onSurface
    // to their accent, which would tint every placeholder.
    final muted = Theme.of(context).hintColor;
    return Container(
      width: widget.width,
      height: widget.height,
      color: muted.withOpacity(0.12),
      child: Icon(icon, color: muted, size: widget.width * 0.4),
    );
  }

  @override
  Widget build(BuildContext context) {
    final cacheWidth = (widget.width * MediaQuery.devicePixelRatioOf(context)).round();
    final localFile = _localFile;

    final Widget image;
    if (localFile != null) {
      image = Image.file(
        localFile,
        width: widget.width,
        height: widget.height,
        fit: widget.fit,
        cacheWidth: cacheWidth,
        errorBuilder: (context, error, stackTrace) => widget.errorWidget ?? _box(PhosphorIconsRegular.microphoneStage),
      );
    } else if (widget.imageUrl.isEmpty) {
      image = widget.placeholder ?? _box(PhosphorIconsRegular.microphoneStage);
    } else {
      image = ExtendedImage.network(
        widget.imageUrl,
        width: widget.width,
        height: widget.height,
        fit: widget.fit,
        cache: true,
        cacheWidth: cacheWidth,
        loadStateChanged: (state) {
          switch (state.extendedImageLoadState) {
            case LoadState.loading:
              return widget.placeholder ?? _box(PhosphorIconsRegular.microphoneStage);
            case LoadState.failed:
              return widget.errorWidget ?? _box(PhosphorIconsRegular.microphoneStage);
            case LoadState.completed:
              return null;
          }
        },
      );
    }

    return ClipRRect(
      borderRadius: widget.borderRadius ?? BorderRadius.zero,
      child: image,
    );
  }
}
