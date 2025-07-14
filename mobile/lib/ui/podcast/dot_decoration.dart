// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:flutter/widgets.dart';

/// Custom [Decoration] for the chapters, episode & notes tab selector
/// shown in the [NowPlaying] page.
class DotDecoration extends Decoration {
  final Color colour;

  const DotDecoration({required this.colour});

  @override
  BoxPainter createBoxPainter([void Function()? onChanged]) {
    return _DotDecorationPainter(decoration: this);
  }
}

class _DotDecorationPainter extends BoxPainter {
  final DotDecoration decoration;

  _DotDecorationPainter({required this.decoration});

  @override
  void paint(Canvas canvas, Offset offset, ImageConfiguration configuration) {
    const double pillWidth = 8.0;
    const double pillHeight = 3.0;

    final center = configuration.size!.center(offset);
    final height = configuration.size!.height;

    final newOffset = Offset(center.dx, height - 8);

    final paint = Paint();
    paint.color = decoration.colour;
    paint.style = PaintingStyle.fill;

    canvas.drawRRect(
        RRect.fromLTRBR(
          newOffset.dx - pillWidth,
          newOffset.dy - pillHeight,
          newOffset.dx + pillWidth,
          newOffset.dy + pillHeight,
          const Radius.circular(12.0),
        ),
        paint);
  }
}
