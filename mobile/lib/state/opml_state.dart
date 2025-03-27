// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

abstract class OPMLState {}

class OPMLNoneState extends OPMLState {}

class OPMLParsingState extends OPMLState {}

class OPMLLoadingState extends OPMLState {
  final int current;
  final int total;
  final String? podcast;

  OPMLLoadingState({
    required this.current,
    required this.total,
    this.podcast,
  });
}

class OPMLCompletedState extends OPMLState {}

class OPMLErrorState extends OPMLState {}

abstract class OPMLEvent {}

class OPMLImportEvent extends OPMLEvent {
  final String? file;

  OPMLImportEvent({
    this.file,
  });
}

class OPMLExportEvent extends OPMLEvent {}

class OPMLCancelEvent extends OPMLEvent {}
