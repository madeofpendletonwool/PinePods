#!/usr/bin/env python3
"""
Check Flutter/Dart source files for hardcoded UI strings that should use i18n.

Usage:
  Full scan:  python3 check_i18n_dart.py mobile/lib/
  Diff mode:  git diff origin/main...HEAD | python3 check_i18n_dart.py --diff

Suppress a false positive by adding  // i18n-ignore  to the end of the line.
"""

import re
import sys
from pathlib import Path

# Generated localizations are never scanned; theme names are proper names
# (the web app leaves them untranslated too).
_SKIP_PATHS = ('lib/l10n/', 'lib/ui/themes.dart')

# A single- or double-quoted Dart string literal (no raw/triple-quoted strings).
_STRING_RE = re.compile(r"""(?<![r\w])(['"])((?:\\.|(?!\1)[^\\])*)\1""")

# Interpolations are replaced before the text heuristics run.
_INTERP_RE = re.compile(r'\$\{[^}]*\}|\$[A-Za-z_]\w*')

_TECHNICAL_RE = re.compile(
    r'^('
    r'[a-z][a-z0-9_.\-/]*'                        # identifiers, keys, paths
    r'|[A-Z][A-Z0-9_]+'                            # ALL_CAPS constants
    r'|[\d\s.,:%x+\-]*'                            # numbers, times, separators
    r'|https?://\S+'                               # URLs
    r'|[\w.\-]+/[\w.\-+]+'                         # mime types, path fragments
    r'|#[0-9a-fA-F]{3,8}'                          # colours
    r'|[A-Z][a-zA-Z]+[A-Z]\w*'                     # CamelCase identifiers
    r')$'
)

# Code that is never user-visible text.
_NON_UI_CONTEXT_RE = re.compile(
    r'(?:'
    r'^\s*(?:import|export|part)\b'
    r'|\b(?:_devLog|_logCrash|Logger|log|_log|logger|_logger|developer\.log|AppLogger\(\))(?:\.\w+)?\('
    r'|\b(?:print|debugPrint|stderr\.writeln)\('
    r'|\b(?:assert)\('
    r'|[!=]=\s*$'
    r'|\.(?:contains|startsWith|endsWith|split|replaceAll|replaceFirst|indexOf|matches)\(\s*$'
    r'|\b(?:getString|setString|getBool|setBool|getInt|setInt|getDouble|setDouble|remove|containsKey)\(\s*$'
    r'|\bcase\s*$'
    r'|\bRegExp\(\s*$'
    r'|\bKey\(\s*$'
    r'|\bfontFamily:\s*$'
    r'|\brouteName\b'
    r'|\bheaders?\b'
    r'|\bjson\[\s*$'
    r'|\[\s*$'
    r')'
)

# A string that is immediately used as a map key or JSON field.
_MAP_KEY_AFTER_RE = re.compile(r'^\s*(?:\]|=>|:(?!\s*\w+\())')


def is_ui_string(s: str) -> bool:
    """Return True if s looks like user-visible UI text that should be translated."""
    text = _INTERP_RE.sub('', s).replace('\\n', ' ').strip()
    if len(text) < 2:
        return False
    if _TECHNICAL_RE.match(text):
        return False
    words = text.split()
    # Starts with an interpolation, e.g. '$count episodes' or '${n} min left'.
    if s.lstrip().startswith('$') and ' ' in _INTERP_RE.sub('', s) and re.search(r'[A-Za-z]{3,}', text) \
            and not re.search(r'[/\\]|\$\{|\(', text):
        return True
    if not words or not re.match(r'[A-Za-z]', words[0]):
        return False
    # Multi-word string starting with uppercase -> almost certainly UI text.
    if len(words) >= 2 and words[0][0].isupper():
        return True
    # Single capitalised word like "Downloads", "Cancel".
    if len(words) == 1 and re.match(r'^(?=.{3})[A-Z][a-z]+(?:-[a-z]+)*[.!?:…]?$', text):
        return True
    return False


