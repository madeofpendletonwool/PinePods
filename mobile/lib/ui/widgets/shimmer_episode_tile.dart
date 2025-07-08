// lib/ui/widgets/shimmer_episode_tile.dart
import 'package:flutter/material.dart';

class ShimmerEpisodeTile extends StatefulWidget {
  const ShimmerEpisodeTile({super.key});

  @override
  State<ShimmerEpisodeTile> createState() => _ShimmerEpisodeTileState();
}

class _ShimmerEpisodeTileState extends State<ShimmerEpisodeTile> 
    with SingleTickerProviderStateMixin {
  late AnimationController _shimmerController;

  @override
  void initState() {
    super.initState();
    _shimmerController = AnimationController.unbounded(vsync: this)
      ..repeat(min: -0.5, max: 1.5, period: const Duration(milliseconds: 1000));
  }

  @override
  void dispose() {
    _shimmerController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Card(
      margin: const EdgeInsets.symmetric(horizontal: 12.0, vertical: 4.0),
      elevation: 1,
      child: Padding(
        padding: const EdgeInsets.all(12.0),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            // Shimmer image placeholder
            AnimatedBuilder(
              animation: _shimmerController,
              builder: (context, child) {
                return Container(
                  width: 50,
                  height: 50,
                  decoration: BoxDecoration(
                    borderRadius: BorderRadius.circular(6),
                    gradient: LinearGradient(
                      colors: [
                        Colors.grey[300]!,
                        Colors.grey[100]!,
                        Colors.grey[300]!,
                      ],
                      stops: const [0.1, 0.3, 0.4],
                      begin: const Alignment(-1.0, -0.3),
                      end: const Alignment(1.0, 0.3),
                      transform: _SlidingGradientTransform(_shimmerController.value),
                    ),
                  ),
                );
              },
            ),
            const SizedBox(width: 12),
            
            // Shimmer text content
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  // Title placeholder
                  AnimatedBuilder(
                    animation: _shimmerController,
                    builder: (context, child) {
                      return Container(
                        width: double.infinity,
                        height: 16,
                        decoration: BoxDecoration(
                          borderRadius: BorderRadius.circular(4),
                          gradient: LinearGradient(
                            colors: [
                              Colors.grey[300]!,
                              Colors.grey[100]!,
                              Colors.grey[300]!,
                            ],
                            stops: const [0.1, 0.3, 0.4],
                            begin: const Alignment(-1.0, -0.3),
                            end: const Alignment(1.0, 0.3),
                            transform: _SlidingGradientTransform(_shimmerController.value),
                          ),
                        ),
                      );
                    },
                  ),
                  const SizedBox(height: 8),
                  
                  // Subtitle placeholder
                  AnimatedBuilder(
                    animation: _shimmerController,
                    builder: (context, child) {
                      return Container(
                        width: 120,
                        height: 12,
                        decoration: BoxDecoration(
                          borderRadius: BorderRadius.circular(4),
                          gradient: LinearGradient(
                            colors: [
                              Colors.grey[300]!,
                              Colors.grey[100]!,
                              Colors.grey[300]!,
                            ],
                            stops: const [0.1, 0.3, 0.4],
                            begin: const Alignment(-1.0, -0.3),
                            end: const Alignment(1.0, 0.3),
                            transform: _SlidingGradientTransform(_shimmerController.value),
                          ),
                        ),
                      );
                    },
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _SlidingGradientTransform extends GradientTransform {
  const _SlidingGradientTransform(this.slidePercent);

  final double slidePercent;

  @override
  Matrix4? transform(Rect bounds, {TextDirection? textDirection}) {
    return Matrix4.translationValues(bounds.width * slidePercent, 0.0, 0.0);
  }
}