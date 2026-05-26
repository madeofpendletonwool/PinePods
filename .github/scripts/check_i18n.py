#!/usr/bin/env python3
"""
Check Rust/Yew source files for hardcoded UI strings that should use i18n.

Usage:
  Full scan:  python3 check_i18n.py web/src/
  Diff mode:  git diff origin/main...HEAD | python3 check_i18n.py --diff

Suppress a false positive by adding  // i18n-ignore  to the end of the line.
"""

import re
import sys
from pathlib import Path

# ── Heuristics ────────────────────────────────────────────────────────────────

# Tailwind keywords — any string containing one of these is a CSS class list
_CSS_KEYWORDS = [
    "flex", "grid", "items-", "justify-", "text-", "bg-", "px-", "py-",
    "pt-", "pb-", "pl-", "pr-", "p-", "w-", "h-", "mx-", "my-", "mt-",
    "mb-", "ml-", "mr-", "m-", "rounded", "border", "font-", "gap-",
    "hover:", "dark:", "focus:", "active:", "space-", "hidden", "block",
    "inline", "flex-", "grid-", "cursor-", "opacity-", "shadow", "ring-",
    "stroke-", "fill-", "transition", "duration-", "leading-", "tracking-",
    "align-", "overflow", "z-", "max-", "min-", "relative", "absolute",
    "fixed", "sticky", "scale-", "rotate-", "translate-", "origin-",
    "col-", "row-", "self-", "basis-", "shrink", "grow",
]

# Attribute names whose values are never user-visible text
_SKIP_ATTR_RE = re.compile(
    r'(?:class|id|name|type|href|src|action|method|target|rel|role|tabindex'
    r'|style|autocomplete|enctype|accept|pattern|d|viewBox|xmlns'
    r'|stroke-width|stroke-linecap|stroke-linejoin|fill|stroke|transform'
    r'|data-[\w-]+|aria-[\w-]+)\s*=\s*\{',
    re.IGNORECASE,
)

# Regex patterns for strings that are clearly technical values (not UI text)
_TECHNICAL_RE = re.compile(
    r'^('
    r'[a-z][a-z0-9_-]*'                           # all-lowercase identifier / snake_case
    r'|[A-Z][A-Z0-9_]+'                            # ALL_CAPS constant
    r'|\d+(\.\d+)?'                                # number
    r'|https?://\S+'                               # URL
    r'|/[a-zA-Z][\w/.-]*'                          # filesystem/URL path
    r'|[a-z][\w]*(\.[a-z][\w.]*){1,}'             # dot.notation.key (i18n key pattern)
    r'|[MmLlHhVvCcSsQqTtAaZz][\d\s,.\-MmLlHhVvCcSsQqTtAaZz]+' # SVG path data
    r')$'
)

# Line-level context that indicates the string is NOT rendered as UI text
_NON_UI_CONTEXT_RE = re.compile(
    r'(?:'
    r'[!=]=\s*"'                                   # comparison: == "..." or != "..."
    r'|\.(?:contains|starts_with|ends_with|find|split|replace)\('  # string methods
    r'|(?:log|error|warn|info|debug|console_log)!\('  # logging macros
    r'|(?:panic|todo|unreachable|unimplemented)!\('    # dev macros
    r'|Some\(|Ok\(|Err\('                          # Result/Option wrapping
    r')'
)


def _is_css(s: str) -> bool:
    return any(kw in s for kw in _CSS_KEYWORDS)


def is_ui_string(s: str) -> bool:
    """Return True if s looks like user-visible UI text that should be translated."""
    s = s.strip()
    if len(s) < 2:
        return False
    if _is_css(s):
        return False
    if _TECHNICAL_RE.match(s):
        return False

    words = s.split()

    # Multi-word string starting with uppercase → almost certainly UI text
    if len(words) >= 2 and words[0][0].isupper():
        return True

    # Single capitalised word like "Username", "Password", "Completed", "Edit"
    # Require 3+ chars and a simple Capitalised pattern (excludes ALL_CAPS and CamelCase)
    if len(words) == 1 and re.match(r'^[A-Z][a-z]{2,}$', s):
        return True

    return False


# ── Line checker ─────────────────────────────────────────────────────────────

# Matches {"..."} or { "..." } — the Yew HTML text-node syntax
_TEXT_NODE_RE = re.compile(r'\{\s*"((?:[^"\\]|\\.)*)"\s*\}')


def check_line(line: str) -> list:
    """Return list of flagged (string_value,) tuples found on this line."""
    stripped = line.lstrip()
    if stripped.startswith('//') or stripped.startswith('/*'):
        return []
    if '// i18n-ignore' in line:
        return []

    flagged = []
    for m in _TEXT_NODE_RE.finditer(line):
        val = m.group(1)
        if not is_ui_string(val):
            continue
        before = line[:m.start()]
        # Skip if it follows a technical HTML attribute (class=, id=, d=, …)
        if _SKIP_ATTR_RE.search(before):
            continue
        # Skip if the surrounding code context is non-UI
        if _NON_UI_CONTEXT_RE.search(before):
            continue
        flagged.append(val)
    return flagged


# ── Scan modes ───────────────────────────────────────────────────────────────

def scan_file(path: Path) -> list:
    violations = []
    try:
        lines = path.read_text('utf-8').splitlines()
    except Exception:
        return violations
    for lineno, line in enumerate(lines, 1):
        for s in check_line(line):
            violations.append({
                'file': str(path),
                'line': lineno,
                'string': s,
                'context': line.strip(),
            })
    return violations


def scan_directory(directory: str) -> list:
    violations = []
    for path in Path(directory).rglob('*.rs'):
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
            if current_file and current_file.endswith('.rs'):
                code_line = line[1:]
                for s in check_line(code_line):
                    violations.append({
                        'file': current_file,
                        'line': current_lineno,
                        'string': s,
                        'context': code_line.strip(),
                    })
        elif not line.startswith('-'):
            current_lineno += 1

    return violations


# ── Entry point ──────────────────────────────────────────────────────────────

def main():
    args = sys.argv[1:]

    if '--diff' in args:
        violations = scan_diff(sys.stdin)
        mode = 'diff'
    elif args:
        violations = scan_directory(args[0])
        mode = 'full'
    else:
        print(
            "Usage:\n"
            "  Full scan:  python3 check_i18n.py web/src/\n"
            "  Diff mode:  git diff origin/main...HEAD | python3 check_i18n.py --diff",
            file=sys.stderr,
        )
        sys.exit(2)

    if not violations:
        if mode == 'diff':
            print("✓ No new hardcoded UI strings introduced in this diff.")
        else:
            print("✓ No hardcoded UI strings found.")
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
    print("  1. Add a translation key to web/src/translations/en.json (and other locales)")
    print("  2. Use  let i18n_label = i18n.t(\"section.key\").to_string();")
    print("  3. Render with  { &i18n_label }  inside the html! macro")
    print()
    print("To suppress a false positive, add  // i18n-ignore  to the end of the line.")
    sys.exit(1)


if __name__ == '__main__':
    main()
