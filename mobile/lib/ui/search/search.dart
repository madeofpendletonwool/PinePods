// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'dart:io';

import 'package:pinepods_mobile/bloc/search/search_bloc.dart';
import 'package:pinepods_mobile/bloc/search/search_state_event.dart';
import 'package:pinepods_mobile/l10n/L.dart';
import 'package:pinepods_mobile/ui/search/search_results.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:provider/provider.dart';

/// This widget renders the search bar and allows the user to search for podcasts.
class Search extends StatefulWidget {
  final String? searchTerm;

  const Search({
    super.key,
    this.searchTerm,
  });

  @override
  State<Search> createState() => _SearchState();
}

class _SearchState extends State<Search> {
  late TextEditingController _searchController;
  late FocusNode _searchFocusNode;

  @override
  void initState() {
    super.initState();

    final bloc = Provider.of<SearchBloc>(context, listen: false);

    bloc.search(SearchClearEvent());

    _searchFocusNode = FocusNode();
    _searchController = TextEditingController();

    if (widget.searchTerm != null) {
      bloc.search(SearchTermEvent(widget.searchTerm!));
      _searchController.text = widget.searchTerm!;
    }
  }

  @override
  void dispose() {
    _searchFocusNode.dispose();
    _searchController.dispose();

    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final bloc = Provider.of<SearchBloc>(context);

    return Scaffold(
      body: CustomScrollView(
        slivers: <Widget>[
          SliverAppBar(
            leading: IconButton(
              tooltip: L.of(context)!.search_back_button_label,
              icon: Platform.isAndroid
                  ? Icon(Icons.arrow_back, color: Theme.of(context).appBarTheme.foregroundColor)
                  : const Icon(Icons.arrow_back_ios),
              onPressed: () => Navigator.pop(context),
            ),
            title: TextField(
                controller: _searchController,
                focusNode: _searchFocusNode,
                autofocus: widget.searchTerm != null ? false : true,
                keyboardType: TextInputType.text,
                textInputAction: TextInputAction.search,
                decoration: InputDecoration(
                  hintText: L.of(context)!.search_for_podcasts_hint,
                  border: InputBorder.none,
                ),
                style: TextStyle(
                    color: Theme.of(context).primaryIconTheme.color,
                    fontSize: 18.0,
                    decorationColor: Theme.of(context).scaffoldBackgroundColor),
                onSubmitted: ((value) {
                  SemanticsService.announce(L.of(context)!.semantic_announce_searching, TextDirection.ltr);
                  bloc.search(SearchTermEvent(value));
                })),
            floating: false,
            pinned: true,
            snap: false,
            actions: <Widget>[
              IconButton(
                tooltip: L.of(context)!.clear_search_button_label,
                icon: const Icon(Icons.clear),
                onPressed: () {
                  _searchController.clear();
                  FocusScope.of(context).requestFocus(_searchFocusNode);
                  SystemChannels.textInput.invokeMethod<String>('TextInput.show');
                },
              ),
            ],
          ),
          SearchResults(data: bloc.results!),
        ],
      ),
    );
  }
}
