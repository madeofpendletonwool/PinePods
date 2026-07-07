import 'package:flutter_widget_from_html_core/flutter_widget_from_html_core.dart';
import 'package:fwfh_svg/fwfh_svg.dart';

/// A [WidgetFactory] for [HtmlWidget] that adds inline/remote SVG rendering on
/// top of the core feature set (text, images, links, tables).
///
/// We intentionally depend on `flutter_widget_from_html_core` + `fwfh_svg`
/// rather than the umbrella `flutter_widget_from_html` package: the latter's
/// video extension pulls in the abandoned `wakelock` plugin, which has no
/// Android namespace and breaks modern Gradle builds.
class PinepodsWidgetFactory extends WidgetFactory with SvgFactory {}
