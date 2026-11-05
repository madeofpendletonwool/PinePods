#!/bin/bash
set -e

# Run as the service user: root may not be allowed to write to NFS downloads.
# Keep self-updated versions across restarts; seed only when no executable exists.
mkdir -p "$(dirname "$YTDLP_PATH")"
if [ ! -x "$YTDLP_PATH" ] && [ -x /usr/local/bin/yt-dlp ]; then
    echo "Seeding managed yt-dlp binary from image..."
    cp /usr/local/bin/yt-dlp "$YTDLP_PATH"
    chmod +x "$YTDLP_PATH"
fi

echo "PinePods startup complete, running Horust in foreground..."
exec horust --services-path /etc/horust/services/
