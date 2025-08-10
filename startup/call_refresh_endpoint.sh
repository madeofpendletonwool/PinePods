#!/bin/bash

# Ensure app has time to start
sleep 10

# Get API key directly from database for background_tasks user (UserID 1)
echo "Getting background tasks API key..."

# Database connection parameters
DB_TYPE=${DB_TYPE:-postgresql}
DB_HOST=${DB_HOST:-127.0.0.1}
DB_PORT=${DB_PORT:-5432}
DB_USER=${DB_USER:-postgres}
DB_PASSWORD=${DB_PASSWORD:-password}
DB_NAME=${DB_NAME:-pinepods_database}

if [ "$DB_TYPE" = "postgresql" ]; then
    API_KEY=$(PGPASSWORD="$DB_PASSWORD" psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -t -c 'SELECT apikey FROM "APIKeys" WHERE userid = 1 LIMIT 1;' 2>/dev/null | xargs)
else
    API_KEY=$(mysql -h "$DB_HOST" -P "$DB_PORT" -u "$DB_USER" -p"$DB_PASSWORD" "$DB_NAME" -se 'SELECT APIKey FROM APIKeys WHERE UserID = 1 LIMIT 1;' 2>/dev/null)
fi

if [ -z "$API_KEY" ]; then
    echo "Failed to get background tasks API key from database" >> /cron.log 2>&1
    exit 1
fi

# Call the FastAPI endpoint using the API key
echo "Refreshing now!"
curl "http://localhost:8032/api/data/refresh_pods" -H "Api-Key: $API_KEY" >> /cron.log 2>&1

echo "Refreshing Nextcloud Subscription now!"
curl -X GET -H "Api-Key: $API_KEY" http://localhost:8032/api/data/refresh_nextcloud_subscriptions >> /cron.log 2>&1

# Run cleanup tasks
echo "Running cleanup tasks..."
curl -X GET "http://localhost:8032/api/data/cleanup_tasks" -H "Api-Key: $API_KEY" >> /cron.log 2>&1

# Refresh Playlists
echo "Refreshing Playlists..."
curl -X GET "http://localhost:8032/api/data/update_playlists" -H "Api-Key: $API_KEY" >> /cron.log 2>&1
