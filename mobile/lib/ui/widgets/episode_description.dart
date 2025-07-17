// lib/ui/widgets/episode_description.dart
import 'package:flutter/material.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter_html/flutter_html.dart';
import 'package:flutter_html_svg/flutter_html_svg.dart';
import 'package:flutter_html_table/flutter_html_table.dart';
import 'package:url_launcher/url_launcher.dart';

/// A specialized widget for displaying episode descriptions with clickable timestamps.
/// 
/// This widget extends the basic HTML display functionality to parse timestamp patterns
/// like "43:53" or "1:23:45" and make them clickable for navigation within the episode.
class EpisodeDescription extends StatelessWidget {
  final String content;
  final FontSize? fontSize;
  final Function(Duration)? onTimestampTap;

  const EpisodeDescription({
    super.key,
    required this.content,
    this.fontSize,
    this.onTimestampTap,
  });

  @override
  Widget build(BuildContext context) {
    // For now, let's use a simpler approach - just display the HTML with custom link handling
    // We'll parse timestamps in the onLinkTap handler
    return Html(
      data: _processTimestamps(content),
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
        '.timestamp': Style(
          color: const Color(0xFF539e8a),
          textDecoration: TextDecoration.underline,
        ),
      },
      onLinkTap: (url, _, __) {
        if (url != null && url.startsWith('timestamp:') && onTimestampTap != null) {
          // Handle timestamp links
          final secondsStr = url.substring(10); // Remove 'timestamp:' prefix
          final seconds = int.tryParse(secondsStr);
          if (seconds != null) {
            final duration = Duration(seconds: seconds);
            onTimestampTap!(duration);
          }
        } else if (url != null) {
          // Handle regular links
          canLaunchUrl(Uri.parse(url)).then((value) => launchUrl(
            Uri.parse(url),
            mode: LaunchMode.externalApplication,
          ));
        }
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