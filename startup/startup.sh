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

# Create nginx runtime directories
mkdir -p /var/log/nginx
mkdir -p /var/lib/nginx
mkdir -p /var/lib/nginx/tmp
mkdir -p /run/nginx

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

# Set up user permissions for download and backup directories
# Set defaults if PUID/PGID are not provided
export PUID=${PUID:-1000}
export PGID=${PGID:-1000}

echo "Setting up user permissions (PUID=${PUID}, PGID=${PGID})..."

# Create group if it doesn't exist
if ! getent group $PGID > /dev/null 2>&1; then
    echo "Creating group with GID $PGID"
    addgroup -g $PGID pinepods
else
    echo "Group with GID $PGID already exists"
fi

# Create user if it doesn't exist
if ! getent passwd $PUID > /dev/null 2>&1; then
    echo "Creating user with UID $PUID"
    adduser -D -u $PUID -G $(getent group $PGID | cut -d: -f1) pinepods
else
    echo "User with UID $PUID already exists"
fi

# Get the actual group name for the GID
GROUP_NAME=$(getent group $PGID | cut -d: -f1)

# Set permissions for directories where possible
echo "Setting permissions for download and backup directories (this may take time if you have many files)..."

# Try to set ownership, but handle failures gracefully (e.g., NFS with root squashing)
set +e  # Don't exit on error for permission setting

# Set ownership of main directories
chown ${PUID}:${PGID} /opt/pinepods/downloads 2>/dev/null || echo "Warning: Could not change ownership of /opt/pinepods/downloads (possibly NFS with root squashing)"
chown ${PUID}:${PGID} /opt/pinepods/backups 2>/dev/null || echo "Warning: Could not change ownership of /opt/pinepods/backups (possibly NFS with root squashing)"

# Try to set ownership recursively, but don't fail the entire startup if it fails
if ! chown -R ${PUID}:${PGID} /opt/pinepods/downloads 2>/dev/null; then
    echo "Warning: Could not recursively change ownership of /opt/pinepods/downloads"
    echo "This is normal for NFS mounts with root squashing. Files will be created with the correct permissions."
fi

if ! chown -R ${PUID}:${PGID} /opt/pinepods/backups 2>/dev/null; then
    echo "Warning: Could not recursively change ownership of /opt/pinepods/backups"
    echo "This is normal for NFS mounts with root squashing. Files will be created with the correct permissions."
fi

set -e  # Re-enable exit on error

# Set permissions for nginx runtime directories
chown -R ${PUID}:${PGID} /var/log/nginx 2>/dev/null || echo "Warning: Could not change ownership of /var/log/nginx"
chown -R ${PUID}:${PGID} /var/lib/nginx 2>/dev/null || echo "Warning: Could not change ownership of /var/lib/nginx"
chown -R ${PUID}:${PGID} /var/tmp/nginx 2>/dev/null || echo "Warning: Could not change ownership of /var/tmp/nginx (may not exist)"
chown -R ${PUID}:${PGID} /run/nginx 2>/dev/null || echo "Warning: Could not change ownership of /run/nginx (may not exist)"

# Set permissions for application log directory  
chown -R ${PUID}:${PGID} /var/log/pinepods 2>/dev/null || echo "Warning: Could not change ownership of /var/log/pinepods"

# Make sure cache directory has correct permissions
chown -R ${PUID}:${PGID} /pinepods/cache 2>/dev/null || echo "Warning: Could not change ownership of /pinepods/cache"

echo "User and permission setup complete"

# Copy service configurations to Horust directory
cp /pinepods/startup/services/*.toml /etc/horust/services/

# Start all services with Horust
echo "Starting services with Horust..."
echo "PinePods startup complete, running Horust in foreground..."
exec horust --services-path /etc/horust/services/

