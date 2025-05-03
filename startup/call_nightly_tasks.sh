#!/bin/bash

sleep 10
# Read the API key from /tmp/web_api_key.txt
API_KEY=$(cat /tmp/web_api_key.txt)


# Call the FastAPI endpoint using the API key
# Run cleanup tasks
echo "Running nightly tasks..."
curl -X GET "http://localhost:8032/api/data/refresh_hosts" -H "Api-Key: $API_KEY" >> /cron.log 2>&1
