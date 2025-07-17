// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

/// Events
class SearchEvent {}

class SearchTermEvent extends SearchEvent {
  final String term;

  SearchTermEvent(this.term);
}

class SearchChartsEvent extends SearchEvent {}

class SearchClearEvent extends SearchEvent {}

/// States
class SearchState {}

class SearchLoadingState extends SearchState {}
