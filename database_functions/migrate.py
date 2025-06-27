#!/usr/bin/env python3
"""
Database Migration Runner for PinePods

This script can be run standalone to apply database migrations.
Useful for updating existing installations.
"""

import os
import sys
import logging
import argparse
from pathlib import Path

# Set up logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

# Add pinepods to path
pinepods_path = Path(__file__).parent.parent
sys.path.insert(0, str(pinepods_path))


def run_migrations(target_version=None, validate_only=False):
    """Run database migrations"""
    try:
        # Import migration system
        import database_functions.migration_definitions
        from database_functions.migrations import get_migration_manager, run_all_migrations
        
        # Register all migrations
        database_functions.migration_definitions.register_all_migrations()
        
        # Get migration manager
        manager = get_migration_manager()
        
        if validate_only:
            logger.info("Validating existing migrations...")
            success = manager.validate_migrations()
            if success:
                logger.info("All migrations validated successfully")
            else:
                logger.error("Migration validation failed")
            return success
        
        # Show current state
        applied = manager.get_applied_migrations()
        logger.info(f"Currently applied migrations: {len(applied)}")
        for version in applied:
            logger.info(f"  - {version}")
        
        # Run migrations
        logger.info("Starting migration process...")
        success = run_all_migrations()
        
        if success:
            logger.info("All migrations completed successfully")
        else:
            logger.error("Migration process failed")
            
        return success
        
    except Exception as e:
        logger.error(f"Migration failed: {e}")
        return False


def list_migrations():
    """List all available migrations"""
    try:
        import database_functions.migration_definitions
        from database_functions.migrations import get_migration_manager
        
        # Register migrations
        database_functions.migration_definitions.register_all_migrations()
        
        # Get manager and list migrations
        manager = get_migration_manager()
        applied = set(manager.get_applied_migrations())
        
        logger.info("Available migrations:")
        for version, migration in sorted(manager.migrations.items()):
            status = "APPLIED" if version in applied else "PENDING"
            logger.info(f"  {version} - {migration.name} [{status}]")
            logger.info(f"    {migration.description}")
            if migration.requires:
                logger.info(f"    Requires: {', '.join(migration.requires)}")
        
        return True
        
    except Exception as e:
        logger.error(f"Failed to list migrations: {e}")
        return False


def main():
    """Main CLI interface"""
    parser = argparse.ArgumentParser(description="PinePods Database Migration Tool")
    parser.add_argument(
        "command", 
        choices=["migrate", "list", "validate"],
        help="Command to execute"
    )
    parser.add_argument(
        "--target", 
        help="Target migration version (migrate only)"
    )
    parser.add_argument(
        "--verbose", "-v",
        action="store_true",
        help="Enable verbose logging"
    )
    
    args = parser.parse_args()
    
    if args.verbose:
        logging.getLogger().setLevel(logging.DEBUG)
    
    # Execute command
    if args.command == "migrate":
        success = run_migrations(args.target)
    elif args.command == "list":
        success = list_migrations()
    elif args.command == "validate":
        success = run_migrations(validate_only=True)
    else:
        logger.error(f"Unknown command: {args.command}")
        success = False
    
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()