// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:flutter/material.dart';

/// This class generates a simple 'handle' icon that can be added on widgets such as
/// scrollable sheets and bottom dialogs.
///
/// When running with a screen reader, the handle icon becomes selectable with an
/// optional label and tap callback. This makes it easier to open/close.
class SliderHandle extends StatelessWidget {
  final GestureTapCallback? onTap;
  final String label;

  const SliderHandle({
    super.key,
    this.onTap,
    this.label = '',
  });

  @override
  Widget build(BuildContext context) {
    return Semantics(
      liveRegion: true,
      label: label,
      child: GestureDetector(
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.all(8.0),
          child: Container(
            width: 36,
            height: 4,
            decoration: BoxDecoration(
              color: Theme.of(context).hintColor,
              borderRadius: const BorderRadius.all(Radius.circular(4.0)),
            ),
          ),
        ),
      ),
    );
  }
}
