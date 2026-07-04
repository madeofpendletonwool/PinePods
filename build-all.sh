#!/usr/bin/env bash
# Build both PinePods images: the main app and the optional AI (transcription) sidecar.
# Run from anywhere; it always builds from the repo root.
set -euo pipefail
cd "$(dirname "$0")"

TAG="${1:-latest}"

echo "==> Building madeofpendletonwool/pinepods:${TAG} (main app)"
docker build -t "madeofpendletonwool/pinepods:${TAG}" .

echo "==> Building madeofpendletonwool/pinepods-ai:${TAG} (AI sidecar)"
docker build -t "madeofpendletonwool/pinepods-ai:${TAG}" ./pinepods-ai

echo "==> Done. Built:"
echo "    madeofpendletonwool/pinepods:${TAG}"
echo "    madeofpendletonwool/pinepods-ai:${TAG}"
