#!/bin/bash
# /usr/local/bin/start-gpodder.sh

# This script is kept for backward compatibility, but shouldn't be needed
# as the gpodder-api is now managed by supervisord

# Check if the gpodder-api is already running under supervisor
if supervisorctl status gpodder_api | grep -q "RUNNING"; then
    echo "gpodder-api is already running under supervisor, exiting"
    exit 0
fi

# Start the gpodder-api only if it's not already managed by supervisor
echo "Starting gpodder-api (standalone mode) with PID logging"
nohup /usr/local/bin/gpodder-api > /var/log/gpodder-api.log 2>&1 &
PID=$!
echo "Started gpodder-api with PID $PID"
echo $PID > /var/run/gpodder-api.pid
