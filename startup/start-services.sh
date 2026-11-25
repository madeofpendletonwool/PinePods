#!/bin/bash
set -e

# Run as the service user: root may not be allowed to write to NFS downloads.
# Keep self-updated versions across restarts; seed only when no executable exists.
if [ ! -x "$YTDLP_PATH" ] && [ -x /usr/local/bin/yt-dlp ]; then
    echo "Seeding managed yt-dlp binary from image..."
    if ! { mkdir -p "$(dirname "$YTDLP_PATH")" && cp /usr/local/bin/yt-dlp "$YTDLP_PATH" && chmod +x "$YTDLP_PATH"; }; then
        # Don't block startup over yt-dlp: use the image's copy (works, but can't self-update).
        echo "WARNING: could not seed yt-dlp at $YTDLP_PATH; using /usr/local/bin/yt-dlp (no self-update)"
        export YTDLP_PATH=/usr/local/bin/yt-dlp
    fi
fi

echo "PinePods startup complete, running Horust in foreground..."
exec horust --services-path /etc/horust/services/
