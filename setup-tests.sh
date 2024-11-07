#!/bin/bash

# Get the directory where the script is located
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

# Create and activate virtual environment if it doesn't exist
if [ ! -d "${SCRIPT_DIR}/venv" ]; then
    python -m venv "${SCRIPT_DIR}/venv"
fi

# Activate virtual environment
source "${SCRIPT_DIR}/venv/bin/activate"

# Install requirements using absolute path
pip install -r "${SCRIPT_DIR}/test-requirements.txt"

# Create test environment file
cat > .env.test << EOL
TEST_MODE=true
EOL

# Create tests directory if it doesn't exist
mkdir -p tests

echo "Test environment setup complete!"
echo "Run tests with: ./run-tests.sh [postgresql|mariadb]"
