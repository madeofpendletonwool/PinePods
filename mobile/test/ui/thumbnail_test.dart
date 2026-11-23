import 'package:flutter_test/flutter_test.dart';
import 'package:pinepods_mobile/ui/utils/thumbnail.dart';

void main() {
  const server = 'https://pods.example.com';
  const art = 'https://cdn.example.org/show/cover.jpg?v=2';

  test('artwork goes through the server proxy at the next fixed width', () {
    final url = Uri.parse(thumbnailUrl(art, 300, server: server));
    expect(url.origin, server);
    expect(url.path, '/api/proxy/image');
    expect(url.queryParameters['url'], art);
    expect(url.queryParameters['w'], '384');
  });

  test('left alone when signed out, server-hosted, local, or too wide', () {
    expect(thumbnailUrl(art, 300), art);
    expect(thumbnailUrl('$server/api/data/art.png', 300, server: server), '$server/api/data/art.png');
    expect(thumbnailUrl('file:///tmp/a.jpg', 300, server: server), 'file:///tmp/a.jpg');
    expect(thumbnailUrl(art, 2000, server: server), art);
    expect(thumbnailUrl(art, double.infinity, server: server), art);
  });
}
