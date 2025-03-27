// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/bloc/podcast/audio_bloc.dart';
import 'package:rxdart/rxdart.dart';

/// Base class for all BLoCs to give each a hook into the mobile
/// lifecycle state of paused, resume or detached.
abstract class Bloc {
  /// Handle lifecycle events
  final PublishSubject<LifecycleState> _lifecycleSubject = PublishSubject<LifecycleState>(sync: true);

  Bloc() {
    _init();
  }

  void _init() {
    _lifecycleSubject.listen((state) async {
      if (state == LifecycleState.resume) {
        resume();
      } else if (state == LifecycleState.pause) {
        pause();
      } else if (state == LifecycleState.detach) {
        detach();
      }
    });
  }

  void dispose() {
    if (_lifecycleSubject.hasListener) {
      _lifecycleSubject.close();
    }
  }

  void resume() {}

  void pause() {}

  void detach() {}

  void Function(LifecycleState) get transitionLifecycleState => _lifecycleSubject.sink.add;
}
