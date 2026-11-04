import 'package:html/parser.dart' show parseFragment;

final _lineBreak = RegExp(r'<br\b[^>]*>', caseSensitive: false);
final _paragraphEnd = RegExp(r'</p\s*>', caseSensitive: false);
final _blockEnd = RegExp(r'</(div|li|ul|ol|blockquote|h[1-6])\s*>', caseSensitive: false);
final _inlineSpace = RegExp(r'[ \t ]+');
final _spaceAroundNewline = RegExp(r' *\n *');
final _extraNewlines = RegExp(r'\n{3,}');

/// Converts a feed description that may contain HTML to plain text, keeping
/// line and paragraph breaks and decoding entities. Text without markup passes
/// through unchanged apart from whitespace cleanup.
String htmlToPlainText(String input) {
  final marked = input
      .replaceAll(_lineBreak, '\n')
      .replaceAll(_paragraphEnd, '\n\n')
      .replaceAll(_blockEnd, '\n');
  return (parseFragment(marked).text ?? '')
      .replaceAll(_inlineSpace, ' ')
      .replaceAll(_spaceAroundNewline, '\n')
      .replaceAll(_extraNewlines, '\n\n')
      .trim();
}
