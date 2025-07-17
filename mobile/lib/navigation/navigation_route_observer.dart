// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

import 'package:flutter/material.dart';
import 'package:flutter/widgets.dart';

/// This class will observe the current route.
///
/// This gives us an easy way to tell what screen we are on from elsewhere within
/// the application. This is useful, for example, when responding to external links
/// and determining if we need to display the podcast details or just update the
/// current screen.
class NavigationRouteObserver extends NavigatorObserver {
  final List<Route<dynamic>?> _routeStack = <Route<dynamic>?>[];

  static final NavigationRouteObserver _instance = NavigationRouteObserver._internal();

  NavigationRouteObserver._internal();

  factory NavigationRouteObserver() {
    return _instance;
  }

  @override
  void didPop(Route<dynamic> route, Route<dynamic>? previousRoute) {
    _routeStack.removeLast();
  }

  @override
  void didPush(Route<dynamic> route, Route<dynamic>? previousRoute) {
    _routeStack.add(route);
  }

  @override
  void didRemove(Route<dynamic> route, Route<dynamic>? previousRoute) {
    _routeStack.remove(route);
  }

  @override
  void didReplace({Route<dynamic>? newRoute, Route<dynamic>? oldRoute}) {
    int oldRouteIndex = _routeStack.indexOf(oldRoute);

    _routeStack.replaceRange(oldRouteIndex, oldRouteIndex + 1, [newRoute]);
  }

  Route<dynamic>? get top => _routeStack.last;
}
