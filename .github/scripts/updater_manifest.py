#!/usr/bin/env python3
"""Write the desktop app's updater manifest (latest.json) for a release.

Usage: updater_manifest.py <tag> <owner/repo> <dir with the release's .sig files>

Each signed update bundle has a <bundle>.sig next to it on the release; the manifest maps
each platform to its bundle's download URL and signature.
"""
import json
import sys
from datetime import datetime, timezone
from pathlib import Path

# (suffix of the signed file, platform keys the updater looks up)
TARGETS = [
    ("_aarch64.app.tar.gz", ["darwin-aarch64", "darwin-aarch64-app"]),
    ("_amd64.AppImage", ["linux-x86_64", "linux-x86_64-appimage"]),
    ("_aarch64.AppImage", ["linux-aarch64", "linux-aarch64-appimage"]),
    ("_x64-setup.exe", ["windows-x86_64", "windows-x86_64-nsis"]),
    ("_x64_en-US.msi", ["windows-x86_64-msi"]),
]


def manifest(tag: str, repo: str, sig_dir: Path) -> dict:
    platforms = {}
    for sig in sorted(sig_dir.glob("*.sig")):
        bundle = sig.name[: -len(".sig")]
        for suffix, keys in TARGETS:
            if bundle.endswith(suffix):
                entry = {
                    "signature": sig.read_text().strip(),
                    "url": f"https://github.com/{repo}/releases/download/{tag}/{bundle}",
                }
                for key in keys:
                    platforms[key] = entry
    return {
        "version": tag.lstrip("v"),
        "notes": f"https://github.com/{repo}/releases/tag/{tag}",
        "pub_date": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "platforms": platforms,
    }


if __name__ == "__main__":
    tag, repo, sig_dir = sys.argv[1], sys.argv[2], Path(sys.argv[3])
    result = manifest(tag, repo, sig_dir)
    if not result["platforms"]:
        sys.exit("no update signatures found")
    json.dump(result, sys.stdout, indent=2)
    print()
