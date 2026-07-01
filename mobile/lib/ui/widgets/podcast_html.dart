// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
import 'package:flutter/material.dart';
import 'package:flutter_widget_from_html_core/flutter_widget_from_html_core.dart';
import 'package:pinepods_mobile/ui/widgets/pinepods_widget_factory.dart';
import 'package:url_launcher/url_launcher.dart';

/// This class is a simple, common wrapper around the flutter_widget_from_html
/// [HtmlWidget].
///
/// This wrapper allows us to remove some of the HTML tags which can cause rendering
/// issues when viewing podcast descriptions on a mobile device.
class PodcastHtml extends StatelessWidget {
  final String content;
  final double? fontSize;

  const PodcastHtml({
    super.key,
    required this.content,
    this.fontSize,
  });

  @override
  Widget build(BuildContext context) {
    return HtmlWidget(
      content,
      factoryBuilder: () => PinepodsWidgetFactory(),
      textStyle: TextStyle(
        fontSize: fontSize ?? 16.25,
        height: 1.1,
      ),
      customStylesBuilder: (element) {
        if (element.localName == 'p') {
          return {'margin': '0 0 12px 0'};
        }
        return null;
      },
      onTapUrl: (url) async {
        final uri = Uri.parse(url);
        if (await canLaunchUrl(uri)) {
          return launchUrl(uri, mode: LaunchMode.externalApplication);
        }
        return false;
      },
    );
  }
}
