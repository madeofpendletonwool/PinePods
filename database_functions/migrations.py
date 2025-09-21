"""
Database Migration System for PinePods

This module provides a robust, idempotent migration framework that tracks
applied migrations and ensures database schema changes are applied safely.
"""

import logging
import os
import sys
from typing import Dict, List, Optional, Callable, Any
from dataclasses import dataclass
from datetime import datetime
import hashlib

# Add pinepods to path for imports
sys.path.append('/pinepods')

# Database imports
try:
    import psycopg
    POSTGRES_AVAILABLE = True
except ImportError:
    POSTGRES_AVAILABLE = False

try:
    import mariadb as mysql_connector
    MYSQL_AVAILABLE = True
except ImportError:
    try:
        import mysql.connector
        MYSQL_AVAILABLE = True
    except ImportError:
        MYSQL_AVAILABLE = False

logger = logging.getLogger(__name__)


@dataclass
class Migration:
    """Represents a single database migration"""
    version: str
    name: str
    description: str
    postgres_sql: Optional[str] = None
    mysql_sql: Optional[str] = None
    python_func: Optional[Callable] = None
    requires: List[str] = None  # List of migration versions this depends on
    
    def __post_init__(self):
        if self.requires is None:
            self.requires = []


class DatabaseMigrationManager:
    """Manages database migrations with support for PostgreSQL and MySQL/MariaDB"""
    
    def __init__(self, db_type: str, connection_params: Dict[str, Any]):
        self.db_type = db_type.lower()
        self.connection_params = connection_params
        self.migrations: Dict[str, Migration] = {}
        self._connection = None
        
        # Validate database type
        if self.db_type not in ['postgresql', 'postgres', 'mariadb', 'mysql']:
            raise ValueError(f"Unsupported database type: {db_type}")
            
        # Normalize database type
        if self.db_type in ['postgres', 'postgresql']:
            self.db_type = 'postgresql'
        elif self.db_type in ['mysql', 'mariadb']:
            self.db_type = 'mysql'

    def get_connection(self):
        """Get database connection based on type"""
        if self._connection:
            return self._connection
            
        if self.db_type == 'postgresql':
            if not POSTGRES_AVAILABLE:
                raise ImportError("psycopg not available for PostgreSQL connections")
            self._connection = psycopg.connect(**self.connection_params)
        elif self.db_type == 'mysql':
            if not MYSQL_AVAILABLE:
                raise ImportError("MariaDB/MySQL connector not available for MySQL connections")
            # Use MariaDB connector parameters
            mysql_params = self.connection_params.copy()
            # Convert mysql.connector parameter names to mariadb parameter names
            if 'connection_timeout' in mysql_params:
                mysql_params['connect_timeout'] = mysql_params.pop('connection_timeout')
            if 'charset' in mysql_params:
                mysql_params.pop('charset')  # MariaDB connector doesn't use charset parameter
            if 'collation' in mysql_params:
                mysql_params.pop('collation')  # MariaDB connector doesn't use collation parameter
            self._connection = mysql_connector.connect(**mysql_params)
        
        return self._connection

    def close_connection(self):
        """Close database connection"""
        if self._connection:
            self._connection.close()
            self._connection = None

    def register_migration(self, migration: Migration):
        """Register a migration to be tracked"""
        self.migrations[migration.version] = migration
        logger.info(f"Registered migration {migration.version}: {migration.name}")

    def create_migration_table(self):
        """Create the migrations tracking table if it doesn't exist"""
        conn = self.get_connection()
        cursor = conn.cursor()
        
        try:
            if self.db_type == 'postgresql':
                cursor.execute("""
                    CREATE TABLE IF NOT EXISTS "schema_migrations" (
                        version VARCHAR(255) PRIMARY KEY,
                        name VARCHAR(255) NOT NULL,
                        description TEXT,
                        checksum VARCHAR(64) NOT NULL,
                        applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                        execution_time_ms INTEGER
                    )
                """)
            else:  # mysql
                cursor.execute("""
                    CREATE TABLE IF NOT EXISTS schema_migrations (
                        version VARCHAR(255) PRIMARY KEY,
                        name VARCHAR(255) NOT NULL,
                        description TEXT,
                        checksum VARCHAR(64) NOT NULL,
                        applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                        execution_time_ms INTEGER
                    )
                """)
            conn.commit()
            logger.info("Migration tracking table created/verified")
        except Exception as e:
            logger.error(f"Failed to create migration table: {e}")
            conn.rollback()
            raise
        finally:
            cursor.close()

    def get_applied_migrations(self) -> List[str]:
        """Get list of applied migration versions"""
        conn = self.get_connection()
        cursor = conn.cursor()
        
        try:
            table_name = '"schema_migrations"' if self.db_type == 'postgresql' else 'schema_migrations'
            cursor.execute(f"SELECT version FROM {table_name} ORDER BY applied_at")
            return [row[0] for row in cursor.fetchall()]
        except Exception as e:
            # If table doesn't exist, return empty list
            logger.warning(f"Could not get applied migrations: {e}")
            return []
        finally:
            cursor.close()

    def calculate_migration_checksum(self, migration: Migration) -> str:
        """Calculate checksum for migration content"""
        content = ""
        if migration.postgres_sql and self.db_type == 'postgresql':
            content += migration.postgres_sql
        elif migration.mysql_sql and self.db_type == 'mysql':
            content += migration.mysql_sql
        
        if migration.python_func:
            content += migration.python_func.__code__.co_code.hex()
            
        return hashlib.sha256(content.encode()).hexdigest()

    def record_migration(self, migration: Migration, execution_time_ms: int):
        """Record a migration as applied"""
        conn = self.get_connection()
        cursor = conn.cursor()
        
        try:
            checksum = self.calculate_migration_checksum(migration)
            table_name = '"schema_migrations"' if self.db_type == 'postgresql' else 'schema_migrations'
            
            if self.db_type == 'postgresql':
                cursor.execute(f"""
                    INSERT INTO {table_name} (version, name, description, checksum, execution_time_ms)
                    VALUES (%s, %s, %s, %s, %s)
                    ON CONFLICT (version) DO NOTHING
                """, (migration.version, migration.name, migration.description, checksum, execution_time_ms))
            else:  # mysql
                cursor.execute(f"""
                    INSERT IGNORE INTO {table_name} (version, name, description, checksum, execution_time_ms)
                    VALUES (%s, %s, %s, %s, %s)
                """, (migration.version, migration.name, migration.description, checksum, execution_time_ms))
            
            conn.commit()
            logger.info(f"Recorded migration {migration.version} as applied")
        except Exception as e:
            logger.error(f"Failed to record migration {migration.version}: {e}")
            conn.rollback()
            raise
        finally:
            cursor.close()

    def check_dependencies(self, migration: Migration, applied_migrations: List[str]) -> bool:
        """Check if migration dependencies are satisfied"""
        for required_version in migration.requires:
            if required_version not in applied_migrations:
                logger.error(f"Migration {migration.version} requires {required_version} but it's not applied")
                return False
        return True

    def execute_migration(self, migration: Migration) -> bool:
        """Execute a single migration"""
        start_time = datetime.now()
        conn = self.get_connection()
        
        try:
            # Choose appropriate SQL based on database type
            sql = None
            if self.db_type == 'postgresql' and migration.postgres_sql:
                sql = migration.postgres_sql
            elif self.db_type == 'mysql' and migration.mysql_sql:
                sql = migration.mysql_sql
            
            # Execute SQL if available
            if sql:
                cursor = conn.cursor()
                try:
                    # Split and execute multiple statements
                    statements = [stmt.strip() for stmt in sql.split(';') if stmt.strip()]
                    for statement in statements:
                        cursor.execute(statement)
                    conn.commit()
                    logger.info(f"Executed SQL for migration {migration.version}")
                except Exception as e:
                    conn.rollback()
                    raise
                finally:
                    cursor.close()
            
            # Execute Python function if available (this is the main path for our migrations)
            if migration.python_func:
                try:
                    migration.python_func(conn, self.db_type)
                    conn.commit()
                    logger.info(f"Executed Python function for migration {migration.version}")
                except Exception as e:
                    conn.rollback()
                    raise
            
            # Record successful migration
            execution_time = int((datetime.now() - start_time).total_seconds() * 1000)
            self.record_migration(migration, execution_time)
            
            logger.info(f"Successfully applied migration {migration.version}: {migration.name}")
            return True
            
        except Exception as e:
            logger.error(f"Failed to execute migration {migration.version}: {e}")
            try:
                conn.rollback()
            except:
                pass  # Connection might already be closed
            return False

    def detect_existing_schema(self) -> List[str]:
        """Detect which migrations have already been applied based on existing schema"""
        conn = self.get_connection()
        cursor = conn.cursor()
        applied = []
        
        try:
            # Check for tables that indicate migrations have been applied
            checks = {
                "001": ['"Users"', '"OIDCProviders"', '"APIKeys"', '"RssKeys"'],
                "002": ['"AppSettings"', '"EmailSettings"'],
                "003": ['"UserStats"', '"UserSettings"'],
                "005": ['"Podcasts"', '"Episodes"', '"YouTubeVideos"'],
                "006": ['"UserEpisodeHistory"', '"UserVideoHistory"'],
                "007": ['"EpisodeQueue"', '"SavedEpisodes"', '"DownloadedEpisodes"']
            }
            
            for version, tables in checks.items():
                all_exist = True
                for table in tables:
                    if self.db_type == 'postgresql':
                        cursor.execute("""
                            SELECT EXISTS (
                                SELECT FROM information_schema.tables 
                                WHERE table_schema = 'public' AND table_name = %s
                            )
                        """, (table.strip('"'),))
                    else:  # mysql
                        cursor.execute("""
                            SELECT COUNT(*) 
                            FROM information_schema.tables 
                            WHERE table_schema = DATABASE() AND table_name = %s
                        """, (table,))
                    
                    exists = cursor.fetchone()[0]
                    if not exists:
                        all_exist = False
                        break
                
                if all_exist:
                    applied.append(version)
                    logger.info(f"Detected existing schema for migration {version}")
            
            # Migration 004 is harder to detect, assume it's applied if 001-003 are
            if "001" in applied and "003" in applied and "004" not in applied:
                # Check if background_tasks user exists
                table_name = '"Users"' if self.db_type == 'postgresql' else 'Users'
                cursor.execute(f"SELECT COUNT(*) FROM {table_name} WHERE Username = %s", ('background_tasks',))
                if cursor.fetchone()[0] > 0:
                    applied.append("004")
                    logger.info("Detected existing schema for migration 004")
            
            # Check for gpodder tables - if ANY exist, ALL gpodder migrations are applied
            # (since they were created by the Go gpodder-api service and haven't changed)
            gpodder_indicator_tables = ['"GpodderSyncMigrations"', '"GpodderSyncDeviceState"', 
                                      '"GpodderSyncSubscriptions"', '"GpodderSyncSettings"', 
                                      '"GpodderSessions"', '"GpodderSyncState"']
            gpodder_migration_versions = ["100", "101", "102", "103", "104"]
            
            gpodder_tables_exist = False
            for table in gpodder_indicator_tables:
                table_name = table.strip('"')
                if self.db_type == 'postgresql':
                    cursor.execute("""
                        SELECT EXISTS (
                            SELECT FROM information_schema.tables 
                            WHERE table_schema = 'public' AND table_name = %s
                        )
                    """, (table_name,))
                else:  # mysql
                    cursor.execute("""
                        SELECT COUNT(*) 
                        FROM information_schema.tables 
                        WHERE table_schema = DATABASE() AND table_name = %s
                    """, (table_name,))
                
                if cursor.fetchone()[0]:
                    gpodder_tables_exist = True
                    break
            
            if gpodder_tables_exist:
                for version in gpodder_migration_versions:
                    if version not in applied:
                        applied.append(version)
                        logger.info(f"Detected existing gpodder tables, marking migration {version} as applied")
            
            # Check for PeopleEpisodes_backup table separately (migration 104)
            backup_table = "PeopleEpisodes_backup"
            if self.db_type == 'postgresql':
                cursor.execute("""
                    SELECT EXISTS (
                        SELECT FROM information_schema.tables 
                        WHERE table_schema = 'public' AND table_name = %s
                    )
                """, (backup_table,))
            else:  # mysql
                cursor.execute("""
                    SELECT COUNT(*) 
                    FROM information_schema.tables 
                    WHERE table_schema = DATABASE() AND table_name = %s
                """, (backup_table,))
            
            if cursor.fetchone()[0] and "104" not in applied:
                applied.append("104")
                logger.info("Detected existing PeopleEpisodes_backup table, marking migration 104 as applied")
            
            return applied
            
        except Exception as e:
            logger.warning(f"Error detecting existing schema: {e}")
            return []
        finally:
            cursor.close()

    def run_migrations(self, target_version: Optional[str] = None) -> bool:
        """Run all pending migrations up to target version"""
        try:
            # Create migration table
            self.create_migration_table()
            
            # Get applied migrations
            applied_migrations = self.get_applied_migrations()
            logger.info(f"Found {len(applied_migrations)} applied migrations")
            
            # If no migrations are recorded but we have existing schema, detect what's there
            if not applied_migrations:
                detected_migrations = self.detect_existing_schema()
                if detected_migrations:
                    logger.info(f"Detected existing schema, marking {len(detected_migrations)} migrations as applied")
                    # Record detected migrations without executing them
                    for version in detected_migrations:
                        if version in self.migrations:
                            migration = self.migrations[version]
                            self.record_migration(migration, 0)  # 0ms execution time for pre-existing
                    
                    # Refresh applied migrations list
                    applied_migrations = self.get_applied_migrations()
            
            # Sort migrations by version
            pending_migrations = []
            for version, migration in sorted(self.migrations.items()):
                if version not in applied_migrations:
                    if target_version and version > target_version:
                        continue
                    pending_migrations.append(migration)
            
            if not pending_migrations:
                logger.info("No pending migrations to apply")
                return True
            
            logger.info(f"Found {len(pending_migrations)} pending migrations")
            
            # Execute pending migrations
            for migration in pending_migrations:
                # Check dependencies
                if not self.check_dependencies(migration, applied_migrations):
                    logger.error(f"Dependency check failed for migration {migration.version}")
                    return False
                
                # Execute migration
                if not self.execute_migration(migration):
                    logger.error(f"Failed to execute migration {migration.version}")
                    return False
                
                # Add to applied list for dependency checking
                applied_migrations.append(migration.version)
            
            logger.info("All migrations applied successfully")
            return True
            
        except Exception as e:
            logger.error(f"Migration run failed: {e}")
            return False
        finally:
            self.close_connection()

    def validate_migrations(self) -> bool:
        """Validate that applied migrations haven't changed"""
        try:
            conn = self.get_connection()
            cursor = conn.cursor()
            
            table_name = '"schema_migrations"' if self.db_type == 'postgresql' else 'schema_migrations'
            cursor.execute(f"SELECT version, checksum FROM {table_name}")
            applied_checksums = dict(cursor.fetchall())
            
            validation_errors = []
            for version, stored_checksum in applied_checksums.items():
                if version in self.migrations:
                    current_checksum = self.calculate_migration_checksum(self.migrations[version])
                    if current_checksum != stored_checksum:
                        validation_errors.append(f"Migration {version} checksum mismatch")
            
            if validation_errors:
                for error in validation_errors:
                    logger.error(error)
                return False
            
            logger.info("All migration checksums validated successfully")
            return True
            
        except Exception as e:
            logger.error(f"Migration validation failed: {e}")
            return False
        finally:
            cursor.close()


