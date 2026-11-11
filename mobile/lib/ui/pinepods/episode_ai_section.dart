// lib/ui/pinepods/episode_ai_section.dart
//
// The AI features block on the episode details screen (#726 transcripts /
// #790 ad-block). Shows the server-generated transcript as timecoded,
// tap-to-seek lines (ad ranges highlighted), a per-ad Confirm/Deny review
// list, and a Detect-ads button. The whole block hides unless the server
// reports the AI sidecar as available.
import 'dart:convert';
import 'dart:io';
import 'package:flutter/material.dart';
import 'package:path_provider/path_provider.dart';
import 'package:share_plus/share_plus.dart';
import 'package:pinepods_mobile/services/pinepods/pinepods_service.dart';
import 'package:pinepods_mobile/entities/pinepods_episode.dart';
import 'package:pinepods_mobile/ui/icons/phosphor_icons.dart';
import 'package:pinepods_mobile/l10n/L.dart';

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
  AdAudio? _adAudio;
  List<TranscriptCue> _cues = const [];
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
        widget.pinepodsService.getEpisodeSkipInfo(
            widget.episode.episodeId, widget.userId),
      ]);
      final transcript = results[0] as StoredTranscript?;
      final info = results[1] as SkipInfo;

      if (mounted) {
        setState(() {
          _aiStatus = status;
          _transcript = transcript;
          _adSegments = info.segments.where((s) => s.kind == 'ad').toList();
          _adAudio = info.adAudio;
          _cues = transcript?.cues ?? const [];
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
      final info = await widget.pinepodsService
          .getEpisodeSkipInfo(widget.episode.episodeId, widget.userId);
      if (mounted) {
        setState(() {
          _adSegments = info.segments.where((s) => s.kind == 'ad').toList();
          _adAudio = info.adAudio;
        });
      }
      await widget.onSegmentsChanged();
    } catch (_) {}
  }

  bool _cueIsAd(TranscriptCue cue) {
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

  /// Many podcasts insert different ads every time an episode is fetched, so
  /// ads found on a one-off copy can land at the wrong times in the copy that
  /// gets played. When the episode isn't on the server, offer to download it
  /// there first so detection and playback use the same copy.
  Future<void> _detectAds() async {
    if (_busy) return;
    if (!widget.episode.downloaded && _aiStatus.downloadsEnabled) {
      final choice = await showDialog<String>(
        context: context,
        builder: (context) => AlertDialog(
          title: Text(L.of(context)!.not_downloaded_to_server),
          content: Text(
            L.of(context)!.not_downloaded_ads_explainer,
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(context),
              child: Text(L.of(context)!.cancel_button_label),
            ),
            TextButton(
              onPressed: () => Navigator.pop(context, 'stream'),
              child: Text(L.of(context)!.detect_anyway),
            ),
            FilledButton(
              onPressed: () => Navigator.pop(context, 'download'),
              child: Text(L.of(context)!.download_first),
            ),
          ],
        ),
      );
      if (choice == null) return;
      await _startDetect(downloadFirst: choice == 'download');
    } else {
      await _startDetect(downloadFirst: false);
    }
  }

  Future<void> _startDetect({required bool downloadFirst}) async {
    if (_busy) return;
    setState(() => _busy = true);
    try {
      final ok = await widget.pinepodsService.detectAds(
          widget.episode.episodeId, widget.userId,
          force: true, downloadFirst: downloadFirst);
      _snack(!ok
          ? L.current.ai_unavailable
          : downloadFirst
              ? L.current.downloading_to_server_for_ads
              : L.current.ad_detection_queued);
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
          ? L.current.transcription_queued
          : L.current.ai_unavailable);
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
        _snack(status == 'confirmed' ? L.current.ad_confirmed : L.current.ad_kept);
        await _reloadSegments();
      } else {
        _snack(L.current.could_not_update_ad);
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
          Icon(PhosphorIconsRegular.sparkle, size: 20, color: theme.primaryColor),
          const SizedBox(width: 8),
          Text(
            L.of(context)!.ai_features_title,
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
            icon: const Icon(PhosphorIconsRegular.prohibit, size: 18),
            label: Text(L.of(context)!.detect_ads),
          ),
        if (_transcript?.status == 'complete')
          PopupMenuButton<String>(
            tooltip: L.of(context)!.export_transcript,
            onSelected: _exportTranscript,
            itemBuilder: (context) => [
              PopupMenuItem(value: 'srt', child: Text(L.of(context)!.export_srt)),
              PopupMenuItem(value: 'vtt', child: Text(L.of(context)!.export_vtt)),
              PopupMenuItem(value: 'txt', child: Text(L.of(context)!.export_txt)),
            ],
            child: IgnorePointer(
              child: OutlinedButton.icon(
                onPressed: () {},
                icon: const Icon(PhosphorIconsRegular.export, size: 18),
                label: Text(L.of(context)!.export_transcript),
              ),
            ),
          ),
        if (_aiStatus.transcriptionReady)
          OutlinedButton.icon(
            onPressed: _busy ? null : _transcribe,
            icon: const Icon(PhosphorIconsRegular.userSound, size: 18),
            label: Text(_transcript?.status == 'complete'
                ? L.of(context)!.re_transcribe
                : L.of(context)!.transcribe),
          ),
      ],
    ));

    // How the ad times relate to the copy this user plays.
    final accuracy = _adSegments.isEmpty ? null : _buildAdAccuracy(theme);
    if (accuracy != null) {
      children.add(const SizedBox(height: 12));
      children.add(accuracy);
    }

    // Detected-ads review list.
    if (_adSegments.isNotEmpty) {
      children.add(const SizedBox(height: 16));
      children.add(Text(
        L.of(context)!.detected_ads,
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
        children: [
          const SizedBox(
              width: 16,
              height: 16,
              child: CircularProgressIndicator(strokeWidth: 2)),
          const SizedBox(width: 8),
          Text(L.of(context)!.transcribing),
        ],
      ));
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: children,
    );
  }

  Widget? _buildAdAccuracy(ThemeData theme) {
    final String text;
    final bool ok;
    var offerDownload = false;
    switch (_adAudio?.download) {
      case 'match':
        ok = true;
        text = L.of(context)!.ads_match_server_download;
      case 'mismatch':
        ok = false;
        text = L.of(context)!.ads_mismatch_server_download;
      case 'none':
        ok = false;
        offerDownload = _aiStatus.downloadsEnabled;
        text = L.of(context)!.ads_streaming_explainer;
      default:
        return null;
    }
    final color = ok ? Colors.green : Colors.orange;
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: color.withOpacity(0.12),
        borderRadius: BorderRadius.circular(8),
        border: Border(left: BorderSide(color: color, width: 3)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Icon(ok ? PhosphorIconsRegular.checkCircle : PhosphorIconsRegular.warning,
                  size: 18, color: color),
              const SizedBox(width: 8),
              Expanded(child: Text(text, style: theme.textTheme.bodySmall)),
            ],
          ),
          if (offerDownload) ...[
            const SizedBox(height: 8),
            OutlinedButton.icon(
              onPressed: _busy ? null : () => _startDetect(downloadFirst: true),
              icon: const Icon(PhosphorIconsRegular.cloudArrowDown, size: 18),
              label: Text(L.of(context)!.download_to_server),
            ),
          ],
        ],
      ),
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
              skipping ? L.of(context)!.ad_skipping : L.of(context)!.ad_kept_label,
              style: theme.textTheme.bodySmall!.copyWith(
                color: skipping ? Colors.green[700] : Colors.grey[700],
              ),
            ),
          ),
          IconButton(
            tooltip: L.of(context)!.confirm_ad_skip,
            visualDensity: VisualDensity.compact,
            icon: const Icon(PhosphorIconsRegular.check, size: 20),
            color: Colors.green,
            onPressed: _busy ? null : () => _review(ad, 'confirmed'),
          ),
          IconButton(
            tooltip: L.of(context)!.keep_ad_no_skip,
            visualDensity: VisualDensity.compact,
            icon: const Icon(PhosphorIconsRegular.x, size: 20),
            color: Colors.red,
            onPressed: _busy ? null : () => _review(ad, 'rejected'),
          ),
        ],
      ),
    );
  }

  /// Save the transcript as a file through the system share sheet.
  Future<void> _exportTranscript(String format) async {
    final messenger = ScaffoldMessenger.of(context);
    final box = context.findRenderObject() as RenderBox?;
    try {
      final body = await widget.pinepodsService
          .exportEpisodeTranscript(widget.episode.episodeId, widget.userId, format);
      final title = widget.episode.episodeTitle.replaceAll(RegExp(r'[/\\:*?"<>|\x00-\x1F]'), '_').trim();
      final name = '${title.isEmpty ? 'episode-${widget.episode.episodeId}' : title} - transcript.$format';
      final file = File('${(await getTemporaryDirectory()).path}/$name');
      await file.writeAsString(body);
      await SharePlus.instance.share(ShareParams(
        files: [XFile(file.path, mimeType: format == 'vtt' ? 'text/vtt' : 'text/plain')],
        subject: name,
        sharePositionOrigin: box == null ? null : box.localToGlobal(Offset.zero) & box.size,
      ));
    } catch (e) {
      messenger.showSnackBar(SnackBar(content: Text(L.current.could_not_export_transcript(e.toString()))));
    }
  }

  Widget _buildTranscript(ThemeData theme) {
    return ExpansionTile(
      tilePadding: EdgeInsets.zero,
      title: Text(
        L.of(context)!.transcript_label,
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
