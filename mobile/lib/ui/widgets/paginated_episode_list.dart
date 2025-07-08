// lib/ui/widgets/paginated_episode_list.dart
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/entities/episode.dart';
import 'package:pinepods_mobile/ui/widgets/pinepods_episode_card.dart';
import 'package:pinepods_mobile/ui/widgets/episode_tile.dart';
import 'package:pinepods_mobile/ui/widgets/shimmer_episode_tile.dart';

class PaginatedEpisodeList extends StatefulWidget {
  final List<dynamic> episodes; // Can be PinepodsEpisode or Episode
  final bool isServerEpisodes;
  final Function(dynamic episode)? onEpisodeTap;
  final Function(dynamic episode, int globalIndex)? onEpisodeLongPress;
  final Function(dynamic episode)? onPlayPressed;
  final int pageSize;

  const PaginatedEpisodeList({
    super.key,
    required this.episodes,
    required this.isServerEpisodes,
    this.onEpisodeTap,
    this.onEpisodeLongPress,
    this.onPlayPressed,
    this.pageSize = 20, // Show 20 episodes at a time
  });

  @override
  State<PaginatedEpisodeList> createState() => _PaginatedEpisodeListState();
}

class _PaginatedEpisodeListState extends State<PaginatedEpisodeList> {
  int _currentPage = 0;
  bool _isLoadingMore = false;

  int get _totalPages => (widget.episodes.length / widget.pageSize).ceil();
  int get _currentEndIndex => (_currentPage + 1) * widget.pageSize;
  int get _displayedCount => _currentEndIndex.clamp(0, widget.episodes.length);

  List<dynamic> get _displayedEpisodes => 
    widget.episodes.take(_displayedCount).toList();

  Future<void> _loadMoreEpisodes() async {
    if (_isLoadingMore || _currentPage + 1 >= _totalPages) return;

    setState(() {
      _isLoadingMore = true;
    });

    // Simulate a small delay to show loading state
    await Future.delayed(const Duration(milliseconds: 500));

    setState(() {
      _currentPage++;
      _isLoadingMore = false;
    });
  }

  Widget _buildEpisodeWidget(dynamic episode, int globalIndex) {
    if (widget.isServerEpisodes && episode is PinepodsEpisode) {
      return PinepodsEpisodeCard(
        episode: episode,
        onTap: widget.onEpisodeTap != null 
          ? () => widget.onEpisodeTap!(episode)
          : null,
        onLongPress: widget.onEpisodeLongPress != null
          ? () => widget.onEpisodeLongPress!(episode, globalIndex)
          : null,
        onPlayPressed: widget.onPlayPressed != null
          ? () => widget.onPlayPressed!(episode)
          : null,
      );
    } else if (!widget.isServerEpisodes && episode is Episode) {
      return EpisodeTile(
        episode: episode,
        download: false,
        play: true,
      );
    }
    
    return const SizedBox.shrink(); // Fallback
  }

  @override
  Widget build(BuildContext context) {
    if (widget.episodes.isEmpty) {
      return const SizedBox.shrink();
    }

    return Column(
      children: [
        // Display current episodes
        ..._displayedEpisodes.asMap().entries.map((entry) {
          final index = entry.key;
          final episode = entry.value;
          final globalIndex = widget.episodes.indexOf(episode);
          
          return _buildEpisodeWidget(episode, globalIndex);
        }).toList(),

        // Loading shimmer for more episodes
        if (_isLoadingMore) ...[
          ...List.generate(3, (index) => const ShimmerEpisodeTile()),
        ],

        // Load more button or loading indicator
        if (_currentPage + 1 < _totalPages && !_isLoadingMore) ...[
          const SizedBox(height: 8),
          if (_isLoadingMore)
            const Padding(
              padding: EdgeInsets.all(16.0),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  SizedBox(
                    width: 20,
                    height: 20,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  ),
                  SizedBox(width: 12),
                  Text('Loading more episodes...'),
                ],
              ),
            )
          else
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 12.0, vertical: 8.0),
              child: SizedBox(
                width: double.infinity,
                child: OutlinedButton.icon(
                  onPressed: _loadMoreEpisodes,
                  icon: const Icon(Icons.expand_more),
                  label: Text(
                    'Load ${(_displayedCount + widget.pageSize).clamp(0, widget.episodes.length) - _displayedCount} more episodes '
                    '(${widget.episodes.length - _displayedCount} remaining)',
                  ),
                  style: OutlinedButton.styleFrom(
                    padding: const EdgeInsets.symmetric(vertical: 12),
                  ),
                ),
              ),
            ),
        ] else if (widget.episodes.length > widget.pageSize) ...[
          // Show completion message for large lists
          Padding(
            padding: const EdgeInsets.all(16.0),
            child: Text(
              'All ${widget.episodes.length} episodes loaded',
              style: Theme.of(context).textTheme.bodySmall?.copyWith(
                color: Colors.grey[600],
              ),
              textAlign: TextAlign.center,
            ),
          ),
        ],
      ],
    );
  }
}