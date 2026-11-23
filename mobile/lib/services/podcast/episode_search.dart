/// Searching one podcast's episodes, matching the server's rules: every term must appear in the
/// title or the description's visible words (not its links or markup), and episodes with the
/// terms in their title come first.
library;

const _maxTerms = 8;
const _maxTermChars = 100;

final _scriptOrStyle = RegExp(r'<(script|style)\b[\s\S]*?</\1\s*>', caseSensitive: false);
final _tag = RegExp(r'<[a-zA-Z/!?][^>]*>');
final _entity = RegExp(r'&(#[xX][0-9a-fA-F]{1,6}|#[0-9]{1,7}|[a-zA-Z]{2,8});');
final _bareAddress = RegExp(r'^[a-z0-9-]+(\.[a-z0-9-]+)*\.[a-z]{2,}/\S*$');

const _named = {
  'amp': '&',
  'lt': '<',
  'gt': '>',
  'quot': '"',
  'apos': "'",
  'rsquo': "'",
  'lsquo': "'",
  'rdquo': '"',
  'ldquo': '"',
  'nbsp': ' ',
  'mdash': '—',
  'ndash': '–',
  'hellip': '…',
  'copy': '©',
  'reg': '®',
  'trade': '™',
};

/// The visible words of an HTML (or plain) description: tags removed, entities decoded, web
/// addresses dropped, whitespace collapsed.
String episodeSearchText(String html) {
  var text = html.replaceAll(_scriptOrStyle, ' ').replaceAll(_tag, ' ');
  text = text.replaceAllMapped(_entity, (m) {
    final name = m[1]!;
    if (name.startsWith('#')) {
      final hex = name.length > 1 && (name[1] == 'x' || name[1] == 'X');
      final code = int.tryParse(name.substring(hex ? 2 : 1), radix: hex ? 16 : 10);
      if (code == null || code > 0x10FFFF) return m[0]!;
      return code == 0xA0 ? ' ' : String.fromCharCode(code);
    }
    return _named[name] ?? m[0]!;
  });
  return text.split(RegExp(r'\s+')).where((w) => w.isNotEmpty && !_isWebAddress(w)).join(' ');
}

bool _isWebAddress(String word) {
  final w = word.replaceFirst(RegExp(r'''^[(\[<"']+'''), '').toLowerCase();
  return w.startsWith('http://') || w.startsWith('https://') || w.startsWith('www.') || _bareAddress.hasMatch(w);
}

/// What a search box holds, split into lowercased terms: words, or "quoted phrases".
class EpisodeSearch {
  final List<String> terms;

  const EpisodeSearch._(this.terms);

  factory EpisodeSearch.parse(String input) {
    final terms = <String>[];
    void add(String t) {
      var term = t.trim().toLowerCase();
      if (term.length > _maxTermChars) term = term.substring(0, _maxTermChars);
      if (term.isNotEmpty && !terms.contains(term) && terms.length < _maxTerms) terms.add(term);
    }

    final parts = input.split('"');
    for (var i = 0; i < parts.length; i++) {
      // Odd parts sit between quotes (an unclosed quote runs to the end).
      if (i.isOdd) {
        add(parts[i].split(RegExp(r'\s+')).where((w) => w.isNotEmpty).join(' '));
      } else {
        parts[i].split(RegExp(r'\s+')).forEach(add);
      }
    }
    return EpisodeSearch._(terms);
  }

  bool get isEmpty => terms.isEmpty;

  /// Both arguments lowercased.
  bool matches(String title, String text) => terms.every((t) => title.contains(t) || text.contains(t));

  /// 0 when every term is in the (lowercased) title, 1 when some are, 2 when none are.
  int titleRank(String title) {
    if (terms.isEmpty) return 0;
    final hits = terms.where(title.contains).length;
    if (hits == terms.length) return 0;
    return hits == 0 ? 2 : 1;
  }
}
