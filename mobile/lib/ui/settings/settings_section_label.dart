// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:flutter/material.dart';

class SettingsDividerLabel extends StatelessWidget {
  final String label;
  final EdgeInsetsGeometry padding;

  const SettingsDividerLabel({
    super.key,
    required this.label,
    this.padding = const EdgeInsets.fromLTRB(16.0, 24.0, 0.0, 0.0),
  });

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: padding,
      child: Semantics(
        header: true,
        child: Text(
          label,
          style: Theme.of(context).textTheme.titleSmall!.copyWith(
                fontSize: 12.0,
                color: Theme.of(context).primaryColor,
              ),
        ),
      ),
    );
  }
}
