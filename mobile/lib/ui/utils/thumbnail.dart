import 'package:pinepods_mobile/services/global_services.dart';

/// Widths the server's image proxy produces. Requests round up to one of these so the
/// on-device cache holds a few variants per image rather than one per pixel width.
const _thumbWidths = [64, 128, 192, 256, 384, 512, 1024];

/// [url] as a server-made thumbnail about [pixels] wide, so lists don't download full-size
/// artwork. Returns [url] unchanged when signed out, for images the server already hosts,
/// for non-http sources, and for anything too wide to bother.
String thumbnailUrl(String url, double pixels, {String? server}) {
  server ??= GlobalServices.pinepodsService?.server;
  if (server == null || server.isEmpty || !pixels.isFinite || pixels <= 0) return url;
  if (!(url.startsWith('http://') || url.startsWith('https://')) || url.startsWith(server)) return url;
  final width = _thumbWidths.where((w) => w >= pixels).firstOrNull;
  if (width == null) return url;
  return '$server/api/proxy/image?url=${Uri.encodeQueryComponent(url)}&w=$width';
}
