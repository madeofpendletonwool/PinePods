#!/usr/bin/env python3
"""
Database Validator for PinePods

This script validates that an existing database matches the expected schema
by using the migration system as the source of truth.

Usage:
    python validate_database.py --db-type mysql --db-host localhost --db-port 3306 --db-user root --db-password pass --db-name pinepods_database
    python validate_database.py --db-type postgresql --db-host localhost --db-port 5432 --db-user postgres --db-password pass --db-name pinepods_database
"""

import argparse
import sys
import os
import tempfile
import logging
from typing import Dict, List, Set, Tuple, Any, Optional
from dataclasses import dataclass
import importlib.util

# Add the parent directory to path so we can import database_functions
parent_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, parent_dir)
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

try:
    import mysql.connector
    MYSQL_AVAILABLE = True
except ImportError:
    MYSQL_AVAILABLE = False

try:
    import psycopg
    POSTGRESQL_AVAILABLE = True
except ImportError:
    POSTGRESQL_AVAILABLE = False

from database_functions.migrations import get_migration_manager


@dataclass
class TableInfo:
    """Information about a database table"""
    name: str
    columns: Dict[str, Dict[str, Any]]
    indexes: Dict[str, Dict[str, Any]]
    constraints: Dict[str, Dict[str, Any]]
    
    
@dataclass
class ValidationResult:
    """Result of database validation"""
    is_valid: bool
    missing_tables: List[str]
    extra_tables: List[str]
    table_differences: Dict[str, Dict[str, Any]]
    missing_indexes: List[Tuple[str, str]]  # (table, index)
    extra_indexes: List[Tuple[str, str]]
    missing_constraints: List[Tuple[str, str]]  # (table, constraint)
    extra_constraints: List[Tuple[str, str]]
    column_differences: Dict[str, Dict[str, Dict[str, Any]]]  # table -> column -> differences


class DatabaseInspector:
    """Base class for database inspection"""
    
    def __init__(self, connection):
        self.connection = connection
        
    def get_tables(self) -> Set[str]:
        """Get all table names"""
        raise NotImplementedError
        
    def get_table_info(self, table_name: str) -> TableInfo:
        """Get detailed information about a table"""
        raise NotImplementedError
        
    def get_all_table_info(self) -> Dict[str, TableInfo]:
        """Get information about all tables"""
        tables = {}
        for table_name in self.get_tables():
            tables[table_name] = self.get_table_info(table_name)
        return tables


class MySQLInspector(DatabaseInspector):
    """MySQL database inspector"""
    
    def get_tables(self) -> Set[str]:
        cursor = self.connection.cursor()
        cursor.execute("SHOW TABLES")
        tables = {row[0] for row in cursor.fetchall()}
        cursor.close()
        return tables
        
    def get_table_info(self, table_name: str) -> TableInfo:
        cursor = self.connection.cursor(dictionary=True)
        
        # Get column information
        cursor.execute(f"DESCRIBE `{table_name}`")
        columns = {}
        for row in cursor.fetchall():
            columns[row['Field']] = {
                'type': row['Type'],
                'null': row['Null'],
                'key': row['Key'],
                'default': row['Default'],
                'extra': row['Extra']
            }
            
        # Get index information
        cursor.execute(f"SHOW INDEX FROM `{table_name}`")
        indexes = {}
        for row in cursor.fetchall():
            index_name = row['Key_name']
            if index_name not in indexes:
                indexes[index_name] = {
                    'columns': [],
                    'unique': not row['Non_unique'],
                    'type': row['Index_type']
                }
            indexes[index_name]['columns'].append(row['Column_name'])
            
        # Get constraint information (foreign keys, etc.)
        cursor.execute(f"""
            SELECT kcu.CONSTRAINT_NAME, tc.CONSTRAINT_TYPE, kcu.COLUMN_NAME, 
                   kcu.REFERENCED_TABLE_NAME, kcu.REFERENCED_COLUMN_NAME
            FROM INFORMATION_SCHEMA.KEY_COLUMN_USAGE kcu
            JOIN INFORMATION_SCHEMA.TABLE_CONSTRAINTS tc 
                ON kcu.CONSTRAINT_NAME = tc.CONSTRAINT_NAME 
                AND kcu.TABLE_SCHEMA = tc.TABLE_SCHEMA
            WHERE kcu.TABLE_SCHEMA = DATABASE() AND kcu.TABLE_NAME = %s
            AND kcu.REFERENCED_TABLE_NAME IS NOT NULL
        """, (table_name,))
        
        constraints = {}
        for row in cursor.fetchall():
            constraint_name = row['CONSTRAINT_NAME']
            constraints[constraint_name] = {
                'type': 'FOREIGN KEY',
                'column': row['COLUMN_NAME'],
                'referenced_table': row['REFERENCED_TABLE_NAME'],
                'referenced_column': row['REFERENCED_COLUMN_NAME']
            }
            
        cursor.close()
        return TableInfo(table_name, columns, indexes, constraints)


