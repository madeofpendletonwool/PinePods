#!/bin/bash

# Source the environment variables directly from the env_file
if [ -f /path/to/env_file ]; then
    source /path/to/env_file
fi

# Confirm credentials are present without printing their values.
if [ -n "${API_KEY}" ] && [ -n "${API_SECRET}" ]; then
    echo "API credentials detected."
else
    echo "Warning: API_KEY/API_SECRET not set."
fi

# Start the Actix web application
/usr/local/bin/pinepods_backend
if [ $? -ne 0 ]; then
    echo "Failed to start pinepods_backend"
    exit 1
fi

# Print debugging information
echo "Actix web application started."

# Keep the container running
tail -f /dev/null
