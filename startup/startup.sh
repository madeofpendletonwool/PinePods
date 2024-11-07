#!/bin/bash

# Function to handle shutdown
shutdown() {
    echo "Shutting down..."
    supervisorctl stop all
    kill -TERM "$child"
    wait "$child"
    exit 0
}

# Set up signal handling
trap shutdown SIGTERM SIGINT

export DB_USER=$DB_USER
export DB_PASSWORD=$DB_PASSWORD
export DB_HOST=$DB_HOST
export DB_NAME=$DB_NAME
export DB_PORT=$DB_PORT
export DB_TYPE=$DB_TYPE
export FULLNAME=${FULLNAME:-'Pinepods Admin'}
export USERNAME=${USERNAME:-'pine-admin'}
export EMAIL=${EMAIL:-'admin@pinepods.online'}
export PASSWORD=${PASSWORD:-$(head /dev/urandom | tr -dc A-Za-z0-9 | head -c14 ; echo '')}
export REVERSE_PROXY=$REVERSE_PROXY
export API_URL=$API_URL
export PEOPLE_URL=$PEOPLE_URL
export PINEPODS_PORT=$PINEPODS_PORT
export PROXY_PROTOCOL=$PROXY_PROTOCOL
export PINEPODS_PORT=$PINEPODS_PORT
export DEBUG_MODE=${DEBUG_MODE:-'False'}
export VALKEY_HOST=${VALKEY_HOST:-'valkey'}
export VALKEY_PORT=${VALKEY_PORT:-'6379'}

if [[ $FULLNAME == 'Pinepods Admin' ]]; then
  echo "Admin User Information:"
  echo "FULLNAME: $FULLNAME"
  echo "USERNAME: $USERNAME"
  echo "EMAIL: $EMAIL"
  echo "PASSWORD: $PASSWORD"
fi

cat << "EOF"
         A
        d$b
      .d\$$b.
    .d$i$$\$$b.      _______   __                                                __
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
# Creating cache directory
mkdir -p /pinepods/cache
mkdir -p /opt/pinepods/backups
mkdir -p /opt/pinepods/downloads
mkdir -p /opt/pinepods/certs
# Database Setup
if [[ $DB_TYPE == "postgresql" ]]; then
echo "Using Postgresdb"
/wait-for-it.sh "${DB_HOST}:${DB_PORT}" --timeout=60 --strict -- python3 /pinepods/startup/setuppostgresdatabase.py
else
echo "Using mysql/mariadb"
/wait-for-it.sh "${DB_HOST}:${DB_PORT}" --timeout=60 --strict -- python3 /pinepods/startup/setupdatabase.py
fi
echo "Database Validation complete"
# Periodic refresh
echo "*/30 * * * * /pinepods/startup/call_refresh_endpoint.sh >/dev/null 2>&1" | crontab -
# Fix permissions on exim email server folders
mkdir -p /var/log/exim4
mkdir -p /var/spool/exim4
chown -R Debian-exim:Debian-exim /var/log/exim4
chown -R Debian-exim:Debian-exim /var/spool/exim4
# Start all services with supervisord
if [[ $DEBUG_MODE == "true" ]]; then
supervisord -c /pinepods/startup/supervisordebug.conf
else
supervisord -c /pinepods/startup/supervisord.conf
fi
# Create Admin User
# python3 /pinepods/create_user.py $DB_USER $DB_PASSWORD $DB_HOST $DB_NAME $DB_PORT "$FULLNAME" "$USERNAME" $EMAIL $PASSWORD
# Store the PID of supervisord
child=$!

# Wait for supervisord to exit
wait "$child"
