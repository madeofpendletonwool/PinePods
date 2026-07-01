// lib/ui/widgets/episode_description.dart
import 'package:flutter/material.dart';
import 'package:flutter_widget_from_html_core/flutter_widget_from_html_core.dart';
import 'package:pinepods_mobile/ui/widgets/pinepods_widget_factory.dart';
import 'package:url_launcher/url_launcher.dart';

/// A specialized widget for displaying episode descriptions with clickable timestamps.
///
/// This widget extends the basic HTML display functionality to parse timestamp patterns
/// like "43:53" or "1:23:45" and make them clickable for navigation within the episode.
class EpisodeDescription extends StatelessWidget {
  final String content;
  final double? fontSize;
  final Function(Duration)? onTimestampTap;

  const EpisodeDescription({
    super.key,
    required this.content,
    this.fontSize,
    this.onTimestampTap,
  });

  @override
  Widget build(BuildContext context) {
    return HtmlWidget(
      _processTimestamps(content),
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
        if (url.startsWith('timestamp:') && onTimestampTap != null) {
          // Handle timestamp links
          final secondsStr = url.substring(10); // Remove 'timestamp:' prefix
          final seconds = int.tryParse(secondsStr);
          if (seconds != null) {
            onTimestampTap!(Duration(seconds: seconds));
            return true;
          }
          return false;
        }
        // Handle regular links
        final uri = Uri.parse(url);
        if (await canLaunchUrl(uri)) {
          return launchUrl(uri, mode: LaunchMode.externalApplication);
        }
        return false;
      },
    );
  }

  /// Parses content and wraps timestamps with clickable links
  String _processTimestamps(String htmlContent) {
    // Regex pattern to match timestamp formats:
    // - MM:SS (e.g., 43:53)
    // - H:MM:SS (e.g., 1:23:45)
    // - HH:MM:SS (e.g., 12:34:56)
    final timestampRegex = RegExp(r'\b(?:(\d{1,2}):)?(\d{1,2}):(\d{2})\b');

    return htmlContent.replaceAllMapped(timestampRegex, (match) {
      final fullMatch = match.group(0)!;
      final hours = match.group(1);
      final minutes = match.group(2)!;
      final seconds = match.group(3)!;

      // Calculate total seconds for the timestamp
      int totalSeconds = int.parse(seconds);
      totalSeconds += int.parse(minutes) * 60;
      if (hours != null) {
        totalSeconds += int.parse(hours) * 3600;
      }

      // Return the timestamp wrapped in a clickable link
      return '<a href="timestamp:$totalSeconds" style="color: #539e8a; text-decoration: underline;">$fullMatch</a>';
    });
  }
}
