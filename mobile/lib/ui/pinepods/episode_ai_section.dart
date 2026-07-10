// lib/ui/pinepods/episode_ai_section.dart
//
// The AI features block on the episode details screen (#726 transcripts /
// #790 ad-block). Shows the server-generated transcript as timecoded,
// tap-to-seek lines (ad ranges highlighted), a per-ad Confirm/Deny review
// list, and a Detect-ads button. The whole block hides unless the server
// reports the AI sidecar as available.
import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';

/// A single transcript cue parsed from the stored `segments` JSON.
class _Cue {
  final double start;
  final double end;
  final String text;
  const _Cue({required this.start, required this.end, required this.text});
}

class EpisodeAiSection extends StatefulWidget {
  final PinepodsEpisode episode;
  final int userId;

  /// Credentialed service (caller has already called setCredentials).
  final PinepodsService pinepodsService;

  /// Seek the player to [position] (starts the episode if needed).
  final Future<void> Function(Duration position) onSeek;

  /// Called after a review/detect changes the skip set, so the caller can
  /// re-supply the native player if this episode is currently playing.
  final Future<void> Function() onSegmentsChanged;

  const EpisodeAiSection({
    Key? key,
    required this.episode,
    required this.userId,
    required this.pinepodsService,
    required this.onSeek,
    required this.onSegmentsChanged,
  }) : super(key: key);

  @override
  State<EpisodeAiSection> createState() => _EpisodeAiSectionState();
}

class _EpisodeAiSectionState extends State<EpisodeAiSection> {
  bool _loading = true;
  AiStatus _aiStatus = const AiStatus();
  StoredTranscript? _transcript;
  List<SkipSegment> _adSegments = const [];
  List<_Cue> _cues = const [];
  bool _busy = false; // guards Detect/Transcribe/review buttons

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      final status = await widget.pinepodsService.getAiStatus();
      if (!status.available) {
        if (mounted) setState(() {
          _aiStatus = status;
          _loading = false;
        });
        return;
      }

      final results = await Future.wait([
        widget.pinepodsService.getEpisodeTranscript(
            widget.episode.episodeId, widget.userId),
        widget.pinepodsService.getEpisodeSkipSegments(
            widget.episode.episodeId, widget.userId),
      ]);
      final transcript = results[0] as StoredTranscript?;
      final segments = results[1] as List<SkipSegment>;