class PostgreSQLInspector(DatabaseInspector):
    """PostgreSQL database inspector"""
    
    def get_tables(self) -> Set[str]:
        cursor = self.connection.cursor()
        cursor.execute("""
            SELECT table_name 
            FROM information_schema.tables 
            WHERE table_schema = 'public' AND table_type = 'BASE TABLE'
        """)
        tables = {row[0] for row in cursor.fetchall()}
        cursor.close()
        return tables
        
    def get_table_info(self, table_name: str) -> TableInfo:
        cursor = self.connection.cursor()
        
        # Get column information
        cursor.execute("""
            SELECT column_name, data_type, is_nullable, column_default, 
                   character_maximum_length, numeric_precision, numeric_scale
            FROM information_schema.columns 
            WHERE table_schema = 'public' AND table_name = %s
            ORDER BY ordinal_position
        """, (table_name,))
        
        columns = {}
        for row in cursor.fetchall():
            col_name, data_type, is_nullable, default, max_length, precision, scale = row
            type_str = data_type
            if max_length:
                type_str += f"({max_length})"
            elif precision:
                if scale:
                    type_str += f"({precision},{scale})"
                else:
                    type_str += f"({precision})"
                    
            columns[col_name] = {
                'type': type_str,
                'null': is_nullable,
                'default': default,
                'max_length': max_length,
                'precision': precision,
                'scale': scale
            }
            
        # Get index information
        cursor.execute("""
            SELECT i.relname as index_name, 
                   array_agg(a.attname ORDER BY c.ordinality) as columns,
                   ix.indisunique as is_unique,
                   ix.indisprimary as is_primary
            FROM pg_class t
            JOIN pg_index ix ON t.oid = ix.indrelid
            JOIN pg_class i ON i.oid = ix.indexrelid
            JOIN unnest(ix.indkey) WITH ORDINALITY c(colnum, ordinality) ON true
            JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = c.colnum
            WHERE t.relname = %s AND t.relkind = 'r'
            GROUP BY i.relname, ix.indisunique, ix.indisprimary
        """, (table_name,))
        
        indexes = {}
        for row in cursor.fetchall():
            index_name, columns_list, is_unique, is_primary = row
            indexes[index_name] = {
                'columns': columns_list,
                'unique': is_unique,
                'primary': is_primary
            }
            
        # Get constraint information
        cursor.execute("""
            SELECT con.conname as constraint_name,
                   con.contype as constraint_type,
                   array_agg(att.attname) as columns,
                   cl.relname as referenced_table,
                   array_agg(att2.attname) as referenced_columns
            FROM pg_constraint con
            JOIN pg_class t ON con.conrelid = t.oid
            JOIN pg_attribute att ON att.attrelid = t.oid AND att.attnum = ANY(con.conkey)
            LEFT JOIN pg_class cl ON con.confrelid = cl.oid
            LEFT JOIN pg_attribute att2 ON att2.attrelid = cl.oid AND att2.attnum = ANY(con.confkey)
            WHERE t.relname = %s
            GROUP BY con.conname, con.contype, cl.relname
        """, (table_name,))
        
        constraints = {}
        for row in cursor.fetchall():
            constraint_name, constraint_type, columns_list, ref_table, ref_columns = row
            constraints[constraint_name] = {
                'type': constraint_type,
                'columns': columns_list,
                'referenced_table': ref_table,
                'referenced_columns': ref_columns
            }
            
        cursor.close()
        return TableInfo(table_name, columns, indexes, constraints)


