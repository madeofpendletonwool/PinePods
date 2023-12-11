#!/bin/bash

# Ensure app has time to start
sleep 10

# Read the API key from /tmp/web_api_key.txt
API_KEY=$(cat /tmp/web_api_key.txt)

# Call the FastAPI endpoint using the API key
echo "Refreshing now!"
curl "http://localhost:8032/api/data/refresh_pods" -H "Api-Key: $API_KEY" >> /cron.log 2>&1
echo "Refreshing Nextcloud Subscription now!"
curl "http://localhost:8032/api/data/refresh_nextcloud_subscription" -H "Api-Key: $API_KEY" >> /cron.log 2>&1