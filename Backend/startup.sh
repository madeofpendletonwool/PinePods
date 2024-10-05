#!/bin/bash

# Source the environment variables directly from the env_file
if [ -f /path/to/env_file ]; then
    source /path/to/env_file
fi

# Log the environment variables to ensure they're set
echo "API_KEY: ${API_KEY}, API_SECRET: ${API_SECRET}"

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
