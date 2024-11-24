#!/bin/bash

# Ensure app has time to start
sleep 10

# Read the API key from /tmp/web_api_key.txt
API_KEY=$(cat /tmp/web_api_key.txt)

# Initialize application tasks
echo "Initializing application tasks..."
curl -X POST "http://localhost:8032/api/init/startup_tasks" \
    -H "Content-Type: application/json" \
    -d "{\"api_key\": \"$API_KEY\"}" >> /cron.log 2>&1
