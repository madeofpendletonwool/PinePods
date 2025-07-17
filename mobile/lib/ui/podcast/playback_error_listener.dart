// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'dart:async';

import 'package:pinepods_mobile/bloc/podcast/audio_bloc.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

/// Listens for errors on the audio BLoC.
///
/// We receive a code which we then map to an error message. This needs to be placed
/// below a [Scaffold].
class PlaybackErrorListener extends StatefulWidget {
  final Widget child;

  const PlaybackErrorListener({
    super.key,
    required this.child,
  });

  @override
  State<PlaybackErrorListener> createState() => _PlaybackErrorListenerState();
}

class _PlaybackErrorListenerState extends State<PlaybackErrorListener> {
  StreamSubscription<int>? errorSubscription;

  @override
  Widget build(BuildContext context) {
    return widget.child;
  }

  @override
  void initState() {
    super.initState();
    final audioBloc = Provider.of<AudioBloc>(context, listen: false);

    errorSubscription = audioBloc.playbackError!.listen((code) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(_codeToMessage(context, code))));
      }
    });
  }

  @override
  void dispose() {
    errorSubscription?.cancel();
    super.dispose();
  }

  /// Ideally the BLoC would pass us the message to display; however, as we need a
  /// context to fetch the correct version of any text string we need to work it out here.
  String _codeToMessage(BuildContext context, int code) {
    var result = '';

    switch (code) {
      case 401:
        result = L.of(context)!.error_no_connection;
        break;
      case 501:
        result = L.of(context)!.error_playback_fail;
        break;
    }

    return result;
  }
}