class DatabaseValidator:
    """Main database validator class"""
    
    def __init__(self, db_type: str, db_config: Dict[str, Any]):
        self.db_type = db_type.lower()
        # Normalize mariadb to mysql since they use the same connector
        if self.db_type == 'mariadb':
            self.db_type = 'mysql'
        self.db_config = db_config
        self.logger = logging.getLogger(__name__)
        
    def create_test_database(self) -> Tuple[Any, str]:
        """Create a temporary database and run all migrations"""
        if self.db_type == 'mysql':
            return self._create_mysql_test_db()
        elif self.db_type == 'postgresql':
            return self._create_postgresql_test_db()
        else:
            raise ValueError(f"Unsupported database type: {self.db_type}")
            
    def _create_mysql_test_db(self) -> Tuple[Any, str]:
        """Create MySQL test database"""
        if not MYSQL_AVAILABLE:
            raise ImportError("mysql-connector-python is required for MySQL validation")
            
        # Create temporary database name
        import uuid
        test_db_name = f"pinepods_test_{uuid.uuid4().hex[:8]}"
        
        # Connect to MySQL server
        config = self.db_config.copy()
        config.pop('database', None)  # Remove database from config
        
        conn = mysql.connector.connect(**config)
        cursor = conn.cursor()
        
        try:
            # Create test database
            cursor.execute(f"CREATE DATABASE `{test_db_name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci")
            cursor.execute(f"USE `{test_db_name}`")
            cursor.close()
            
            # Run all migrations
            self._run_migrations(conn, 'mysql')
            
            # Create a fresh connection to the test database for schema inspection
            config['database'] = test_db_name
            test_conn = mysql.connector.connect(**config)
            
            # Close the migration connection
            conn.close()
            
            return test_conn, test_db_name
            
        except Exception as e:
            if cursor:
                cursor.close()
            if conn:
                conn.close()
            raise e
            
    def _create_postgresql_test_db(self) -> Tuple[Any, str]:
        """Create PostgreSQL test database"""
        if not POSTGRESQL_AVAILABLE:
            raise ImportError("psycopg is required for PostgreSQL validation")
            
        # Create temporary database name
        import uuid
        test_db_name = f"pinepods_test_{uuid.uuid4().hex[:8]}"
        
        # Connect to PostgreSQL server
        config = self.db_config.copy()
        config.pop('dbname', None)  # Remove database from config
        config['dbname'] = 'postgres'  # Connect to default database
        
        conn = psycopg.connect(**config)
        conn.autocommit = True
        cursor = conn.cursor()
        
        try:
            # Create test database
            cursor.execute(f'CREATE DATABASE "{test_db_name}"')
            cursor.close()
            conn.close()
            
            # Connect to the new test database
            config['dbname'] = test_db_name
            test_conn = psycopg.connect(**config)
            test_conn.autocommit = True
            
            # Run all migrations
            self._run_migrations(test_conn, 'postgresql')
            
            return test_conn, test_db_name
            
        except Exception as e:
            cursor.close()
            conn.close()
            raise e
            
    def _run_migrations(self, conn: Any, db_type: str):
        """Run all migrations on the test database using existing migration system"""
        # Set environment variables for the migration manager
        import os
        original_env = {}
        
        try:
            # Backup original environment
            for key in ['DB_TYPE', 'DB_HOST', 'DB_PORT', 'DB_USER', 'DB_PASSWORD', 'DB_NAME']:
                original_env[key] = os.environ.get(key)
            
            # Set environment for test database
            if db_type == 'mysql':
                os.environ['DB_TYPE'] = 'mysql'
                os.environ['DB_HOST'] = 'localhost'  # We'll override the connection
                os.environ['DB_PORT'] = '3306'
                os.environ['DB_USER'] = 'test'
                os.environ['DB_PASSWORD'] = 'test'
                os.environ['DB_NAME'] = 'test'
            else:
                os.environ['DB_TYPE'] = 'postgresql'
                os.environ['DB_HOST'] = 'localhost'
                os.environ['DB_PORT'] = '5432'
                os.environ['DB_USER'] = 'test'
                os.environ['DB_PASSWORD'] = 'test'
                os.environ['DB_NAME'] = 'test'
            
            # Import and register migrations
            import database_functions.migration_definitions
            
            # Get migration manager and override its connection
            manager = get_migration_manager()
            manager._connection = conn
            
            # Run all migrations
            success = manager.run_migrations()
            if not success:
                raise RuntimeError("Failed to apply migrations")
                
        finally:
            # Restore original environment
            for key, value in original_env.items():
                if value is not None:
                    os.environ[key] = value
                elif key in os.environ:
                    del os.environ[key]
        
    def validate_database(self) -> ValidationResult:
        """Validate the actual database against the expected schema"""
        # Create test database with perfect schema
        test_conn, test_db_name = self.create_test_database()
        
        try:
            # Connect to actual database
            actual_conn = self._connect_to_actual_database()
            
            try:
                # Get schema information from both databases
                if self.db_type == 'mysql':
                    expected_inspector = MySQLInspector(test_conn)
                    actual_inspector = MySQLInspector(actual_conn)
                    # Extract schemas
                    expected_schema = expected_inspector.get_all_table_info()
                    actual_schema = actual_inspector.get_all_table_info()
                else:
                    # For PostgreSQL, create fresh connection for expected schema since migration manager closes it
                    fresh_test_conn = psycopg.connect(
                        host=self.db_config['host'],
                        port=self.db_config['port'],
                        user=self.db_config['user'],
                        password=self.db_config['password'],
                        dbname=test_db_name
                    )
                    fresh_test_conn.autocommit = True
                    
                    try:
                        expected_inspector = PostgreSQLInspector(fresh_test_conn)
                        actual_inspector = PostgreSQLInspector(actual_conn)
                        
                        # Extract schemas
                        expected_schema = expected_inspector.get_all_table_info()
                        actual_schema = actual_inspector.get_all_table_info()
                    finally:
                        fresh_test_conn.close()
                
                # DEBUG: Print what we're actually comparing
                print(f"\n🔍 DEBUG: Expected schema has {len(expected_schema)} tables:")
                for table in sorted(expected_schema.keys()):
                    cols = list(expected_schema[table].columns.keys())
                    print(f"  {table}: {len(cols)} columns - {', '.join(cols[:5])}{'...' if len(cols) > 5 else ''}")
                
                print(f"\n🔍 DEBUG: Actual schema has {len(actual_schema)} tables:")
                for table in sorted(actual_schema.keys()):
                    cols = list(actual_schema[table].columns.keys())
                    print(f"  {table}: {len(cols)} columns - {', '.join(cols[:5])}{'...' if len(cols) > 5 else ''}")
                
                # Check specifically for Playlists table
                if 'Playlists' in expected_schema and 'Playlists' in actual_schema:
                    exp_cols = set(expected_schema['Playlists'].columns.keys())
                    act_cols = set(actual_schema['Playlists'].columns.keys())
                    print(f"\n🔍 DEBUG: Playlists comparison:")
                    print(f"  Expected columns: {sorted(exp_cols)}")
                    print(f"  Actual columns: {sorted(act_cols)}")
                    print(f"  Missing from actual: {sorted(exp_cols - act_cols)}")
                    print(f"  Extra in actual: {sorted(act_cols - exp_cols)}")
                
                # Compare schemas
                result = self._compare_schemas(expected_schema, actual_schema)
                
                return result
                
            finally:
                actual_conn.close()
                
        finally:
            # Clean up test database - this will close test_conn
            self._cleanup_test_database(test_conn, test_db_name)
            
    def _connect_to_actual_database(self) -> Any:
        """Connect to the actual database"""
        if self.db_type == 'mysql':
            config = self.db_config.copy()
            # Ensure autocommit is enabled for MySQL
            config['autocommit'] = True
            return mysql.connector.connect(**config)
        else:
            return psycopg.connect(**self.db_config)
            
    def _cleanup_test_database(self, test_conn: Any, test_db_name: str):
        """Clean up the test database"""
        try:
            # Close the test connection first
            if test_conn:
                test_conn.close()
                
            if self.db_type == 'mysql':
                config = self.db_config.copy()
                config.pop('database', None)
                cleanup_conn = mysql.connector.connect(**config)
                cursor = cleanup_conn.cursor()
                cursor.execute(f"DROP DATABASE IF EXISTS `{test_db_name}`")
                cursor.close()
                cleanup_conn.close()
            else:
                config = self.db_config.copy()
                config.pop('dbname', None)
                config['dbname'] = 'postgres'
                cleanup_conn = psycopg.connect(**config)
                cleanup_conn.autocommit = True
                cursor = cleanup_conn.cursor()
                cursor.execute(f'DROP DATABASE IF EXISTS "{test_db_name}"')
                cursor.close()
                cleanup_conn.close()
        except Exception as e:
            self.logger.warning(f"Failed to clean up test database {test_db_name}: {e}")
            
    def _compare_schemas(self, expected: Dict[str, TableInfo], actual: Dict[str, TableInfo]) -> ValidationResult:
        """Compare expected and actual database schemas"""
        expected_tables = set(expected.keys())
        actual_tables = set(actual.keys())
        
        missing_tables = list(expected_tables - actual_tables)
        extra_tables = list(actual_tables - expected_tables)
        
        table_differences = {}
        missing_indexes = []
        extra_indexes = []
        missing_constraints = []
        extra_constraints = []
        column_differences = {}
        
        # Compare common tables
        common_tables = expected_tables & actual_tables
        for table_name in common_tables:
            expected_table = expected[table_name]
            actual_table = actual[table_name]
            
            # Compare columns
            table_col_diffs = self._compare_columns(expected_table.columns, actual_table.columns)
            if table_col_diffs:
                column_differences[table_name] = table_col_diffs
            
            # Compare indexes
            expected_indexes = set(expected_table.indexes.keys())
            actual_indexes = set(actual_table.indexes.keys())
            
            for missing_idx in expected_indexes - actual_indexes:
                missing_indexes.append((table_name, missing_idx))
            for extra_idx in actual_indexes - expected_indexes:
                extra_indexes.append((table_name, extra_idx))
            
            # Compare constraints
            expected_constraints = set(expected_table.constraints.keys())
            actual_constraints = set(actual_table.constraints.keys())
            
            for missing_const in expected_constraints - actual_constraints:
                missing_constraints.append((table_name, missing_const))
            for extra_const in actual_constraints - expected_constraints:
                extra_constraints.append((table_name, extra_const))
        
        # Only fail on critical issues:
        # - Missing tables (CRITICAL)
        # - Missing columns (CRITICAL) 
        # Extra tables, extra columns, and type differences are warnings only
        critical_issues = []
        critical_issues.extend(missing_tables)
        
        # Check for missing columns (critical) - but only in expected tables
        for table, col_diffs in column_differences.items():
            # Skip extra tables entirely - they shouldn't be validated
            if table in extra_tables:
                continue
                
            for col, diff in col_diffs.items():
                if diff['status'] == 'missing':
                    critical_issues.append(f"missing column {col} in table {table}")
        
        is_valid = len(critical_issues) == 0
        
        return ValidationResult(
            is_valid=is_valid,
            missing_tables=missing_tables,
            extra_tables=extra_tables,
            table_differences=table_differences,
            missing_indexes=missing_indexes,
            extra_indexes=extra_indexes,
            missing_constraints=missing_constraints,
            extra_constraints=extra_constraints,
            column_differences=column_differences
        )
        
    def _compare_columns(self, expected: Dict[str, Dict[str, Any]], actual: Dict[str, Dict[str, Any]]) -> Dict[str, Dict[str, Any]]:
        """Compare column definitions between expected and actual"""
        differences = {}
        
        expected_cols = set(expected.keys())
        actual_cols = set(actual.keys())
        
        # Missing columns
        for missing_col in expected_cols - actual_cols:
            differences[missing_col] = {'status': 'missing', 'expected': expected[missing_col]}
        
        # Extra columns
        for extra_col in actual_cols - expected_cols:
            differences[extra_col] = {'status': 'extra', 'actual': actual[extra_col]}
        
        # Different columns
        for col_name in expected_cols & actual_cols:
            expected_col = expected[col_name]
            actual_col = actual[col_name]
            
            col_diffs = {}
            for key in expected_col:
                if key in actual_col and expected_col[key] != actual_col[key]:
                    col_diffs[key] = {'expected': expected_col[key], 'actual': actual_col[key]}
            
            if col_diffs:
                differences[col_name] = {'status': 'different', 'differences': col_diffs}
        
        return differences


