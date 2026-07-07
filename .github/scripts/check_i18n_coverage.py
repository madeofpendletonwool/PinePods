#!/usr/bin/env python3
"""
Check that every i18n key referenced in Rust source exists in en.json.

Usage:
  Full scan:  python3 check_i18n_coverage.py [web/src/] [web/src/translations/en.json]
  Diff mode:  git diff origin/main...HEAD | python3 check_i18n_coverage.py --diff [web/src/translations/en.json]

Defaults:
  source dir  → web/src/
  en.json     → web/src/translations/en.json

Diff mode only fails on keys introduced in the diff that are absent from en.json.
Orphan reporting is skipped in diff mode.

Exit codes:
  0  all checked keys are present
  1  missing keys found
  2  bad arguments / file not found
"""

import json
import re
import sys
from pathlib import Path

_KEY_RE = re.compile(r'i18n\.t\("([^"]+)"\)')


def flatten_json(obj: dict, prefix: str = "") -> set[str]:
    """Flatten nested JSON into a set of dotted keys."""
    keys: set[str] = set()
    for k, v in obj.items():
        full = f"{prefix}.{k}" if prefix else k
        if isinstance(v, dict):
            keys |= flatten_json(v, full)
        else:
            keys.add(full)
    return keys


def collect_source_keys(src_dir: Path) -> dict[str, list[tuple[str, int]]]:
    """Return {dotted_key: [(file, lineno), ...]} for every i18n.t("...") call."""
    refs: dict[str, list[tuple[str, int]]] = {}
    for path in sorted(src_dir.rglob("*.rs")):
        try:
            lines = path.read_text("utf-8").splitlines()
        except Exception:
            continue
        for lineno, line in enumerate(lines, 1):
            stripped = line.lstrip()
            if stripped.startswith("//") or stripped.startswith("/*"):
                continue
            for m in _KEY_RE.finditer(line):
                key = m.group(1)
                refs.setdefault(key, []).append((str(path), lineno))
    return refs


def collect_diff_keys(stream) -> dict[str, list[tuple[str, int]]]:
    """Extract i18n keys only from added lines in a unified diff."""
    refs: dict[str, list[tuple[str, int]]] = {}
    current_file = None
    current_lineno = 0

    for raw_line in stream:
        line = raw_line.rstrip("\n")

        if line.startswith("+++ b/"):
            current_file = line[6:]
            current_lineno = 0
            continue
        if line.startswith("---") or line.startswith("diff ") or line.startswith("index "):
            continue

        hunk = re.match(r"^@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@", line)
        if hunk:
            current_lineno = int(hunk.group(1)) - 1
            continue

        if line.startswith("+"):
            current_lineno += 1
            if current_file and current_file.endswith(".rs"):
                code_line = line[1:]
                stripped = code_line.lstrip()
                if stripped.startswith("//") or stripped.startswith("/*"):
                    continue
                for m in _KEY_RE.finditer(code_line):
                    key = m.group(1)
                    refs.setdefault(key, []).append((current_file, current_lineno))
        elif not line.startswith("-"):
            current_lineno += 1

    return refs


def report_missing(missing: dict[str, list[tuple[str, int]]], label: str) -> None:
    print(f"MISSING from en.json — {len(missing)} {label}key(s) referenced in code but not defined:\n")
    for key in sorted(missing):
        print(f"  \"{key}\"")
        for path, lineno in missing[key][:3]:
            print(f"    {path}:{lineno}")
        if len(missing[key]) > 3:
            print(f"    … and {len(missing[key]) - 3} more location(s)")
    print()
    print("─" * 60)
    print("How to fix:")
    print("  Add each key to web/src/translations/en.json under the matching section.")
    print("  Then add the same key to all other locale files (or leave for translators).")


def main() -> None:
    args = sys.argv[1:]
    diff_mode = "--diff" in args
    positional = [a for a in args if not a.startswith("--")]

    if diff_mode:
        en_json = Path(positional[0]) if positional else Path("web/src/translations/en.json")
    else:
        src_dir = Path(positional[0]) if len(positional) > 0 else Path("web/src")
        en_json = Path(positional[1]) if len(positional) > 1 else Path("web/src/translations/en.json")

    if not en_json.is_file():
        print(f"Error: en.json not found: {en_json}", file=sys.stderr)
        sys.exit(2)

    defined_keys = flatten_json(json.loads(en_json.read_text("utf-8")))

    if diff_mode:
        source_refs = collect_diff_keys(sys.stdin)
        if not source_refs:
            print("✓ No new i18n key references in this diff.")
            sys.exit(0)
        missing = {k: v for k, v in source_refs.items() if k not in defined_keys}
        if not missing:
            print(f"✓ All {len(source_refs)} new i18n key reference(s) are present in en.json.")
            sys.exit(0)
        report_missing(missing, "new ")
        sys.exit(1)
    else:
        if not src_dir.is_dir():
            print(f"Error: source directory not found: {src_dir}", file=sys.stderr)
            sys.exit(2)
        source_refs = collect_source_keys(src_dir)
        missing = {k: v for k, v in source_refs.items() if k not in defined_keys}
        orphaned = defined_keys - set(source_refs)

        if not missing and not orphaned:
            print(f"✓ All {len(source_refs)} referenced keys are present in en.json.")
            sys.exit(0)

        if missing:
            report_missing(missing, "")
            print()

        if orphaned:
            print(f"ORPHANED in en.json — {len(orphaned)} key(s) defined but never referenced in code:\n")
            for key in sorted(orphaned):
                print(f"  \"{key}\"")
            print()

        if missing:
            sys.exit(1)


if __name__ == "__main__":
    main()
