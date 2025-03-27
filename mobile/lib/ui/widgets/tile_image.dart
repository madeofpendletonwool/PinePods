// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/ui/widgets/placeholder_builder.dart';
import 'package:pinepods_mobile/ui/widgets/podcast_image.dart';
import 'package:flutter/material.dart';

class TileImage extends StatelessWidget {
  const TileImage({
    super.key,
    required this.url,
    required this.size,
    this.highlight = false,
  });

  /// The URL of the image to display.
  final String url;

  /// The size of the image container; both height and width.
  final double size;

  final bool highlight;

  @override
  Widget build(BuildContext context) {
    final placeholderBuilder = PlaceholderBuilder.of(context);

    return PodcastImage(
      key: Key('tile$url'),
      highlight: highlight,
      url: url,
      height: size,
      width: size,
      borderRadius: 4.0,
      fit: BoxFit.contain,
      placeholder: placeholderBuilder != null
          ? placeholderBuilder.builder()(context)
          : const Image(
              fit: BoxFit.contain,
              image: AssetImage('assets/images/favicon.png'),
            ),
      errorPlaceholder: placeholderBuilder != null
          ? placeholderBuilder.errorBuilder()(context)
          : const Image(
              fit: BoxFit.contain,
              image: AssetImage('assets/images/favicon.png'),
            ),
    );
  }
}
