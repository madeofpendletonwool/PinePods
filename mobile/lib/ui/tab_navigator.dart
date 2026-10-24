import 'package:flutter/widgets.dart';

/// The app shell's nested navigator, which holds tab content and the pages
/// pushed from it. Surfaces opened above the shell (such as the full-screen
/// player) use it to open a page in the app after closing themselves.
final tabNavigatorKey = GlobalKey<NavigatorState>();
