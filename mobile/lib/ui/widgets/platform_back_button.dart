// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'dart:io';

import 'package:pinepods_mobile/l10n/L.dart';
import 'package:flutter/material.dart';

/// Simple widget for rendering either the standard Android close or iOS Back button.
class PlatformBackButton extends StatelessWidget {
  final Color decorationColour;
  final Color iconColour;
  final VoidCallback onPressed;

  const PlatformBackButton({
    super.key,
    required this.iconColour,
    required this.decorationColour,
    required this.onPressed,
  });

  @override
  Widget build(BuildContext context) {
    return Material(
      color: Colors.transparent,
      child: Semantics(
        button: true,
        child: Center(
          child: SizedBox(
            height: 48.0,
            width: 48.0,
            child: InkWell(
              onTap: onPressed,
              child: Container(
                margin: const EdgeInsets.all(6.0),
                height: 48.0,
                width: 48.0,
                decoration: ShapeDecoration(
                  color: decorationColour,
                  shape: const CircleBorder(),
                ),
                child: Padding(
                  padding: EdgeInsets.only(left: Platform.isIOS ? 8.0 : 0.0),
                  child: Icon(
                    Platform.isIOS ? Icons.arrow_back_ios : Icons.close,
                    size: Platform.isIOS ? 20.0 : 26.0,
                    semanticLabel: L.of(context)?.go_back_button_label,
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}
