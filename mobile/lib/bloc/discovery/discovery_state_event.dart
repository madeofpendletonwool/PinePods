// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

/// Events
class DiscoveryEvent {}

class DiscoveryChartEvent extends DiscoveryEvent {
  final int count;
  String genre;
  String countryCode;
  String languageCode;

  DiscoveryChartEvent({
    required this.count,
    this.genre = '',
    this.countryCode = '',
    this.languageCode = '',
  });
}

/// States
class DiscoveryState {}

class DiscoveryLoadingState extends DiscoveryState {}

class DiscoveryPopulatedState<T> extends DiscoveryState {
  final String? genre;
  final int index;
  final T? results;

  DiscoveryPopulatedState({
    this.genre,
    this.index = 0,
    this.results,
  });
}
