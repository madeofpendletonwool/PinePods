// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/entities/transcript.dart';

/// Events
abstract class TranscriptEvent {}

class TranscriptClearEvent extends TranscriptEvent {}

class TranscriptFilterEvent extends TranscriptEvent {
  final String search;

  TranscriptFilterEvent({required this.search});
}

/// State
abstract class TranscriptState {
  final Transcript? transcript;
  final bool isFiltered;

  TranscriptState({
    this.transcript,
    this.isFiltered = false,
  });
}

class TranscriptUnavailableState extends TranscriptState {}

class TranscriptLoadingState extends TranscriptState {}

class TranscriptUpdateState extends TranscriptState {
  TranscriptUpdateState({required Transcript transcript}) : super(transcript: transcript);
}
