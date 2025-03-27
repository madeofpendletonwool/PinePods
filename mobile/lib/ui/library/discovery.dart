// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:pinepods_mobile/bloc/discovery/discovery_bloc.dart';
import 'package:pinepods_mobile/bloc/discovery/discovery_state_event.dart';
import 'package:pinepods_mobile/ui/library/discovery_results.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:scrollable_positioned_list/scrollable_positioned_list.dart';
import 'package:sliver_tools/sliver_tools.dart';

/// This class is the root class for rendering the Discover tab.
///
/// This UI can optionally show a list of genres provided by iTunes/PodcastIndex.
class Discovery extends StatefulWidget {
  static const fetchSize = 20;
  final bool categories;

  const Discovery({
    super.key,
    this.categories = false,
  });

  @override
  State<StatefulWidget> createState() => _DiscoveryState();
}

class _DiscoveryState extends State<Discovery> {
  @override
  void initState() {
    super.initState();

    final bloc = Provider.of<DiscoveryBloc>(context, listen: false);

    bloc.discover(DiscoveryChartEvent(
      count: Discovery.fetchSize,
      genre: bloc.selectedGenre.genre,
      countryCode: PlatformDispatcher.instance.locale.countryCode?.toLowerCase() ?? '',
      languageCode: PlatformDispatcher.instance.locale.languageCode,
    ));
  }

  @override
  Widget build(BuildContext context) {
    final bloc = Provider.of<DiscoveryBloc>(context);

    return widget.categories
        ? MultiSliver(
            children: [
              SliverPersistentHeader(
                delegate: MyHeaderDelegate(bloc),
                pinned: true,
                floating: false,
              ),
              DiscoveryResults(data: bloc.results),
            ],
          )
        : DiscoveryResults(data: bloc.results);
  }
}

/// This delegate is responsible for rendering the horizontal scrolling list of categories
/// that can optionally be displayed at the top of the Discovery results page.
class MyHeaderDelegate extends SliverPersistentHeaderDelegate {
  final DiscoveryBloc discoveryBloc;

  MyHeaderDelegate(this.discoveryBloc);

  @override
  Widget build(BuildContext context, double shrinkOffset, bool overlapsContent) {
    return CategorySelectorWidget(discoveryBloc: discoveryBloc);
  }

  @override
  double get maxExtent => 56.0;

  @override
  double get minExtent => 56.0;

  @override
  bool shouldRebuild(covariant SliverPersistentHeaderDelegate oldDelegate) => true;
}

class CategorySelectorWidget extends StatefulWidget {
  final ItemScrollController itemScrollController = ItemScrollController();

  CategorySelectorWidget({
    super.key,
    required this.discoveryBloc,
  });

  final DiscoveryBloc discoveryBloc;

  @override
  State<CategorySelectorWidget> createState() => _CategorySelectorWidgetState();
}

class _CategorySelectorWidgetState extends State<CategorySelectorWidget> {
  @override
  Widget build(BuildContext context) {
    String selectedCategory = widget.discoveryBloc.selectedGenre.genre;

    return Container(
      width: double.infinity,
      color: Theme.of(context).canvasColor,
      child: StreamBuilder<List<String>>(
          stream: widget.discoveryBloc.genres,
          initialData: const [],
          builder: (context, snapshot) {
            var i = widget.discoveryBloc.selectedGenre.index;

            return snapshot.hasData && snapshot.data!.isNotEmpty
                ? ScrollablePositionedList.builder(
                    initialScrollIndex: (i > 0) ? i : 0,
                    itemScrollController: widget.itemScrollController,
                    itemCount: snapshot.data!.length,
                    scrollDirection: Axis.horizontal,
                    itemBuilder: (context, i) {
                      final item = snapshot.data![i];
                      final padding = i == 0 ? 14.0 : 2.0;

                      return Container(
                        margin: EdgeInsets.only(left: padding),
                        child: Card(
                          color: item == selectedCategory || (selectedCategory.isEmpty && i == 0)
                              ? Theme.of(context).cardTheme.shadowColor
                              : Theme.of(context).cardTheme.color,
                          child: TextButton(
                            style: TextButton.styleFrom(
                              foregroundColor: const Color(0xffffffff),
                              visualDensity: VisualDensity.compact,
                            ),
                            onPressed: () {
                              setState(() {
                                selectedCategory = item;
                              });

                              widget.discoveryBloc.discover(DiscoveryChartEvent(
                                count: Discovery.fetchSize,
                                genre: item,
                                countryCode: PlatformDispatcher.instance.locale.countryCode?.toLowerCase() ?? '',
                                languageCode: PlatformDispatcher.instance.locale.languageCode,
                              ));
                            },
                            child: Text(item),
                          ),
                        ),
                      );
                    })
                : Container();
          }),
    );
  }
}