def print_validation_report(result: ValidationResult):
    """Print a detailed validation report"""
    print("=" * 80)
    print("DATABASE VALIDATION REPORT")
    print("=" * 80)
    
    # Count critical vs warning issues
    critical_issues = []
    warning_issues = []
    
    # Missing tables are critical
    critical_issues.extend(result.missing_tables)
    
    # Missing columns are critical, others are warnings
    for table, col_diffs in result.column_differences.items():
        for col, diff in col_diffs.items():
            if diff['status'] == 'missing':
                critical_issues.append(f"Missing column {col} in table {table}")
            else:
                warning_issues.append((table, col, diff))
    
    # Extra tables are warnings
    warning_issues.extend([('EXTRA_TABLE', table, None) for table in result.extra_tables])
    
    if result.is_valid:
        if warning_issues:
            print("✅ DATABASE IS VALID - No critical issues found!")
            print("⚠️  Some warnings exist but don't affect functionality")
        else:
            print("✅ DATABASE IS PERFECT - All checks passed!")
    else:
        print("❌ DATABASE VALIDATION FAILED - Critical issues found")
    
    print()
    
    # Show critical issues
    if critical_issues:
        print("🔴 CRITICAL ISSUES (MUST BE FIXED):")
        if result.missing_tables:
            print("  Missing Tables:")
            for table in result.missing_tables:
                print(f"    - {table}")
        
        # Show missing columns
        for table, col_diffs in result.column_differences.items():
            missing_cols = [col for col, diff in col_diffs.items() if diff['status'] == 'missing']
            if missing_cols:
                print(f"  Missing Columns in {table}:")
                for col in missing_cols:
                    print(f"    - {col}")
        print()
    
    # Show warnings
    if warning_issues:
        print("⚠️  WARNINGS (ACCEPTABLE DIFFERENCES):")
        
        if result.extra_tables:
            print("  Extra Tables (ignored):")
            for table in result.extra_tables:
                print(f"    - {table}")
        
        # Show column warnings
        for table, col_diffs in result.column_differences.items():
            table_warnings = []
            for col, diff in col_diffs.items():
                if diff['status'] == 'extra':
                    table_warnings.append(f"Extra column: {col}")
                elif diff['status'] == 'different':
                    details = []
                    for key, values in diff['differences'].items():
                        details.append(f"{key}: {values}")
                    table_warnings.append(f"Different column {col}: {', '.join(details)}")
            
            if table_warnings:
                print(f"  Table {table}:")
                for warning in table_warnings:
                    print(f"    - {warning}")
        print()
    
    if result.missing_indexes:
        print("🟡 MISSING INDEXES:")
        for table, index in result.missing_indexes:
            print(f"  - {table}.{index}")
        print()
    
    if result.extra_indexes:
        print("🟡 EXTRA INDEXES:")
        for table, index in result.extra_indexes:
            print(f"  - {table}.{index}")
        print()
    
    if result.missing_constraints:
        print("🟡 MISSING CONSTRAINTS:")
        for table, constraint in result.missing_constraints:
            print(f"  - {table}.{constraint}")
        print()
    
    if result.extra_constraints:
        print("🟡 EXTRA CONSTRAINTS:")
        for table, constraint in result.extra_constraints:
            print(f"  - {table}.{constraint}")
        print()


