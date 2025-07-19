#!/bin/bash

# Ensure app has time to start
sleep 10

echo "Getting background tasks API key..."

# Get API key from database for background_tasks user (UserID = 1)
if [ "$DB_TYPE" = "postgresql" ]; then
    API_KEY=$(PGPASSWORD="$DB_PASSWORD" psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -t -c 'SELECT apikey FROM "APIKeys" WHERE userid = 1 LIMIT 1;' 2>/dev/null | xargs)
else
    API_KEY=$(mysql -h "$DB_HOST" -P "$DB_PORT" -u "$DB_USER" -p"$DB_PASSWORD" "$DB_NAME" -se 'SELECT APIKey FROM APIKeys WHERE UserID = 1 LIMIT 1;' 2>/dev/null)
fi

if [ -z "$API_KEY" ]; then
    echo "Error: Could not retrieve API key for background tasks"
    exit 1
fi

# Initialize application tasks
echo "Initializing application tasks..."
curl -X POST "http://localhost:8032/api/init/startup_tasks" \
    -H "Content-Type: application/json" \
    -d "{\"api_key\": \"$API_KEY\"}" >> /cron.log 2>&1