      if (mounted) {
        setState(() {
          _aiStatus = status;
          _transcript = transcript;
          _adSegments = segments.where((s) => s.kind == 'ad').toList();
          _cues = _parseCues(transcript?.segments);
          _loading = false;
        });
      }
    } catch (e) {
      if (mounted) setState(() => _loading = false);
    }
  }

  // Re-fetch just the skip segments (after a review/detect) and re-supply the
  // native player via the parent. Never caches a stale snapshot.
  Future<void> _reloadSegments() async {
    try {
      final segments = await widget.pinepodsService
          .getEpisodeSkipSegments(widget.episode.episodeId, widget.userId);
      if (mounted) {
        setState(() => _adSegments = segments.where((s) => s.kind == 'ad').toList());
      }
      await widget.onSegmentsChanged();
    } catch (_) {}
  }

  List<_Cue> _parseCues(String? segmentsJson) {
    if (segmentsJson == null || segmentsJson.isEmpty) return const [];
    try {
      final raw = jsonDecode(segmentsJson);
      if (raw is! List) return const [];
      final cues = <_Cue>[];
      for (final item in raw) {
        if (item is! Map) continue;
        final text = (item['text'] as String?)?.trim() ?? '';
        if (text.isEmpty) continue;
        cues.add(_Cue(
          start: (item['start'] as num?)?.toDouble() ?? 0.0,
          end: (item['end'] as num?)?.toDouble() ?? 0.0,
          text: text,
        ));
      }
      return cues;
    } catch (_) {
      return const [];
    }
  }

  bool _cueIsAd(_Cue cue) {
    for (final ad in _adSegments) {
      // Overlap test (matches the web player's ad highlight).
      if (cue.start < ad.endTime && cue.end > ad.startTime) return true;
    }
    return false;
  }

  String _fmt(double seconds) {
    if (!seconds.isFinite || seconds < 0) seconds = 0;
    final total = seconds.floor();
    final h = total ~/ 3600;
    final m = (total % 3600) ~/ 60;
    final s = total % 60;
    final mm = m.toString().padLeft(2, '0');
    final ss = s.toString().padLeft(2, '0');
    return h > 0 ? '$h:$mm:$ss' : '$mm:$ss';
  }

  void _snack(String msg) {
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text(msg), duration: const Duration(seconds: 2)),
    );
  }

  Future<void> _detectAds() async {
    if (_busy) return;
    setState(() => _busy = true);
    try {
      final ok = await widget.pinepodsService
          .detectAds(widget.episode.episodeId, widget.userId, force: true);
      _snack(ok
          ? 'Ad detection queued — this can take a few minutes.'
          : 'AI is unavailable right now.');
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _transcribe() async {
    if (_busy) return;
    setState(() => _busy = true);
    try {
      final ok = await widget.pinepodsService
          .transcribeEpisode(widget.episode.episodeId, widget.userId, force: true);
      _snack(ok
          ? 'Transcription queued — this can take a few minutes.'
          : 'AI is unavailable right now.');
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _review(SkipSegment ad, String status) async {
    if (_busy) return;
    setState(() => _busy = true);
    try {
      final ok = await widget.pinepodsService
          .adjustAdSegmentReview(ad.segmentId, widget.userId, status);
      if (ok) {
        _snack(status == 'confirmed' ? 'Ad confirmed' : 'Ad kept');
        await _reloadSegments();
      } else {
        _snack('Could not update ad');
      }
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_loading) {
      return const SizedBox.shrink();
    }
    // Hide the whole block when the AI sidecar isn't wired up.
    if (!_aiStatus.available) {
      return const SizedBox.shrink();
    }

    final theme = Theme.of(context);
    final children = <Widget>[
      const SizedBox(height: 32),
      Row(
        children: [
          Icon(Icons.auto_awesome, size: 20, color: theme.primaryColor),
          const SizedBox(width: 8),
          Text(
            'AI Features',
            style: theme.textTheme.titleMedium!
                .copyWith(fontWeight: FontWeight.bold),
          ),
        ],
      ),
      const SizedBox(height: 12),
    ];

    // Action buttons: detect ads / (re)transcribe.
    children.add(Wrap(
      spacing: 8,
      runSpacing: 8,
      children: [
        if (_aiStatus.adRemovalReady)
          OutlinedButton.icon(
            onPressed: _busy ? null : _detectAds,
            icon: const Icon(Icons.block, size: 18),
            label: const Text('Detect ads'),
          ),
        if (_aiStatus.transcriptionReady)
          OutlinedButton.icon(
            onPressed: _busy ? null : _transcribe,
            icon: const Icon(Icons.record_voice_over, size: 18),
            label: Text(_transcript?.status == 'complete'
                ? 'Re-transcribe'
                : 'Transcribe'),
          ),
      ],
    ));

    // Detected-ads review list.
    if (_adSegments.isNotEmpty) {
      children.add(const SizedBox(height: 16));
      children.add(Text(
        'Detected ads',
        style: theme.textTheme.titleSmall!.copyWith(fontWeight: FontWeight.bold),
      ));
      for (final ad in _adSegments) {
        children.add(_buildAdRow(ad, theme));
      }
    }

    // Timecoded, tap-to-seek transcript.
    if (_cues.isNotEmpty) {
      children.add(const SizedBox(height: 8));
      children.add(_buildTranscript(theme));
    } else if (_transcript?.status == 'running') {
      children.add(const SizedBox(height: 12));
      children.add(Row(
        children: const [
          SizedBox(
              width: 16,
              height: 16,
              child: CircularProgressIndicator(strokeWidth: 2)),
          SizedBox(width: 8),
          Text('Transcribing…'),
        ],
      ));
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: children,
    );
  }

  Widget _buildAdRow(SkipSegment ad, ThemeData theme) {
    final skipping = ad.isActiveAd;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: Row(
        children: [
          Expanded(
            child: Text(
              '${_fmt(ad.startTime)} – ${_fmt(ad.endTime)}',
              style: theme.textTheme.bodyMedium,
            ),
          ),
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
            decoration: BoxDecoration(
              color: (skipping ? Colors.green : Colors.grey).withOpacity(0.2),
              borderRadius: BorderRadius.circular(10),
            ),
            child: Text(
              skipping ? 'Skipping' : 'Kept',
              style: theme.textTheme.bodySmall!.copyWith(
                color: skipping ? Colors.green[700] : Colors.grey[700],
              ),
            ),
          ),
          IconButton(
            tooltip: 'Confirm ad (skip)',
            visualDensity: VisualDensity.compact,
            icon: const Icon(Icons.check, size: 20),
            color: Colors.green,
            onPressed: _busy ? null : () => _review(ad, 'confirmed'),
          ),
          IconButton(
            tooltip: 'Keep (do not skip)',
            visualDensity: VisualDensity.compact,
            icon: const Icon(Icons.close, size: 20),
            color: Colors.red,
            onPressed: _busy ? null : () => _review(ad, 'rejected'),
          ),
        ],
      ),
    );
  }

  Widget _buildTranscript(ThemeData theme) {
    return ExpansionTile(
      tilePadding: EdgeInsets.zero,
      title: Text(
        'Transcript',
        style: theme.textTheme.titleSmall!.copyWith(fontWeight: FontWeight.bold),
      ),
      children: _cues.map((cue) {
        final isAd = _cueIsAd(cue);
        return InkWell(
          onTap: () => widget.onSeek(Duration(seconds: cue.start.floor())),
          child: Container(
            width: double.infinity,
            color: isAd ? Colors.orange.withOpacity(0.12) : null,
            padding: const EdgeInsets.symmetric(vertical: 6, horizontal: 4),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  _fmt(cue.start),
                  style: theme.textTheme.bodySmall!.copyWith(
                    color: theme.primaryColor,
                  ),
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: Text(
                    cue.text,
                    style: theme.textTheme.bodyMedium!.copyWith(
                      fontStyle: isAd ? FontStyle.italic : FontStyle.normal,
                    ),
                  ),
                ),
              ],
            ),
          ),
        );
      }).toList(),
    );
  }
}
