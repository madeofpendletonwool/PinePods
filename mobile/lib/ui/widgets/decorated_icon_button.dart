// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:flutter/material.dart';

/// An [IconButton] cannot have a background or border.
///
/// This class wraps an IconButton in a shape so that it can have a background.
class DecoratedIconButton extends StatelessWidget {
  final Color decorationColour;
  final Color iconColour;
  final IconData icon;
  final VoidCallback onPressed;

  const DecoratedIconButton({
    super.key,
    required this.iconColour,
    required this.decorationColour,
    required this.icon,
    required this.onPressed,
  });

  @override
  Widget build(BuildContext context) {
    return Material(
      color: Colors.transparent,
      child: Center(
        child: Ink(
          width: 42.0,
          height: 42.0,
          decoration: ShapeDecoration(
            color: decorationColour,
            shape: const CircleBorder(),
          ),
          child: IconButton(
            icon: Icon(icon),
            padding: const EdgeInsets.all(0.0),
            color: iconColour,
            onPressed: onPressed,
          ),
        ),
      ),
    );
  }
}