def main():
    """Main function"""
    parser = argparse.ArgumentParser(description='Validate PinePods database schema')
    parser.add_argument('--db-type', required=True, choices=['mysql', 'mariadb', 'postgresql'], help='Database type')
    parser.add_argument('--db-host', required=True, help='Database host')
    parser.add_argument('--db-port', required=True, type=int, help='Database port')
    parser.add_argument('--db-user', required=True, help='Database user')
    parser.add_argument('--db-password', required=True, help='Database password')
    parser.add_argument('--db-name', required=True, help='Database name')
    parser.add_argument('--verbose', '-v', action='store_true', help='Enable verbose logging')
    
    args = parser.parse_args()
    
    # Set up logging
    level = logging.DEBUG if args.verbose else logging.INFO
    logging.basicConfig(level=level, format='%(asctime)s - %(levelname)s - %(message)s')
    
    # Build database config
    if args.db_type in ['mysql', 'mariadb']:
        db_config = {
            'host': args.db_host,
            'port': args.db_port,
            'user': args.db_user,
            'password': args.db_password,
            'database': args.db_name,
            'charset': 'utf8mb4',
            'collation': 'utf8mb4_unicode_ci'
        }
    else:  # postgresql
        db_config = {
            'host': args.db_host,
            'port': args.db_port,
            'user': args.db_user,
            'password': args.db_password,
            'dbname': args.db_name
        }
    
    try:
        # Create validator and run validation
        validator = DatabaseValidator(args.db_type, db_config)
        result = validator.validate_database()
        
        # Print report
        print_validation_report(result)
        
        # Exit with appropriate code
        sys.exit(0 if result.is_valid else 1)
        
    except Exception as e:
        logging.error(f"Validation failed with error: {e}")
        if args.verbose:
            import traceback
            traceback.print_exc()
        sys.exit(2)


if __name__ == '__main__':
    main()