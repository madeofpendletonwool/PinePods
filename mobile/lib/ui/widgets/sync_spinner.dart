// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'dart:async';

import 'package:pinepods_mobile/bloc/podcast/podcast_bloc.dart';
import 'package:pinepods_mobile/state/bloc_state.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

class SyncSpinner extends StatefulWidget {
  const SyncSpinner({super.key});

  @override
  State<SyncSpinner> createState() => _SyncSpinnerState();
}

class _SyncSpinnerState extends State<SyncSpinner> with SingleTickerProviderStateMixin {
  late AnimationController _controller;
  StreamSubscription<BlocState<void>>? subscription;
  Widget? _child;

  @override
  void initState() {
    super.initState();

    _controller = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 1500),
    )..repeat();

    _child = const Icon(
      Icons.refresh,
      size: 16.0,
    );

    final podcastBloc = Provider.of<PodcastBloc>(context, listen: false);

    subscription = podcastBloc.backgroundLoading.listen((event) {
      if (event is BlocSuccessfulState<void> || event is BlocErrorState<void>) {
        _controller.stop();
      }
    });
  }

  @override
  void dispose() {
    _controller.dispose();
    subscription?.cancel();

    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final podcastBloc = Provider.of<PodcastBloc>(context, listen: false);

    return StreamBuilder<BlocState<void>>(
        initialData: BlocEmptyState<void>(),
        stream: podcastBloc.backgroundLoading,
        builder: (context, snapshot) {
          final state = snapshot.data;

          return state is BlocLoadingState<void>
              ? RotationTransition(
                  turns: _controller,
                  child: _child,
                )
              : const SizedBox(
                  width: 0.0,
                  height: 0.0,
                );
        });
  }
}
