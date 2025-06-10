import os
import logging
import traceback
from fastapi import HTTPException
import psycopg
from psycopg_pool import ConnectionPool
from psycopg.rows import dict_row
from mysql.connector import pooling

# Set up logging
logger = logging.getLogger(__name__)

# Get database type from environment variable
database_type = str(os.getenv('DB_TYPE', 'mariadb'))

# Create a singleton for the connection pool
class DatabaseConnectionPool:
    _instance = None
    _pool = None

    @classmethod
    def get_instance(cls):
        if cls._instance is None:
            cls._instance = DatabaseConnectionPool()
        return cls._instance

    def __init__(self):
        if self._pool is None:
            self._pool = self._create_pool()

    def _create_pool(self):
        """Create a new connection pool based on the database type"""
        db_host = os.environ.get("DB_HOST", "127.0.0.1")
        db_port = os.environ.get("DB_PORT", "3306")
        db_user = os.environ.get("DB_USER", "root")
        db_password = os.environ.get("DB_PASSWORD", "password")
        db_name = os.environ.get("DB_NAME", "pypods_database")

        print(f"Creating new database connection pool for {database_type}")

        if database_type == "postgresql":
            conninfo = f"host={db_host} port={db_port} user={db_user} password={db_password} dbname={db_name}"
            return ConnectionPool(conninfo=conninfo, min_size=1, max_size=32, open=True)
        else:
            # Add the autocommit and consume_results options to MySQL
            return pooling.MySQLConnectionPool(
                pool_name="pinepods_api_pool",
                pool_size=32,
                pool_reset_session=True,
                autocommit=True,  # Add this to prevent transaction issues
                consume_results=True,  # Add this to automatically consume unread results
                collation="utf8mb4_general_ci",
                host=db_host,
                port=db_port,
                user=db_user,
                password=db_password,
                database=db_name,
            )

    def get_connection(self):
        """Get a connection from the pool"""
        if database_type == "postgresql":
            return self._pool.getconn()
        else:
            return self._pool.get_connection()

    def return_connection(self, cnx):
        """Return a connection to the pool"""
        if database_type == "postgresql":
            self._pool.putconn(cnx)  # PostgreSQL path unchanged
        else:
            # MySQL-specific cleanup
            try:
                # Clear any unread results before returning to pool
                if hasattr(cnx, 'unread_result') and cnx.unread_result:
                    cursor = cnx.cursor()
                    cursor.fetchall()
                    cursor.close()
            except Exception as e:
                logger.warning(f"Failed to clean up MySQL connection: {str(e)}")
            finally:
                cnx.close()

# Initialize the singleton pool
pool = DatabaseConnectionPool.get_instance()

def create_database_connection():
    """Create and return a new database connection"""
    try:
        return pool.get_connection()
    except Exception as e:
        print(f"Database connection error: {str(e)}")
        logger.error(f"Database connection error of type {type(e).__name__} with arguments: {e.args}")
        logger.error(traceback.format_exc())
        raise RuntimeError("Unable to connect to the database")

def close_database_connection(cnx):
    """Close a database connection and handle both PostgreSQL and MySQL connections properly"""
    if cnx is None:
        return

    try:
        # First determine the connection type
        is_psql = hasattr(cnx, 'closed')  # PostgreSQL has a 'closed' attribute

        if is_psql:
            # PostgreSQL connection - try to return to pool first
            try:
                if not cnx.closed and pool is not None:
                    pool.return_connection(cnx)
                    return
            except Exception as pool_err:
                print(f"Could not return connection to pool: {str(pool_err)}")
                # Fall back to direct close if return fails
                if not cnx.closed:
                    cnx.close()
        else:
            # MySQL connection - just close directly, don't try to use the pool
            if hasattr(cnx, 'close'):
                cnx.close()
    except Exception as e:
        print(f"Error closing connection: {str(e)}")
        logger.error(f"Error closing connection: {str(e)}")

# For FastAPI dependency injection
def get_database_connection():
    """FastAPI dependency for getting a database connection"""
    try:
        cnx = create_database_connection()
        yield cnx
    except HTTPException:
        raise  # Re-raise the HTTPException to let FastAPI handle it properly
    except Exception as e:
        logger.error(f"Database connection error of type {type(e).__name__} with arguments: {e.args}")
        logger.error(traceback.format_exc())
        raise HTTPException(500, "Unable to connect to the database")
    finally:
        try:
            close_database_connection(cnx)
        except Exception as e:
            logger.error(f"Error in connection cleanup: {str(e)}")
