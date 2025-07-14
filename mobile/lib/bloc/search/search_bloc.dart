// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'dart:async';

import 'package:pinepods_mobile/bloc/bloc.dart';
import 'package:pinepods_mobile/bloc/search/search_state_event.dart';
import 'package:pinepods_mobile/services/podcast/podcast_service.dart';
import 'package:pinepods_mobile/state/bloc_state.dart';
import 'package:connectivity_plus/connectivity_plus.dart';
import 'package:logging/logging.dart';
import 'package:podcast_search/podcast_search.dart' as pcast;
import 'package:rxdart/rxdart.dart';

/// This BLoC interacts with the [PodcastService] to search for podcasts for
/// a given term and to fetch the current podcast charts.
class SearchBloc extends Bloc {
  final log = Logger('SearchBloc');
  final PodcastService podcastService;

  /// Add to the Sink to trigger a search using the [SearchEvent].
  final BehaviorSubject<SearchEvent> _searchInput = BehaviorSubject<SearchEvent>();

  /// Add to the Sink to fetch the current podcast top x.
  final BehaviorSubject<int> _chartsInput = BehaviorSubject<int>();

  /// Stream of the current search results, be it from search or charts.
  Stream<BlocState<pcast.SearchResult>>? _searchResults;

  /// Cache of last results.
  pcast.SearchResult? _resultsCache;

  SearchBloc({required this.podcastService}) {
    _init();
  }

  void _init() {
    _searchResults = _searchInput.switchMap<BlocState<pcast.SearchResult>>(
      (SearchEvent event) => _search(event),
    );
  }

  /// Takes the [SearchEvent] to perform either a search, chart fetch or clearing
  /// of the current results cache.
  ///
  /// To improve resilience, when performing a search the current network status is
  /// checked. a [BlocErrorState] is pushed if we have no connectivity.
  Stream<BlocState<pcast.SearchResult>> _search(SearchEvent event) async* {
    if (event is SearchClearEvent) {
      yield BlocDefaultState();
    } else if (event is SearchChartsEvent) {
      yield BlocLoadingState();

      _resultsCache ??= await podcastService.charts(size: 10);

      yield BlocPopulatedState<pcast.SearchResult>(results: _resultsCache);
    } else if (event is SearchTermEvent) {
      final term = event.term;

      if (term.isEmpty) {
        yield BlocNoInputState();
      } else {
        yield BlocLoadingState();

        // Check we have network
        var connectivityResult = await Connectivity().checkConnectivity();

        // TODO: Docs do not recommend this approach as a reliable way to
        //       determine if network is available.
        if (connectivityResult.contains(ConnectivityResult.none)) {
          yield BlocErrorState(error: BlocErrorType.connectivity);
        } else {
          final results = await podcastService.search(term: term);

          // Was the search successful?
          if (results.successful) {
            yield BlocPopulatedState<pcast.SearchResult>(results: results);
          } else {
            yield BlocErrorState();
          }
        }
      }
    }
  }

  @override
  void dispose() {
    _searchInput.close();
    _chartsInput.close();
  }

  void Function(SearchEvent) get search => _searchInput.add;

  Stream<BlocState>? get results => _searchResults;
}
