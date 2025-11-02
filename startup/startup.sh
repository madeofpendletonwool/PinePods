#!/bin/bash
set -e  # Exit immediately if a command exits with a non-zero status

# Function to handle shutdown
shutdown() {
    echo "Shutting down..."
    pkill -TERM horust
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
export DEFAULT_LANGUAGE=${DEFAULT_LANGUAGE:-'en'}

# Save user's HOSTNAME to SERVER_URL before Docker overwrites it with container ID
# This preserves the user-configured server URL for RSS feed generation
export SERVER_URL=${HOSTNAME}

# Export OIDC environment variables
export OIDC_DISABLE_STANDARD_LOGIN=${OIDC_DISABLE_STANDARD_LOGIN:-'false'}
export OIDC_PROVIDER_NAME=${OIDC_PROVIDER_NAME}
export OIDC_CLIENT_ID=${OIDC_CLIENT_ID}
export OIDC_CLIENT_SECRET=${OIDC_CLIENT_SECRET}
export OIDC_AUTHORIZATION_URL=${OIDC_AUTHORIZATION_URL}
export OIDC_TOKEN_URL=${OIDC_TOKEN_URL}
export OIDC_USER_INFO_URL=${OIDC_USER_INFO_URL}
export OIDC_BUTTON_TEXT=${OIDC_BUTTON_TEXT}
export OIDC_SCOPE=${OIDC_SCOPE}
export OIDC_BUTTON_COLOR=${OIDC_BUTTON_COLOR}
export OIDC_BUTTON_TEXT_COLOR=${OIDC_BUTTON_TEXT_COLOR}
export OIDC_ICON_SVG=${OIDC_ICON_SVG}
export OIDC_NAME_CLAIM=${OIDC_NAME_CLAIM}
export OIDC_EMAIL_CLAIM=${OIDC_EMAIL_CLAIM}
export OIDC_USERNAME_CLAIM=${OIDC_USERNAME_CLAIM}
export OIDC_ROLES_CLAIM=${OIDC_ROLES_CLAIM}
export OIDC_USER_ROLE=${OIDC_USER_ROLE}
export OIDC_ADMIN_ROLE=${OIDC_ADMIN_ROLE}

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

# Configure timezone based on TZ environment variable
if [ -n "$TZ" ]; then
    echo "Setting timezone to $TZ"
    # For Alpine, we need to copy the zoneinfo file
    if [ -f "/usr/share/zoneinfo/$TZ" ]; then
        # Check if /etc/localtime is a mounted volume
        if [ -f "/etc/localtime" ] && ! [ -L "/etc/localtime" ]; then
            echo "Using mounted timezone file from host"
        else
            # If it's not mounted or is a symlink, we can modify it
            cp /usr/share/zoneinfo/$TZ /etc/localtime
            echo "$TZ" > /etc/timezone
        fi
    else
        echo "Timezone $TZ not found, using UTC"
        # Only modify if not mounted
        if ! [ -f "/etc/localtime" ] || [ -L "/etc/localtime" ]; then
            cp /usr/share/zoneinfo/UTC /etc/localtime
            echo "UTC" > /etc/timezone
        fi
    fi
else
    echo "No timezone specified, using UTC"
    # Only modify if not mounted
    if ! [ -f "/etc/localtime" ] || [ -L "/etc/localtime" ]; then
        cp /usr/share/zoneinfo/UTC /etc/localtime
        echo "UTC" > /etc/timezone
    fi
fi

# Export TZ to the environment for all child processes
export TZ

# Create required directories
echo "Creating required directories..."
mkdir -p /pinepods/cache
mkdir -p /opt/pinepods/backups
mkdir -p /opt/pinepods/downloads
mkdir -p /opt/pinepods/certs
mkdir -p /var/log/pinepods  # Make sure log directory exists

# Database Setup
echo "Using $DB_TYPE database"
# Use compiled database setup binary (no Python dependency)
# Web API key file creation has been removed for security
/usr/local/bin/pinepods-db-setup
echo "Database validation complete"

# Cron jobs removed - now handled by internal Rust scheduler

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

# Set up environment variables for Horust logging modes
if [[ $DEBUG_MODE == "true" ]]; then
    export HORUST_STDOUT_MODE="STDOUT"
    export HORUST_STDERR_MODE="STDERR"
    echo "Starting Horust in debug mode (logs to stdout)..."
else
    export HORUST_STDOUT_MODE="/var/log/pinepods/service.log"
    export HORUST_STDERR_MODE="/var/log/pinepods/service.log"
    echo "Starting Horust in production mode (logs to files)..."
fi

# Set permissions for download and backup directories BEFORE starting services
# Only do this if PUID and PGID are set
if [[ -n "$PUID" && -n "$PGID" ]]; then
    echo "Setting permissions for download and backup directories...(Be patient this might take a while if you have a lot of downloads)"
    chown -R ${PUID}:${PGID} /opt/pinepods/downloads
    chown -R ${PUID}:${PGID} /opt/pinepods/backups
else
    echo "Skipping permission setting as PUID/PGID are not set"
fi

# Copy service configurations to Horust directory
cp /pinepods/startup/services/*.toml /etc/horust/services/

# Start all services with Horust
echo "Starting services with Horust..."
echo "PinePods startup complete, running Horust in foreground..."
exec horust --services-path /etc/horust/services/

