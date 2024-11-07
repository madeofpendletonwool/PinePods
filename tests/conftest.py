import pytest
import pytest_asyncio
from httpx import AsyncClient
import os
from typing import Generator
import mysql.connector
import psycopg
from psycopg_pool import ConnectionPool
from mysql.connector import pooling

# Import and run environment setup before any other imports
from test_environment import setup_test_environment
setup_test_environment()

# Set test environment variables BEFORE any app imports
os.environ['TEST_MODE'] = 'True'
os.environ['DB_TYPE'] = os.getenv('TEST_DB_TYPE', 'postgresql')

# Debug print statements
print(f"Current directory: {os.getcwd()}")
print(f"Setting up test environment...")

print(f"DB_TYPE set to: {os.getenv('DB_TYPE')}")
print(f"TEST_DB_TYPE set to: {os.getenv('TEST_DB_TYPE')}")


# Test database configurations
MYSQL_CONFIG = {
    'user': 'test_user',
    'password': 'test_password',
    'host': '127.0.0.1',
    'port': 3306,
    'database': 'test_db'
}

POSTGRES_CONFIG = {
    'user': 'test_user',
    'password': 'test_password',
    'host': '127.0.0.1',
    'port': 5432,
    'dbname': 'test_db'
}

# Only import app after environment variables are set
from clients.clientapi import app

@pytest.fixture(scope="session", autouse=True)
def setup_test_env():
    """Set up test environment variables"""
    # Environment variables already set at module level

    # Set up test database configuration
    if os.getenv('DB_TYPE') == 'postgresql':
        os.environ['DATABASE_URL'] = 'postgresql://test_user:test_password@localhost:5432/test_db'
    else:
        os.environ['DATABASE_URL'] = 'mysql://test_user:test_password@localhost:3306/test_db'

    yield

    # Cleanup
    if 'TEST_MODE' in os.environ:
        del os.environ['TEST_MODE']

@pytest.fixture(scope="session")
def db_connection():
    """Create database connection based on configured database type"""
    db_type = os.getenv('DB_TYPE', 'postgresql')

    if db_type == 'postgresql':
        conn = psycopg.connect(**POSTGRES_CONFIG)
        yield conn
        conn.close()
    else:
        pool = pooling.MySQLConnectionPool(
            pool_name="test_pool",
            pool_size=5,
            **MYSQL_CONFIG
        )
        conn = pool.get_connection()
        yield conn
        conn.close()

@pytest_asyncio.fixture
async def async_client():
    """Create async client for testing FastAPI endpoints"""
    async with AsyncClient(app=app, base_url="http://test") as client:
        yield client
