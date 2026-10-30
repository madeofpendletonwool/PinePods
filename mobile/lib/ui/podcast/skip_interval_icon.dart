import 'package:flutter/material.dart';

/// A circular rewind / fast-forward arrow with the skip interval written inside.
class SkipIntervalIcon extends StatelessWidget {
  final IconData icon;
  final int seconds;
  final double size;
  final Color? color;
  final String? semanticLabel;

  const SkipIntervalIcon({
    super.key,
    required this.icon,
    required this.seconds,
    this.size = 46,
    this.color,
    this.semanticLabel,
  });

  @override
  Widget build(BuildContext context) {
    final color = this.color ?? IconTheme.of(context).color;
    return SizedBox(
      width: size,
      height: size,
      child: Stack(
        alignment: Alignment.center,
        children: [
          Icon(icon, size: size, color: color, semanticLabel: semanticLabel),
          Padding(
            padding: EdgeInsets.only(top: size * 0.04),
            child: Text(
              '$seconds',
              style: TextStyle(fontSize: size * 0.27, fontWeight: FontWeight.w700, color: color, height: 1),
            ),
          ),
        ],
      ),
    );
  }
}
