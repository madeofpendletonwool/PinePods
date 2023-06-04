#!/bin/bash

# Set environment variables
export API_KEY=$API_KEY
export API_SECRET=$API_SECRET

# Start gunicorn with pypods-apicall.py
gunicorn --bind 0.0.0.0:5000 pypodsapicall:app --capture-output &

# Print debugging information
echo "Gunicorn started."

# Keep the container running
tail -f /dev/null
