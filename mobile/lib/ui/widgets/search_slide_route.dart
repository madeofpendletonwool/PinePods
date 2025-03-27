// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:flutter/material.dart';

/// A transitioning route that slides the child in from the
/// right.
class SlideRightRoute extends PageRouteBuilder<void> {
  final Widget widget;

  @override
  final RouteSettings settings;

  SlideRightRoute({
    required this.widget,
    required this.settings,
  }) : super(
            pageBuilder: (
              BuildContext context,
              Animation<double> animation,
              Animation<double> secondaryAnimation,
            ) {
              return widget;
            },
            settings: settings,
            transitionsBuilder: (
              BuildContext context,
              Animation<double> animation,
              Animation<double> secondaryAnimation,
              Widget child,
            ) {
              return SlideTransition(
                position: Tween<Offset>(
                  begin: const Offset(
                    1.0,
                    0.0,
                  ),
                  end: Offset.zero,
                ).animate(animation),
                child: child,
              );
            });
}
