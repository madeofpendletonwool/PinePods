#!/bin/bash
set -e  # Exit immediately if a command exits with a non-zero status

# Function to handle shutdown
shutdown() {
    echo "Shutting down..."
    supervisorctl stop all
    exit 0
}

# Set up signal handling
trap shutdown SIGTERM SIGINT

# Export all environment variables
export DB_USER=$DB_USER
export DB_PASSWORD=$DB_PASSWORD
export DB_HOST=$DB_HOST
export DB_NAME=$DB_NAME
export DB_PORT=$DB_PORT
export DB_TYPE=$DB_TYPE
export FULLNAME=${FULLNAME}
export USERNAME=${USERNAME}
export EMAIL=${EMAIL}
export PASSWORD=${PASSWORD}
export REVERSE_PROXY=$REVERSE_PROXY
export SEARCH_API_URL=$SEARCH_API_URL
export PEOPLE_API_URL=$PEOPLE_API_URL
export PINEPODS_PORT=$PINEPODS_PORT
export PROXY_PROTOCOL=$PROXY_PROTOCOL
export DEBUG_MODE=${DEBUG_MODE:-'False'}
export VALKEY_HOST=${VALKEY_HOST:-'valkey'}
export VALKEY_PORT=${VALKEY_PORT:-'6379'}

# Print admin info if default admin is used
if [[ $FULLNAME == 'Pinepods Admin' ]]; then
  echo "Admin User Information:"
  echo "FULLNAME: $FULLNAME"
  echo "USERNAME: $USERNAME"
  echo "EMAIL: $EMAIL"
  echo "PASSWORD: $PASSWORD"
fi

# Print PinePods logo
cat << "EOF"
         A
        d$b
      .d\$$b.
    .d$i$$\$$b.      _______   **                                                **
       d$$@b        /       \ /  |                                              /  |
      d\$$$ib       $$$$$$$  |$$/  _______    ______    ______    ______    ____$$ |  _______
    .d$$$\$$$b      $$ |__$$ |/  |/       \  /      \  /      \  /      \  /    $$ | /       |
  .d$$@$$$$\$$ib.   $$    $$/ $$ |$$$$$$$  |/$$$$$$  |/$$$$$$  |/$$$$$$  |/$$$$$$$ |/$$$$$$$/
      d$$i$$b       $$$$$$$/  $$ |$$ |  $$ |$$    $$ |$$ |  $$ |$$ |  $$ |$$ |  $$ |$$      \
     d\$$$$@$b.     $$ |      $$ |$$ |  $$ |$$$$$$$$/ $$ |__$$ |$$ \__$$ |$$ \__$$ | $$$$$$  |
  .d$@$$\$$$$$@b.   $$ |      $$ |$$ |  $$ |$$       |$$    $$/ $$    $$/ $$    $$ |/     $$/
.d$$$$i$$$\$$$$$$b. $$/       $$/ $$/   $$/  $$$$$$$/ $$$$$$$/   $$$$$$/   $$$$$$$/ $$$$$$$/
        ###                                           $$ |
        ###                                           $$ |
        ###                                           $$/
A project created and written by Collin Pendleton
collinp@gooseberrydevelopment.com
EOF

# Create required directories
echo "Creating required directories..."
mkdir -p /pinepods/cache
mkdir -p /opt/pinepods/backups
mkdir -p /opt/pinepods/downloads
mkdir -p /opt/pinepods/certs
mkdir -p /var/log/supervisor  # Make sure supervisor log directory exists

# Database Setup
if [[ $DB_TYPE == "postgresql" ]]; then
    echo "Using PostgreSQL database"
    /wait-for-it.sh "${DB_HOST}:${DB_PORT}" --timeout=60 --strict -- python3 /pinepods/startup/setuppostgresdatabase.py
else
    echo "Using MySQL/MariaDB database"
    /wait-for-it.sh "${DB_HOST}:${DB_PORT}" --timeout=60 --strict -- python3 /pinepods/startup/setupdatabase.py
fi
echo "Database validation complete"

# Set up cron jobs
echo -e "*/30 * * * * /pinepods/startup/call_refresh_endpoint.sh >/dev/null 2>&1\n0 0 * * * /pinepods/startup/call_nightly_tasks.sh >/dev/null 2>&1" > /etc/crontabs/root

# Check if we need to create exim directories
# Only do this if the user/group exists on the system
if getent group | grep -q "Debian-exim"; then
    echo "Setting up exim directories and permissions..."
    mkdir -p /var/log/exim4
    mkdir -p /var/spool/exim4
    chown -R Debian-exim:Debian-exim /var/log/exim4
    chown -R Debian-exim:Debian-exim /var/spool/exim4
else
    echo "Skipping exim setup as user/group doesn't exist on this system"
fi

# Start all services with supervisord
echo "Starting supervisord..."
if [[ $DEBUG_MODE == "true" ]]; then
    supervisord -c /pinepods/startup/supervisordebug.conf
else
    supervisord -c /pinepods/startup/supervisord.conf
fi

# Set permissions for download and backup directories
# Only do this if PUID and PGID are set
if [[ -n "$PUID" && -n "$PGID" ]]; then
    echo "Setting permissions for download and backup directories..."
    chown -R ${PUID}:${PGID} /opt/pinepods/downloads
    chown -R ${PUID}:${PGID} /opt/pinepods/backups
else
    echo "Skipping permission setting as PUID/PGID are not set"
fi

# Keep container running
echo "PinePods startup complete, running supervisord in foreground..."
exec supervisorctl tail -f all
