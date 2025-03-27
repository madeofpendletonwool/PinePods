// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/ui/widgets/platform_progress_indicator.dart';
import 'package:flutter/material.dart';

/// This class returns a platform-specific spinning indicator after a time specified
/// in milliseconds.
///
/// Defaults to 1 second. This can be used as a place holder for cached images. By
/// delaying for several milliseconds it can reduce the occurrences of placeholders
/// flashing on screen as the cached image is loaded. Images that take longer to fetch
/// or process from the cache will result in a [PlatformProgressIndicator] indicator
/// being displayed.
class DelayedCircularProgressIndicator extends StatelessWidget {
  final f = Future.delayed(const Duration(milliseconds: 1000), () => Container());

  DelayedCircularProgressIndicator({
    super.key,
  });

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<Widget>(
        future: f,
        builder: (context, snapshot) {
          if (snapshot.connectionState == ConnectionState.done) {
            return const Center(
              child: PlatformProgressIndicator(),
            );
          } else {
            return Container();
          }
        });
  }
}
