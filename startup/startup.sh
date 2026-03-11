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

# Timezone is configured via the TZ environment variable, which musl/glibc
# respect without needing to write to /etc/localtime (a root-owned file).
# The TZ variable is already exported above and will be inherited by all
# child processes started by Horust.
if [ -n "$TZ" ]; then
    echo "Timezone set to $TZ via TZ environment variable"
else
    echo "No timezone specified, defaulting to UTC"
    export TZ=UTC
fi

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

# Exim setup skipped - container runs as non-root (pinepods, UID 1000) and
# cannot chown system directories.

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

# Volume permissions are no longer set at runtime. The container runs as
# pinepods (UID 1000, GID 1000). Host directories mounted at
# /opt/pinepods/downloads and /opt/pinepods/backups must be owned by
# UID 1000 on the host before starting the container.

# Start all services with Horust
echo "Starting services with Horust..."
echo "PinePods startup complete, running Horust in foreground..."
exec horust --services-path /etc/horust/services/