# Migration manager instance (singleton pattern)
_migration_manager: Optional[DatabaseMigrationManager] = None


def get_migration_manager() -> DatabaseMigrationManager:
    """Get the global migration manager instance"""
    global _migration_manager
    
    if _migration_manager is None:
        # Get database configuration from environment
        db_type = os.environ.get("DB_TYPE", "postgresql")
        
        if db_type.lower() in ['postgresql', 'postgres']:
            connection_params = {
                'host': os.environ.get("DB_HOST", "127.0.0.1"),
                'port': int(os.environ.get("DB_PORT", "5432")),
                'user': os.environ.get("DB_USER", "postgres"),
                'password': os.environ.get("DB_PASSWORD", "password"),
                'dbname': os.environ.get("DB_NAME", "pinepods_database")
            }
        else:  # mysql/mariadb
            connection_params = {
                'host': os.environ.get("DB_HOST", "127.0.0.1"),
                'port': int(os.environ.get("DB_PORT", "3306")),
                'user': os.environ.get("DB_USER", "root"),
                'password': os.environ.get("DB_PASSWORD", "password"),
                'database': os.environ.get("DB_NAME", "pinepods_database"),
                'charset': 'utf8mb4',
                'collation': 'utf8mb4_general_ci'
            }
        
        _migration_manager = DatabaseMigrationManager(db_type, connection_params)
    
    return _migration_manager


def register_migration(version: str, name: str, description: str, **kwargs):
    """Decorator to register a migration"""
    def decorator(func):
        migration = Migration(
            version=version,
            name=name,
            description=description,
            python_func=func,
            **kwargs
        )
        get_migration_manager().register_migration(migration)
        return func
    return decorator


def run_all_migrations() -> bool:
    """Run all registered migrations"""
    manager = get_migration_manager()
    return manager.run_migrations()