def check_line(line: str) -> list:
    stripped = line.lstrip()
    if stripped.startswith('//') or stripped.startswith('*') or stripped.startswith('/*'):
        return []
    if '// i18n-ignore' in line:
        return []
    # Ignore trailing comments.
    code = re.sub(r'\s//\s.*$', '', line)

    flagged = []
    for m in _STRING_RE.finditer(code):
        val = m.group(2)
        if not is_ui_string(val):
            continue
        before = code[:m.start()]
        after = code[m.end():]
        if _NON_UI_CONTEXT_RE.search(before):
            continue
        if _MAP_KEY_AFTER_RE.match(after):
            continue
        if re.search(r'\bthrow\b|Exception\(|Error\(', before):
            continue
        flagged.append(val)
    return flagged


def _skipped(path: str) -> bool:
    p = path.replace('\\', '/')
    return any(s in p for s in _SKIP_PATHS)


def scan_file(path: Path) -> list:
    violations = []
    if _skipped(str(path)):
        return violations
    try:
        lines = path.read_text('utf-8').splitlines()
    except Exception:
        return violations
    for lineno, line in enumerate(lines, 1):
        for s in check_line(line):
            violations.append({'file': str(path), 'line': lineno, 'string': s, 'context': line.strip()})
    return violations


def scan_directory(directory: str) -> list:
    violations = []
    root = Path(directory)
    paths = [root] if root.is_file() else sorted(root.rglob('*.dart'))
    for path in paths:
        violations.extend(scan_file(path))
    return violations


def scan_diff(stream) -> list:
    """Parse unified git diff from stream; check only added (+) lines."""
    violations = []
    current_file = None
    current_lineno = 0
    for raw_line in stream:
        line = raw_line.rstrip('\n')
        if line.startswith('+++ b/'):
            current_file = line[6:]
            current_lineno = 0
            continue
        if line.startswith('---') or line.startswith('diff ') or line.startswith('index '):
            continue
        hunk = re.match(r'^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@', line)
        if hunk:
            current_lineno = int(hunk.group(1)) - 1
            continue
        if line.startswith('+'):
            current_lineno += 1
            if current_file and current_file.endswith('.dart') and not _skipped(current_file):
                code_line = line[1:]
                for s in check_line(code_line):
                    violations.append({'file': current_file, 'line': current_lineno, 'string': s,
                                       'context': code_line.strip()})
        elif not line.startswith('-'):
            current_lineno += 1
    return violations


def main():
    args = sys.argv[1:]
    if '--diff' in args:
        violations = scan_diff(sys.stdin)
        mode = 'diff'
    elif args:
        violations = [v for a in args for v in scan_directory(a)]
        mode = 'full'
    else:
        print("Usage:\n"
              "  Full scan:  python3 check_i18n_dart.py mobile/lib/\n"
              "  Diff mode:  git diff origin/main...HEAD | python3 check_i18n_dart.py --diff",
              file=sys.stderr)
        sys.exit(2)

    if not violations:
        print("✓ No new hardcoded UI strings introduced in this diff." if mode == 'diff'
              else "✓ No hardcoded UI strings found.")
        sys.exit(0)

    label = "new " if mode == 'diff' else ""
    print(f"Found {len(violations)} {label}hardcoded UI string(s) that should use i18n:\n")
    for v in violations:
        print(f"  {v['file']}:{v['line']}")
        print(f"    String:  \"{v['string']}\"")
        print(f"    Context: {v['context']}")
        print()
    print("─" * 60)
    print("How to fix:")
    print("  1. Add the key to mobile/lib/l10n/app_en.arb (English only; Weblate handles the rest)")
    print("  2. Use  L.of(context)!.your_key  in the widget")
    print()
    print("To suppress a false positive, add  // i18n-ignore  to the end of the line.")
    sys.exit(1)


if __name__ == '__main__':
    main()
