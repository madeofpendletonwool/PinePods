// Copyright 2020 Ben Hills and the project contributors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
import 'package:flutter/material.dart';
import 'package:flutter_html/flutter_html.dart';
import 'package:flutter_html_svg/flutter_html_svg.dart';
import 'package:flutter_html_table/flutter_html_table.dart';
import 'package:url_launcher/url_launcher.dart';

/// This class is a simple, common wrapper around the flutter_html Html widget.
///
/// This wrapper allows us to remove some of the HTML tags which can cause rendering
/// issues when viewing podcast descriptions on a mobile device.
class PodcastHtml extends StatelessWidget {
  final String content;
  final FontSize? fontSize;

  const PodcastHtml({
    super.key,
    required this.content,
    this.fontSize,
  });

  @override
  Widget build(BuildContext context) {
    return Html(
      data: content,
      extensions: const [
        SvgHtmlExtension(),
        TableHtmlExtension(),
      ],
      style: {
        'html': Style(
          fontSize: FontSize(16.25),
          lineHeight: LineHeight.percent(110),
        ),
        'p': Style(
          margin: Margins.only(
            top: 0,
            bottom: 12,
          ),
        ),
      },
      onLinkTap: (url, _, __) => canLaunchUrl(Uri.parse(url!)).then((value) => launchUrl(
            Uri.parse(url),
            mode: LaunchMode.externalApplication,
          )),
    );
  }
}
