#!/usr/bin/env python3
"""
Simple wrapper for database validation that reads from environment variables
"""

import os
import sys
import subprocess
import psycopg
def main():
    # Get database config from environment variables (same as the app uses)
    db_type = os.environ.get('DB_TYPE', 'postgresql')
    db_host = os.environ.get('DB_HOST', 'localhost')
    db_port = os.environ.get('DB_PORT', '5432' if db_type == 'postgresql' else '3306')
    db_user = os.environ.get('DB_USER', 'postgres' if db_type == 'postgresql' else 'root')
    db_password = os.environ.get('DB_PASSWORD', '')
    db_name = os.environ.get('DB_NAME', 'pinepods_database')
    
    if not db_password:
        print("Error: DB_PASSWORD environment variable is required")
        sys.exit(1)
    
    # Build command
    cmd = [
        sys.executable,
        'database_functions/validate_database.py',
        '--db-type', db_type,
        '--db-host', db_host,
        '--db-port', db_port,
        '--db-user', db_user,
        '--db-password', db_password,
        '--db-name', db_name
    ]
    
    # Add verbose flag if requested
    if '--verbose' in sys.argv or '-v' in sys.argv:
        cmd.append('--verbose')
    
    print(f"Validating {db_type} database: {db_user}@{db_host}:{db_port}/{db_name}")
    print("Running database validation...")
    print()
    
    # Run the validator
    result = subprocess.run(cmd)
    sys.exit(result.returncode)

if __name__ == '__main__':
    main()
