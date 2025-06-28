#!/usr/bin/env python3
"""
New Idempotent Database Setup for PinePods

This script replaces the old setupdatabase.py and setuppostgresdatabase.py
with a proper migration-based system that is fully idempotent.
"""

import os
import sys
import logging
from pathlib import Path

# Set up basic configuration for logging
logging.basicConfig(
    level=logging.INFO, 
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

# Add pinepods directory to sys.path for module import
pinepods_path = Path(__file__).parent.parent
sys.path.insert(0, str(pinepods_path))
sys.path.insert(0, '/pinepods')  # Also add the container path for Docker

def wait_for_postgresql_ready():
    """Wait for PostgreSQL to be ready to accept connections (not just port open)"""
    import time
    import psycopg
    
    db_host = os.environ.get("DB_HOST", "127.0.0.1")
    db_port = os.environ.get("DB_PORT", "5432")
    db_user = os.environ.get("DB_USER", "postgres")
    db_password = os.environ.get("DB_PASSWORD", "password")
    
    max_attempts = 30  # 30 seconds
    attempt = 1
    
    logger.info(f"Waiting for PostgreSQL at {db_host}:{db_port} to be ready...")
    
    while attempt <= max_attempts:
        try:
            # Try to connect to the postgres database
            with psycopg.connect(
                host=db_host,
                port=db_port,
                user=db_user,
                password=db_password,
                dbname='postgres',
                connect_timeout=3
            ) as conn:
                with conn.cursor() as cur:
                    # Test if PostgreSQL is ready to accept queries
                    cur.execute("SELECT 1")
                    cur.fetchone()
                logger.info(f"PostgreSQL is ready after {attempt} attempts")
                return True
        except Exception as e:
            if "not yet accepting connections" in str(e) or "recovery" in str(e).lower():
                logger.info(f"PostgreSQL not ready yet (attempt {attempt}/{max_attempts}): {e}")
            else:
                logger.warning(f"Connection attempt {attempt}/{max_attempts} failed: {e}")
            
            if attempt < max_attempts:
                time.sleep(1)
            attempt += 1
    
    logger.error(f"PostgreSQL failed to become ready after {max_attempts} attempts")
    return False

def wait_for_mysql_ready():
    """Wait for MySQL/MariaDB to be ready to accept connections"""
    import time
    import mysql.connector
    
    db_host = os.environ.get("DB_HOST", "127.0.0.1")
    db_port = int(os.environ.get("DB_PORT", "3306"))
    db_user = os.environ.get("DB_USER", "root")
    db_password = os.environ.get("DB_PASSWORD", "password")
    
    max_attempts = 30  # 30 seconds
    attempt = 1
    
    logger.info(f"Waiting for MySQL/MariaDB at {db_host}:{db_port} to be ready...")
    
    while attempt <= max_attempts:
        try:
            # Try to connect to MySQL
            conn = mysql.connector.connect(
                host=db_host,
                port=db_port,
                user=db_user,
                password=db_password,
                connection_timeout=3
            )
            cursor = conn.cursor()
            # Test if MySQL is ready to accept queries
            cursor.execute("SELECT 1")
            cursor.fetchone()
            cursor.close()
            conn.close()
            logger.info(f"MySQL/MariaDB is ready after {attempt} attempts")
            return True
        except Exception as e:
            logger.info(f"MySQL/MariaDB not ready yet (attempt {attempt}/{max_attempts}): {e}")
            
            if attempt < max_attempts:
                time.sleep(1)
            attempt += 1
    
    logger.error(f"MySQL/MariaDB failed to become ready after {max_attempts} attempts")
    return False

def create_database_if_not_exists():
    """Create the database if it doesn't exist and wait for database to be ready"""
    db_type = os.environ.get("DB_TYPE", "postgresql").lower()
    
    if db_type in ['postgresql', 'postgres']:
        # First, wait for PostgreSQL to be ready
        if not wait_for_postgresql_ready():
            raise Exception("PostgreSQL did not become ready in time")
    else:
        # Wait for MySQL/MariaDB to be ready
        if not wait_for_mysql_ready():
            raise Exception("MySQL/MariaDB did not become ready in time")
        logger.info("MySQL/MariaDB is ready (database creation handled by container)")
        return
    
    # PostgreSQL database creation logic continues below
    if not wait_for_postgresql_ready():
        raise Exception("PostgreSQL did not become ready in time")
    
    try:
        import psycopg
        
        # Database connection parameters
        db_host = os.environ.get("DB_HOST", "127.0.0.1")
        db_port = os.environ.get("DB_PORT", "5432")
        db_user = os.environ.get("DB_USER", "postgres")
        db_password = os.environ.get("DB_PASSWORD", "password")
        db_name = os.environ.get("DB_NAME", "pinepods_database")
        
        # Connect to the default 'postgres' database to check/create target database
        with psycopg.connect(
            host=db_host,
            port=db_port,
            user=db_user,
            password=db_password,
            dbname='postgres'
        ) as conn:
            conn.autocommit = True
            with conn.cursor() as cur:
                # Check if the database exists
                cur.execute("SELECT 1 FROM pg_database WHERE datname = %s", (db_name,))
                exists = cur.fetchone()
                if not exists:
                    logger.info(f"Database {db_name} does not exist. Creating...")
                    cur.execute(f"CREATE DATABASE {db_name}")
                    logger.info(f"Database {db_name} created successfully.")
                else:
                    logger.info(f"Database {db_name} already exists.")
                    
    except ImportError:
        logger.error("psycopg not available for PostgreSQL database creation")
        raise
    except Exception as e:
        logger.error(f"Error creating database: {e}")
        raise


def ensure_usernames_lowercase():
    """Ensure all usernames are lowercase for consistency"""
    try:
        from database_functions.migrations import get_migration_manager
        
        manager = get_migration_manager()
        conn = manager.get_connection()
        cursor = conn.cursor()
        
        db_type = manager.db_type
        table_name = '"Users"' if db_type == 'postgresql' else 'Users'
        
        try:
            cursor.execute(f'SELECT UserID, Username FROM {table_name}')
            users = cursor.fetchall()
            
            for user_id, username in users:
                if username and username != username.lower():
                    cursor.execute(
                        f'UPDATE {table_name} SET Username = %s WHERE UserID = %s', 
                        (username.lower(), user_id)
                    )
                    logger.info(f"Updated Username for UserID {user_id} to lowercase")
            
            conn.commit()
            logger.info("Username normalization completed")
            
        finally:
            cursor.close()
            manager.close_connection()
            
    except Exception as e:
        logger.error(f"Error normalizing usernames: {e}")


def ensure_web_api_key_file():
    """Ensure the web API key file exists for background tasks"""
    try:
        from database_functions.migrations import get_migration_manager
        
        manager = get_migration_manager()
        conn = manager.get_connection()
        cursor = conn.cursor()
        
        db_type = manager.db_type
        api_keys_table = '"APIKeys"' if db_type == 'postgresql' else 'APIKeys'
        
        try:
            # Get the API key for background tasks user (UserID = 1)
            cursor.execute(f'SELECT APIKey FROM {api_keys_table} WHERE UserID = 1')
            result = cursor.fetchone()
            
            if result:
                # Extract API key from result (handle both tuple and dict formats)
                api_key = result[0] if isinstance(result, tuple) else result['apikey']
                
                # Write API key to temp file for web services
                with open("/tmp/web_api_key.txt", "w") as f:
                    f.write(api_key)
                logger.info("Web API key file created successfully")
            else:
                logger.warning("No API key found for background tasks user")
            
        finally:
            cursor.close()
            manager.close_connection()
            
    except Exception as e:
        logger.error(f"Error creating web API key file: {e}")


def main():
    """Main setup function"""
    try:
        logger.info("Starting PinePods database setup...")
        
        # Step 1: Create database if needed (PostgreSQL only)
        create_database_if_not_exists()
        
        # Step 2: Import and register all migrations
        logger.info("Loading migration definitions...")
        import database_functions.migration_definitions
        database_functions.migration_definitions.register_all_migrations()
        
        # Step 3: Run migrations
        logger.info("Running database migrations...")
        from database_functions.migrations import run_all_migrations
        
        success = run_all_migrations()
        if not success:
            logger.error("Database migrations failed!")
            return False
        
        # Step 4: Ensure username consistency
        logger.info("Ensuring username consistency...")
        ensure_usernames_lowercase()
        
        # Step 5: Ensure web API key file exists
        logger.info("Ensuring web API key file exists...")
        ensure_web_api_key_file()
        
        logger.info("Database setup completed successfully!")
        logger.info("Database validation complete")
        
        return True
        
    except Exception as e:
        logger.error(f"Database setup failed: {e}")
        return False


if __name__ == "__main__":
    success = main()
    sys.exit(0 if success else 1)