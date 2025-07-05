// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/ui/pinepods/downloads.dart';
import 'package:flutter/material.dart';

/// Displays a list of currently downloaded podcast episodes.
/// This is a wrapper that redirects to the new PinePods downloads implementation.
class Downloads extends StatelessWidget {
  const Downloads({
    super.key,
  });

  @override
  Widget build(BuildContext context) {
    return const PinepodsDownloads();
  }
}
