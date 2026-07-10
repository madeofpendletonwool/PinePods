"""
Migration Definitions for PinePods Database Schema

This file contains all database migrations in chronological order.
Each migration is versioned and idempotent.
"""

import logging
import os
import sys
from cryptography.fernet import Fernet
import string
import secrets
import random
from typing import Any

# Add pinepods to path for imports
sys.path.append('/pinepods')

from database_functions.migrations import Migration, get_migration_manager, register_migration

# Import password hashing utilities
try:
    from passlib.hash import argon2
    from argon2 import PasswordHasher
    from argon2.exceptions import HashingError
except ImportError:
    pass

logger = logging.getLogger(__name__)


def generate_random_password(length=12):
    """Generate a random password"""
    characters = string.ascii_letters + string.digits + string.punctuation
    return ''.join(random.choice(characters) for i in range(length))


def hash_password(password: str):
    """Hash password using Argon2"""
    try:
        ph = PasswordHasher()
        return ph.hash(password)
    except (HashingError, NameError) as e:
        logger.error(f"Error hashing password: {e}")
        return None


def safe_execute_sql(cursor, sql: str, params=None, conn=None):
    """Safely execute SQL with error handling"""
    try:
        if params:
            cursor.execute(sql, params)
        else:
            cursor.execute(sql)
        return True
    except Exception as e:
        error_msg = str(e).lower()
        # These are expected errors when objects already exist
        expected_errors = [
            'already exists',
            'duplicate column',
            'duplicate key name',
            'constraint already exists',
            'relation already exists'
        ]
        
        if any(expected in error_msg for expected in expected_errors):
            logger.info(f"Skipping SQL (object already exists): {error_msg}")
            # For PostgreSQL, we need to rollback the transaction and start fresh
            if conn and 'current transaction is aborted' in str(e).lower():
                try:
                    conn.rollback()
                except:
                    pass
            return True
        else:
            logger.warning(f"SQL execution warning: {e}")
            # For PostgreSQL, rollback if transaction is aborted
            if conn and 'current transaction is aborted' in str(e).lower():
                try:
                    conn.rollback()
                except:
                    pass
            return False


def check_constraint_exists(cursor, db_type: str, table_name: str, constraint_name: str) -> bool:
    """Check if a constraint exists"""
    try:
        if db_type == 'postgresql':
            cursor.execute("""
                SELECT COUNT(*)
                FROM information_schema.table_constraints
                WHERE constraint_name = %s AND table_name = %s
            """, (constraint_name, table_name.strip('"')))
        else:  # mysql
            cursor.execute("""
                SELECT COUNT(*)
                FROM information_schema.table_constraints
                WHERE constraint_name = %s AND table_name = %s
                AND table_schema = DATABASE()
            """, (constraint_name, table_name))
        
        return cursor.fetchone()[0] > 0
    except:
        return False


def check_index_exists(cursor, db_type: str, index_name: str) -> bool:
    """Check if an index exists"""
    try:
        if db_type == 'postgresql':
            cursor.execute("""
                SELECT COUNT(*)
                FROM pg_indexes
                WHERE indexname = %s
            """, (index_name,))
        else:  # mysql
            cursor.execute("""
                SELECT COUNT(*)
                FROM information_schema.statistics
                WHERE index_name = %s AND table_schema = DATABASE()
            """, (index_name,))
        
        return cursor.fetchone()[0] > 0
    except:
        return False


def safe_add_constraint(cursor, db_type: str, sql: str, table_name: str, constraint_name: str, conn=None):
    """Safely add a constraint if it doesn't exist"""
    if not check_constraint_exists(cursor, db_type, table_name, constraint_name):
        try:
            cursor.execute(sql)
            logger.info(f"Added constraint {constraint_name}")
            return True
        except Exception as e:
            logger.warning(f"Failed to add constraint {constraint_name}: {e}")
            return False
    else:
        logger.info(f"Constraint {constraint_name} already exists, skipping")
        return True


def safe_add_index(cursor, db_type: str, sql: str, index_name: str, conn=None):
    """Safely add an index if it doesn't exist"""
    if not check_index_exists(cursor, db_type, index_name):
        try:
            cursor.execute(sql)
            logger.info(f"Added index {index_name}")
            return True
        except Exception as e:
            logger.warning(f"Failed to add index {index_name}: {e}")
            return False
    else:
        logger.info(f"Index {index_name} already exists, skipping")
        return True


# Migration 001: Core Tables Creation
@register_migration("001", "create_core_tables", "Create core database tables (Users, OIDCProviders, APIKeys, etc.)")
def migration_001_core_tables(conn, db_type: str):
    """Create core database tables"""
    cursor = conn.cursor()
    
    try:
        if db_type == 'postgresql':
            # Create Users table
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "Users" (
                    UserID SERIAL PRIMARY KEY,
                    Fullname VARCHAR(255),
                    Username VARCHAR(255) UNIQUE,
                    Email VARCHAR(255),
                    Hashed_PW VARCHAR(500),
                    IsAdmin BOOLEAN,
                    Reset_Code TEXT,
                    Reset_Expiry TIMESTAMP,
                    MFA_Secret VARCHAR(70),
                    TimeZone VARCHAR(50) DEFAULT 'UTC',
                    TimeFormat INT DEFAULT 24,
                    DateFormat VARCHAR(3) DEFAULT 'ISO',
                    FirstLogin BOOLEAN DEFAULT false,
                    GpodderUrl VARCHAR(255) DEFAULT '',
                    Pod_Sync_Type VARCHAR(50) DEFAULT 'None',
                    GpodderLoginName VARCHAR(255) DEFAULT '',
                    GpodderToken VARCHAR(255) DEFAULT '',
                    EnableRSSFeeds BOOLEAN DEFAULT FALSE,
                    auth_type VARCHAR(50) DEFAULT 'standard',
                    oidc_provider_id INT,
                    oidc_subject VARCHAR(255),
                    PlaybackSpeed NUMERIC(2,1) DEFAULT 1.0,
                    AutoDownloadDeleteDays INT DEFAULT 0,
                    DefaultVolume INT DEFAULT 100
                )
            """)

            # Create OIDCProviders table
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "OIDCProviders" (
                    ProviderID SERIAL PRIMARY KEY,
                    ProviderName VARCHAR(255) NOT NULL,
                    ClientID VARCHAR(255) NOT NULL,
                    ClientSecret VARCHAR(500) NOT NULL,
                    AuthorizationURL VARCHAR(255) NOT NULL,
                    TokenURL VARCHAR(255) NOT NULL,
                    UserInfoURL VARCHAR(255) NOT NULL,
                    Scope VARCHAR(255) DEFAULT 'openid email profile',
                    ButtonColor VARCHAR(50) DEFAULT '#000000',
                    ButtonText VARCHAR(255) NOT NULL,
                    ButtonTextColor VARCHAR(50) DEFAULT '#000000',
                    IconSVG TEXT,
                    Enabled BOOLEAN DEFAULT true,
                    Created TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    Modified TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )
            """)
            
            # Add foreign key constraint
            safe_add_constraint(cursor, db_type, """
                ALTER TABLE "Users"
                ADD CONSTRAINT fk_oidc_provider
                FOREIGN KEY (oidc_provider_id)
                REFERENCES "OIDCProviders"(ProviderID)
            """, "Users", "fk_oidc_provider")
            
        else:  # mysql
            # Create Users table
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS Users (
                    UserID INT AUTO_INCREMENT PRIMARY KEY,
                    Fullname VARCHAR(255),
                    Username VARCHAR(255) UNIQUE,
                    Email VARCHAR(255),
                    Hashed_PW CHAR(255),
                    IsAdmin TINYINT(1),
                    Reset_Code TEXT,
                    Reset_Expiry DATETIME,
                    MFA_Secret VARCHAR(70),
                    TimeZone VARCHAR(50) DEFAULT 'UTC',
                    TimeFormat INT DEFAULT 24,
                    DateFormat VARCHAR(3) DEFAULT 'ISO',
                    FirstLogin TINYINT(1) DEFAULT 0,
                    GpodderUrl VARCHAR(255) DEFAULT '',
                    Pod_Sync_Type VARCHAR(50) DEFAULT 'None',
                    GpodderLoginName VARCHAR(255) DEFAULT '',
                    GpodderToken VARCHAR(255) DEFAULT '',
                    EnableRSSFeeds TINYINT(1) DEFAULT 0,
                    auth_type VARCHAR(50) DEFAULT 'standard',
                    oidc_provider_id INT,
                    oidc_subject VARCHAR(255),
                    PlaybackSpeed DECIMAL(2,1) UNSIGNED DEFAULT 1.0,
                    AutoDownloadDeleteDays INT DEFAULT 0,
                    DefaultVolume INT DEFAULT 100
                )
            """)

            # Create OIDCProviders table
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS OIDCProviders (
                    ProviderID INT AUTO_INCREMENT PRIMARY KEY,
                    ProviderName VARCHAR(255) NOT NULL,
                    ClientID VARCHAR(255) NOT NULL,
                    ClientSecret VARCHAR(500) NOT NULL,
                    AuthorizationURL VARCHAR(255) NOT NULL,
                    TokenURL VARCHAR(255) NOT NULL,
                    UserInfoURL VARCHAR(255) NOT NULL,
                    Scope VARCHAR(255) DEFAULT 'openid email profile',
                    ButtonColor VARCHAR(50) DEFAULT '#000000',
                    ButtonText VARCHAR(255) NOT NULL,
                    ButtonTextColor VARCHAR(50) DEFAULT '#000000',
                    IconSVG TEXT,
                    Enabled TINYINT(1) DEFAULT 1,
                    Created DATETIME DEFAULT CURRENT_TIMESTAMP,
                    Modified DATETIME DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
                )
            """)
            
            # Add foreign key constraint
            safe_add_constraint(cursor, db_type, """
                ALTER TABLE Users
                ADD CONSTRAINT fk_oidc_provider
                FOREIGN KEY (oidc_provider_id)
                REFERENCES OIDCProviders(ProviderID)
            """, "Users", "fk_oidc_provider")
        
        # Create API and RSS key tables (same for both databases)
        table_prefix = '"' if db_type == 'postgresql' else ''
        table_suffix = '"' if db_type == 'postgresql' else ''
        
        cursor.execute(f"""
            CREATE TABLE IF NOT EXISTS {table_prefix}APIKeys{table_suffix} (
                APIKeyID {'SERIAL' if db_type == 'postgresql' else 'INT AUTO_INCREMENT'} PRIMARY KEY,
                UserID INT,
                APIKey TEXT,
                Created {'TIMESTAMP' if db_type == 'postgresql' else 'TIMESTAMP'} DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (UserID) REFERENCES {table_prefix}Users{table_suffix}(UserID) ON DELETE CASCADE
            )
        """)
        
        cursor.execute(f"""
            CREATE TABLE IF NOT EXISTS {table_prefix}RssKeys{table_suffix} (
                RssKeyID {'SERIAL' if db_type == 'postgresql' else 'INT AUTO_INCREMENT'} PRIMARY KEY,
                UserID INT,
                RssKey TEXT,
                Created {'TIMESTAMP' if db_type == 'postgresql' else 'TIMESTAMP'} DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (UserID) REFERENCES {table_prefix}Users{table_suffix}(UserID) ON DELETE CASCADE
            )
        """)
        
        cursor.execute(f"""
            CREATE TABLE IF NOT EXISTS {table_prefix}RssKeyMap{table_suffix} (
                RssKeyID INT,
                PodcastID INT,
                FOREIGN KEY (RssKeyID) REFERENCES {table_prefix}RssKeys{table_suffix}(RssKeyID) ON DELETE CASCADE
            )
        """)
        
        logger.info("Created core tables successfully")
        
    finally:
        cursor.close()


# Migration 002: App Settings and Configuration Tables
@register_migration("002", "app_settings", "Create app settings and configuration tables", requires=["001"])
def migration_002_app_settings(conn, db_type: str):
    """Create app settings and configuration tables"""
    cursor = conn.cursor()
    
    try:
        table_prefix = '"' if db_type == 'postgresql' else ''
        table_suffix = '"' if db_type == 'postgresql' else ''
        
        # Generate encryption key
        key = Fernet.generate_key()
        
        if db_type == 'postgresql':
            # Create AppSettings table
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "AppSettings" (
                    AppSettingsID SERIAL PRIMARY KEY,
                    SelfServiceUser BOOLEAN DEFAULT false,
                    DownloadEnabled BOOLEAN DEFAULT true,
                    EncryptionKey BYTEA,
                    NewsFeedSubscribed BOOLEAN DEFAULT false,
                    DownloadFolderCover BOOLEAN DEFAULT false,
                    DownloadEpisodeCover BOOLEAN DEFAULT false,
                    DownloadMetadataSidecar BOOLEAN DEFAULT false,
                    DownloadMetadataFormat VARCHAR(20) DEFAULT 'both',
                    DownloadMetadataSubfolder BOOLEAN DEFAULT true
                )
            """)
            
            # Insert default settings if not exists
            cursor.execute('SELECT COUNT(*) FROM "AppSettings" WHERE AppSettingsID = 1')
            count = cursor.fetchone()[0]
            if count == 0:
                cursor.execute("""
                    INSERT INTO "AppSettings" (SelfServiceUser, DownloadEnabled, EncryptionKey)
                    VALUES (false, true, %s)
                """, (key,))
            
            # Create EmailSettings table
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "EmailSettings" (
                    EmailSettingsID SERIAL PRIMARY KEY,
                    Server_Name VARCHAR(255),
                    Server_Port INT,
                    From_Email VARCHAR(255),
                    Send_Mode VARCHAR(255),
                    Encryption VARCHAR(255),
                    Auth_Required BOOLEAN,
                    Username VARCHAR(255),
                    Password VARCHAR(255)
                )
            """)
            
        else:  # mysql
            # Create AppSettings table
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS AppSettings (
                    AppSettingsID INT AUTO_INCREMENT PRIMARY KEY,
                    SelfServiceUser TINYINT(1) DEFAULT 0,
                    DownloadEnabled TINYINT(1) DEFAULT 1,
                    EncryptionKey BINARY(44),
                    NewsFeedSubscribed TINYINT(1) DEFAULT 0,
                    DownloadFolderCover TINYINT(1) DEFAULT 0,
                    DownloadEpisodeCover TINYINT(1) DEFAULT 0,
                    DownloadMetadataSidecar TINYINT(1) DEFAULT 0,
                    DownloadMetadataFormat VARCHAR(20) DEFAULT 'both',
                    DownloadMetadataSubfolder TINYINT(1) DEFAULT 1
                )
            """)
            
            # Insert default settings if not exists
            cursor.execute("SELECT COUNT(*) FROM AppSettings WHERE AppSettingsID = 1")
            count = cursor.fetchone()[0]
            if count == 0:
                cursor.execute("""
                    INSERT INTO AppSettings (SelfServiceUser, DownloadEnabled, EncryptionKey)
                    VALUES (0, 1, %s)
                """, (key,))
            
            # Create EmailSettings table
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS EmailSettings (
                    EmailSettingsID INT AUTO_INCREMENT PRIMARY KEY,
                    Server_Name VARCHAR(255),
                    Server_Port INT,
                    From_Email VARCHAR(255),
                    Send_Mode VARCHAR(255),
                    Encryption VARCHAR(255),
                    Auth_Required TINYINT(1),
                    Username VARCHAR(255),
                    Password VARCHAR(255)
                )
            """)
        
        # Insert default email settings if not exists
        cursor.execute(f"SELECT COUNT(*) FROM {table_prefix}EmailSettings{table_suffix}")
        rows = cursor.fetchone()
        if rows[0] == 0:
            cursor.execute(f"""
                INSERT INTO {table_prefix}EmailSettings{table_suffix} 
                (Server_Name, Server_Port, From_Email, Send_Mode, Encryption, Auth_Required, Username, Password)
                VALUES ('default_server', 587, 'default_email@domain.com', 'default_mode', 'default_encryption', 
                       {'true' if db_type == 'postgresql' else '1'}, 'default_username', 'default_password')
            """)
        
        logger.info("Created app settings tables successfully")
        
    finally:
        cursor.close()


# Migration 003: User Management Tables
@register_migration("003", "user_tables", "Create user stats and settings tables", requires=["001"])
def migration_003_user_tables(conn, db_type: str):
    """Create user management tables"""
    cursor = conn.cursor()
    
    try:
        table_prefix = '"' if db_type == 'postgresql' else ''
        table_suffix = '"' if db_type == 'postgresql' else ''
        
        # Create UserStats table
        if db_type == 'postgresql':
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "UserStats" (
                    UserStatsID SERIAL PRIMARY KEY,
                    UserID INT UNIQUE,
                    UserCreated TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    PodcastsPlayed INT DEFAULT 0,
                    TimeListened INT DEFAULT 0,
                    PodcastsAdded INT DEFAULT 0,
                    EpisodesSaved INT DEFAULT 0,
                    EpisodesDownloaded INT DEFAULT 0,
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID)
                )
            """)
            
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "UserSettings" (
                    usersettingid SERIAL PRIMARY KEY,
                    userid INT UNIQUE,
                    theme VARCHAR(255) DEFAULT 'Nordic',
                    startpage VARCHAR(255) DEFAULT 'home',
                    FOREIGN KEY (userid) REFERENCES "Users"(userid)
                )
            """)
        else:  # mysql
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS UserStats (
                    UserStatsID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT UNIQUE,
                    UserCreated TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    PodcastsPlayed INT DEFAULT 0,
                    TimeListened INT DEFAULT 0,
                    PodcastsAdded INT DEFAULT 0,
                    EpisodesSaved INT DEFAULT 0,
                    EpisodesDownloaded INT DEFAULT 0,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID)
                )
            """)
            
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS UserSettings (
                    UserSettingID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT UNIQUE,
                    Theme VARCHAR(255) DEFAULT 'Nordic',
                    StartPage VARCHAR(255) DEFAULT 'home',
                    FOREIGN KEY (UserID) REFERENCES Users(UserID)
                )
            """)
        
        logger.info("Created user management tables successfully")
        
    finally:
        cursor.close()


# Migration 004: Default Users Creation
@register_migration("004", "default_users", "Create default background tasks and admin users", requires=["001", "003"])
def migration_004_default_users(conn, db_type: str):
    """Create default users"""
    cursor = conn.cursor()
    
    try:
        table_prefix = '"' if db_type == 'postgresql' else ''
        table_suffix = '"' if db_type == 'postgresql' else ''
        
        # Create background tasks user
        random_password = generate_random_password()
        hashed_password = hash_password(random_password)
        
        if hashed_password:
            if db_type == 'postgresql':
                cursor.execute("""
                    INSERT INTO "Users" (Fullname, Username, Email, Hashed_PW, IsAdmin)
                    VALUES (%s, %s, %s, %s, %s)
                    ON CONFLICT (Username) DO NOTHING
                """, ('Background Tasks', 'background_tasks', 'inactive', hashed_password, False))
            else:  # mysql
                cursor.execute("""
                    INSERT IGNORE INTO Users (Fullname, Username, Email, Hashed_PW, IsAdmin)
                    VALUES (%s, %s, %s, %s, %s)
                """, ('Background Tasks', 'background_tasks', 'inactive', hashed_password, False))
        
        # Create admin user from environment variables if provided
        admin_fullname = os.environ.get("FULLNAME")
        admin_username = os.environ.get("USERNAME")
        admin_email = os.environ.get("EMAIL")
        admin_pw = os.environ.get("PASSWORD")
        
        admin_created = False
        if all([admin_fullname, admin_username, admin_email, admin_pw]):
            hashed_pw = hash_password(admin_pw)
            if hashed_pw:
                if db_type == 'postgresql':
                    cursor.execute("""
                        INSERT INTO "Users" (Fullname, Username, Email, Hashed_PW, IsAdmin)
                        VALUES (%s, %s, %s, %s, %s)
                        ON CONFLICT (Username) DO NOTHING
                        RETURNING UserID
                    """, (admin_fullname, admin_username, admin_email, hashed_pw, True))
                    admin_created = cursor.fetchone() is not None
                else:  # mysql
                    cursor.execute("""
                        INSERT IGNORE INTO Users (Fullname, Username, Email, Hashed_PW, IsAdmin)
                        VALUES (%s, %s, %s, %s, %s)
                    """, (admin_fullname, admin_username, admin_email, hashed_pw, True))
                    admin_created = cursor.rowcount > 0
        
        # Create user stats and settings for default users
        if db_type == 'postgresql':
            cursor.execute("""
                INSERT INTO "UserStats" (UserID) VALUES (1)
                ON CONFLICT (UserID) DO NOTHING
            """)
            cursor.execute("""
                INSERT INTO "UserSettings" (UserID, Theme) VALUES (1, 'Nordic')
                ON CONFLICT (UserID) DO NOTHING
            """)
            if admin_created:
                cursor.execute("""
                    INSERT INTO "UserStats" (UserID) VALUES (2)
                    ON CONFLICT (UserID) DO NOTHING
                """)
                cursor.execute("""
                    INSERT INTO "UserSettings" (UserID, Theme) VALUES (2, 'Nordic')
                    ON CONFLICT (UserID) DO NOTHING
                """)
        else:  # mysql
            cursor.execute("INSERT IGNORE INTO UserStats (UserID) VALUES (1)")
            cursor.execute("INSERT IGNORE INTO UserSettings (UserID, Theme) VALUES (1, 'Nordic')")
            if admin_created:
                cursor.execute("INSERT IGNORE INTO UserStats (UserID) VALUES (2)")
                cursor.execute("INSERT IGNORE INTO UserSettings (UserID, Theme) VALUES (2, 'Nordic')")
        
        # Create API key for background tasks user
        cursor.execute(f'SELECT APIKey FROM {table_prefix}APIKeys{table_suffix} WHERE UserID = 1')
        result = cursor.fetchone()
        
        if not result:
            alphabet = string.ascii_letters + string.digits
            api_key = ''.join(secrets.choice(alphabet) for _ in range(64))
            cursor.execute(f'INSERT INTO {table_prefix}APIKeys{table_suffix} (UserID, APIKey) VALUES (1, %s)', (api_key,))
        else:
            # Extract API key from existing record
            api_key = result[0] if isinstance(result, tuple) else result['apikey']
        
        # Note: Web API key file removed for security - background tasks now authenticate via database
        
        logger.info("Created default users successfully")
        
    finally:
        cursor.close()


# Migration 005: Podcast and Episode Tables
@register_migration("005", "podcast_episode_tables", "Create podcast and episode management tables", requires=["001"])
def migration_005_podcast_episode_tables(conn, db_type: str):
    """Create podcast and episode tables"""
    cursor = conn.cursor()
    
    try:
        table_prefix = '"' if db_type == 'postgresql' else ''
        table_suffix = '"' if db_type == 'postgresql' else ''
        
        if db_type == 'postgresql':
            # Create Podcasts table
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "Podcasts" (
                    PodcastID SERIAL PRIMARY KEY,
                    PodcastIndexID INT,
                    PodcastName TEXT,
                    ArtworkURL TEXT,
                    Author TEXT,
                    Categories TEXT,
                    Description TEXT,
                    EpisodeCount INT,
                    FeedURL TEXT,
                    WebsiteURL TEXT,
                    Explicit BOOLEAN,
                    UserID INT,
                    AutoDownload BOOLEAN DEFAULT FALSE,
                    AutoQueue BOOLEAN DEFAULT FALSE,
                    StartSkip INT DEFAULT 0,
                    EndSkip INT DEFAULT 0,
                    Username TEXT,
                    Password TEXT,
                    IsYouTubeChannel BOOLEAN DEFAULT FALSE,
                    NotificationsEnabled BOOLEAN DEFAULT FALSE,
                    FeedCutoffDays INT DEFAULT 0,
                    PlaybackSpeed NUMERIC(2,1) DEFAULT 1.0,
                    PlaybackSpeedCustomized BOOLEAN DEFAULT FALSE,
                    AutoDownloadDeleteDays INT DEFAULT 0,
                    AutoDownloadDeleteCustomized BOOLEAN DEFAULT FALSE,
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID)
                )
            """)
            
            # Add unique constraint
            safe_add_constraint(cursor, db_type, """
                ALTER TABLE "Podcasts"
                ADD CONSTRAINT podcasts_userid_feedurl_key
                UNIQUE (UserID, FeedURL)
            """, "Podcasts", "podcasts_userid_feedurl_key")
            
            # Create Episodes table
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "Episodes" (
                    EpisodeID SERIAL PRIMARY KEY,
                    PodcastID INT,
                    EpisodeTitle TEXT,
                    EpisodeDescription TEXT,
                    EpisodeURL TEXT,
                    EpisodeArtwork TEXT,
                    EpisodePubDate TIMESTAMP,
                    EpisodeDuration INT,
                    Completed BOOLEAN DEFAULT FALSE,
                    FOREIGN KEY (PodcastID) REFERENCES "Podcasts"(PodcastID)
                )
            """)
            
            # Create YouTube Videos table
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "YouTubeVideos" (
                    VideoID SERIAL PRIMARY KEY,
                    PodcastID INT,
                    VideoTitle TEXT,
                    VideoDescription TEXT,
                    VideoURL TEXT,
                    ThumbnailURL TEXT,
                    PublishedAt TIMESTAMP,
                    Duration INT,
                    YouTubeVideoID TEXT,
                    Completed BOOLEAN DEFAULT FALSE,
                    ListenPosition INT DEFAULT 0,
                    FOREIGN KEY (PodcastID) REFERENCES "Podcasts"(PodcastID)
                )
            """)
            
        else:  # mysql
            # Create Podcasts table
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS Podcasts (
                    PodcastID INT AUTO_INCREMENT PRIMARY KEY,
                    PodcastIndexID INT,
                    PodcastName TEXT,
                    ArtworkURL TEXT,
                    Author TEXT,
                    Categories TEXT,
                    Description TEXT,
                    EpisodeCount INT,
                    FeedURL TEXT,
                    WebsiteURL TEXT,
                    Explicit TINYINT(1),
                    UserID INT,
                    AutoDownload TINYINT(1) DEFAULT 0,
                    AutoQueue TINYINT(1) DEFAULT 0,
                    StartSkip INT DEFAULT 0,
                    EndSkip INT DEFAULT 0,
                    Username TEXT,
                    Password TEXT,
                    IsYouTubeChannel TINYINT(1) DEFAULT 0,
                    NotificationsEnabled TINYINT(1) DEFAULT 0,
                    FeedCutoffDays INT DEFAULT 0,
                    PlaybackSpeed DECIMAL(2,1) UNSIGNED DEFAULT 1.0,
                    PlaybackSpeedCustomized TINYINT(1) DEFAULT 0,
                    AutoDownloadDeleteDays INT DEFAULT 0,
                    AutoDownloadDeleteCustomized TINYINT(1) DEFAULT 0,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID)
                )
            """)
            
            # Create Episodes table
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS Episodes (
                    EpisodeID INT AUTO_INCREMENT PRIMARY KEY,
                    PodcastID INT,
                    EpisodeTitle TEXT,
                    EpisodeDescription TEXT,
                    EpisodeURL TEXT,
                    EpisodeArtwork TEXT,
                    EpisodePubDate DATETIME,
                    EpisodeDuration INT,
                    Completed TINYINT(1) DEFAULT 0,
                    FOREIGN KEY (PodcastID) REFERENCES Podcasts(PodcastID)
                )
            """)
            
            # Create YouTube Videos table
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS YouTubeVideos (
                    VideoID INT AUTO_INCREMENT PRIMARY KEY,
                    PodcastID INT,
                    VideoTitle TEXT,
                    VideoDescription TEXT,
                    VideoURL TEXT,
                    ThumbnailURL TEXT,
                    PublishedAt TIMESTAMP,
                    Duration INT,
                    YouTubeVideoID TEXT,
                    Completed TINYINT(1) DEFAULT 0,
                    ListenPosition INT DEFAULT 0,
                    FOREIGN KEY (PodcastID) REFERENCES Podcasts(PodcastID)
                )
            """)
        
        # Create indexes for performance
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_podcasts_userid ON {table_prefix}Podcasts{table_suffix}(UserID)', 'idx_podcasts_userid')
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_episodes_podcastid ON {table_prefix}Episodes{table_suffix}(PodcastID)', 'idx_episodes_podcastid')
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_episodes_episodepubdate ON {table_prefix}Episodes{table_suffix}(EpisodePubDate)', 'idx_episodes_episodepubdate')
        
        logger.info("Created podcast and episode tables successfully")
        
    finally:
        cursor.close()


# Migration 006: User Activity Tables
@register_migration("006", "user_activity_tables", "Create user activity tracking tables", requires=["005"])
def migration_006_user_activity_tables(conn, db_type: str):
    """Create user activity tracking tables"""
    cursor = conn.cursor()
    
    try:
        table_prefix = '"' if db_type == 'postgresql' else ''
        table_suffix = '"' if db_type == 'postgresql' else ''
        
        if db_type == 'postgresql':
            # User Episode History
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "UserEpisodeHistory" (
                    UserEpisodeHistoryID SERIAL PRIMARY KEY,
                    UserID INT,
                    EpisodeID INT,
                    ListenDate TIMESTAMP,
                    ListenDuration INT,
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID),
                    FOREIGN KEY (EpisodeID) REFERENCES "Episodes"(EpisodeID)
                )
            """)
            
            # User Video History
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "UserVideoHistory" (
                    UserVideoHistoryID SERIAL PRIMARY KEY,
                    UserID INT,
                    VideoID INT,
                    ListenDate TIMESTAMP,
                    ListenDuration INT DEFAULT 0,
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID),
                    FOREIGN KEY (VideoID) REFERENCES "YouTubeVideos"(VideoID)
                )
            """)
            
            # Add unique constraints
            safe_add_constraint(cursor, db_type, """
                ALTER TABLE "UserEpisodeHistory"
                ADD CONSTRAINT user_episode_unique
                UNIQUE (UserID, EpisodeID)
            """, "UserEpisodeHistory", "user_episode_unique")
            
            safe_add_constraint(cursor, db_type, """
                ALTER TABLE "UserVideoHistory"
                ADD CONSTRAINT user_video_unique
                UNIQUE (UserID, VideoID)
            """, "UserVideoHistory", "user_video_unique")
            
        else:  # mysql
            # User Episode History
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS UserEpisodeHistory (
                    UserEpisodeHistoryID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT,
                    EpisodeID INT,
                    ListenDate DATETIME,
                    ListenDuration INT,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID),
                    FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID)
                )
            """)
            
            # User Video History
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS UserVideoHistory (
                    UserVideoHistoryID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT,
                    VideoID INT,
                    ListenDate TIMESTAMP,
                    ListenDuration INT DEFAULT 0,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID),
                    FOREIGN KEY (VideoID) REFERENCES YouTubeVideos(VideoID)
                )
            """)
        
        logger.info("Created user activity tables successfully")
        
    finally:
        cursor.close()


# Migration 007: Queue and Download Tables
@register_migration("007", "queue_download_tables", "Create queue and download management tables", requires=["005"])
def migration_007_queue_download_tables(conn, db_type: str):
    """Create queue and download tables"""
    cursor = conn.cursor()
    
    try:
        table_prefix = '"' if db_type == 'postgresql' else ''
        table_suffix = '"' if db_type == 'postgresql' else ''
        
        if db_type == 'postgresql':
            # Episode Queue
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "EpisodeQueue" (
                    QueueID SERIAL PRIMARY KEY,
                    QueueDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    UserID INT,
                    EpisodeID INT,
                    QueuePosition INT NOT NULL DEFAULT 0,
                    is_youtube BOOLEAN DEFAULT FALSE,
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID)
                )
            """)
            
            # Saved Episodes and Videos
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "SavedEpisodes" (
                    SaveID SERIAL PRIMARY KEY,
                    UserID INT,
                    EpisodeID INT,
                    SaveDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID),
                    FOREIGN KEY (EpisodeID) REFERENCES "Episodes"(EpisodeID)
                )
            """)
            
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "SavedVideos" (
                    SaveID SERIAL PRIMARY KEY,
                    UserID INT,
                    VideoID INT,
                    SaveDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID),
                    FOREIGN KEY (VideoID) REFERENCES "YouTubeVideos"(VideoID)
                )
            """)
            
            # Downloaded Episodes and Videos
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "DownloadedEpisodes" (
                    DownloadID SERIAL PRIMARY KEY,
                    UserID INT,
                    EpisodeID INT,
                    DownloadedDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    DownloadedSize INT,
                    DownloadedLocation VARCHAR(255),
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID),
                    FOREIGN KEY (EpisodeID) REFERENCES "Episodes"(EpisodeID)
                )
            """)
            
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "DownloadedVideos" (
                    DownloadID SERIAL PRIMARY KEY,
                    UserID INT,
                    VideoID INT,
                    DownloadedDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    DownloadedSize INT,
                    DownloadedLocation VARCHAR(255),
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID),
                    FOREIGN KEY (VideoID) REFERENCES "YouTubeVideos"(VideoID)
                )
            """)
            
        else:  # mysql
            # Episode Queue
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS EpisodeQueue (
                    QueueID INT AUTO_INCREMENT PRIMARY KEY,
                    QueueDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    UserID INT,
                    EpisodeID INT,
                    QueuePosition INT NOT NULL DEFAULT 0,
                    is_youtube TINYINT(1) DEFAULT 0,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID)
                )
            """)
            
            # Saved Episodes and Videos
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS SavedEpisodes (
                    SaveID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT,
                    EpisodeID INT,
                    SaveDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID),
                    FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID)
                )
            """)
            
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS SavedVideos (
                    SaveID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT,
                    VideoID INT,
                    SaveDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID),
                    FOREIGN KEY (VideoID) REFERENCES YouTubeVideos(VideoID)
                )
            """)
            
            # Downloaded Episodes and Videos
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS DownloadedEpisodes (
                    DownloadID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT,
                    EpisodeID INT,
                    DownloadedDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    DownloadedSize INT,
                    DownloadedLocation VARCHAR(255),
                    FOREIGN KEY (UserID) REFERENCES Users(UserID),
                    FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID)
                )
            """)
            
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS DownloadedVideos (
                    DownloadID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT,
                    VideoID INT,
                    DownloadedDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    DownloadedSize INT,
                    DownloadedLocation VARCHAR(255),
                    FOREIGN KEY (UserID) REFERENCES Users(UserID),
                    FOREIGN KEY (VideoID) REFERENCES YouTubeVideos(VideoID)
                )
            """)
        
        logger.info("Created queue and download tables successfully")
        
    finally:
        cursor.close()


@register_migration("008", "gpodder_tables", "Create GPodder sync tables", requires=["001"])
def migration_008_gpodder_tables(conn, db_type: str):
    """Create GPodder sync tables"""
    cursor = conn.cursor()
    
    try:
        if db_type == "postgresql":
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "GpodderDevices" (
                    DeviceID SERIAL PRIMARY KEY,
                    UserID INT NOT NULL,
                    DeviceName VARCHAR(255) NOT NULL,
                    DeviceType VARCHAR(50) DEFAULT 'desktop',
                    DeviceCaption VARCHAR(255),
                    IsDefault BOOLEAN DEFAULT FALSE,
                    LastSync TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    IsActive BOOLEAN DEFAULT TRUE,
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE,
                    UNIQUE(UserID, DeviceName)
                )
            """)
            
            cursor.execute("""
                CREATE INDEX IF NOT EXISTS idx_gpodder_devices_userid
                ON "GpodderDevices"(UserID)
            """)
            
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "GpodderSyncState" (
                    SyncStateID SERIAL PRIMARY KEY,
                    UserID INT NOT NULL,
                    DeviceID INT NOT NULL,
                    LastTimestamp BIGINT DEFAULT 0,
                    EpisodesTimestamp BIGINT DEFAULT 0,
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE,
                    FOREIGN KEY (DeviceID) REFERENCES "GpodderDevices"(DeviceID) ON DELETE CASCADE,
                    UNIQUE(UserID, DeviceID)
                )
            """)
        else:
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS GpodderDevices (
                    DeviceID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT NOT NULL,
                    DeviceName VARCHAR(255) NOT NULL,
                    DeviceType VARCHAR(50) DEFAULT 'desktop',
                    DeviceCaption VARCHAR(255),
                    IsDefault BOOLEAN DEFAULT FALSE,
                    LastSync TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    IsActive BOOLEAN DEFAULT TRUE,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE,
                    UNIQUE(UserID, DeviceName)
                )
            """)
            
            # Check if index exists before creating it
            try:
                cursor.execute("""
                    CREATE INDEX idx_gpodder_devices_userid
                    ON GpodderDevices(UserID)
                """)
                logger.info("Created index idx_gpodder_devices_userid")
            except Exception as e:
                if "Duplicate key name" in str(e) or "1061" in str(e):
                    logger.info("Index idx_gpodder_devices_userid already exists, skipping")
                else:
                    raise
            
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS GpodderSyncState (
                    SyncStateID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT NOT NULL,
                    DeviceID INT NOT NULL,
                    LastTimestamp BIGINT DEFAULT 0,
                    EpisodesTimestamp BIGINT DEFAULT 0,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE,
                    FOREIGN KEY (DeviceID) REFERENCES GpodderDevices(DeviceID) ON DELETE CASCADE,
                    UNIQUE(UserID, DeviceID)
                )
            """)
        
        logger.info("Created GPodder tables")
        
    finally:
        cursor.close()


@register_migration("009", "people_sharing_tables", "Create people and episode sharing tables", requires=["005"])
def migration_009_people_sharing_tables(conn, db_type: str):
    """Create people and episode sharing tables"""
    cursor = conn.cursor()
    
    try:
        if db_type == "postgresql":
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "People" (
                    PersonID SERIAL PRIMARY KEY,
                    Name TEXT,
                    PersonImg TEXT,
                    PeopleDBID INT,
                    AssociatedPodcasts TEXT,
                    UserID INT,
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE
                )
            """)
            
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "PeopleEpisodes" (
                    EpisodeID SERIAL PRIMARY KEY,
                    PersonID INT,
                    PodcastID INT,
                    EpisodeTitle TEXT,
                    EpisodeDescription TEXT,
                    EpisodeURL TEXT,
                    EpisodeArtwork TEXT,
                    EpisodePubDate TIMESTAMP,
                    EpisodeDuration INT,
                    AddedDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (PersonID) REFERENCES "People"(PersonID),
                    FOREIGN KEY (PodcastID) REFERENCES "Podcasts"(PodcastID)
                )
            """)
            
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "SharedEpisodes" (
                    SharedEpisodeID SERIAL PRIMARY KEY,
                    EpisodeID INT NOT NULL,
                    SharedBy INT NOT NULL,
                    SharedWith INT,
                    ShareCode TEXT UNIQUE,
                    ExpirationDate TIMESTAMP,
                    FOREIGN KEY (EpisodeID) REFERENCES "Episodes"(EpisodeID) ON DELETE CASCADE,
                    FOREIGN KEY (SharedBy) REFERENCES "Users"(UserID) ON DELETE CASCADE,
                    FOREIGN KEY (SharedWith) REFERENCES "Users"(UserID) ON DELETE CASCADE
                )
            """)
        else:
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS People (
                    PersonID INT AUTO_INCREMENT PRIMARY KEY,
                    Name TEXT,
                    PersonImg TEXT,
                    PeopleDBID INT,
                    AssociatedPodcasts TEXT,
                    UserID INT,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE
                )
            """)
            
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS PeopleEpisodes (
                    EpisodeID INT AUTO_INCREMENT PRIMARY KEY,
                    PersonID INT,
                    PodcastID INT,
                    EpisodeTitle TEXT,
                    EpisodeDescription TEXT,
                    EpisodeURL TEXT,
                    EpisodeArtwork TEXT,
                    EpisodePubDate TIMESTAMP,
                    EpisodeDuration INT,
                    AddedDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (PersonID) REFERENCES People(PersonID),
                    FOREIGN KEY (PodcastID) REFERENCES Podcasts(PodcastID)
                )
            """)
            
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS SharedEpisodes (
                    SharedEpisodeID INT AUTO_INCREMENT PRIMARY KEY,
                    EpisodeID INT NOT NULL,
                    SharedBy INT NOT NULL,
                    SharedWith INT,
                    ShareCode TEXT,
                    ExpirationDate TIMESTAMP,
                    FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID) ON DELETE CASCADE,
                    FOREIGN KEY (SharedBy) REFERENCES Users(UserID) ON DELETE CASCADE,
                    FOREIGN KEY (SharedWith) REFERENCES Users(UserID) ON DELETE CASCADE,
                    UNIQUE(ShareCode(255))
                )
            """)
        
        logger.info("Created people and sharing tables")
        
    finally:
        cursor.close()


@register_migration("010", "playlist_tables", "Create playlist management tables", requires=["005"])
def migration_010_playlist_tables(conn, db_type: str):
    """Create playlist management tables"""
    cursor = conn.cursor()
    
    try:
        if db_type == "postgresql":
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "Playlists" (
                    PlaylistID SERIAL PRIMARY KEY,
                    UserID INT NOT NULL,
                    Name VARCHAR(255) NOT NULL,
                    Description TEXT,
                    IsSystemPlaylist BOOLEAN NOT NULL DEFAULT FALSE,
                    PodcastIDs INTEGER[],
                    IncludeUnplayed BOOLEAN NOT NULL DEFAULT TRUE,
                    IncludePartiallyPlayed BOOLEAN NOT NULL DEFAULT TRUE,
                    IncludePlayed BOOLEAN NOT NULL DEFAULT FALSE,
                    MinDuration INTEGER,
                    MaxDuration INTEGER,
                    SortOrder VARCHAR(50) NOT NULL DEFAULT 'date_desc'
                        CHECK (SortOrder IN ('date_asc', 'date_desc',
                                           'duration_asc', 'duration_desc',
                                           'listen_progress', 'completion')),
                    GroupByPodcast BOOLEAN NOT NULL DEFAULT FALSE,
                    MaxEpisodes INTEGER,
                    PlayProgressMin FLOAT,
                    PlayProgressMax FLOAT,
                    TimeFilterHours INTEGER,
                    LastUpdated TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    Created TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    IconName VARCHAR(50) NOT NULL DEFAULT 'ph-playlist',
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE,
                    UNIQUE(UserID, Name),
                    CHECK (PlayProgressMin IS NULL OR (PlayProgressMin >= 0 AND PlayProgressMin <= 100)),
                    CHECK (PlayProgressMax IS NULL OR (PlayProgressMax >= 0 AND PlayProgressMax <= 100)),
                    CHECK (PlayProgressMin IS NULL OR PlayProgressMax IS NULL OR PlayProgressMin <= PlayProgressMax),
                    CHECK (MinDuration IS NULL OR MinDuration >= 0),
                    CHECK (MaxDuration IS NULL OR MaxDuration >= 0)
                )
            """)
            
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "PlaylistContents" (
                    PlaylistContentID SERIAL PRIMARY KEY,
                    PlaylistID INT,
                    EpisodeID INT,
                    VideoID INT,
                    Position INT,
                    DateAdded TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (PlaylistID) REFERENCES "Playlists"(PlaylistID) ON DELETE CASCADE,
                    FOREIGN KEY (EpisodeID) REFERENCES "Episodes"(EpisodeID) ON DELETE CASCADE,
                    FOREIGN KEY (VideoID) REFERENCES "YouTubeVideos"(VideoID) ON DELETE CASCADE,
                    CHECK ((EpisodeID IS NOT NULL AND VideoID IS NULL) OR (EpisodeID IS NULL AND VideoID IS NOT NULL))
                )
            """)
            
            # Create indexes for better performance
            cursor.execute("""
                CREATE INDEX IF NOT EXISTS idx_playlists_userid ON "Playlists"(UserID);
                CREATE INDEX IF NOT EXISTS idx_playlist_contents_playlistid ON "PlaylistContents"(PlaylistID);
                CREATE INDEX IF NOT EXISTS idx_playlist_contents_episodeid ON "PlaylistContents"(EpisodeID);
                CREATE INDEX IF NOT EXISTS idx_playlist_contents_videoid ON "PlaylistContents"(VideoID);
            """)
        else:
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS Playlists (
                    PlaylistID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT NOT NULL,
                    Name VARCHAR(255) NOT NULL,
                    Description TEXT,
                    IsSystemPlaylist BOOLEAN NOT NULL DEFAULT FALSE,
                    PodcastIDs JSON,
                    IncludeUnplayed BOOLEAN NOT NULL DEFAULT TRUE,
                    IncludePartiallyPlayed BOOLEAN NOT NULL DEFAULT TRUE,
                    IncludePlayed BOOLEAN NOT NULL DEFAULT FALSE,
                    MinDuration INT,
                    MaxDuration INT,
                    SortOrder VARCHAR(50) NOT NULL DEFAULT 'date_desc'
                        CHECK (SortOrder IN ('date_asc', 'date_desc',
                                           'duration_asc', 'duration_desc',
                                           'listen_progress', 'completion')),
                    GroupByPodcast BOOLEAN NOT NULL DEFAULT FALSE,
                    MaxEpisodes INT,
                    PlayProgressMin FLOAT,
                    PlayProgressMax FLOAT,
                    TimeFilterHours INT,
                    LastUpdated TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    Created TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    IconName VARCHAR(50) NOT NULL DEFAULT 'ph-playlist',
                    FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE,
                    UNIQUE(UserID, Name),
                    CHECK (PlayProgressMin IS NULL OR (PlayProgressMin >= 0 AND PlayProgressMin <= 100)),
                    CHECK (PlayProgressMax IS NULL OR (PlayProgressMax >= 0 AND PlayProgressMax <= 100)),
                    CHECK (PlayProgressMin IS NULL OR PlayProgressMax IS NULL OR PlayProgressMin <= PlayProgressMax),
                    CHECK (MinDuration IS NULL OR MinDuration >= 0),
                    CHECK (MaxDuration IS NULL OR MaxDuration >= 0)
                )
            """)
            
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS PlaylistContents (
                    PlaylistContentID INT AUTO_INCREMENT PRIMARY KEY,
                    PlaylistID INT,
                    EpisodeID INT,
                    VideoID INT,
                    Position INT,
                    DateAdded TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (PlaylistID) REFERENCES Playlists(PlaylistID) ON DELETE CASCADE,
                    FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID) ON DELETE CASCADE,
                    FOREIGN KEY (VideoID) REFERENCES YouTubeVideos(VideoID) ON DELETE CASCADE,
                    CHECK ((EpisodeID IS NOT NULL AND VideoID IS NULL) OR (EpisodeID IS NULL AND VideoID IS NOT NULL))
                )
            """)
            
            # Create indexes for better performance (MySQL doesn't support IF NOT EXISTS for indexes)
            try:
                cursor.execute("CREATE INDEX idx_playlists_userid ON Playlists(UserID)")
            except:
                pass  # Index may already exist
            try:
                cursor.execute("CREATE INDEX idx_playlist_contents_playlistid ON PlaylistContents(PlaylistID)")
            except:
                pass  # Index may already exist
            try:
                cursor.execute("CREATE INDEX idx_playlist_contents_episodeid ON PlaylistContents(EpisodeID)")
            except:
                pass  # Index may already exist
            try:
                cursor.execute("CREATE INDEX idx_playlist_contents_videoid ON PlaylistContents(VideoID)")
            except:
                pass  # Index may already exist
        
        logger.info("Created playlist tables")
        
    finally:
        cursor.close()


@register_migration("011", "session_notification_tables", "Create session and notification tables", requires=["001"])
def migration_011_session_notification_tables(conn, db_type: str):
    """Create session and notification tables"""
    cursor = conn.cursor()
    
    try:
        if db_type == "postgresql":
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "Sessions" (
                    SessionID SERIAL PRIMARY KEY,
                    UserID INT NOT NULL,
                    SessionToken TEXT NOT NULL,
                    ExpirationTime TIMESTAMP NOT NULL,
                    IsActive BOOLEAN DEFAULT TRUE,
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE
                )
            """)
            
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "UserNotificationSettings" (
                    SettingID SERIAL PRIMARY KEY,
                    UserID INT,
                    Platform VARCHAR(50) NOT NULL,
                    Enabled BOOLEAN DEFAULT TRUE,
                    NtfyTopic VARCHAR(255),
                    NtfyServerUrl VARCHAR(255) DEFAULT 'https://ntfy.sh',
                    GotifyUrl VARCHAR(255),
                    GotifyToken VARCHAR(255),
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE,
                    UNIQUE(UserID, Platform)
                )
            """)
        else:
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS Sessions (
                    SessionID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT NOT NULL,
                    SessionToken TEXT NOT NULL,
                    ExpirationTime TIMESTAMP NOT NULL,
                    IsActive BOOLEAN DEFAULT TRUE,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE
                )
            """)
            
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS UserNotificationSettings (
                    SettingID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT,
                    Platform VARCHAR(50) NOT NULL,
                    Enabled BOOLEAN DEFAULT TRUE,
                    NtfyTopic VARCHAR(255),
                    NtfyServerUrl VARCHAR(255) DEFAULT 'https://ntfy.sh',
                    GotifyUrl VARCHAR(255),
                    GotifyToken VARCHAR(255),
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE,
                    UNIQUE(UserID, Platform)
                )
            """)
        
        logger.info("Created session and notification tables")
        
    finally:
        cursor.close()


@register_migration("012", "create_system_playlists", "Create default system playlists", requires=["010"])
def migration_012_create_system_playlists(conn, db_type: str):
    """Create default system playlists"""
    cursor = conn.cursor()
    
    try:
        # Define system playlists
        system_playlists = [
            {
                'name': 'Quick Listens',
                'description': 'Short episodes under 15 minutes, perfect for quick breaks',
                'min_duration': None,
                'max_duration': 900,  # 15 minutes
                'sort_order': 'duration_asc',
                'icon_name': 'ph-fast-forward'
            },
            {
                'name': 'Longform',
                'description': 'Extended episodes over 1 hour, ideal for long drives or deep dives',
                'min_duration': 3600,  # 1 hour
                'max_duration': None,
                'sort_order': 'duration_desc',
                'icon_name': 'ph-car'
            },
            {
                'name': 'Currently Listening',
                'description': 'Episodes you\'ve started but haven\'t finished',
                'min_duration': None,
                'max_duration': None,
                'sort_order': 'date_desc',
                'include_unplayed': False,
                'include_partially_played': True,
                'include_played': False,
                'icon_name': 'ph-play'
            },
            {
                'name': 'Fresh Releases',
                'description': 'Latest episodes from the last 24 hours',
                'min_duration': None,
                'max_duration': None,
                'sort_order': 'date_desc',
                'include_unplayed': True,
                'include_partially_played': False,
                'include_played': False,
                'time_filter_hours': 24,
                'icon_name': 'ph-sparkle'
            },
            {
                'name': 'Weekend Marathon',
                'description': 'Longer episodes (30+ minutes) perfect for weekend listening',
                'min_duration': 1800,  # 30 minutes
                'max_duration': None,
                'sort_order': 'duration_desc',
                'group_by_podcast': True,
                'icon_name': 'ph-couch'
            },
            {
                'name': 'Commuter Mix',
                'description': 'Episodes between 20-40 minutes, ideal for average commute times',
                'min_duration': 1200,  # 20 minutes
                'max_duration': 2400,  # 40 minutes
                'sort_order': 'date_desc',
                'icon_name': 'ph-train'
            },
            {
                'name': 'Almost Done',
                'description': 'Episodes you\'re close to finishing (75%+ complete)',
                'min_duration': None,
                'max_duration': None,
                'sort_order': 'date_asc',
                'include_unplayed': False,
                'include_partially_played': True,
                'include_played': False,
                'play_progress_min': 75.0,
                'play_progress_max': None,
                'icon_name': 'ph-hourglass'
            }
        ]

        # Insert system playlists for background tasks user (UserID = 1)
        for playlist in system_playlists:
            try:
                # First check if this playlist already exists
                if db_type == "postgresql":
                    cursor.execute("""
                        SELECT COUNT(*)
                        FROM "Playlists"
                        WHERE UserID = 1 AND Name = %s AND IsSystemPlaylist = TRUE
                    """, (playlist['name'],))
                else:
                    cursor.execute("""
                        SELECT COUNT(*)
                        FROM Playlists
                        WHERE UserID = 1 AND Name = %s AND IsSystemPlaylist = TRUE
                    """, (playlist['name'],))

                if cursor.fetchone()[0] == 0:
                    if db_type == "postgresql":
                        cursor.execute("""
                            INSERT INTO "Playlists" (
                                UserID,
                                Name,
                                Description,
                                IsSystemPlaylist,
                                MinDuration,
                                MaxDuration,
                                SortOrder,
                                GroupByPodcast,
                                IncludeUnplayed,
                                IncludePartiallyPlayed,
                                IncludePlayed,
                                IconName,
                                TimeFilterHours,
                                PlayProgressMin,
                                PlayProgressMax
                            ) VALUES (
                                1, %s, %s, TRUE, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s
                            )
                        """, (
                            playlist['name'],
                            playlist['description'],
                            playlist.get('min_duration'),
                            playlist.get('max_duration'),
                            playlist.get('sort_order', 'date_asc'),
                            playlist.get('group_by_podcast', False),
                            playlist.get('include_unplayed', True),
                            playlist.get('include_partially_played', True),
                            playlist.get('include_played', False),
                            playlist.get('icon_name', 'ph-playlist'),
                            playlist.get('time_filter_hours'),
                            playlist.get('play_progress_min'),
                            playlist.get('play_progress_max')
                        ))
                    else:
                        cursor.execute("""
                            INSERT INTO Playlists (
                                UserID,
                                Name,
                                Description,
                                IsSystemPlaylist,
                                MinDuration,
                                MaxDuration,
                                SortOrder,
                                GroupByPodcast,
                                IncludeUnplayed,
                                IncludePartiallyPlayed,
                                IncludePlayed,
                                IconName,
                                TimeFilterHours,
                                PlayProgressMin,
                                PlayProgressMax
                            ) VALUES (
                                1, %s, %s, TRUE, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s
                            )
                        """, (
                            playlist['name'],
                            playlist['description'],
                            playlist.get('min_duration'),
                            playlist.get('max_duration'),
                            playlist.get('sort_order', 'date_asc'),
                            playlist.get('group_by_podcast', False),
                            playlist.get('include_unplayed', True),
                            playlist.get('include_partially_played', True),
                            playlist.get('include_played', False),
                            playlist.get('icon_name', 'ph-playlist'),
                            playlist.get('time_filter_hours'),
                            playlist.get('play_progress_min'),
                            playlist.get('play_progress_max')
                        ))
                    
                    logger.info(f"Created system playlist: {playlist['name']}")
                else:
                    logger.info(f"System playlist already exists: {playlist['name']}")

            except Exception as e:
                logger.error(f"Error creating system playlist {playlist['name']}: {e}")
                continue

        logger.info("System playlists creation completed")
        
    finally:
        cursor.close()


@register_migration("013", "add_playback_speed_columns", "Add PlaybackSpeed columns to Users and Podcasts tables for existing installations")
def add_playback_speed_columns(conn, db_type: str) -> None:
    """Add PlaybackSpeed columns to Users and Podcasts tables for existing installations"""
    
    cursor = conn.cursor()
    
    try:
        # Add PlaybackSpeed to Users table if it doesn't exist
        try:
            if db_type == "postgresql":
                cursor.execute("""
                    ALTER TABLE "Users" 
                    ADD COLUMN IF NOT EXISTS PlaybackSpeed NUMERIC(2,1) DEFAULT 1.0
                """)
            else:  # MySQL/MariaDB
                # Check if column exists first
                cursor.execute("""
                    SELECT COUNT(*) 
                    FROM INFORMATION_SCHEMA.COLUMNS 
                    WHERE TABLE_SCHEMA = DATABASE() 
                    AND TABLE_NAME = 'Users' 
                    AND COLUMN_NAME = 'PlaybackSpeed'
                """)
                if cursor.fetchone()[0] == 0:
                    cursor.execute("""
                        ALTER TABLE Users 
                        ADD COLUMN PlaybackSpeed DECIMAL(2,1) UNSIGNED DEFAULT 1.0
                    """)
                    logger.info("Added PlaybackSpeed column to Users table")
                else:
                    logger.info("PlaybackSpeed column already exists in Users table")
                    
        except Exception as e:
            logger.error(f"Error adding PlaybackSpeed to Users table: {e}")
            # Don't fail the migration for this
    
        # Add PlaybackSpeed columns to Podcasts table if they don't exist
        try:
            if db_type == "postgresql":
                cursor.execute("""
                    ALTER TABLE "Podcasts" 
                    ADD COLUMN IF NOT EXISTS PlaybackSpeed NUMERIC(2,1) DEFAULT 1.0,
                    ADD COLUMN IF NOT EXISTS PlaybackSpeedCustomized BOOLEAN DEFAULT FALSE
                """)
            else:  # MySQL/MariaDB
                # Check if PlaybackSpeed column exists
                cursor.execute("""
                    SELECT COUNT(*) 
                    FROM INFORMATION_SCHEMA.COLUMNS 
                    WHERE TABLE_SCHEMA = DATABASE() 
                    AND TABLE_NAME = 'Podcasts' 
                    AND COLUMN_NAME = 'PlaybackSpeed'
                """)
                if cursor.fetchone()[0] == 0:
                    cursor.execute("""
                        ALTER TABLE Podcasts 
                        ADD COLUMN PlaybackSpeed DECIMAL(2,1) UNSIGNED DEFAULT 1.0
                    """)
                    logger.info("Added PlaybackSpeed column to Podcasts table")
                else:
                    logger.info("PlaybackSpeed column already exists in Podcasts table")
                    
                # Check if PlaybackSpeedCustomized column exists
                cursor.execute("""
                    SELECT COUNT(*) 
                    FROM INFORMATION_SCHEMA.COLUMNS 
                    WHERE TABLE_SCHEMA = DATABASE() 
                    AND TABLE_NAME = 'Podcasts' 
                    AND COLUMN_NAME = 'PlaybackSpeedCustomized'
                """)
                if cursor.fetchone()[0] == 0:
                    cursor.execute("""
                        ALTER TABLE Podcasts 
                        ADD COLUMN PlaybackSpeedCustomized TINYINT(1) DEFAULT 0
                    """)
                    logger.info("Added PlaybackSpeedCustomized column to Podcasts table")
                else:
                    logger.info("PlaybackSpeedCustomized column already exists in Podcasts table")
                    
        except Exception as e:
            logger.error(f"Error adding PlaybackSpeed columns to Podcasts table: {e}")
            # Don't fail the migration for this
        
        logger.info("Playback speed columns migration completed")
    
    finally:
        cursor.close()


@register_migration("014", "fix_missing_rss_tables", "Create missing RSS tables from migration 001 for 0.7.8 upgrades")
def fix_missing_rss_tables(conn, db_type: str) -> None:
    """Create missing RSS tables for users upgrading from 0.7.8"""
    
    cursor = conn.cursor()
    
    try:
        # Check and create RssKeys table if it doesn't exist
        if db_type == 'postgresql':
            table_name = '"RssKeys"'
            cursor.execute("""
                SELECT EXISTS (
                    SELECT FROM information_schema.tables 
                    WHERE table_schema = 'public' AND table_name = %s
                )
            """, ('RssKeys',))
        else:  # mysql
            table_name = 'RssKeys'
            cursor.execute("""
                SELECT COUNT(*) 
                FROM information_schema.tables 
                WHERE table_schema = DATABASE() AND table_name = %s
            """, ('RssKeys',))
        
        table_exists = cursor.fetchone()[0]
        
        if not table_exists:
            logger.info("Creating missing RssKeys table")
            if db_type == 'postgresql':
                cursor.execute("""
                    CREATE TABLE "RssKeys" (
                        RssKeyID SERIAL PRIMARY KEY,
                        UserID INT,
                        RssKey TEXT,
                        Created TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                        FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE
                    )
                """)
            else:  # mysql
                cursor.execute("""
                    CREATE TABLE RssKeys (
                        RssKeyID INT AUTO_INCREMENT PRIMARY KEY,
                        UserID INT,
                        RssKey TEXT,
                        Created TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                        FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE
                    )
                """)
            logger.info("Created RssKeys table")
        else:
            logger.info("RssKeys table already exists")
        
        # Check and create RssKeyMap table if it doesn't exist
        if db_type == 'postgresql':
            cursor.execute("""
                SELECT EXISTS (
                    SELECT FROM information_schema.tables 
                    WHERE table_schema = 'public' AND table_name = %s
                )
            """, ('RssKeyMap',))
        else:  # mysql
            cursor.execute("""
                SELECT COUNT(*) 
                FROM information_schema.tables 
                WHERE table_schema = DATABASE() AND table_name = %s
            """, ('RssKeyMap',))
        
        table_exists = cursor.fetchone()[0]
        
        if not table_exists:
            logger.info("Creating missing RssKeyMap table")
            if db_type == 'postgresql':
                cursor.execute("""
                    CREATE TABLE "RssKeyMap" (
                        RssKeyID INT,
                        PodcastID INT,
                        FOREIGN KEY (RssKeyID) REFERENCES "RssKeys"(RssKeyID) ON DELETE CASCADE
                    )
                """)
            else:  # mysql
                cursor.execute("""
                    CREATE TABLE RssKeyMap (
                        RssKeyID INT,
                        PodcastID INT,
                        FOREIGN KEY (RssKeyID) REFERENCES RssKeys(RssKeyID) ON DELETE CASCADE
                    )
                """)
            logger.info("Created RssKeyMap table")
        else:
            logger.info("RssKeyMap table already exists")
        
        logger.info("Missing RSS tables migration completed")

    finally:
        cursor.close()
    

# Migration 015: OIDC settings for claims & roles.
@register_migration("015", "oidc_claims_and_roles", "Add columns for OIDC claims & roles settings", requires=["002"])
def migration_015_oidc_claims_and_roles(conn, db_type: str):
    """Add columns for OIDC claims & roles settings"""
    cursor = conn.cursor()

    try:
        if db_type == "postgresql":
            cursor.execute("""
                ALTER TABLE "OIDCProviders"
                ADD COLUMN IF NOT EXISTS NameClaim VARCHAR(255),
                ADD COLUMN IF NOT EXISTS EmailClaim VARCHAR(255),
                ADD COLUMN IF NOT EXISTS UsernameClaim VARCHAR(255),
                ADD COLUMN IF NOT EXISTS RolesClaim VARCHAR(255),
                ADD COLUMN IF NOT EXISTS UserRole VARCHAR(255),
                ADD COLUMN IF NOT EXISTS AdminRole VARCHAR(255);
            """)
        else:
            cursor.execute("""
                ALTER TABLE OIDCProviders
                ADD COLUMN NameClaim VARCHAR(255) AFTER IconSVG,
                ADD COLUMN EmailClaim VARCHAR(255) AFTER NameClaim,
                ADD COLUMN UsernameClaim VARCHAR(255) AFTER EmailClaim,
                ADD COLUMN RolesClaim VARCHAR(255) AFTER UsernameClaim,
                ADD COLUMN UserRole VARCHAR(255) AFTER RolesClaim,
                ADD COLUMN AdminRole VARCHAR(255) AFTER UserRole;
            """)

        logger.info("Added claim & roles settings to OIDC table")

    finally:
        cursor.close()


# Migration 016: Add autocompleteseconds to UserSettings
@register_migration("016", "add_auto_complete_seconds", "Add autocompleteseconds column to UserSettings table", requires=["003"])
def migration_016_add_auto_complete_seconds(conn, db_type: str):
    """Add autocompleteseconds column to UserSettings table"""
    cursor = conn.cursor()
    
    try:
        if db_type == 'postgresql':
            # Check if column exists first
            cursor.execute("""
                SELECT COUNT(*)
                FROM information_schema.columns
                WHERE table_name = 'UserSettings' 
                AND column_name = 'autocompleteseconds'
                AND table_schema = 'public'
            """)
            column_exists = cursor.fetchone()[0] > 0
            
            if not column_exists:
                cursor.execute("""
                    ALTER TABLE "UserSettings"
                    ADD COLUMN autocompleteseconds INTEGER DEFAULT 0
                """)
                logger.info("Added autocompleteseconds column to UserSettings table (PostgreSQL)")
            else:
                logger.info("autocompleteseconds column already exists in UserSettings table (PostgreSQL)")
                
        else:  # mysql
            # Check if column exists first
            cursor.execute("""
                SELECT COUNT(*)
                FROM information_schema.columns
                WHERE table_name = 'UserSettings' 
                AND column_name = 'AutoCompleteSeconds'
                AND table_schema = DATABASE()
            """)
            column_exists = cursor.fetchone()[0] > 0
            
            if not column_exists:
                cursor.execute("""
                    ALTER TABLE UserSettings
                    ADD COLUMN AutoCompleteSeconds INT DEFAULT 0
                """)
                logger.info("Added AutoCompleteSeconds column to UserSettings table (MySQL)")
            else:
                logger.info("AutoCompleteSeconds column already exists in UserSettings table (MySQL)")
        
        logger.info("Auto complete seconds migration completed successfully")
        
    finally:
        cursor.close()


@register_migration("017", "add_ntfy_auth_columns", "Add ntfy authentication columns to UserNotificationSettings table", requires=["011"])
def migration_017_add_ntfy_auth_columns(conn, db_type: str):
    """Add ntfy authentication columns (username, password, access_token) to UserNotificationSettings table"""
    cursor = conn.cursor()
    
    try:
        if db_type == "postgresql":
            # Check if columns already exist (PostgreSQL - lowercase column names)
            cursor.execute("""
                SELECT column_name FROM information_schema.columns 
                WHERE table_name = 'UserNotificationSettings' 
                AND column_name IN ('ntfyusername', 'ntfypassword', 'ntfyaccesstoken')
            """)
            existing_columns = [row[0] for row in cursor.fetchall()]
            
            if 'ntfyusername' not in existing_columns:
                cursor.execute("""
                    ALTER TABLE "UserNotificationSettings"
                    ADD COLUMN ntfyusername VARCHAR(255)
                """)
                logger.info("Added ntfyusername column to UserNotificationSettings table (PostgreSQL)")
            
            if 'ntfypassword' not in existing_columns:
                cursor.execute("""
                    ALTER TABLE "UserNotificationSettings"
                    ADD COLUMN ntfypassword VARCHAR(255)
                """)
                logger.info("Added ntfypassword column to UserNotificationSettings table (PostgreSQL)")
            
            if 'ntfyaccesstoken' not in existing_columns:
                cursor.execute("""
                    ALTER TABLE "UserNotificationSettings"
                    ADD COLUMN ntfyaccesstoken VARCHAR(255)
                """)
                logger.info("Added ntfyaccesstoken column to UserNotificationSettings table (PostgreSQL)")
        
        else:
            # Check if columns already exist (MySQL)
            cursor.execute("""
                SELECT COUNT(*)
                FROM information_schema.columns
                WHERE table_name = 'UserNotificationSettings' 
                AND column_name = 'NtfyUsername'
                AND table_schema = DATABASE()
            """)
            username_exists = cursor.fetchone()[0] > 0
            
            cursor.execute("""
                SELECT COUNT(*)
                FROM information_schema.columns
                WHERE table_name = 'UserNotificationSettings' 
                AND column_name = 'NtfyPassword'
                AND table_schema = DATABASE()
            """)
            password_exists = cursor.fetchone()[0] > 0
            
            cursor.execute("""
                SELECT COUNT(*)
                FROM information_schema.columns
                WHERE table_name = 'UserNotificationSettings' 
                AND column_name = 'NtfyAccessToken'
                AND table_schema = DATABASE()
            """)
            token_exists = cursor.fetchone()[0] > 0
            
            if not username_exists:
                cursor.execute("""
                    ALTER TABLE UserNotificationSettings
                    ADD COLUMN NtfyUsername VARCHAR(255)
                """)
                logger.info("Added NtfyUsername column to UserNotificationSettings table (MySQL)")
            
            if not password_exists:
                cursor.execute("""
                    ALTER TABLE UserNotificationSettings
                    ADD COLUMN NtfyPassword VARCHAR(255)
                """)
                logger.info("Added NtfyPassword column to UserNotificationSettings table (MySQL)")
            
            if not token_exists:
                cursor.execute("""
                    ALTER TABLE UserNotificationSettings
                    ADD COLUMN NtfyAccessToken VARCHAR(255)
                """)
                logger.info("Added NtfyAccessToken column to UserNotificationSettings table (MySQL)")
        
        logger.info("Ntfy authentication columns migration completed successfully")
        
    finally:
        cursor.close()


@register_migration("018", "add_gpodder_sync_timestamp", "Add GPodder last sync timestamp for incremental sync", requires=["001"])
def migration_018_gpodder_sync_timestamp(conn, db_type: str):
    """Add GPodder last sync timestamp column for proper incremental sync per GPodder spec"""
    cursor = conn.cursor()
    
    try:
        if db_type == "postgresql":
            # Check if column already exists (PostgreSQL)
            cursor.execute("""
                SELECT column_name FROM information_schema.columns 
                WHERE table_name = 'Users' 
                AND column_name = 'lastsynctime'
            """)
            existing_columns = [row[0] for row in cursor.fetchall()]
            
            if 'lastsynctime' not in existing_columns:
                cursor.execute("""
                    ALTER TABLE "Users"
                    ADD COLUMN LastSyncTime TIMESTAMP WITH TIME ZONE
                """)
                logger.info("Added LastSyncTime column to Users table (PostgreSQL)")
        
        else:
            # Check if column already exists (MySQL)
            cursor.execute("""
                SELECT COUNT(*)
                FROM information_schema.columns
                WHERE table_name = 'Users' 
                AND column_name = 'LastSyncTime'
                AND table_schema = DATABASE()
            """)
            column_exists = cursor.fetchone()[0] > 0
            
            if not column_exists:
                cursor.execute("""
                    ALTER TABLE Users
                    ADD COLUMN LastSyncTime DATETIME
                """)
                logger.info("Added LastSyncTime column to Users table (MySQL)")
        
        logger.info("GPodder sync timestamp migration completed successfully")
        
    finally:
        cursor.close()


@register_migration("019", "fix_encryption_key_storage", "Convert EncryptionKey from binary to text format for consistency", requires=["001"])
def migration_019_fix_encryption_key_storage(conn, db_type: str):
    """Convert EncryptionKey storage from binary to text format"""
    cursor = conn.cursor()
    
    try:
        if db_type == "postgresql":
            # First, get the current encryption key value as bytes
            cursor.execute('SELECT encryptionkey FROM "AppSettings" WHERE appsettingsid = 1')
            result = cursor.fetchone()
            
            if result and result[0]:
                # Convert bytes to string
                key_bytes = result[0]
                if isinstance(key_bytes, bytes):
                    key_string = key_bytes.decode('utf-8')
                else:
                    key_string = str(key_bytes)
                
                # Drop and recreate column as TEXT
                cursor.execute('ALTER TABLE "AppSettings" DROP COLUMN encryptionkey')
                cursor.execute('ALTER TABLE "AppSettings" ADD COLUMN encryptionkey TEXT')
                
                # Insert the key back as text
                cursor.execute('UPDATE "AppSettings" SET encryptionkey = %s WHERE appsettingsid = 1', (key_string,))
                logger.info("Converted PostgreSQL encryptionkey from BYTEA to TEXT")
            else:
                # No existing key, just change the column type
                cursor.execute('ALTER TABLE "AppSettings" DROP COLUMN encryptionkey')
                cursor.execute('ALTER TABLE "AppSettings" ADD COLUMN encryptionkey TEXT')
                logger.info("Changed PostgreSQL encryptionkey column to TEXT (no existing data)")
        
        else:  # MySQL
            # First, get the current encryption key value
            cursor.execute('SELECT EncryptionKey FROM AppSettings WHERE AppSettingsID = 1')
            result = cursor.fetchone()
            
            if result and result[0]:
                # Convert binary to string
                key_data = result[0]
                if isinstance(key_data, bytes):
                    # Remove null padding and decode
                    key_string = key_data.rstrip(b'\x00').decode('utf-8')
                else:
                    key_string = str(key_data)
                
                # Change column type and update value
                cursor.execute('ALTER TABLE AppSettings MODIFY EncryptionKey VARCHAR(255)')
                cursor.execute('UPDATE AppSettings SET EncryptionKey = %s WHERE AppSettingsID = 1', (key_string,))
                logger.info("Converted MySQL EncryptionKey from BINARY to VARCHAR")
            else:
                # No existing key, just change the column type  
                cursor.execute('ALTER TABLE AppSettings MODIFY EncryptionKey VARCHAR(255)')
                logger.info("Changed MySQL EncryptionKey column to VARCHAR (no existing data)")
        
        logger.info("Encryption key storage migration completed successfully")
        
    except Exception as e:
        logger.error(f"Error in encryption key migration: {e}")
        raise
    finally:
        cursor.close()


@register_migration("020", "add_default_gpodder_device", "Add DefaultGpodderDevice column to Users table for tracking user's selected GPodder device", requires=["001"])
def migration_020_add_default_gpodder_device(conn, db_type: str):
    """Add DefaultGpodderDevice column to Users table"""
    cursor = conn.cursor()
    
    try:
        if db_type == "postgresql":
            # Add defaultgpodderdevice column to Users table
            safe_execute_sql(cursor, 'ALTER TABLE "Users" ADD COLUMN defaultgpodderdevice VARCHAR(255)')
            logger.info("Added defaultgpodderdevice column to Users table (PostgreSQL)")
        
        else:  # MySQL
            # Add DefaultGpodderDevice column to Users table
            safe_execute_sql(cursor, 'ALTER TABLE Users ADD COLUMN DefaultGpodderDevice VARCHAR(255)')
            logger.info("Added DefaultGpodderDevice column to Users table (MySQL)")
        
        logger.info("Default GPodder device column migration completed successfully")
        
    except Exception as e:
        logger.error(f"Error in default GPodder device migration: {e}")
        raise
    finally:
        cursor.close()


@register_migration("021", "limit_system_playlists_episodes", "Add MaxEpisodes limit to high-volume system playlists", requires=["010"])
def migration_021_limit_system_playlists_episodes(conn, db_type: str):
    """Add MaxEpisodes limit to Commuter Mix, Longform, and Weekend Marathon system playlists"""
    cursor = conn.cursor()
    
    try:
        logger.info("Starting system playlist episodes limit migration")
        
        # Define the playlists to update with 1000 episode limit
        playlists_to_update = ['Commuter Mix', 'Longform', 'Weekend Marathon']
        
        if db_type == "postgresql":
            for playlist_name in playlists_to_update:
                safe_execute_sql(cursor, '''
                    UPDATE "Playlists" 
                    SET maxepisodes = 1000 
                    WHERE name = %s AND issystemplaylist = TRUE
                ''', (playlist_name,))
                logger.info(f"Updated {playlist_name} system playlist with maxepisodes=1000 (PostgreSQL)")
        
        else:  # MySQL
            for playlist_name in playlists_to_update:
                safe_execute_sql(cursor, '''
                    UPDATE Playlists 
                    SET MaxEpisodes = 1000 
                    WHERE Name = %s AND IsSystemPlaylist = TRUE
                ''', (playlist_name,))
                logger.info(f"Updated {playlist_name} system playlist with MaxEpisodes=1000 (MySQL)")
        
        logger.info("System playlist episodes limit migration completed successfully")
        
    except Exception as e:
        logger.error(f"Error in system playlist episodes limit migration: {e}")
        raise
    finally:
        cursor.close()


@register_migration("022", "expand_downloaded_location_column", "Expand DownloadedLocation column size to handle long file paths", requires=["007"])
def migration_022_expand_downloaded_location_column(conn, db_type: str):
    """Expand DownloadedLocation column size to handle long file paths"""
    cursor = conn.cursor()
    
    try:
        logger.info("Starting downloaded location column expansion migration")
        
        if db_type == "postgresql":
            # Expand DownloadedLocation in DownloadedEpisodes table
            safe_execute_sql(cursor, '''
                ALTER TABLE "DownloadedEpisodes" 
                ALTER COLUMN downloadedlocation TYPE TEXT
            ''', conn=conn)
            logger.info("Expanded downloadedlocation column in DownloadedEpisodes table (PostgreSQL)")
            
            # Expand DownloadedLocation in DownloadedVideos table
            safe_execute_sql(cursor, '''
                ALTER TABLE "DownloadedVideos" 
                ALTER COLUMN downloadedlocation TYPE TEXT
            ''', conn=conn)
            logger.info("Expanded downloadedlocation column in DownloadedVideos table (PostgreSQL)")
        
        else:  # MySQL
            # Expand DownloadedLocation in DownloadedEpisodes table
            safe_execute_sql(cursor, '''
                ALTER TABLE DownloadedEpisodes 
                MODIFY DownloadedLocation TEXT
            ''', conn=conn)
            logger.info("Expanded DownloadedLocation column in DownloadedEpisodes table (MySQL)")
            
            # Expand DownloadedLocation in DownloadedVideos table
            safe_execute_sql(cursor, '''
                ALTER TABLE DownloadedVideos 
                MODIFY DownloadedLocation TEXT
            ''', conn=conn)
            logger.info("Expanded DownloadedLocation column in DownloadedVideos table (MySQL)")
        
        logger.info("Downloaded location column expansion migration completed successfully")
        
    except Exception as e:
        logger.error(f"Error in downloaded location column expansion migration: {e}")
        raise
    finally:
        cursor.close()


@register_migration("023", "add_missing_performance_indexes", "Add missing performance indexes for queue, saved, downloaded, and history tables", requires=["006", "007"])
def migration_023_add_missing_performance_indexes(conn, db_type: str):
    """Add missing performance indexes for queue, saved, downloaded, and history tables"""
    cursor = conn.cursor()
    
    try:
        logger.info("Starting missing performance indexes migration")
        
        table_prefix = '"' if db_type == 'postgresql' else ''
        table_suffix = '"' if db_type == 'postgresql' else ''
        
        # EpisodeQueue indexes (critical for get_queued_episodes performance)
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_episodequeue_userid ON {table_prefix}EpisodeQueue{table_suffix}(UserID)', 'idx_episodequeue_userid')
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_episodequeue_episodeid ON {table_prefix}EpisodeQueue{table_suffix}(EpisodeID)', 'idx_episodequeue_episodeid')
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_episodequeue_queueposition ON {table_prefix}EpisodeQueue{table_suffix}(QueuePosition)', 'idx_episodequeue_queueposition')
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_episodequeue_userid_queueposition ON {table_prefix}EpisodeQueue{table_suffix}(UserID, QueuePosition)', 'idx_episodequeue_userid_queueposition')
        
        # SavedEpisodes indexes (for return_episodes LEFT JOIN performance)
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_savedepisodes_userid ON {table_prefix}SavedEpisodes{table_suffix}(UserID)', 'idx_savedepisodes_userid')
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_savedepisodes_episodeid ON {table_prefix}SavedEpisodes{table_suffix}(EpisodeID)', 'idx_savedepisodes_episodeid')
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_savedepisodes_userid_episodeid ON {table_prefix}SavedEpisodes{table_suffix}(UserID, EpisodeID)', 'idx_savedepisodes_userid_episodeid')
        
        # SavedVideos indexes (for YouTube video queries)
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_savedvideos_userid ON {table_prefix}SavedVideos{table_suffix}(UserID)', 'idx_savedvideos_userid')
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_savedvideos_videoid ON {table_prefix}SavedVideos{table_suffix}(VideoID)', 'idx_savedvideos_videoid')
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_savedvideos_userid_videoid ON {table_prefix}SavedVideos{table_suffix}(UserID, VideoID)', 'idx_savedvideos_userid_videoid')
        
        # DownloadedEpisodes indexes (for return_episodes LEFT JOIN performance)
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_downloadedepisodes_userid ON {table_prefix}DownloadedEpisodes{table_suffix}(UserID)', 'idx_downloadedepisodes_userid')
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_downloadedepisodes_episodeid ON {table_prefix}DownloadedEpisodes{table_suffix}(EpisodeID)', 'idx_downloadedepisodes_episodeid')
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_downloadedepisodes_userid_episodeid ON {table_prefix}DownloadedEpisodes{table_suffix}(UserID, EpisodeID)', 'idx_downloadedepisodes_userid_episodeid')
        
        # DownloadedVideos indexes (for YouTube video queries)
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_downloadedvideos_userid ON {table_prefix}DownloadedVideos{table_suffix}(UserID)', 'idx_downloadedvideos_userid')
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_downloadedvideos_videoid ON {table_prefix}DownloadedVideos{table_suffix}(VideoID)', 'idx_downloadedvideos_videoid')
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_downloadedvideos_userid_videoid ON {table_prefix}DownloadedVideos{table_suffix}(UserID, VideoID)', 'idx_downloadedvideos_userid_videoid')
        
        # UserEpisodeHistory indexes (for return_episodes LEFT JOIN performance)
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_userepisodehistory_userid ON {table_prefix}UserEpisodeHistory{table_suffix}(UserID)', 'idx_userepisodehistory_userid')
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_userepisodehistory_episodeid ON {table_prefix}UserEpisodeHistory{table_suffix}(EpisodeID)', 'idx_userepisodehistory_episodeid')
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_userepisodehistory_userid_episodeid ON {table_prefix}UserEpisodeHistory{table_suffix}(UserID, EpisodeID)', 'idx_userepisodehistory_userid_episodeid')
        
        # UserVideoHistory indexes (for YouTube video queries)
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_uservideohistory_userid ON {table_prefix}UserVideoHistory{table_suffix}(UserID)', 'idx_uservideohistory_userid')
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_uservideohistory_videoid ON {table_prefix}UserVideoHistory{table_suffix}(VideoID)', 'idx_uservideohistory_videoid')
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_uservideohistory_userid_videoid ON {table_prefix}UserVideoHistory{table_suffix}(UserID, VideoID)', 'idx_uservideohistory_userid_videoid')
        
        # Additional useful indexes for query performance
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_episodes_completed ON {table_prefix}Episodes{table_suffix}(Completed)', 'idx_episodes_completed')
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_youtubevideos_completed ON {table_prefix}YouTubeVideos{table_suffix}(Completed)', 'idx_youtubevideos_completed')
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_youtubevideos_podcastid ON {table_prefix}YouTubeVideos{table_suffix}(PodcastID)', 'idx_youtubevideos_podcastid')
        safe_add_index(cursor, db_type, f'CREATE INDEX idx_youtubevideos_publishedat ON {table_prefix}YouTubeVideos{table_suffix}(PublishedAt)', 'idx_youtubevideos_publishedat')
        
        logger.info("Missing performance indexes migration completed successfully")
        
    except Exception as e:
        logger.error(f"Error in missing performance indexes migration: {e}")
        raise
    finally:
        cursor.close()


@register_migration("025", "fix_people_table_columns", "Add missing PersonImg, PeopleDBID, and AssociatedPodcasts columns to existing People tables", requires=["009"])
def migration_025_fix_people_table_columns(conn, db_type: str):
    """Add missing columns to existing People tables for users who upgraded from older versions"""
    cursor = conn.cursor()
    
    try:
        logger.info("Starting People table columns fix migration")
        
        if db_type == "postgresql":
            # Check if PersonImg column exists, if not add it
            safe_execute_sql(cursor, '''
                DO $$ 
                BEGIN 
                    IF NOT EXISTS (
                        SELECT 1 FROM information_schema.columns 
                        WHERE table_name = 'People' AND column_name = 'personimg'
                    ) THEN
                        ALTER TABLE "People" ADD COLUMN PersonImg TEXT;
                    END IF;
                END $$;
            ''', conn=conn)
            
            # Check if PeopleDBID column exists, if not add it
            safe_execute_sql(cursor, '''
                DO $$ 
                BEGIN 
                    IF NOT EXISTS (
                        SELECT 1 FROM information_schema.columns 
                        WHERE table_name = 'People' AND column_name = 'peopledbid'
                    ) THEN
                        ALTER TABLE "People" ADD COLUMN PeopleDBID INT;
                    END IF;
                END $$;
            ''', conn=conn)
            
            # Check if AssociatedPodcasts column exists, if not add it
            safe_execute_sql(cursor, '''
                DO $$ 
                BEGIN 
                    IF NOT EXISTS (
                        SELECT 1 FROM information_schema.columns 
                        WHERE table_name = 'People' AND column_name = 'associatedpodcasts'
                    ) THEN
                        ALTER TABLE "People" ADD COLUMN AssociatedPodcasts TEXT;
                    END IF;
                END $$;
            ''', conn=conn)
            
            logger.info("Added missing columns to People table (PostgreSQL)")
        
        else:  # MySQL
            # For MySQL, use IF NOT EXISTS syntax or try-catch approach
            try:
                safe_execute_sql(cursor, 'ALTER TABLE People ADD COLUMN PersonImg TEXT', conn=conn)
                logger.info("Added PersonImg column to People table (MySQL)")
            except Exception:
                logger.debug("PersonImg column already exists in People table (MySQL)")
            
            try:
                safe_execute_sql(cursor, 'ALTER TABLE People ADD COLUMN PeopleDBID INT', conn=conn)
                logger.info("Added PeopleDBID column to People table (MySQL)")
            except Exception:
                logger.debug("PeopleDBID column already exists in People table (MySQL)")
                
            try:
                safe_execute_sql(cursor, 'ALTER TABLE People ADD COLUMN AssociatedPodcasts TEXT', conn=conn)
                logger.info("Added AssociatedPodcasts column to People table (MySQL)")
            except Exception:
                logger.debug("AssociatedPodcasts column already exists in People table (MySQL)")
        
        logger.info("People table columns fix migration completed successfully")
        
    except Exception as e:
        logger.error(f"Error in People table columns fix migration: {e}")
        raise
    finally:
        cursor.close()


@register_migration("026", "limit_quick_listens_episodes", "Add MaxEpisodes limit to Quick Listens system playlist", requires=["012"])
def migration_026_limit_quick_listens_episodes(conn, db_type: str):
    """Add MaxEpisodes limit to Quick Listens system playlist"""
    cursor = conn.cursor()
    
    try:
        logger.info("Starting Quick Listens MaxEpisodes limit migration")
        
        if db_type == "postgresql":
            # Update Quick Listens playlist to have maxepisodes = 1000
            safe_execute_sql(cursor, '''
                UPDATE "Playlists" 
                SET maxepisodes = 1000 
                WHERE name = 'Quick Listens' AND issystemplaylist = TRUE
            ''', conn=conn)
            logger.info("Updated Quick Listens system playlist maxepisodes=1000 (PostgreSQL)")
        
        else:  # MySQL
            # Update Quick Listens playlist to have MaxEpisodes = 1000
            safe_execute_sql(cursor, '''
                UPDATE Playlists 
                SET MaxEpisodes = 1000 
                WHERE Name = 'Quick Listens' AND IsSystemPlaylist = TRUE
            ''', conn=conn)
            logger.info("Updated Quick Listens system playlist MaxEpisodes=1000 (MySQL)")
        
        logger.info("Quick Listens MaxEpisodes limit migration completed successfully")
        
    except Exception as e:
        logger.error(f"Error in Quick Listens MaxEpisodes limit migration: {e}")
        raise
    finally:
        cursor.close()


def register_all_migrations():
    """Register all migrations with the migration manager"""
    # Migrations are auto-registered via decorators
    logger.info("All migrations registered")


@register_migration("024", "fix_quick_listens_min_duration", "Update Quick Listens playlist to exclude 0-duration episodes", requires=["012"])
def migration_024_fix_quick_listens_min_duration(conn, db_type: str):
    """Update Quick Listens system playlist to exclude episodes with 0 duration"""
    cursor = conn.cursor()
    
    try:
        logger.info("Starting Quick Listens min duration fix migration")
        
        if db_type == "postgresql":
            # Update Quick Listens playlist to have min_duration = 1 second
            safe_execute_sql(cursor, '''
                UPDATE "Playlists" 
                SET minduration = 1 
                WHERE name = 'Quick Listens' AND issystemplaylist = TRUE
            ''', conn=conn)
            logger.info("Updated Quick Listens system playlist minduration=1 (PostgreSQL)")
        
        else:  # MySQL
            # Update Quick Listens playlist to have MinDuration = 1 second
            safe_execute_sql(cursor, '''
                UPDATE Playlists 
                SET MinDuration = 1 
                WHERE Name = 'Quick Listens' AND IsSystemPlaylist = TRUE
            ''', conn=conn)
            logger.info("Updated Quick Listens system playlist MinDuration=1 (MySQL)")
        
        logger.info("Quick Listens min duration fix migration completed successfully")
        
    except Exception as e:
        logger.error(f"Error in Quick Listens min duration fix migration: {e}")
        raise
    finally:
        cursor.close()


@register_migration("027", "add_scheduled_backups_table", "Create ScheduledBackups table for automated backup management", requires=["026"])
def migration_027_add_scheduled_backups_table(conn, db_type: str):
    """Create ScheduledBackups table for automated backup management"""
    cursor = conn.cursor()
    
    try:
        logger.info("Starting ScheduledBackups table creation migration")
        
        if db_type == "postgresql":
            # Create ScheduledBackups table for PostgreSQL
            safe_execute_sql(cursor, '''
                CREATE TABLE IF NOT EXISTS "ScheduledBackups" (
                    id SERIAL PRIMARY KEY,
                    userid INTEGER NOT NULL,
                    cron_schedule VARCHAR(50) NOT NULL,
                    enabled BOOLEAN NOT NULL DEFAULT false,
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    UNIQUE(userid),
                    FOREIGN KEY (userid) REFERENCES "Users"(userid) ON DELETE CASCADE
                )
            ''', conn=conn)
            logger.info("Created ScheduledBackups table (PostgreSQL)")
            
            # Create index for performance
            safe_execute_sql(cursor, '''
                CREATE INDEX IF NOT EXISTS idx_scheduled_backups_enabled 
                ON "ScheduledBackups"(enabled)
            ''', conn=conn)
            logger.info("Created index on enabled column (PostgreSQL)")
        
        else:  # MySQL
            # Create ScheduledBackups table for MySQL
            safe_execute_sql(cursor, '''
                CREATE TABLE IF NOT EXISTS ScheduledBackups (
                    ID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT NOT NULL,
                    CronSchedule VARCHAR(50) NOT NULL,
                    Enabled BOOLEAN NOT NULL DEFAULT FALSE,
                    CreatedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    UpdatedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                    UNIQUE KEY unique_user (UserID),
                    FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE
                )
            ''', conn=conn)
            logger.info("Created ScheduledBackups table (MySQL)")
            
            # Create index for performance
            safe_execute_sql(cursor, '''
                CREATE INDEX idx_scheduled_backups_enabled 
                ON ScheduledBackups(Enabled)
            ''', conn=conn)
            logger.info("Created index on Enabled column (MySQL)")
        
        logger.info("ScheduledBackups table creation migration completed successfully")
        
    except Exception as e:
        logger.error(f"Error in ScheduledBackups table creation migration: {e}")
        raise
    finally:
        cursor.close()


@register_migration("028", "add_ignore_podcast_index_column", "Add IgnorePodcastIndex column to Podcasts table", requires=["027"])
def migration_028_add_ignore_podcast_index_column(conn, db_type: str):
    """
    Migration 028: Add IgnorePodcastIndex column to Podcasts table
    """
    logger.info("Starting migration 028: Add IgnorePodcastIndex column to Podcasts table")
    cursor = conn.cursor()
    
    try:
        if db_type == 'postgresql':
            safe_execute_sql(cursor, '''
                ALTER TABLE "Podcasts" 
                ADD COLUMN IF NOT EXISTS IgnorePodcastIndex BOOLEAN DEFAULT FALSE
            ''', conn=conn)
            logger.info("Added IgnorePodcastIndex column to Podcasts table (PostgreSQL)")
        
        else:  # MySQL
            # Check if column already exists to avoid duplicate column error
            safe_execute_sql(cursor, '''
                SELECT COUNT(*) 
                FROM information_schema.columns 
                WHERE table_name = 'Podcasts' 
                AND column_name = 'IgnorePodcastIndex' 
                AND table_schema = DATABASE()
            ''', conn=conn)
            
            result = cursor.fetchone()
            if result[0] == 0:  # Column doesn't exist
                safe_execute_sql(cursor, '''
                    ALTER TABLE Podcasts 
                    ADD COLUMN IgnorePodcastIndex TINYINT(1) DEFAULT 0
                ''', conn=conn)
                logger.info("Added IgnorePodcastIndex column to Podcasts table (MySQL)")
            else:
                logger.info("IgnorePodcastIndex column already exists in Podcasts table (MySQL)")
        
        logger.info("IgnorePodcastIndex column migration completed successfully")
        
    except Exception as e:
        logger.error(f"Error in IgnorePodcastIndex column migration: {e}")
        raise
    finally:
        cursor.close()


@register_migration("029", "fix_people_episodes_table_schema", "Fix PeopleEpisodes table schema to match expected format", requires=["009"])
def migration_029_fix_people_episodes_table_schema(conn, db_type: str):
    """
    Migration 029: Fix PeopleEpisodes table schema
    
    This migration ensures the PeopleEpisodes table has the correct schema with all required columns.
    Some databases may have an incomplete PeopleEpisodes table from migration 009.
    """
    logger.info("Starting migration 029: Fix PeopleEpisodes table schema")
    cursor = conn.cursor()
    
    try:
        if db_type == 'postgresql':
            # For PostgreSQL, we'll recreate the table with the correct schema
            # First check if table exists and get its current structure
            safe_execute_sql(cursor, '''
                SELECT column_name 
                FROM information_schema.columns 
                WHERE table_name = 'PeopleEpisodes' 
                AND table_schema = current_schema()
            ''', conn=conn)
            
            existing_columns = [row[0] for row in cursor.fetchall()]
            
            if 'podcastid' not in [col.lower() for col in existing_columns]:
                logger.info("PeopleEpisodes table missing required columns, recreating...")
                
                # Drop existing table if it exists with wrong schema
                safe_execute_sql(cursor, 'DROP TABLE IF EXISTS "PeopleEpisodes"', conn=conn)
                
                # Create with correct schema
                safe_execute_sql(cursor, '''
                    CREATE TABLE "PeopleEpisodes" (
                        EpisodeID SERIAL PRIMARY KEY,
                        PersonID INT,
                        PodcastID INT,
                        EpisodeTitle TEXT,
                        EpisodeDescription TEXT,
                        EpisodeURL TEXT,
                        EpisodeArtwork TEXT,
                        EpisodePubDate TIMESTAMP,
                        EpisodeDuration INT,
                        AddedDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                        FOREIGN KEY (PersonID) REFERENCES "People"(PersonID),
                        FOREIGN KEY (PodcastID) REFERENCES "Podcasts"(PodcastID)
                    )
                ''', conn=conn)
                logger.info("Recreated PeopleEpisodes table with correct schema (PostgreSQL)")
            else:
                logger.info("PeopleEpisodes table already has correct schema (PostgreSQL)")
        
        else:  # MySQL
            # For MySQL, check current table structure
            safe_execute_sql(cursor, '''
                SELECT column_name 
                FROM information_schema.columns 
                WHERE table_name = 'PeopleEpisodes' 
                AND table_schema = DATABASE()
            ''', conn=conn)
            
            existing_columns = [row[0] for row in cursor.fetchall()]
            logger.info(f"Current PeopleEpisodes columns: {existing_columns}")
            
            if 'PodcastID' not in existing_columns:
                logger.info("PeopleEpisodes table missing required columns, recreating...")
                
                # Backup any existing data first (if the table has useful data)
                safe_execute_sql(cursor, '''
                    CREATE TABLE IF NOT EXISTS PeopleEpisodes_backup AS 
                    SELECT * FROM PeopleEpisodes
                ''', conn=conn)
                logger.info("Created backup of existing PeopleEpisodes table")
                
                # Drop existing table
                safe_execute_sql(cursor, 'DROP TABLE IF EXISTS PeopleEpisodes', conn=conn)
                
                # Create with correct schema
                safe_execute_sql(cursor, '''
                    CREATE TABLE PeopleEpisodes (
                        EpisodeID INT AUTO_INCREMENT PRIMARY KEY,
                        PersonID INT,
                        PodcastID INT,
                        EpisodeTitle TEXT,
                        EpisodeDescription TEXT,
                        EpisodeURL TEXT,
                        EpisodeArtwork TEXT,
                        EpisodePubDate TIMESTAMP,
                        EpisodeDuration INT,
                        AddedDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                        FOREIGN KEY (PersonID) REFERENCES People(PersonID),
                        FOREIGN KEY (PodcastID) REFERENCES Podcasts(PodcastID)
                    )
                ''', conn=conn)
                logger.info("Recreated PeopleEpisodes table with correct schema (MySQL)")
            else:
                logger.info("PeopleEpisodes table already has correct schema (MySQL)")
        
        logger.info("PeopleEpisodes table schema fix completed successfully")
        
    except Exception as e:
        logger.error(f"Error in PeopleEpisodes table schema fix migration: {e}")
        raise
    finally:
        cursor.close()


@register_migration("030", "add_user_language_preference", "Add Language column to Users table for user-specific language preferences", requires=["001"])
def migration_030_add_user_language_preference(conn, db_type: str):
    """Add Language column to Users table for user-specific language preferences"""
    cursor = conn.cursor()
    
    try:
        # Get the default language from environment variable, fallback to 'en'
        default_language = os.environ.get("DEFAULT_LANGUAGE", "en")
        
        # Validate language code (basic validation)
        if not default_language or len(default_language) > 10:
            default_language = "en"
            
        logger.info(f"Adding Language column to Users table with default '{default_language}'")
        
        if db_type == 'postgresql':
            # Add Language column with default from environment variable
            safe_execute_sql(cursor, f'''
                ALTER TABLE "Users" 
                ADD COLUMN IF NOT EXISTS Language VARCHAR(10) DEFAULT '{default_language}'
            ''', conn=conn)
            
            # Add comment to document the column
            safe_execute_sql(cursor, '''
                COMMENT ON COLUMN "Users".Language IS 'ISO 639-1 language code for user interface language preference'
            ''', conn=conn)
            
        else:  # mysql/mariadb
            # Check if column exists first
            cursor.execute("""
                SELECT COUNT(*) 
                FROM INFORMATION_SCHEMA.COLUMNS 
                WHERE TABLE_SCHEMA = DATABASE() 
                AND TABLE_NAME = 'Users' 
                AND COLUMN_NAME = 'Language'
            """)
            
            if cursor.fetchone()[0] == 0:
                safe_execute_sql(cursor, f'''
                    ALTER TABLE Users 
                    ADD COLUMN Language VARCHAR(10) DEFAULT '{default_language}' 
                    COMMENT 'ISO 639-1 language code for user interface language preference'
                ''', conn=conn)
        
        logger.info(f"Successfully added Language column to Users table with default '{default_language}'")

    except Exception as e:
        logger.error(f"Error in migration 030: {e}")
        raise
    finally:
        cursor.close()


@register_migration("031", "add_oidc_env_initialized_column", "Add InitializedFromEnv column to OIDCProviders table to track env-initialized providers", requires=["001"])
def migration_031_add_oidc_env_initialized_column(conn, db_type: str):
    """Add InitializedFromEnv column to OIDCProviders table to track providers created from environment variables"""
    cursor = conn.cursor()
    
    try:
        logger.info("Adding InitializedFromEnv column to OIDCProviders table")
        
        if db_type == 'postgresql':
            # Add InitializedFromEnv column (defaults to false for existing providers)
            safe_execute_sql(cursor, '''
                ALTER TABLE "OIDCProviders" 
                ADD COLUMN IF NOT EXISTS InitializedFromEnv BOOLEAN DEFAULT false
            ''', conn=conn)
            
            # Add comment to document the column
            safe_execute_sql(cursor, '''
                COMMENT ON COLUMN "OIDCProviders".InitializedFromEnv IS 'Indicates if this provider was created from environment variables and should not be removable via UI'
            ''', conn=conn)
            
        else:  # mysql/mariadb
            # Check if column exists first
            cursor.execute("""
                SELECT COUNT(*) 
                FROM INFORMATION_SCHEMA.COLUMNS 
                WHERE TABLE_SCHEMA = DATABASE() 
                AND TABLE_NAME = 'OIDCProviders' 
                AND COLUMN_NAME = 'InitializedFromEnv'
            """)
            
            if cursor.fetchone()[0] == 0:
                safe_execute_sql(cursor, '''
                    ALTER TABLE OIDCProviders 
                    ADD COLUMN InitializedFromEnv TINYINT(1) DEFAULT 0 
                    COMMENT 'Indicates if this provider was created from environment variables and should not be removable via UI'
                ''', conn=conn)
        
        logger.info("Successfully added InitializedFromEnv column to OIDCProviders table")
    except Exception as e:
        logger.error(f"Error in migration 031: {e}")
        raise
    finally:
        cursor.close()


@register_migration("032", "create_user_default_playlists", "Create default playlists for all existing users", requires=["012"])
def migration_032_create_user_default_playlists(conn, db_type: str):
    """Create default playlists for all existing users, eliminating system playlists"""
    cursor = conn.cursor()
    
    try:
        logger.info("Starting user default playlists migration")
        
        # First, add the episode_count column to Playlists table if it doesn't exist
        if db_type == "postgresql":
            # Check if episode_count column exists
            cursor.execute("""
                SELECT column_name FROM information_schema.columns 
                WHERE table_name = 'Playlists' 
                AND column_name = 'episodecount'
            """)
            column_exists = len(cursor.fetchall()) > 0
            
            if not column_exists:
                cursor.execute("""
                    ALTER TABLE "Playlists"
                    ADD COLUMN episodecount INTEGER DEFAULT 0
                """)
                logger.info("Added episode_count column to Playlists table (PostgreSQL)")
            else:
                logger.info("episode_count column already exists in Playlists table (PostgreSQL)")
        else:
            # Check if episode_count column exists (MySQL)
            cursor.execute("""
                SELECT COUNT(*)
                FROM INFORMATION_SCHEMA.COLUMNS
                WHERE TABLE_NAME = 'Playlists' 
                AND COLUMN_NAME = 'EpisodeCount'
                AND TABLE_SCHEMA = DATABASE()
            """)
            column_exists = cursor.fetchone()[0] > 0
            
            if not column_exists:
                cursor.execute("""
                    ALTER TABLE Playlists
                    ADD COLUMN EpisodeCount INT DEFAULT 0
                """)
                logger.info("Added EpisodeCount column to Playlists table (MySQL)")
            else:
                logger.info("EpisodeCount column already exists in Playlists table (MySQL)")
        
        # Define default playlists (same as migration 012 but will be assigned to each user)
        default_playlists = [
            {
                'name': 'Quick Listens',
                'description': 'Short episodes under 15 minutes, perfect for quick breaks',
                'min_duration': 1,  # Exclude 0-duration episodes
                'max_duration': 900,  # 15 minutes
                'sort_order': 'duration_asc',
                'icon_name': 'ph-fast-forward',
                'max_episodes': 1000
            },
            {
                'name': 'Longform',
                'description': 'Extended episodes over 1 hour, ideal for long drives or deep dives',
                'min_duration': 3600,  # 1 hour
                'max_duration': None,
                'sort_order': 'duration_desc',
                'icon_name': 'ph-car',
                'max_episodes': 1000
            },
            {
                'name': 'Currently Listening',
                'description': 'Episodes you\'ve started but haven\'t finished',
                'min_duration': None,
                'max_duration': None,
                'sort_order': 'date_desc',
                'include_unplayed': False,
                'include_partially_played': True,
                'include_played': False,
                'icon_name': 'ph-play'
            },
            {
                'name': 'Fresh Releases',
                'description': 'Latest episodes from the last 24 hours',
                'min_duration': None,
                'max_duration': None,
                'sort_order': 'date_desc',
                'include_unplayed': True,
                'include_partially_played': False,
                'include_played': False,
                'time_filter_hours': 24,
                'icon_name': 'ph-sparkle'
            },
            {
                'name': 'Weekend Marathon',
                'description': 'Longer episodes (30+ minutes) perfect for weekend listening',
                'min_duration': 1800,  # 30 minutes
                'max_duration': None,
                'sort_order': 'duration_desc',
                'group_by_podcast': True,
                'icon_name': 'ph-couch',
                'max_episodes': 1000
            },
            {
                'name': 'Commuter Mix',
                'description': 'Perfect-length episodes (15-45 minutes) for your daily commute',
                'min_duration': 900,   # 15 minutes
                'max_duration': 2700,  # 45 minutes
                'sort_order': 'date_desc',
                'icon_name': 'ph-car-simple',
                'max_episodes': 1000
            }
        ]
        
        # Get all existing users (excluding background user if present)
        if db_type == "postgresql":
            cursor.execute('SELECT userid FROM "Users" WHERE userid > 1')
        else:
            cursor.execute('SELECT UserID FROM Users WHERE UserID > 1')
        
        users = cursor.fetchall()
        logger.info(f"Found {len(users)} users to create default playlists for")
        
        # Create default playlists for each user
        for user_row in users:
            user_id = user_row[0] if isinstance(user_row, tuple) else user_row['userid' if db_type == "postgresql" else 'UserID']
            logger.info(f"Creating default playlists for user {user_id}")
            
            for playlist in default_playlists:
                try:
                    # Check if this playlist already exists for this user
                    if db_type == "postgresql":
                        cursor.execute("""
                            SELECT COUNT(*)
                            FROM "Playlists"
                            WHERE userid = %s AND name = %s
                        """, (user_id, playlist['name']))
                    else:
                        cursor.execute("""
                            SELECT COUNT(*)
                            FROM Playlists
                            WHERE UserID = %s AND Name = %s
                        """, (user_id, playlist['name']))
                    
                    if cursor.fetchone()[0] == 0:
                        # Create the playlist for this user
                        if db_type == "postgresql":
                            cursor.execute("""
                                INSERT INTO "Playlists" (
                                    userid,
                                    name,
                                    description,
                                    issystemplaylist,
                                    minduration,
                                    maxduration,
                                    sortorder,
                                    includeunplayed,
                                    includepartiallyplayed,
                                    includeplayed,
                                    timefilterhours,
                                    groupbypodcast,
                                    maxepisodes,
                                    iconname,
                                    episodecount
                                ) VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
                            """, (
                                user_id,
                                playlist['name'],
                                playlist['description'],
                                False,  # No longer system playlists
                                playlist.get('min_duration'),
                                playlist.get('max_duration'),
                                playlist['sort_order'],
                                playlist.get('include_unplayed', True),
                                playlist.get('include_partially_played', True),
                                playlist.get('include_played', True),
                                playlist.get('time_filter_hours'),
                                playlist.get('group_by_podcast', False),
                                playlist.get('max_episodes'),
                                playlist['icon_name'],
                                0  # Will be updated by scheduled count update
                            ))
                        else:
                            cursor.execute("""
                                INSERT INTO Playlists (
                                    UserID,
                                    Name,
                                    Description,
                                    IsSystemPlaylist,
                                    MinDuration,
                                    MaxDuration,
                                    SortOrder,
                                    IncludeUnplayed,
                                    IncludePartiallyPlayed,
                                    IncludePlayed,
                                    TimeFilterHours,
                                    GroupByPodcast,
                                    MaxEpisodes,
                                    IconName,
                                    EpisodeCount
                                ) VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
                            """, (
                                user_id,
                                playlist['name'],
                                playlist['description'],
                                False,  # No longer system playlists
                                playlist.get('min_duration'),
                                playlist.get('max_duration'),
                                playlist['sort_order'],
                                playlist.get('include_unplayed', True),
                                playlist.get('include_partially_played', True),
                                playlist.get('include_played', True),
                                playlist.get('time_filter_hours'),
                                playlist.get('group_by_podcast', False),
                                playlist.get('max_episodes'),
                                playlist['icon_name'],
                                0  # Will be updated by scheduled count update
                            ))
                        
                        logger.info(f"Created playlist '{playlist['name']}' for user {user_id}")
                    else:
                        logger.info(f"Playlist '{playlist['name']}' already exists for user {user_id}")
                        
                except Exception as e:
                    logger.error(f"Failed to create playlist '{playlist['name']}' for user {user_id}: {e}")
                    # Continue with other playlists even if one fails
        
        # Commit all changes
        conn.commit()
        logger.info("Successfully created default playlists for all existing users")
        
    except Exception as e:
        logger.error(f"Error in user default playlists migration: {e}")
        raise
    finally:
        cursor.close()


# ============================================================================
# GPODDER SYNC MIGRATIONS
# These migrations match the gpodder-api service migrations from Go code
# ============================================================================

@register_migration("100", "gpodder_initial_schema", "Create initial gpodder sync tables")
def migration_100_gpodder_initial_schema(conn, db_type: str):
    """Create initial gpodder sync schema - matches Go migration version 1"""
    cursor = conn.cursor()
    
    try:
        logger.info("Starting gpodder migration 100: Initial schema creation")
        
        if db_type == 'postgresql':
            # Create all gpodder sync tables for PostgreSQL
            tables_sql = [
                '''
                CREATE TABLE IF NOT EXISTS "GpodderSyncMigrations" (
                    Version INT PRIMARY KEY,
                    Description TEXT NOT NULL,
                    AppliedAt TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
                )
                ''',
                '''
                CREATE TABLE IF NOT EXISTS "GpodderSyncDeviceState" (
                    DeviceStateID SERIAL PRIMARY KEY,
                    UserID INT NOT NULL,
                    DeviceID INT NOT NULL,
                    SubscriptionCount INT DEFAULT 0,
                    LastUpdated TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE,
                    FOREIGN KEY (DeviceID) REFERENCES "GpodderDevices"(DeviceID) ON DELETE CASCADE,
                    UNIQUE(UserID, DeviceID)
                )
                ''',
                '''
                CREATE TABLE IF NOT EXISTS "GpodderSyncSubscriptions" (
                    SubscriptionID SERIAL PRIMARY KEY,
                    UserID INT NOT NULL,
                    DeviceID INT NOT NULL,
                    PodcastURL TEXT NOT NULL,
                    Action VARCHAR(10) NOT NULL,
                    Timestamp BIGINT NOT NULL,
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE,
                    FOREIGN KEY (DeviceID) REFERENCES "GpodderDevices"(DeviceID) ON DELETE CASCADE
                )
                ''',
                '''
                CREATE TABLE IF NOT EXISTS "GpodderSyncEpisodeActions" (
                    ActionID SERIAL PRIMARY KEY,
                    UserID INT NOT NULL,
                    DeviceID INT,
                    PodcastURL TEXT NOT NULL,
                    EpisodeURL TEXT NOT NULL,
                    Action VARCHAR(20) NOT NULL,
                    Timestamp BIGINT NOT NULL,
                    Started INT,
                    Position INT,
                    Total INT,
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE,
                    FOREIGN KEY (DeviceID) REFERENCES "GpodderDevices"(DeviceID) ON DELETE CASCADE
                )
                ''',
                '''
                CREATE TABLE IF NOT EXISTS "GpodderSyncPodcastLists" (
                    ListID SERIAL PRIMARY KEY,
                    UserID INT NOT NULL,
                    Name VARCHAR(255) NOT NULL,
                    Title VARCHAR(255) NOT NULL,
                    CreatedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE,
                    UNIQUE(UserID, Name)
                )
                ''',
                '''
                CREATE TABLE IF NOT EXISTS "GpodderSyncPodcastListEntries" (
                    EntryID SERIAL PRIMARY KEY,
                    ListID INT NOT NULL,
                    PodcastURL TEXT NOT NULL,
                    CreatedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (ListID) REFERENCES "GpodderSyncPodcastLists"(ListID) ON DELETE CASCADE
                )
                ''',
                '''
                CREATE TABLE IF NOT EXISTS "GpodderSyncDevicePairs" (
                    PairID SERIAL PRIMARY KEY,
                    UserID INT NOT NULL,
                    DeviceID1 INT NOT NULL,
                    DeviceID2 INT NOT NULL,
                    CreatedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE,
                    FOREIGN KEY (DeviceID1) REFERENCES "GpodderDevices"(DeviceID) ON DELETE CASCADE,
                    FOREIGN KEY (DeviceID2) REFERENCES "GpodderDevices"(DeviceID) ON DELETE CASCADE,
                    UNIQUE(UserID, DeviceID1, DeviceID2)
                )
                ''',
                '''
                CREATE TABLE IF NOT EXISTS "GpodderSyncSettings" (
                    SettingID SERIAL PRIMARY KEY,
                    UserID INT NOT NULL,
                    Scope VARCHAR(20) NOT NULL,
                    DeviceID INT,
                    PodcastURL TEXT,
                    EpisodeURL TEXT,
                    SettingKey VARCHAR(255) NOT NULL,
                    SettingValue TEXT,
                    CreatedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    LastUpdated TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE,
                    FOREIGN KEY (DeviceID) REFERENCES "GpodderDevices"(DeviceID) ON DELETE CASCADE
                )
                '''
            ]
            
            # Create indexes
            indexes_sql = [
                'CREATE INDEX IF NOT EXISTS idx_gpodder_sync_subscriptions_userid ON "GpodderSyncSubscriptions"(UserID)',
                'CREATE INDEX IF NOT EXISTS idx_gpodder_sync_subscriptions_deviceid ON "GpodderSyncSubscriptions"(DeviceID)',
                'CREATE INDEX IF NOT EXISTS idx_gpodder_sync_episode_actions_userid ON "GpodderSyncEpisodeActions"(UserID)',
                'CREATE INDEX IF NOT EXISTS idx_gpodder_sync_podcast_lists_userid ON "GpodderSyncPodcastLists"(UserID)'
            ]
            
        else:  # mysql
            # Create all gpodder sync tables for MySQL
            tables_sql = [
                '''
                CREATE TABLE IF NOT EXISTS GpodderSyncMigrations (
                    Version INT PRIMARY KEY,
                    Description TEXT NOT NULL,
                    AppliedAt TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
                )
                ''',
                '''
                CREATE TABLE IF NOT EXISTS GpodderSyncDeviceState (
                    DeviceStateID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT NOT NULL,
                    DeviceID INT NOT NULL,
                    SubscriptionCount INT DEFAULT 0,
                    LastUpdated TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE,
                    FOREIGN KEY (DeviceID) REFERENCES GpodderDevices(DeviceID) ON DELETE CASCADE,
                    UNIQUE(UserID, DeviceID)
                )
                ''',
                '''
                CREATE TABLE IF NOT EXISTS GpodderSyncSubscriptions (
                    SubscriptionID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT NOT NULL,
                    DeviceID INT NOT NULL,
                    PodcastURL TEXT NOT NULL,
                    Action VARCHAR(10) NOT NULL,
                    Timestamp BIGINT NOT NULL,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE,
                    FOREIGN KEY (DeviceID) REFERENCES GpodderDevices(DeviceID) ON DELETE CASCADE
                )
                ''',
                '''
                CREATE TABLE IF NOT EXISTS GpodderSyncEpisodeActions (
                    ActionID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT NOT NULL,
                    DeviceID INT,
                    PodcastURL TEXT NOT NULL,
                    EpisodeURL TEXT NOT NULL,
                    Action VARCHAR(20) NOT NULL,
                    Timestamp BIGINT NOT NULL,
                    Started INT,
                    Position INT,
                    Total INT,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE,
                    FOREIGN KEY (DeviceID) REFERENCES GpodderDevices(DeviceID) ON DELETE CASCADE
                )
                ''',
                '''
                CREATE TABLE IF NOT EXISTS GpodderSyncPodcastLists (
                    ListID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT NOT NULL,
                    Name VARCHAR(255) NOT NULL,
                    Title VARCHAR(255) NOT NULL,
                    CreatedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE,
                    UNIQUE(UserID, Name)
                )
                ''',
                '''
                CREATE TABLE IF NOT EXISTS GpodderSyncPodcastListEntries (
                    EntryID INT AUTO_INCREMENT PRIMARY KEY,
                    ListID INT NOT NULL,
                    PodcastURL TEXT NOT NULL,
                    CreatedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (ListID) REFERENCES GpodderSyncPodcastLists(ListID) ON DELETE CASCADE
                )
                ''',
                '''
                CREATE TABLE IF NOT EXISTS GpodderSyncDevicePairs (
                    PairID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT NOT NULL,
                    DeviceID1 INT NOT NULL,
                    DeviceID2 INT NOT NULL,
                    CreatedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE,
                    FOREIGN KEY (DeviceID1) REFERENCES GpodderDevices(DeviceID) ON DELETE CASCADE,
                    FOREIGN KEY (DeviceID2) REFERENCES GpodderDevices(DeviceID) ON DELETE CASCADE,
                    UNIQUE(UserID, DeviceID1, DeviceID2)
                )
                ''',
                '''
                CREATE TABLE IF NOT EXISTS GpodderSyncSettings (
                    SettingID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT NOT NULL,
                    Scope VARCHAR(20) NOT NULL,
                    DeviceID INT,
                    PodcastURL TEXT,
                    EpisodeURL TEXT,
                    SettingKey VARCHAR(255) NOT NULL,
                    SettingValue TEXT,
                    CreatedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    LastUpdated TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE,
                    FOREIGN KEY (DeviceID) REFERENCES GpodderDevices(DeviceID) ON DELETE CASCADE
                )
                '''
            ]
            
            # Create indexes
            indexes_sql = [
                'CREATE INDEX idx_gpodder_sync_subscriptions_userid ON GpodderSyncSubscriptions(UserID)',
                'CREATE INDEX idx_gpodder_sync_subscriptions_deviceid ON GpodderSyncSubscriptions(DeviceID)',
                'CREATE INDEX idx_gpodder_sync_episode_actions_userid ON GpodderSyncEpisodeActions(UserID)',
                'CREATE INDEX idx_gpodder_sync_podcast_lists_userid ON GpodderSyncPodcastLists(UserID)'
            ]
        
        # Execute table creation
        for sql in tables_sql:
            safe_execute_sql(cursor, sql, conn=conn)
        
        # Execute index creation
        for sql in indexes_sql:
            safe_execute_sql(cursor, sql, conn=conn)
        
        logger.info("Created gpodder sync initial schema successfully")
        
    except Exception as e:
        logger.error(f"Error in gpodder migration 100: {e}")
        raise
    finally:
        cursor.close()


@register_migration("101", "gpodder_add_api_version", "Add API version column to GpodderSyncSettings")
def migration_101_gpodder_add_api_version(conn, db_type: str):
    """Add API version column - matches Go migration version 2"""
    cursor = conn.cursor()
    
    try:
        logger.info("Starting gpodder migration 101: Add API version column")
        
        if db_type == 'postgresql':
            safe_execute_sql(cursor, '''
                ALTER TABLE "GpodderSyncSettings"
                ADD COLUMN IF NOT EXISTS APIVersion VARCHAR(10) DEFAULT '2.0'
            ''', conn=conn)
        else:  # mysql
            # Check if column exists first, then add if it doesn't
            cursor.execute("""
                SELECT COUNT(*) FROM INFORMATION_SCHEMA.COLUMNS
                WHERE TABLE_NAME = 'GpodderSyncSettings'
                AND COLUMN_NAME = 'APIVersion'
                AND TABLE_SCHEMA = DATABASE()
            """)
            
            if cursor.fetchone()[0] == 0:
                safe_execute_sql(cursor, '''
                    ALTER TABLE GpodderSyncSettings
                    ADD COLUMN APIVersion VARCHAR(10) DEFAULT '2.0'
                ''', conn=conn)
                logger.info("Added APIVersion column to GpodderSyncSettings")
            else:
                logger.info("APIVersion column already exists in GpodderSyncSettings")
        
        logger.info("Gpodder API version migration completed successfully")
        
    except Exception as e:
        logger.error(f"Error in gpodder migration 101: {e}")
        raise
    finally:
        cursor.close()


@register_migration("102", "gpodder_create_sessions", "Create GpodderSessions table for API sessions")
def migration_102_gpodder_create_sessions(conn, db_type: str):
    """Create GpodderSessions table - matches Go migration version 3"""
    cursor = conn.cursor()
    
    try:
        logger.info("Starting gpodder migration 102: Create GpodderSessions table")
        
        if db_type == 'postgresql':
            safe_execute_sql(cursor, '''
                CREATE TABLE IF NOT EXISTS "GpodderSessions" (
                    SessionID SERIAL PRIMARY KEY,
                    UserID INT NOT NULL,
                    SessionToken TEXT NOT NULL,
                    CreatedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    ExpiresAt TIMESTAMP NOT NULL,
                    LastActive TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    UserAgent TEXT,
                    ClientIP TEXT,
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE,
                    UNIQUE(SessionToken)
                )
            ''', conn=conn)
            
            # Create indexes
            indexes_sql = [
                'CREATE INDEX IF NOT EXISTS idx_gpodder_sessions_token ON "GpodderSessions"(SessionToken)',
                'CREATE INDEX IF NOT EXISTS idx_gpodder_sessions_userid ON "GpodderSessions"(UserID)',
                'CREATE INDEX IF NOT EXISTS idx_gpodder_sessions_expires ON "GpodderSessions"(ExpiresAt)'
            ]
        else:  # mysql
            safe_execute_sql(cursor, '''
                CREATE TABLE IF NOT EXISTS GpodderSessions (
                    SessionID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT NOT NULL,
                    SessionToken TEXT NOT NULL,
                    CreatedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    ExpiresAt TIMESTAMP NOT NULL,
                    LastActive TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    UserAgent TEXT,
                    ClientIP TEXT,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE
                )
            ''', conn=conn)
            
            # Create indexes
            indexes_sql = [
                'CREATE INDEX idx_gpodder_sessions_userid ON GpodderSessions(UserID)',
                'CREATE INDEX idx_gpodder_sessions_expires ON GpodderSessions(ExpiresAt)'
            ]
        
        # Execute index creation
        for sql in indexes_sql:
            safe_execute_sql(cursor, sql, conn=conn)
        
        logger.info("Created GpodderSessions table successfully")
        
    except Exception as e:
        logger.error(f"Error in gpodder migration 102: {e}")
        raise
    finally:
        cursor.close()


@register_migration("103", "gpodder_sync_state_table", "Add sync state table for tracking device sync status")
def migration_103_gpodder_sync_state_table(conn, db_type: str):
    """Create GpodderSyncState table - matches Go migration version 4"""
    cursor = conn.cursor()
    
    try:
        logger.info("Starting gpodder migration 103: Add sync state table")
        
        if db_type == 'postgresql':
            safe_execute_sql(cursor, '''
                CREATE TABLE IF NOT EXISTS "GpodderSyncState" (
                    SyncStateID SERIAL PRIMARY KEY,
                    UserID INT NOT NULL,
                    DeviceID INT NOT NULL,
                    LastTimestamp BIGINT DEFAULT 0,
                    LastSync TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE,
                    FOREIGN KEY (DeviceID) REFERENCES "GpodderDevices"(DeviceID) ON DELETE CASCADE,
                    UNIQUE(UserID, DeviceID)
                )
            ''', conn=conn)
            
            safe_execute_sql(cursor, '''
                CREATE INDEX IF NOT EXISTS idx_gpodder_syncstate_userid_deviceid ON "GpodderSyncState"(UserID, DeviceID)
            ''', conn=conn)
        else:  # mysql
            safe_execute_sql(cursor, '''
                CREATE TABLE IF NOT EXISTS GpodderSyncState (
                    SyncStateID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT NOT NULL,
                    DeviceID INT NOT NULL,
                    LastTimestamp BIGINT DEFAULT 0,
                    LastSync TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE,
                    FOREIGN KEY (DeviceID) REFERENCES GpodderDevices(DeviceID) ON DELETE CASCADE,
                    UNIQUE(UserID, DeviceID)
                )
            ''', conn=conn)
            
            safe_execute_sql(cursor, '''
                CREATE INDEX idx_gpodder_syncstate_userid_deviceid ON GpodderSyncState(UserID, DeviceID)
            ''', conn=conn)
        
        logger.info("Created GpodderSyncState table successfully")
        
    except Exception as e:
        logger.error(f"Error in gpodder migration 103: {e}")
        raise
    finally:
        cursor.close()


@register_migration("104", "create_people_episodes_backup", "Skip PeopleEpisodes_backup - varies by installation")
def migration_104_create_people_episodes_backup(conn, db_type: str):
    """Skip PeopleEpisodes_backup table - this varies by installation and shouldn't be validated"""
    logger.info("Skipping migration 104: PeopleEpisodes_backup table varies by installation")
    # This migration is a no-op since backup tables vary by installation
    # and shouldn't be part of the expected schema


@register_migration("105", "optimize_episode_actions_performance", "Add indexes and optimize episode actions queries")
def migration_105_optimize_episode_actions_performance(conn, db_type: str):
    """Add critical indexes for episode actions performance and create optimized views"""
    cursor = conn.cursor()
    
    try:
        logger.info("Adding performance indexes for episode actions...")
        
        if db_type == 'postgresql':
            # Critical indexes for episode actions performance
            safe_execute_sql(cursor, '''
                CREATE INDEX IF NOT EXISTS idx_episode_actions_user_timestamp 
                ON "GpodderSyncEpisodeActions"(UserID, Timestamp DESC)
            ''', conn=conn)
            
            safe_execute_sql(cursor, '''
                CREATE INDEX IF NOT EXISTS idx_episode_actions_device_timestamp 
                ON "GpodderSyncEpisodeActions"(DeviceID, Timestamp DESC) 
                WHERE DeviceID IS NOT NULL
            ''', conn=conn)
            
            safe_execute_sql(cursor, '''
                CREATE INDEX IF NOT EXISTS idx_episode_actions_podcast_episode 
                ON "GpodderSyncEpisodeActions"(UserID, PodcastURL, EpisodeURL, Timestamp DESC)
            ''', conn=conn)
            
            safe_execute_sql(cursor, '''
                CREATE INDEX IF NOT EXISTS idx_episode_actions_since_filter 
                ON "GpodderSyncEpisodeActions"(UserID, Timestamp DESC, DeviceID) 
                WHERE Timestamp > 0
            ''', conn=conn)
            
            # Optimize devices table lookups
            safe_execute_sql(cursor, '''
                CREATE INDEX IF NOT EXISTS idx_gpodder_devices_user_name 
                ON "GpodderDevices"(UserID, DeviceName) 
                WHERE IsActive = true
            ''', conn=conn)
            
        else:  # mysql/mariadb
            # Critical indexes for episode actions performance
            safe_execute_sql(cursor, '''
                CREATE INDEX idx_episode_actions_user_timestamp 
                ON GpodderSyncEpisodeActions(UserID, Timestamp DESC)
            ''', conn=conn)
            
            safe_execute_sql(cursor, '''
                CREATE INDEX idx_episode_actions_device_timestamp 
                ON GpodderSyncEpisodeActions(DeviceID, Timestamp DESC)
            ''', conn=conn)
            
            safe_execute_sql(cursor, '''
                CREATE INDEX idx_episode_actions_podcast_episode 
                ON GpodderSyncEpisodeActions(UserID, PodcastURL(255), EpisodeURL(255), Timestamp DESC)
            ''', conn=conn)
            
            safe_execute_sql(cursor, '''
                CREATE INDEX idx_episode_actions_since_filter 
                ON GpodderSyncEpisodeActions(UserID, Timestamp DESC, DeviceID)
            ''', conn=conn)
            
            # Optimize devices table lookups
            safe_execute_sql(cursor, '''
                CREATE INDEX idx_gpodder_devices_user_name 
                ON GpodderDevices(UserID, DeviceName)
            ''', conn=conn)
        
        logger.info("Successfully added episode actions performance indexes")

    except Exception as e:
        logger.error(f"Error in gpodder migration 105: {e}")
        raise
    finally:
        cursor.close()


@register_migration("106", "optimize_subscription_sync_performance", "Add missing indexes for subscription sync queries", requires=["103"])
def migration_106_optimize_subscription_sync_performance(conn, db_type: str):
    """Add critical indexes for subscription sync performance to prevent AntennaPod timeouts"""
    cursor = conn.cursor()

    try:
        logger.info("Adding performance indexes for subscription sync...")

        if db_type == 'postgresql':
            # Critical indexes for subscription sync performance
            safe_execute_sql(cursor, '''
                CREATE INDEX IF NOT EXISTS idx_gpodder_sync_subs_user_device_timestamp
                ON "GpodderSyncSubscriptions"(UserID, DeviceID, Timestamp DESC)
            ''', conn=conn)

            safe_execute_sql(cursor, '''
                CREATE INDEX IF NOT EXISTS idx_gpodder_sync_subs_user_action_timestamp
                ON "GpodderSyncSubscriptions"(UserID, Action, Timestamp DESC)
            ''', conn=conn)

            safe_execute_sql(cursor, '''
                CREATE INDEX IF NOT EXISTS idx_gpodder_sync_subs_podcast_url_user
                ON "GpodderSyncSubscriptions"(UserID, PodcastURL, Timestamp DESC)
            ''', conn=conn)

            # Optimize subscription change queries with compound index
            safe_execute_sql(cursor, '''
                CREATE INDEX IF NOT EXISTS idx_gpodder_sync_subs_complex_query
                ON "GpodderSyncSubscriptions"(UserID, DeviceID, Action, Timestamp DESC, PodcastURL)
            ''', conn=conn)

        else:  # mysql/mariadb
            # Critical indexes for subscription sync performance
            safe_execute_sql(cursor, '''
                CREATE INDEX idx_gpodder_sync_subs_user_device_timestamp
                ON GpodderSyncSubscriptions(UserID, DeviceID, Timestamp DESC)
            ''', conn=conn)

            safe_execute_sql(cursor, '''
                CREATE INDEX idx_gpodder_sync_subs_user_action_timestamp
                ON GpodderSyncSubscriptions(UserID, Action, Timestamp DESC)
            ''', conn=conn)

            safe_execute_sql(cursor, '''
                CREATE INDEX idx_gpodder_sync_subs_podcast_url_user
                ON GpodderSyncSubscriptions(UserID, PodcastURL(255), Timestamp DESC)
            ''', conn=conn)

            # Optimize subscription change queries with compound index
            safe_execute_sql(cursor, '''
                CREATE INDEX idx_gpodder_sync_subs_complex_query
                ON GpodderSyncSubscriptions(UserID, DeviceID, Action, Timestamp DESC, PodcastURL(255))
            ''', conn=conn)

        logger.info("Successfully added subscription sync performance indexes")

    except Exception as e:
        logger.error(f"Error in gpodder migration 106: {e}")
        raise
    finally:
        cursor.close()


@register_migration("108", "gpodder_subscription_snapshot", "Add subscription snapshot table for delta-based sync upload", requires=["008"])
def migration_108_gpodder_subscription_snapshot(conn, db_type: str):
    """Create GpodderSubscriptionSnapshot table.

    Stores, per (user, sync target), the set of local feed URLs at the end of the last sync.
    The sync code diffs the current local feeds against this snapshot to compute genuine local
    add/remove deltas to push up - instead of re-uploading the full list every sync (which bloats
    the server change log) and without a per-change queue.
    """
    cursor = conn.cursor()

    try:
        logger.info("Starting gpodder migration 108: Add subscription snapshot table")

        if db_type == 'postgresql':
            safe_execute_sql(cursor, '''
                CREATE TABLE IF NOT EXISTS "GpodderSubscriptionSnapshot" (
                    SnapshotID SERIAL PRIMARY KEY,
                    UserID INT NOT NULL,
                    SyncTarget TEXT NOT NULL,
                    FeedURL TEXT NOT NULL,
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE,
                    UNIQUE(UserID, SyncTarget, FeedURL)
                )
            ''', conn=conn)

            safe_execute_sql(cursor, '''
                CREATE INDEX IF NOT EXISTS idx_gpodder_subsnapshot_user_target ON "GpodderSubscriptionSnapshot"(UserID, SyncTarget)
            ''', conn=conn)
        else:  # mysql
            safe_execute_sql(cursor, '''
                CREATE TABLE IF NOT EXISTS GpodderSubscriptionSnapshot (
                    SnapshotID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT NOT NULL,
                    SyncTarget VARCHAR(512) NOT NULL,
                    FeedURL VARCHAR(2048) NOT NULL,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE,
                    UNIQUE(UserID, SyncTarget, FeedURL(512))
                )
            ''', conn=conn)

            safe_execute_sql(cursor, '''
                CREATE INDEX idx_gpodder_subsnapshot_user_target ON GpodderSubscriptionSnapshot(UserID, SyncTarget)
            ''', conn=conn)

        logger.info("Created GpodderSubscriptionSnapshot table successfully")

    except Exception as e:
        logger.error(f"Error in gpodder migration 108: {e}")
        raise
    finally:
        cursor.close()


@register_migration("109", "host_feed_cache", "Add shared host-feed cache table for the live person feed", requires=["009"])
def migration_109_host_feed_cache(conn, db_type: str):
    """Create HostFeedCache table.

    A DB-backed warm cache for the live host feed (/api/data/person/feed). Building a host's feed
    means fetching+parsing every RSS feed they appear in, which is expensive for prolific hosts.
    The short-lived Redis cache absorbs repeat visits; this table is the longer-lived warm layer so
    a cold/expired Redis entry doesn't force a full N-feed rebuild, and so the cache survives a
    Redis restart. Keyed by the lowercased host name + whether podcasts are included.
    """
    cursor = conn.cursor()

    try:
        logger.info("Starting migration 109: Add host feed cache table")

        if db_type == 'postgresql':
            safe_execute_sql(cursor, '''
                CREATE TABLE IF NOT EXISTS "HostFeedCache" (
                    CacheKey TEXT PRIMARY KEY,
                    FeedJSON TEXT NOT NULL,
                    RefreshedAt TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
                )
            ''', conn=conn)
        else:  # mysql
            safe_execute_sql(cursor, '''
                CREATE TABLE IF NOT EXISTS HostFeedCache (
                    CacheKey VARCHAR(512) PRIMARY KEY,
                    FeedJSON LONGTEXT NOT NULL,
                    RefreshedAt TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
                )
            ''', conn=conn)

        logger.info("Created HostFeedCache table successfully")

    except Exception as e:
        logger.error(f"Error in migration 109: {e}")
        raise
    finally:
        cursor.close()


@register_migration("033", "add_http_notification_columns", "Add generic HTTP notification columns to UserNotificationSettings table", requires=["011"])
def migration_033_add_http_notification_columns(conn, db_type: str):
    """Add generic HTTP notification columns for platforms like Telegram"""
    cursor = conn.cursor()
    
    try:
        if db_type == "postgresql":
            # Check if columns already exist (PostgreSQL - lowercase column names)
            cursor.execute("""
                SELECT column_name FROM information_schema.columns 
                WHERE table_name = 'UserNotificationSettings' 
                AND column_name IN ('httpurl', 'httptoken', 'httpmethod')
            """)
            existing_columns = [row[0] for row in cursor.fetchall()]
            
            if 'httpurl' not in existing_columns:
                cursor.execute("""
                    ALTER TABLE "UserNotificationSettings"
                    ADD COLUMN HttpUrl VARCHAR(500)
                """)
                logger.info("Added HttpUrl column to UserNotificationSettings table (PostgreSQL)")
            
            if 'httptoken' not in existing_columns:
                cursor.execute("""
                    ALTER TABLE "UserNotificationSettings"
                    ADD COLUMN HttpToken VARCHAR(255)
                """)
                logger.info("Added HttpToken column to UserNotificationSettings table (PostgreSQL)")
            
            if 'httpmethod' not in existing_columns:
                cursor.execute("""
                    ALTER TABLE "UserNotificationSettings"
                    ADD COLUMN HttpMethod VARCHAR(10) DEFAULT 'POST'
                """)
                logger.info("Added HttpMethod column to UserNotificationSettings table (PostgreSQL)")
        
        else:
            # Check if columns already exist (MySQL)
            cursor.execute("""
                SELECT COUNT(*)
                FROM information_schema.columns
                WHERE table_name = 'UserNotificationSettings' 
                AND column_name = 'HttpUrl'
                AND table_schema = DATABASE()
            """)
            url_exists = cursor.fetchone()[0] > 0
            
            cursor.execute("""
                SELECT COUNT(*)
                FROM information_schema.columns
                WHERE table_name = 'UserNotificationSettings' 
                AND column_name = 'HttpToken'
                AND table_schema = DATABASE()
            """)
            token_exists = cursor.fetchone()[0] > 0
            
            cursor.execute("""
                SELECT COUNT(*)
                FROM information_schema.columns
                WHERE table_name = 'UserNotificationSettings' 
                AND column_name = 'HttpMethod'
                AND table_schema = DATABASE()
            """)
            method_exists = cursor.fetchone()[0] > 0
            
            if not url_exists:
                cursor.execute("""
                    ALTER TABLE UserNotificationSettings
                    ADD COLUMN HttpUrl VARCHAR(500)
                """)
                logger.info("Added HttpUrl column to UserNotificationSettings table (MySQL)")
            
            if not token_exists:
                cursor.execute("""
                    ALTER TABLE UserNotificationSettings
                    ADD COLUMN HttpToken VARCHAR(255)
                """)
                logger.info("Added HttpToken column to UserNotificationSettings table (MySQL)")
            
            if not method_exists:
                cursor.execute("""
                    ALTER TABLE UserNotificationSettings
                    ADD COLUMN HttpMethod VARCHAR(10) DEFAULT 'POST'
                """)
                logger.info("Added HttpMethod column to UserNotificationSettings table (MySQL)")
        
        logger.info("HTTP notification columns migration completed successfully")
        
    finally:
        cursor.close()


@register_migration("034", "add_podcast_merge_columns", "Add podcast merge columns to support merging podcasts", requires=["033"])
def migration_034_add_podcast_merge_columns(conn, db_type: str):
    """Add DisplayPodcast, RefreshPodcast, and MergedPodcastIDs columns to Podcasts table"""
    cursor = conn.cursor()
    
    try:
        if db_type == "postgresql":
            # Check if columns already exist (PostgreSQL)
            cursor.execute("""
                SELECT column_name FROM information_schema.columns 
                WHERE table_name = 'Podcasts' 
                AND column_name IN ('displaypodcast', 'refreshpodcast', 'mergedpodcastids')
            """)
            existing_columns = [row[0] for row in cursor.fetchall()]
            
            if 'displaypodcast' not in existing_columns:
                cursor.execute("""
                    ALTER TABLE "Podcasts"
                    ADD COLUMN DisplayPodcast BOOLEAN DEFAULT TRUE
                """)
                logger.info("Added DisplayPodcast column to Podcasts table (PostgreSQL)")
            
            if 'refreshpodcast' not in existing_columns:
                cursor.execute("""
                    ALTER TABLE "Podcasts"
                    ADD COLUMN RefreshPodcast BOOLEAN DEFAULT TRUE
                """)
                logger.info("Added RefreshPodcast column to Podcasts table (PostgreSQL)")
            
            if 'mergedpodcastids' not in existing_columns:
                cursor.execute("""
                    ALTER TABLE "Podcasts"
                    ADD COLUMN MergedPodcastIDs TEXT
                """)
                logger.info("Added MergedPodcastIDs column to Podcasts table (PostgreSQL)")
        
        else:  # MySQL
            # Check if columns already exist (MySQL)
            cursor.execute("""
                SELECT COUNT(*)
                FROM information_schema.columns
                WHERE table_name = 'Podcasts' 
                AND column_name = 'DisplayPodcast'
                AND table_schema = DATABASE()
            """)
            display_exists = cursor.fetchone()[0] > 0
            
            cursor.execute("""
                SELECT COUNT(*)
                FROM information_schema.columns
                WHERE table_name = 'Podcasts' 
                AND column_name = 'RefreshPodcast'
                AND table_schema = DATABASE()
            """)
            refresh_exists = cursor.fetchone()[0] > 0
            
            cursor.execute("""
                SELECT COUNT(*)
                FROM information_schema.columns
                WHERE table_name = 'Podcasts' 
                AND column_name = 'MergedPodcastIDs'
                AND table_schema = DATABASE()
            """)
            merged_exists = cursor.fetchone()[0] > 0
            
            if not display_exists:
                cursor.execute("""
                    ALTER TABLE Podcasts
                    ADD COLUMN DisplayPodcast TINYINT(1) DEFAULT 1
                """)
                logger.info("Added DisplayPodcast column to Podcasts table (MySQL)")
            
            if not refresh_exists:
                cursor.execute("""
                    ALTER TABLE Podcasts
                    ADD COLUMN RefreshPodcast TINYINT(1) DEFAULT 1
                """)
                logger.info("Added RefreshPodcast column to Podcasts table (MySQL)")
            
            if not merged_exists:
                cursor.execute("""
                    ALTER TABLE Podcasts
                    ADD COLUMN MergedPodcastIDs TEXT
                """)
                logger.info("Added MergedPodcastIDs column to Podcasts table (MySQL)")
        
        # Add index on DisplayPodcast for performance
        table_quote = "`" if db_type != "postgresql" else '"'
        safe_add_index(cursor, db_type, 
            f'CREATE INDEX idx_podcasts_displaypodcast ON {table_quote}Podcasts{table_quote} (DisplayPodcast)', 
            'idx_podcasts_displaypodcast')
        
        logger.info("Podcast merge columns migration completed successfully")
        
    finally:
        cursor.close()


@register_migration("035", "add_podcast_cover_preference_columns", "Add podcast cover preference columns to Users and Podcasts tables", requires=["034"])
def migration_035_add_podcast_cover_preference_columns(conn, db_type: str):
    """Add podcast cover preference columns to Users and Podcasts tables for existing installations"""
    cursor = conn.cursor()
    
    try:
        # Add UsePodcastCovers to Users table if it doesn't exist
        try:
            if db_type == "postgresql":
                cursor.execute("""
                    ALTER TABLE "Users" 
                    ADD COLUMN IF NOT EXISTS UsePodcastCovers BOOLEAN DEFAULT FALSE
                """)
            else:  # MySQL/MariaDB
                # Check if column exists first
                cursor.execute("""
                    SELECT COUNT(*) 
                    FROM INFORMATION_SCHEMA.COLUMNS 
                    WHERE TABLE_SCHEMA = DATABASE() 
                    AND TABLE_NAME = 'Users' 
                    AND COLUMN_NAME = 'UsePodcastCovers'
                """)
                if cursor.fetchone()[0] == 0:
                    cursor.execute("""
                        ALTER TABLE Users 
                        ADD COLUMN UsePodcastCovers TINYINT(1) DEFAULT 0
                    """)
                    logger.info("Added UsePodcastCovers column to Users table")
                else:
                    logger.info("UsePodcastCovers column already exists in Users table")
                    
        except Exception as e:
            logger.error(f"Error adding UsePodcastCovers to Users table: {e}")
    
        # Add UsePodcastCovers columns to Podcasts table if they don't exist
        try:
            if db_type == "postgresql":
                cursor.execute("""
                    ALTER TABLE "Podcasts" 
                    ADD COLUMN IF NOT EXISTS UsePodcastCovers BOOLEAN DEFAULT FALSE,
                    ADD COLUMN IF NOT EXISTS UsePodcastCoversCustomized BOOLEAN DEFAULT FALSE
                """)
            else:  # MySQL/MariaDB
                # Check if UsePodcastCovers column exists
                cursor.execute("""
                    SELECT COUNT(*) 
                    FROM INFORMATION_SCHEMA.COLUMNS 
                    WHERE TABLE_SCHEMA = DATABASE() 
                    AND TABLE_NAME = 'Podcasts' 
                    AND COLUMN_NAME = 'UsePodcastCovers'
                """)
                if cursor.fetchone()[0] == 0:
                    cursor.execute("""
                        ALTER TABLE Podcasts 
                        ADD COLUMN UsePodcastCovers TINYINT(1) DEFAULT 0
                    """)
                    logger.info("Added UsePodcastCovers column to Podcasts table")
                else:
                    logger.info("UsePodcastCovers column already exists in Podcasts table")
                
                # Check if UsePodcastCoversCustomized column exists
                cursor.execute("""
                    SELECT COUNT(*) 
                    FROM INFORMATION_SCHEMA.COLUMNS 
                    WHERE TABLE_SCHEMA = DATABASE() 
                    AND TABLE_NAME = 'Podcasts' 
                    AND COLUMN_NAME = 'UsePodcastCoversCustomized'
                """)
                if cursor.fetchone()[0] == 0:
                    cursor.execute("""
                        ALTER TABLE Podcasts 
                        ADD COLUMN UsePodcastCoversCustomized TINYINT(1) DEFAULT 0
                    """)
                    logger.info("Added UsePodcastCoversCustomized column to Podcasts table")
                else:
                    logger.info("UsePodcastCoversCustomized column already exists in Podcasts table")
                    
        except Exception as e:
            logger.error(f"Error adding UsePodcastCovers columns to Podcasts table: {e}")
    
        logger.info("Podcast cover preference columns migration completed successfully")
        
    finally:
        cursor.close()


@register_migration("036", "add_episodecount_column_to_playlists", "Add episodecount column to Playlists table for tracking episode counts", requires=["010"])
def migration_036_add_episodecount_column(conn, db_type: str):
    """Add episodecount column to Playlists table if it doesn't exist

    This migration was needed because migration 032 was applied to existing databases
    before the episodecount column addition was added to it. Since migration 032 is
    already marked as applied in those databases, the column was never created.
    """
    cursor = conn.cursor()

    try:
        logger.info("Checking for episodecount column in Playlists table")

        if db_type == "postgresql":
            # Check if episodecount column exists
            cursor.execute("""
                SELECT column_name FROM information_schema.columns
                WHERE table_name = 'Playlists'
                AND column_name = 'episodecount'
            """)
            column_exists = len(cursor.fetchall()) > 0

            if not column_exists:
                cursor.execute("""
                    ALTER TABLE "Playlists"
                    ADD COLUMN episodecount INTEGER DEFAULT 0
                """)
                logger.info("Added episodecount column to Playlists table (PostgreSQL)")
            else:
                logger.info("episodecount column already exists in Playlists table (PostgreSQL)")
        else:
            # Check if episodecount column exists (MySQL/MariaDB)
            cursor.execute("""
                SELECT COUNT(*)
                FROM INFORMATION_SCHEMA.COLUMNS
                WHERE TABLE_NAME = 'Playlists'
                AND COLUMN_NAME = 'EpisodeCount'
                AND TABLE_SCHEMA = DATABASE()
            """)
            column_exists = cursor.fetchone()[0] > 0

            if not column_exists:
                cursor.execute("""
                    ALTER TABLE Playlists
                    ADD COLUMN EpisodeCount INT DEFAULT 0
                """)
                logger.info("Added EpisodeCount column to Playlists table (MySQL/MariaDB)")
            else:
                logger.info("EpisodeCount column already exists in Playlists table (MySQL/MariaDB)")

        logger.info("episodecount column migration completed successfully")

    except Exception as e:
        logger.error(f"Error in migration 036: {e}")
        raise
    finally:
        cursor.close()


@register_migration("037", "fix_shared_episodes_schema", "Add missing SharedBy and SharedWith columns to SharedEpisodes table", requires=["009"])
def migration_037_fix_shared_episodes_schema(conn, db_type: str):
    """Add missing SharedBy and SharedWith columns to SharedEpisodes table

    Old schema had: EpisodeID, UrlKey, ExpirationDate
    New schema needs: EpisodeID, SharedBy, SharedWith, ShareCode, ExpirationDate
    """
    cursor = conn.cursor()

    try:
        logger.info("Starting SharedEpisodes schema fix migration")

        if db_type == "postgresql":
            # Check if sharedby column exists
            cursor.execute("""
                SELECT column_name FROM information_schema.columns
                WHERE table_name = 'SharedEpisodes'
                AND column_name = 'sharedby'
            """)
            sharedby_exists = len(cursor.fetchall()) > 0

            if not sharedby_exists:
                logger.info("Adding sharedby column to SharedEpisodes table (PostgreSQL)")
                cursor.execute("""
                    ALTER TABLE "SharedEpisodes"
                    ADD COLUMN sharedby INTEGER NOT NULL DEFAULT 1
                """)
                conn.commit()

            # Check if sharedwith column exists
            cursor.execute("""
                SELECT column_name FROM information_schema.columns
                WHERE table_name = 'SharedEpisodes'
                AND column_name = 'sharedwith'
            """)
            sharedwith_exists = len(cursor.fetchall()) > 0

            if not sharedwith_exists:
                logger.info("Adding sharedwith column to SharedEpisodes table (PostgreSQL)")
                cursor.execute("""
                    ALTER TABLE "SharedEpisodes"
                    ADD COLUMN sharedwith INTEGER
                """)
                conn.commit()

            # Check if sharecode column exists (might have been UrlKey)
            cursor.execute("""
                SELECT column_name FROM information_schema.columns
                WHERE table_name = 'SharedEpisodes'
                AND column_name = 'sharecode'
            """)
            sharecode_exists = len(cursor.fetchall()) > 0

            if not sharecode_exists:
                # Check if UrlKey exists
                cursor.execute("""
                    SELECT column_name FROM information_schema.columns
                    WHERE table_name = 'SharedEpisodes'
                    AND column_name IN ('UrlKey', 'urlkey')
                """)
                urlkey_result = cursor.fetchall()

                if urlkey_result:
                    urlkey_name = urlkey_result[0][0]
                    logger.info(f"Renaming {urlkey_name} to sharecode (PostgreSQL)")
                    cursor.execute(f"""
                        ALTER TABLE "SharedEpisodes"
                        RENAME COLUMN "{urlkey_name}" TO sharecode
                    """)
                else:
                    logger.info("Adding sharecode column to SharedEpisodes table (PostgreSQL)")
                    cursor.execute("""
                        ALTER TABLE "SharedEpisodes"
                        ADD COLUMN sharecode TEXT UNIQUE
                    """)
                conn.commit()

            logger.info("SharedEpisodes schema fix completed (PostgreSQL)")

        else:  # MySQL/MariaDB
            # Check if SharedBy column exists
            cursor.execute("""
                SELECT COUNT(*)
                FROM INFORMATION_SCHEMA.COLUMNS
                WHERE TABLE_NAME = 'SharedEpisodes'
                AND COLUMN_NAME = 'SharedBy'
                AND TABLE_SCHEMA = DATABASE()
            """)
            sharedby_exists = cursor.fetchone()[0] > 0

            if not sharedby_exists:
                logger.info("Adding SharedBy column to SharedEpisodes table (MySQL)")
                cursor.execute("""
                    ALTER TABLE SharedEpisodes
                    ADD COLUMN SharedBy INT NOT NULL DEFAULT 1
                """)
                conn.commit()

            # Check if SharedWith column exists
            cursor.execute("""
                SELECT COUNT(*)
                FROM INFORMATION_SCHEMA.COLUMNS
                WHERE TABLE_NAME = 'SharedEpisodes'
                AND COLUMN_NAME = 'SharedWith'
                AND TABLE_SCHEMA = DATABASE()
            """)
            sharedwith_exists = cursor.fetchone()[0] > 0

            if not sharedwith_exists:
                logger.info("Adding SharedWith column to SharedEpisodes table (MySQL)")
                cursor.execute("""
                    ALTER TABLE SharedEpisodes
                    ADD COLUMN SharedWith INT
                """)
                conn.commit()

            # Check if ShareCode column exists (might have been UrlKey)
            cursor.execute("""
                SELECT COUNT(*)
                FROM INFORMATION_SCHEMA.COLUMNS
                WHERE TABLE_NAME = 'SharedEpisodes'
                AND COLUMN_NAME = 'ShareCode'
                AND TABLE_SCHEMA = DATABASE()
            """)
            sharecode_exists = cursor.fetchone()[0] > 0

            if not sharecode_exists:
                # Check if UrlKey exists
                cursor.execute("""
                    SELECT COUNT(*)
                    FROM INFORMATION_SCHEMA.COLUMNS
                    WHERE TABLE_NAME = 'SharedEpisodes'
                    AND COLUMN_NAME = 'UrlKey'
                    AND TABLE_SCHEMA = DATABASE()
                """)
                urlkey_exists = cursor.fetchone()[0] > 0

                if urlkey_exists:
                    logger.info("Renaming UrlKey to ShareCode (MySQL)")
                    cursor.execute("""
                        ALTER TABLE SharedEpisodes
                        CHANGE COLUMN UrlKey ShareCode TEXT
                    """)
                else:
                    logger.info("Adding ShareCode column to SharedEpisodes table (MySQL)")
                    cursor.execute("""
                        ALTER TABLE SharedEpisodes
                        ADD COLUMN ShareCode TEXT
                    """)
                conn.commit()

            logger.info("SharedEpisodes schema fix completed (MySQL)")

        logger.info("SharedEpisodes schema fix migration completed successfully")

    except Exception as e:
        logger.error(f"Error in migration 037: {e}")
        raise
    finally:
        cursor.close()


@register_migration("107", "fix_gpodder_episode_actions_antennapod", "Fix existing GPodder episode actions to include Started and Total fields for AntennaPod compatibility", requires=["103"])
def migration_107_fix_gpodder_episode_actions(conn, db_type: str):
    """
    Fix existing GPodder episode actions to be compatible with AntennaPod.
    AntennaPod requires all play actions to have Started, Position, and Total fields.
    This migration adds those fields by joining with the Episodes table to get duration.
    """
    cursor = conn.cursor()

    try:
        logger.info("Starting GPodder episode actions fix for AntennaPod compatibility...")

        if db_type == "postgresql":
            # First, count how many actions need fixing
            cursor.execute("""
                SELECT COUNT(*)
                FROM "GpodderSyncEpisodeActions"
                WHERE action = 'play'
                AND (started IS NULL OR total IS NULL OR started < 0 OR total <= 0)
            """)
            count_result = cursor.fetchone()
            actions_to_fix = count_result[0] if count_result else 0

            logger.info(f"Found {actions_to_fix} play actions that need fixing (PostgreSQL)")

            if actions_to_fix > 0:
                # Update from Episodes table join
                logger.info("Updating episode actions with duration from Episodes table...")
                cursor.execute("""
                    UPDATE "GpodderSyncEpisodeActions" AS gsa
                    SET
                        started = 0,
                        total = e.episodeduration
                    FROM "Episodes" e
                    WHERE gsa.action = 'play'
                    AND gsa.episodeurl = e.episodeurl
                    AND e.episodeduration IS NOT NULL
                    AND e.episodeduration > 0
                    AND (gsa.started IS NULL OR gsa.total IS NULL OR gsa.started < 0 OR gsa.total <= 0)
                """)
                conn.commit()

                # Fallback: use Position as Total for episodes not in Episodes table
                logger.info("Updating remaining actions using Position as fallback for Total...")
                cursor.execute("""
                    UPDATE "GpodderSyncEpisodeActions"
                    SET
                        started = 0,
                        total = COALESCE(position, 1)
                    WHERE action = 'play'
                    AND (started IS NULL OR total IS NULL OR started < 0 OR total <= 0)
                    AND position IS NOT NULL
                    AND position > 0
                """)
                conn.commit()

                # Final cleanup: set minimal valid values for any remaining invalid actions
                logger.info("Final cleanup: setting minimal valid values for remaining invalid actions...")
                cursor.execute("""
                    UPDATE "GpodderSyncEpisodeActions"
                    SET
                        started = 0,
                        total = 1
                    WHERE action = 'play'
                    AND (started IS NULL OR total IS NULL OR started < 0 OR total <= 0)
                """)
                conn.commit()

                # Verify the fix
                cursor.execute("""
                    SELECT COUNT(*)
                    FROM "GpodderSyncEpisodeActions"
                    WHERE action = 'play'
                    AND (started IS NULL OR total IS NULL OR started < 0 OR total <= 0 OR position <= 0)
                """)
                remaining_result = cursor.fetchone()
                remaining_broken = remaining_result[0] if remaining_result else 0

                logger.info(f"Fixed {actions_to_fix - remaining_broken} episode actions (PostgreSQL)")
                if remaining_broken > 0:
                    logger.warning(f"{remaining_broken} actions still have invalid fields - these may need manual review")
            else:
                logger.info("No actions need fixing (PostgreSQL)")

        else:  # MySQL/MariaDB
            # First, count how many actions need fixing
            cursor.execute("""
                SELECT COUNT(*)
                FROM GpodderSyncEpisodeActions
                WHERE Action = 'play'
                AND (Started IS NULL OR Total IS NULL OR Started < 0 OR Total <= 0)
            """)
            count_result = cursor.fetchone()
            actions_to_fix = count_result[0] if count_result else 0

            logger.info(f"Found {actions_to_fix} play actions that need fixing (MySQL)")

            if actions_to_fix > 0:
                # MySQL: Update using JOIN
                logger.info("Updating episode actions with duration from Episodes table...")
                cursor.execute("""
                    UPDATE GpodderSyncEpisodeActions AS gsa
                    LEFT JOIN Episodes e ON gsa.EpisodeURL = e.EpisodeURL
                        AND e.EpisodeDuration IS NOT NULL
                        AND e.EpisodeDuration > 0
                    SET
                        gsa.Started = 0,
                        gsa.Total = COALESCE(e.EpisodeDuration, gsa.Position, 1)
                    WHERE gsa.Action = 'play'
                    AND (gsa.Started IS NULL OR gsa.Total IS NULL OR gsa.Started < 0 OR gsa.Total <= 0)
                """)
                conn.commit()

                # Verify the fix
                cursor.execute("""
                    SELECT COUNT(*)
                    FROM GpodderSyncEpisodeActions
                    WHERE Action = 'play'
                    AND (Started IS NULL OR Total IS NULL OR Started < 0 OR Total <= 0 OR Position <= 0)
                """)
                remaining_result = cursor.fetchone()
                remaining_broken = remaining_result[0] if remaining_result else 0

                logger.info(f"Fixed {actions_to_fix - remaining_broken} episode actions (MySQL)")
                if remaining_broken > 0:
                    logger.warning(f"{remaining_broken} actions still have invalid fields - these may need manual review")
            else:
                logger.info("No actions need fixing (MySQL)")

        logger.info("GPodder episode actions fix migration completed successfully")
        logger.info("AntennaPod should now be able to sync episode actions correctly")

    except Exception as e:
        logger.error(f"Error in migration 107: {e}")
        raise
    finally:
        cursor.close()


if __name__ == "__main__":
    # Register all migrations and run them
    register_all_migrations()
    from database_functions.migrations import run_all_migrations
    success = run_all_migrations()
    sys.exit(0 if success else 1)


@register_migration("038", "add_is_video_to_episodes", "Add is_video column to Episodes table to support video podcasts", requires=["001"])
def migration_038_add_is_video_to_episodes(conn, db_type: str):
    """
    Add is_video column to Episodes table to track whether an episode is video or audio.
    This enables support for video podcasts while maintaining backward compatibility.
    """
    cursor = conn.cursor()

    try:
        logger.info("Starting migration to add is_video column to Episodes table...")

        if db_type == "postgresql":
            # Check if column already exists
            cursor.execute("""
                SELECT column_name
                FROM information_schema.columns
                WHERE table_name = 'Episodes'
                AND column_name = 'is_video'
            """)

            if cursor.fetchone():
                logger.info("is_video column already exists in Episodes table (PostgreSQL)")
                return

            # Add is_video column (default to false for existing episodes)
            logger.info("Adding is_video column to Episodes table (PostgreSQL)...")
            cursor.execute("""
                ALTER TABLE "Episodes"
                ADD COLUMN is_video BOOLEAN DEFAULT FALSE
            """)
            conn.commit()
            logger.info("Successfully added is_video column to Episodes table (PostgreSQL)")

        else:  # MySQL/MariaDB
            # Check if column already exists
            cursor.execute("""
                SELECT COLUMN_NAME
                FROM INFORMATION_SCHEMA.COLUMNS
                WHERE TABLE_NAME = 'Episodes'
                AND COLUMN_NAME = 'IsVideo'
            """)

            if cursor.fetchone():
                logger.info("IsVideo column already exists in Episodes table (MySQL)")
                return

            # Add IsVideo column (default to 0/false for existing episodes)
            logger.info("Adding IsVideo column to Episodes table (MySQL)...")
            cursor.execute("""
                ALTER TABLE Episodes
                ADD COLUMN IsVideo BOOLEAN DEFAULT FALSE
            """)
            conn.commit()
            logger.info("Successfully added IsVideo column to Episodes table (MySQL)")

    except Exception as e:
        logger.error(f"Error adding is_video column to Episodes: {e}")
        conn.rollback()
        raise


@register_migration("039", "add_auto_play_next_to_podcasts", "Add autoplaynext column to Podcasts table for serial podcast auto-play", requires=["005"])
def migration_039_add_auto_play_next_to_podcasts(conn, db_type: str):
    """
    Add autoplaynext column to Podcasts table to support auto-playing
    the next chronological episode when the current one finishes.
    This is useful for serial/fiction podcasts where episode order matters.
    """
    cursor = conn.cursor()

    try:
        logger.info("Starting migration to add autoplaynext column to Podcasts table...")

        if db_type == "postgresql":
            # Check if column already exists
            cursor.execute("""
                SELECT column_name
                FROM information_schema.columns
                WHERE table_name = 'Podcasts'
                AND column_name = 'autoplaynext'
            """)

            if cursor.fetchone():
                logger.info("autoplaynext column already exists in Podcasts table (PostgreSQL)")
                return

            # Add autoplaynext column (default to false for existing podcasts)
            logger.info("Adding autoplaynext column to Podcasts table (PostgreSQL)...")
            cursor.execute("""
                ALTER TABLE "Podcasts"
                ADD COLUMN autoplaynext BOOLEAN DEFAULT FALSE
            """)
            conn.commit()
            logger.info("Successfully added autoplaynext column to Podcasts table (PostgreSQL)")

        else:  # MySQL/MariaDB
            # Check if column already exists
            cursor.execute("""
                SELECT COLUMN_NAME
                FROM INFORMATION_SCHEMA.COLUMNS
                WHERE TABLE_NAME = 'Podcasts'
                AND COLUMN_NAME = 'AutoPlayNext'
            """)

            if cursor.fetchone():
                logger.info("AutoPlayNext column already exists in Podcasts table (MySQL)")
                return

            # Add AutoPlayNext column (default to 0/false for existing podcasts)
            logger.info("Adding AutoPlayNext column to Podcasts table (MySQL)...")
            cursor.execute("""
                ALTER TABLE Podcasts
                ADD COLUMN AutoPlayNext BOOLEAN DEFAULT FALSE
            """)
            conn.commit()
            logger.info("Successfully added AutoPlayNext column to Podcasts table (MySQL)")

    except Exception as e:
        logger.error(f"Error adding autoplaynext column to Podcasts: {e}")
        conn.rollback()
        raise


@register_migration("040", "add_is_favorite_to_podcasts", "Add IsFavorite column to Podcasts table for favoriting podcasts", requires=["005"])
def migration_040_add_is_favorite_to_podcasts(conn, db_type: str):
    cursor = conn.cursor()

    try:
        logger.info("Starting migration to add isfavorite column to Podcasts table...")

        if db_type == "postgresql":
            cursor.execute("""
                SELECT column_name
                FROM information_schema.columns
                WHERE table_name = 'Podcasts'
                AND column_name = 'isfavorite'
            """)
            if cursor.fetchone():
                logger.info("isfavorite column already exists in Podcasts table (PostgreSQL)")
                return
            logger.info("Adding isfavorite column to Podcasts table (PostgreSQL)...")
            cursor.execute("""
                ALTER TABLE "Podcasts"
                ADD COLUMN isfavorite BOOLEAN DEFAULT FALSE
            """)
            conn.commit()
            logger.info("Successfully added isfavorite column to Podcasts table (PostgreSQL)")

        else:  # MySQL/MariaDB
            cursor.execute("""
                SELECT COLUMN_NAME
                FROM INFORMATION_SCHEMA.COLUMNS
                WHERE TABLE_NAME = 'Podcasts'
                AND COLUMN_NAME = 'IsFavorite'
            """)
            if cursor.fetchone():
                logger.info("IsFavorite column already exists in Podcasts table (MySQL)")
                return
            logger.info("Adding IsFavorite column to Podcasts table (MySQL)...")
            cursor.execute("""
                ALTER TABLE Podcasts
                ADD COLUMN IsFavorite BOOLEAN DEFAULT FALSE
            """)
            conn.commit()
            logger.info("Successfully added IsFavorite column to Podcasts table (MySQL)")

    except Exception as e:
        logger.error(f"Error adding isfavorite column to Podcasts: {e}")
        conn.rollback()
        raise


@register_migration("041", "create_custom_themes_table", "Create CustomThemes table for user-defined themes", requires=["001"])
def migration_041_create_custom_themes_table(conn, db_type: str):
    cursor = conn.cursor()

    try:
        logger.info("Starting migration to create CustomThemes table...")

        if db_type == "postgresql":
            cursor.execute("""
                SELECT table_name
                FROM information_schema.tables
                WHERE table_name = 'CustomThemes'
            """)
            if cursor.fetchone():
                logger.info("CustomThemes table already exists (PostgreSQL)")
                return
            logger.info("Creating CustomThemes table (PostgreSQL)...")
            cursor.execute("""
                CREATE TABLE "CustomThemes" (
                    themeid SERIAL PRIMARY KEY,
                    userid INT NOT NULL,
                    name VARCHAR(255) NOT NULL,
                    background_color VARCHAR(7) NOT NULL DEFAULT '#3C4252',
                    button_color VARCHAR(7) NOT NULL DEFAULT '#3e4555',
                    container_button_color VARCHAR(7) NOT NULL DEFAULT 'transparent',
                    button_text_color VARCHAR(7) NOT NULL DEFAULT '#f6f5f4',
                    text_color VARCHAR(7) NOT NULL DEFAULT '#f6f5f4',
                    text_secondary_color VARCHAR(7) NOT NULL DEFAULT '#f6f5f4',
                    border_color VARCHAR(7) NOT NULL DEFAULT '#000000',
                    accent_color VARCHAR(7) NOT NULL DEFAULT '#6d747f',
                    prog_bar_color VARCHAR(7) NOT NULL DEFAULT '#3550af',
                    error_color VARCHAR(7) NOT NULL DEFAULT '#ff0000',
                    bonus_color VARCHAR(7) NOT NULL DEFAULT '#000000',
                    secondary_background VARCHAR(7) NOT NULL DEFAULT '#2e3440',
                    container_background VARCHAR(7) NOT NULL DEFAULT '#2b2f3a',
                    standout_color VARCHAR(7) NOT NULL DEFAULT '#6e8e92',
                    hover_color VARCHAR(7) NOT NULL DEFAULT '#5d80aa',
                    link_color VARCHAR(7) NOT NULL DEFAULT '#5d80aa',
                    thumb_color VARCHAR(7) NOT NULL DEFAULT '#3550af',
                    unfilled_color VARCHAR(7) NOT NULL DEFAULT '#d4d6d7',
                    check_box_color VARCHAR(7) NOT NULL DEFAULT '#ffffff',
                    FOREIGN KEY (userid) REFERENCES "Users"(userid) ON DELETE CASCADE,
                    UNIQUE(userid, name)
                )
            """)
            conn.commit()
            logger.info("Successfully created CustomThemes table (PostgreSQL)")

        else:  # MySQL/MariaDB
            cursor.execute("""
                SELECT table_name
                FROM information_schema.tables
                WHERE table_schema = DATABASE()
                AND table_name = 'CustomThemes'
            """)
            if cursor.fetchone():
                logger.info("CustomThemes table already exists (MySQL)")
                return
            logger.info("Creating CustomThemes table (MySQL)...")
            cursor.execute("""
                CREATE TABLE CustomThemes (
                    ThemeID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT NOT NULL,
                    Name VARCHAR(255) NOT NULL,
                    BackgroundColor VARCHAR(7) NOT NULL DEFAULT '#3C4252',
                    ButtonColor VARCHAR(7) NOT NULL DEFAULT '#3e4555',
                    ContainerButtonColor VARCHAR(7) NOT NULL DEFAULT '#3C4252',
                    ButtonTextColor VARCHAR(7) NOT NULL DEFAULT '#f6f5f4',
                    TextColor VARCHAR(7) NOT NULL DEFAULT '#f6f5f4',
                    TextSecondaryColor VARCHAR(7) NOT NULL DEFAULT '#f6f5f4',
                    BorderColor VARCHAR(7) NOT NULL DEFAULT '#000000',
                    AccentColor VARCHAR(7) NOT NULL DEFAULT '#6d747f',
                    ProgBarColor VARCHAR(7) NOT NULL DEFAULT '#3550af',
                    ErrorColor VARCHAR(7) NOT NULL DEFAULT '#ff0000',
                    BonusColor VARCHAR(7) NOT NULL DEFAULT '#000000',
                    SecondaryBackground VARCHAR(7) NOT NULL DEFAULT '#2e3440',
                    ContainerBackground VARCHAR(7) NOT NULL DEFAULT '#2b2f3a',
                    StandoutColor VARCHAR(7) NOT NULL DEFAULT '#6e8e92',
                    HoverColor VARCHAR(7) NOT NULL DEFAULT '#5d80aa',
                    LinkColor VARCHAR(7) NOT NULL DEFAULT '#5d80aa',
                    ThumbColor VARCHAR(7) NOT NULL DEFAULT '#3550af',
                    UnfilledColor VARCHAR(7) NOT NULL DEFAULT '#d4d6d7',
                    CheckBoxColor VARCHAR(7) NOT NULL DEFAULT '#ffffff',
                    FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE,
                    UNIQUE KEY unique_user_theme (UserID, Name)
                )
            """)
            conn.commit()
            logger.info("Successfully created CustomThemes table (MySQL)")

    except Exception as e:
        logger.error(f"Error creating CustomThemes table: {e}")
        conn.rollback()
        raise


@register_migration("042", "add_episode_pagination_composite_indexes", "Add composite (PodcastID, EpisodePubDate) indexes so per-podcast paginated scans are index-only", requires=["001"])
def migration_042_add_episode_pagination_composite_indexes(conn, db_type: str):
    """The episode_layout pagination query orders by EpisodePubDate DESC and filters by
    PodcastID. With only single-column indexes Postgres/MySQL must scan and sort. A composite
    (PodcastID, EpisodePubDate DESC) makes it an index-only scan and is the difference between
    sub-100ms and multi-second response times on large podcasts."""
    cursor = conn.cursor()

    try:
        logger.info("Starting episode pagination composite indexes migration")

        table_prefix = '"' if db_type == 'postgresql' else ''
        table_suffix = '"' if db_type == 'postgresql' else ''

        safe_add_index(
            cursor, db_type,
            f'CREATE INDEX idx_episodes_podcastid_pubdate ON {table_prefix}Episodes{table_suffix}(PodcastID, EpisodePubDate DESC)',
            'idx_episodes_podcastid_pubdate'
        )
        safe_add_index(
            cursor, db_type,
            f'CREATE INDEX idx_youtubevideos_podcastid_publishedat ON {table_prefix}YouTubeVideos{table_suffix}(PodcastID, PublishedAt DESC)',
            'idx_youtubevideos_podcastid_publishedat'
        )

        logger.info("Episode pagination composite indexes migration completed successfully")

    except Exception as e:
        logger.error(f"Error in episode pagination composite indexes migration: {e}")
        raise
    finally:
        cursor.close()


@register_migration("043", "add_episode_dedup_indexes", "Add (PodcastID, EpisodeTitle) index to speed up title-based episode deduplication", requires=["001"])
def migration_043_add_episode_dedup_indexes(conn, db_type: str):
    """The episode insertion path uses title-based dedup for episodes without audio URLs.
    A composite (PodcastID, EpisodeTitle) index makes those lookups index-only instead of
    full-podcast table scans."""
    cursor = conn.cursor()

    try:
        logger.info("Starting episode dedup indexes migration")

        table_prefix = '"' if db_type == 'postgresql' else ''
        table_suffix = '"' if db_type == 'postgresql' else ''

        safe_add_index(
            cursor, db_type,
            f'CREATE INDEX idx_episodes_podcastid_title ON {table_prefix}Episodes{table_suffix}(PodcastID, EpisodeTitle)',
            'idx_episodes_podcastid_title'
        )

        logger.info("Episode dedup indexes migration completed successfully")

    except Exception as e:
        logger.error(f"Error in episode dedup indexes migration: {e}")
        raise
    finally:
        cursor.close()


@register_migration("044", "add_feed_refresh_metadata", "Add HTTP caching (ETag/Last-Modified) and refresh failure-tracking columns to Podcasts", requires=["001"])
def migration_044_add_feed_refresh_metadata(conn, db_type: str):
    """Conditional-GET caching and failure backoff for the refresh system.

    FeedETag / FeedLastModified let the refresher send If-None-Match / If-Modified-Since so
    unchanged feeds short-circuit on a 304 instead of being fully re-downloaded and re-parsed.
    LastRefreshAttempt/Success + ConsecutiveFailures + LastErrorMessage let the refresher back
    off feeds that keep failing instead of retrying them at full cost every cycle."""
    cursor = conn.cursor()

    try:
        logger.info("Starting migration to add feed refresh metadata columns to Podcasts...")

        if db_type == "postgresql":
            columns = [
                ("feedetag", "TEXT"),
                ("feedlastmodified", "TEXT"),
                ("lastrefreshattempt", "TIMESTAMP"),
                ("lastrefreshsuccess", "TIMESTAMP"),
                ("consecutivefailures", "INT DEFAULT 0"),
                ("lasterrormessage", "TEXT"),
            ]
            for col_name, col_def in columns:
                cursor.execute(
                    """
                    SELECT column_name FROM information_schema.columns
                    WHERE table_name = 'Podcasts' AND column_name = %s
                    """,
                    (col_name,),
                )
                if cursor.fetchone():
                    logger.info(f"Column {col_name} already exists in Podcasts (PostgreSQL)")
                    continue
                cursor.execute(f'ALTER TABLE "Podcasts" ADD COLUMN {col_name} {col_def}')
                logger.info(f"Added column {col_name} to Podcasts (PostgreSQL)")
            conn.commit()

        else:  # MySQL/MariaDB
            columns = [
                ("FeedETag", "TEXT"),
                ("FeedLastModified", "TEXT"),
                ("LastRefreshAttempt", "DATETIME NULL DEFAULT NULL"),
                ("LastRefreshSuccess", "DATETIME NULL DEFAULT NULL"),
                ("ConsecutiveFailures", "INT DEFAULT 0"),
                ("LastErrorMessage", "TEXT"),
            ]
            for col_name, col_def in columns:
                cursor.execute(
                    """
                    SELECT COLUMN_NAME FROM INFORMATION_SCHEMA.COLUMNS
                    WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'Podcasts' AND COLUMN_NAME = %s
                    """,
                    (col_name,),
                )
                if cursor.fetchone():
                    logger.info(f"Column {col_name} already exists in Podcasts (MySQL)")
                    continue
                cursor.execute(f"ALTER TABLE Podcasts ADD COLUMN {col_name} {col_def}")
                logger.info(f"Added column {col_name} to Podcasts (MySQL)")
            conn.commit()

        logger.info("Feed refresh metadata migration completed successfully")

    except Exception as e:
        logger.error(f"Error adding feed refresh metadata columns to Podcasts: {e}")
        conn.rollback()
        raise
    finally:
        cursor.close()


@register_migration("045", "add_episode_guid", "Add EpisodeGUID column + index to Episodes for stable RSS GUID-based deduplication", requires=["001"])
def migration_045_add_episode_guid(conn, db_type: str):
    """RSS <guid> is the canonical, stable identity of an episode. Dedup previously relied on
    audio-URL base / title only, which duplicates episodes when a feed migrates CDNs (URL change)
    and collapses distinct episodes that share a title. Storing the feed GUID lets the refresher
    dedup on GUID first, falling back to URL/title for legacy rows that have a NULL guid."""
    cursor = conn.cursor()

    try:
        logger.info("Starting migration to add EpisodeGUID column to Episodes...")

        if db_type == "postgresql":
            cursor.execute(
                """
                SELECT column_name FROM information_schema.columns
                WHERE table_name = 'Episodes' AND column_name = 'episodeguid'
                """
            )
            if not cursor.fetchone():
                cursor.execute('ALTER TABLE "Episodes" ADD COLUMN episodeguid TEXT')
                logger.info("Added episodeguid column to Episodes (PostgreSQL)")
            else:
                logger.info("episodeguid column already exists in Episodes (PostgreSQL)")
            conn.commit()

            safe_add_index(
                cursor, db_type,
                'CREATE INDEX idx_episodes_podcastid_guid ON "Episodes"(PodcastID, EpisodeGUID)',
                'idx_episodes_podcastid_guid'
            )
            conn.commit()

        else:  # MySQL/MariaDB
            cursor.execute(
                """
                SELECT COLUMN_NAME FROM INFORMATION_SCHEMA.COLUMNS
                WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'Episodes' AND COLUMN_NAME = 'EpisodeGUID'
                """
            )
            if not cursor.fetchone():
                cursor.execute("ALTER TABLE Episodes ADD COLUMN EpisodeGUID TEXT")
                logger.info("Added EpisodeGUID column to Episodes (MySQL)")
            else:
                logger.info("EpisodeGUID column already exists in Episodes (MySQL)")
            conn.commit()

            # MySQL cannot index a full TEXT column; use a prefix length.
            safe_add_index(
                cursor, db_type,
                'CREATE INDEX idx_episodes_podcastid_guid ON Episodes(PodcastID, EpisodeGUID(255))',
                'idx_episodes_podcastid_guid'
            )
            conn.commit()

        logger.info("Episode GUID migration completed successfully")

    except Exception as e:
        logger.error(f"Error adding EpisodeGUID column to Episodes: {e}")
        conn.rollback()
        raise
    finally:
        cursor.close()


@register_migration("046", "add_home_overview_history_index", "Add composite (UserID, ListenDate) index on UserEpisodeHistory for the home overview in-progress sort and weekly-stats window", requires=["006"])
def migration_046_add_home_overview_history_index(conn, db_type: str):
    """The home overview's in-progress section orders UserEpisodeHistory by ListenDate DESC
    per user, and the new weekly-stats widget sums listenduration over a 7-day ListenDate
    window. Existing UserEpisodeHistory indexes cover (UserID) and (UserID, EpisodeID) but
    not ListenDate, forcing a filter+sort. A composite (UserID, ListenDate DESC) makes both
    the recent-history sort and the time-window scan index-friendly."""
    cursor = conn.cursor()

    try:
        logger.info("Starting home overview history index migration")

        table_prefix = '"' if db_type == 'postgresql' else ''
        table_suffix = '"' if db_type == 'postgresql' else ''

        safe_add_index(
            cursor, db_type,
            f'CREATE INDEX idx_userepisodehistory_userid_listendate ON {table_prefix}UserEpisodeHistory{table_suffix}(UserID, ListenDate DESC)',
            'idx_userepisodehistory_userid_listendate'
        )

        logger.info("Home overview history index migration completed successfully")

    except Exception as e:
        logger.error(f"Error in home overview history index migration: {e}")
        raise
    finally:
        cursor.close()


@register_migration("047", "add_scheduled_backup_retention", "Add retention_count and last_run columns to ScheduledBackups for working scheduled backups + retention", requires=["027"])
def migration_047_add_scheduled_backup_retention(conn, db_type: str):
    """Scheduled backups now actually run via the background scheduler, which needs a
    last_run timestamp to decide when a schedule is due, and an optional retention_count
    so old scheduled backups can be pruned automatically (NULL/0 = keep all)."""
    logger.info("Starting migration 047: Add retention/last_run columns to ScheduledBackups")
    cursor = conn.cursor()

    try:
        if db_type == 'postgresql':
            safe_execute_sql(cursor, '''
                ALTER TABLE "ScheduledBackups"
                ADD COLUMN IF NOT EXISTS retention_count INTEGER
            ''', conn=conn)
            safe_execute_sql(cursor, '''
                ALTER TABLE "ScheduledBackups"
                ADD COLUMN IF NOT EXISTS last_run TIMESTAMP
            ''', conn=conn)
            logger.info("Added retention_count and last_run columns to ScheduledBackups (PostgreSQL)")

        else:  # MySQL
            for column_name, column_def in (
                ('RetentionCount', 'INT NULL'),
                ('LastRun', 'TIMESTAMP NULL'),
            ):
                safe_execute_sql(cursor, f'''
                    SELECT COUNT(*)
                    FROM information_schema.columns
                    WHERE table_name = 'ScheduledBackups'
                    AND column_name = '{column_name}'
                    AND table_schema = DATABASE()
                ''', conn=conn)

                result = cursor.fetchone()
                if result[0] == 0:
                    safe_execute_sql(cursor, f'''
                        ALTER TABLE ScheduledBackups
                        ADD COLUMN {column_name} {column_def}
                    ''', conn=conn)
                    logger.info(f"Added {column_name} column to ScheduledBackups (MySQL)")
                else:
                    logger.info(f"{column_name} column already exists in ScheduledBackups (MySQL)")

        logger.info("ScheduledBackups retention/last_run migration completed successfully")

    except Exception as e:
        logger.error(f"Error in ScheduledBackups retention/last_run migration: {e}")
        raise
    finally:
        cursor.close()


@register_migration("048", "create_collections_tables", "Create Collections + CollectionEpisodes tables and a per-user default 'Saved' collection", requires=["010"])
def migration_048_create_collections_tables(conn, db_type: str):
    """Collections evolve the flat Saved page into manual, user-curated lists.

    The legacy "Saved" bucket becomes a pinned, undeletable default collection per user.
    Episodes can belong to multiple collections at once, and both podcast episodes and
    YouTube videos are supported via the same dual-FK CHECK pattern used by PlaylistContents.

    Note: the default ("Saved") collection keeps SavedEpisodes/SavedVideos as its backing
    store — we do NOT copy those rows into CollectionEpisodes. CollectionEpisodes only ever
    holds membership rows for non-default collections.
    """
    logger.info("Starting migration 048: Create Collections tables")
    cursor = conn.cursor()

    try:
        if db_type == "postgresql":
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "Collections" (
                    CollectionID SERIAL PRIMARY KEY,
                    UserID INT NOT NULL,
                    Name VARCHAR(255) NOT NULL,
                    Description TEXT,
                    IsDefault BOOLEAN NOT NULL DEFAULT FALSE,
                    Icon VARCHAR(50) NOT NULL DEFAULT 'ph-bookmark-simple',
                    CreatedAt TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    LastUpdated TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE,
                    UNIQUE(UserID, Name)
                )
            """)

            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "CollectionEpisodes" (
                    CollectionEpisodeID SERIAL PRIMARY KEY,
                    CollectionID INT NOT NULL,
                    EpisodeID INT,
                    VideoID INT,
                    AddedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (CollectionID) REFERENCES "Collections"(CollectionID) ON DELETE CASCADE,
                    FOREIGN KEY (EpisodeID) REFERENCES "Episodes"(EpisodeID) ON DELETE CASCADE,
                    FOREIGN KEY (VideoID) REFERENCES "YouTubeVideos"(VideoID) ON DELETE CASCADE,
                    CHECK ((EpisodeID IS NOT NULL AND VideoID IS NULL) OR (EpisodeID IS NULL AND VideoID IS NOT NULL))
                )
            """)

            # Indexes for fast per-collection scans and membership lookups
            cursor.execute("""
                CREATE INDEX IF NOT EXISTS idx_collections_userid ON "Collections"(UserID);
                CREATE INDEX IF NOT EXISTS idx_collection_episodes_collectionid ON "CollectionEpisodes"(CollectionID);
                CREATE INDEX IF NOT EXISTS idx_collection_episodes_episodeid ON "CollectionEpisodes"(EpisodeID);
                CREATE INDEX IF NOT EXISTS idx_collection_episodes_videoid ON "CollectionEpisodes"(VideoID);
            """)

            # Enforce exactly one default collection per user (Postgres partial unique index)
            cursor.execute("""
                CREATE UNIQUE INDEX IF NOT EXISTS idx_collections_one_default_per_user
                ON "Collections"(UserID) WHERE IsDefault = TRUE
            """)

            # Seed a default "Saved" collection for every existing user (idempotent)
            cursor.execute("""
                INSERT INTO "Collections" (UserID, Name, IsDefault, Icon)
                SELECT u.UserID, 'Saved', TRUE, 'ph-bookmark-simple'
                FROM "Users" u
                WHERE NOT EXISTS (
                    SELECT 1 FROM "Collections" c
                    WHERE c.UserID = u.UserID AND c.IsDefault = TRUE
                )
            """)
        else:  # MySQL / MariaDB
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS Collections (
                    CollectionID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT NOT NULL,
                    Name VARCHAR(255) NOT NULL,
                    Description TEXT,
                    IsDefault BOOLEAN NOT NULL DEFAULT FALSE,
                    Icon VARCHAR(50) NOT NULL DEFAULT 'ph-bookmark-simple',
                    CreatedAt TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    LastUpdated TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE,
                    UNIQUE(UserID, Name)
                )
            """)

            cursor.execute("""
                CREATE TABLE IF NOT EXISTS CollectionEpisodes (
                    CollectionEpisodeID INT AUTO_INCREMENT PRIMARY KEY,
                    CollectionID INT NOT NULL,
                    EpisodeID INT,
                    VideoID INT,
                    AddedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (CollectionID) REFERENCES Collections(CollectionID) ON DELETE CASCADE,
                    FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID) ON DELETE CASCADE,
                    FOREIGN KEY (VideoID) REFERENCES YouTubeVideos(VideoID) ON DELETE CASCADE,
                    CHECK ((EpisodeID IS NOT NULL AND VideoID IS NULL) OR (EpisodeID IS NULL AND VideoID IS NOT NULL))
                )
            """)

            # Create indexes for better performance (MySQL doesn't support IF NOT EXISTS for indexes)
            try:
                cursor.execute("CREATE INDEX idx_collections_userid ON Collections(UserID)")
            except:
                pass  # Index may already exist
            try:
                cursor.execute("CREATE INDEX idx_collection_episodes_collectionid ON CollectionEpisodes(CollectionID)")
            except:
                pass  # Index may already exist
            try:
                cursor.execute("CREATE INDEX idx_collection_episodes_episodeid ON CollectionEpisodes(EpisodeID)")
            except:
                pass  # Index may already exist
            try:
                cursor.execute("CREATE INDEX idx_collection_episodes_videoid ON CollectionEpisodes(VideoID)")
            except:
                pass  # Index may already exist

            # MySQL/MariaDB has no partial unique index — "one default per user" is enforced
            # by the idempotent seed below plus application logic in the API layer.
            cursor.execute("""
                INSERT INTO Collections (UserID, Name, IsDefault, Icon)
                SELECT u.UserID, 'Saved', TRUE, 'ph-bookmark-simple'
                FROM Users u
                WHERE NOT EXISTS (
                    SELECT 1 FROM Collections c
                    WHERE c.UserID = u.UserID AND c.IsDefault = TRUE
                )
            """)

        logger.info("Created Collections tables and seeded default collections")

    except Exception as e:
        logger.error(f"Error in Collections tables migration: {e}")
        raise
    finally:
        cursor.close()


@register_migration("049", "add_collection_add_ui_to_usersettings", "Add collection_add_ui preference column to UserSettings", requires=["003"])
def migration_049_add_collection_add_ui(conn, db_type: str):
    """Drives the 'Add to Collection' UX choice: 'modal' (default) picker or 'submenu' flyout."""
    logger.info("Starting migration 049: Add collection_add_ui column to UserSettings")
    cursor = conn.cursor()

    try:
        if db_type == 'postgresql':
            cursor.execute("""
                SELECT COUNT(*)
                FROM information_schema.columns
                WHERE table_name = 'UserSettings'
                AND column_name = 'collection_add_ui'
                AND table_schema = 'public'
            """)
            column_exists = cursor.fetchone()[0] > 0

            if not column_exists:
                cursor.execute("""
                    ALTER TABLE "UserSettings"
                    ADD COLUMN collection_add_ui VARCHAR(10) DEFAULT 'modal'
                """)
                logger.info("Added collection_add_ui column to UserSettings table (PostgreSQL)")
            else:
                logger.info("collection_add_ui column already exists in UserSettings table (PostgreSQL)")
        else:  # MySQL
            cursor.execute("""
                SELECT COUNT(*)
                FROM information_schema.columns
                WHERE table_name = 'UserSettings'
                AND column_name = 'CollectionAddUI'
                AND table_schema = DATABASE()
            """)
            column_exists = cursor.fetchone()[0] > 0

            if not column_exists:
                cursor.execute("""
                    ALTER TABLE UserSettings
                    ADD COLUMN CollectionAddUI VARCHAR(10) DEFAULT 'modal'
                """)
                logger.info("Added CollectionAddUI column to UserSettings table (MySQL)")
            else:
                logger.info("CollectionAddUI column already exists in UserSettings table (MySQL)")

        logger.info("collection_add_ui migration completed successfully")

    except Exception as e:
        logger.error(f"Error in collection_add_ui migration: {e}")
        raise
    finally:
        cursor.close()


@register_migration("050", "create_skip_segments_and_silence_settings", "Create EpisodeSkipSegments table and add per-podcast silence-trim settings", requires=["001", "005"])
def migration_050_create_skip_segments(conn, db_type: str):
    """Server-side, multi-range skip model (foundation for silence-trim #727, later ad-skip #790).

    EpisodeSkipSegments holds content-level (per-episode, NOT per-user) time ranges the player
    should auto-skip. Because a given episode's audio is identical for every subscriber, the
    segments are computed once and shared across all users — users only opt in to *applying* a
    given Kind. It mirrors the dual-FK (EpisodeID/VideoID) CHECK pattern used by
    CollectionEpisodes/PlaylistContents so podcast episodes and YouTube videos share one table.

    Per-podcast controls live on Podcasts (matching the existing PlaybackSpeed/StartSkip pattern):
      TrimSilence      - opt-in toggle to auto-detect silence on new episodes of this podcast
      SilenceThreshold - aggressiveness preset (1=low, 2=medium, 3=high); default medium

    Episodes.SilenceDetected marks that silence detection has already run for an episode, so we
    don't re-analyze the same file on every playback (absence of segment rows is otherwise
    indistinguishable from "not yet analyzed")."""
    logger.info("Starting migration 050: Create EpisodeSkipSegments + silence-trim settings")
    cursor = conn.cursor()

    try:
        if db_type == "postgresql":
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "EpisodeSkipSegments" (
                    SegmentID SERIAL PRIMARY KEY,
                    EpisodeID INT,
                    VideoID INT,
                    Kind VARCHAR(20) NOT NULL,
                    StartTime DOUBLE PRECISION NOT NULL,
                    EndTime DOUBLE PRECISION NOT NULL,
                    Source VARCHAR(30) NOT NULL,
                    CreatedAt TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (EpisodeID) REFERENCES "Episodes"(EpisodeID) ON DELETE CASCADE,
                    FOREIGN KEY (VideoID) REFERENCES "YouTubeVideos"(VideoID) ON DELETE CASCADE,
                    CHECK ((EpisodeID IS NOT NULL AND VideoID IS NULL) OR (EpisodeID IS NULL AND VideoID IS NOT NULL))
                )
            """)
            cursor.execute("""
                CREATE INDEX IF NOT EXISTS idx_skip_segments_episodeid ON "EpisodeSkipSegments"(EpisodeID);
                CREATE INDEX IF NOT EXISTS idx_skip_segments_videoid ON "EpisodeSkipSegments"(VideoID);
            """)

            # Per-podcast silence-trim controls
            for col_name, col_def in (
                ("trimsilence", "BOOLEAN DEFAULT FALSE"),
                ("silencethreshold", "INT DEFAULT 2"),
            ):
                cursor.execute(
                    """
                    SELECT column_name FROM information_schema.columns
                    WHERE table_name = 'Podcasts' AND column_name = %s
                    """,
                    (col_name,),
                )
                if not cursor.fetchone():
                    cursor.execute(f'ALTER TABLE "Podcasts" ADD COLUMN {col_name} {col_def}')
                    logger.info(f"Added column {col_name} to Podcasts (PostgreSQL)")

            # Detection-complete marker so we don't re-analyze on every playback
            cursor.execute("""
                SELECT column_name FROM information_schema.columns
                WHERE table_name = 'Episodes' AND column_name = 'silencedetected'
            """)
            if not cursor.fetchone():
                cursor.execute('ALTER TABLE "Episodes" ADD COLUMN silencedetected BOOLEAN DEFAULT FALSE')
                logger.info("Added silencedetected column to Episodes (PostgreSQL)")

        else:  # MySQL / MariaDB
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS EpisodeSkipSegments (
                    SegmentID INT AUTO_INCREMENT PRIMARY KEY,
                    EpisodeID INT,
                    VideoID INT,
                    Kind VARCHAR(20) NOT NULL,
                    StartTime DOUBLE NOT NULL,
                    EndTime DOUBLE NOT NULL,
                    Source VARCHAR(30) NOT NULL,
                    CreatedAt TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID) ON DELETE CASCADE,
                    FOREIGN KEY (VideoID) REFERENCES YouTubeVideos(VideoID) ON DELETE CASCADE,
                    CHECK ((EpisodeID IS NOT NULL AND VideoID IS NULL) OR (EpisodeID IS NULL AND VideoID IS NOT NULL))
                )
            """)
            for idx_sql in (
                "CREATE INDEX idx_skip_segments_episodeid ON EpisodeSkipSegments(EpisodeID)",
                "CREATE INDEX idx_skip_segments_videoid ON EpisodeSkipSegments(VideoID)",
            ):
                try:
                    cursor.execute(idx_sql)
                except Exception:
                    pass  # Index may already exist

            for table, col_name, col_def in (
                ("Podcasts", "TrimSilence", "BOOLEAN DEFAULT FALSE"),
                ("Podcasts", "SilenceThreshold", "INT DEFAULT 2"),
                ("Episodes", "SilenceDetected", "BOOLEAN DEFAULT FALSE"),
            ):
                cursor.execute(
                    """
                    SELECT COLUMN_NAME FROM INFORMATION_SCHEMA.COLUMNS
                    WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = %s AND COLUMN_NAME = %s
                    """,
                    (table, col_name),
                )
                if not cursor.fetchone():
                    cursor.execute(f"ALTER TABLE {table} ADD COLUMN {col_name} {col_def}")
                    logger.info(f"Added column {col_name} to {table} (MySQL)")

        logger.info("Skip segments + silence-trim settings migration completed successfully")

    except Exception as e:
        logger.error(f"Error in skip segments migration: {e}")
        raise
    finally:
        cursor.close()


@register_migration("051", "create_episode_transcripts", "Create EpisodeTranscripts table and add per-podcast AutoTranscribe", requires=["001", "005"])
def migration_051_create_episode_transcripts(conn, db_type: str):
    """Persistent store for generated (and, optionally, cached feed) transcripts (#726).

    Today transcripts are never stored — they're parsed live from the podcast:transcript RSS
    tag on every view. This table lets the AI sidecar's speech-to-text output be saved once and
    reused. Like EpisodeSkipSegments, rows are content-level (per episode, NOT per user), since
    an episode's audio/transcript is identical for every subscriber; it mirrors the dual-FK
    (EpisodeID/VideoID) CHECK pattern for podcast episodes vs YouTube videos.

    Columns: Source ('generated'|'feed'), Language, Model (which whisper model produced it),
    TranscriptText (the whole transcript, for search later; 'FullText' avoided as it is a MySQL
    reserved word), Segments (JSON: [{start,end,text}]),
    Status ('pending'|'running'|'complete'|'failed') to reflect the async pipeline.

    Podcasts.AutoTranscribe is the per-podcast opt-in for auto-transcribing new episodes
    (default FALSE — transcription is never on by default; also triggerable manually per episode)."""
    logger.info("Starting migration 051: Create EpisodeTranscripts + AutoTranscribe")
    cursor = conn.cursor()

    try:
        if db_type == "postgresql":
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "EpisodeTranscripts" (
                    TranscriptID SERIAL PRIMARY KEY,
                    EpisodeID INT,
                    VideoID INT,
                    Source VARCHAR(20) NOT NULL DEFAULT 'generated',
                    Language VARCHAR(20),
                    Model VARCHAR(100),
                    TranscriptText TEXT,
                    Segments JSONB,
                    Status VARCHAR(20) NOT NULL DEFAULT 'complete',
                    CreatedAt TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (EpisodeID) REFERENCES "Episodes"(EpisodeID) ON DELETE CASCADE,
                    FOREIGN KEY (VideoID) REFERENCES "YouTubeVideos"(VideoID) ON DELETE CASCADE,
                    CHECK ((EpisodeID IS NOT NULL AND VideoID IS NULL) OR (EpisodeID IS NULL AND VideoID IS NOT NULL))
                )
            """)
            cursor.execute("""
                CREATE INDEX IF NOT EXISTS idx_episode_transcripts_episodeid ON "EpisodeTranscripts"(EpisodeID);
                CREATE INDEX IF NOT EXISTS idx_episode_transcripts_videoid ON "EpisodeTranscripts"(VideoID);
            """)

            cursor.execute("""
                SELECT column_name FROM information_schema.columns
                WHERE table_name = 'Podcasts' AND column_name = 'autotranscribe'
            """)
            if not cursor.fetchone():
                cursor.execute('ALTER TABLE "Podcasts" ADD COLUMN autotranscribe BOOLEAN DEFAULT FALSE')
                logger.info("Added autotranscribe column to Podcasts (PostgreSQL)")

        else:  # MySQL / MariaDB
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS EpisodeTranscripts (
                    TranscriptID INT AUTO_INCREMENT PRIMARY KEY,
                    EpisodeID INT,
                    VideoID INT,
                    Source VARCHAR(20) NOT NULL DEFAULT 'generated',
                    Language VARCHAR(20),
                    Model VARCHAR(100),
                    TranscriptText LONGTEXT,
                    Segments JSON,
                    Status VARCHAR(20) NOT NULL DEFAULT 'complete',
                    CreatedAt TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID) ON DELETE CASCADE,
                    FOREIGN KEY (VideoID) REFERENCES YouTubeVideos(VideoID) ON DELETE CASCADE,
                    CHECK ((EpisodeID IS NOT NULL AND VideoID IS NULL) OR (EpisodeID IS NULL AND VideoID IS NOT NULL))
                )
            """)
            for idx_sql in (
                "CREATE INDEX idx_episode_transcripts_episodeid ON EpisodeTranscripts(EpisodeID)",
                "CREATE INDEX idx_episode_transcripts_videoid ON EpisodeTranscripts(VideoID)",
            ):
                try:
                    cursor.execute(idx_sql)
                except Exception:
                    pass  # Index may already exist

            cursor.execute("""
                SELECT COLUMN_NAME FROM INFORMATION_SCHEMA.COLUMNS
                WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'Podcasts' AND COLUMN_NAME = 'AutoTranscribe'
            """)
            if not cursor.fetchone():
                cursor.execute("ALTER TABLE Podcasts ADD COLUMN AutoTranscribe BOOLEAN DEFAULT FALSE")
                logger.info("Added AutoTranscribe column to Podcasts (MySQL)")

        logger.info("EpisodeTranscripts migration completed successfully")

    except Exception as e:
        logger.error(f"Error in EpisodeTranscripts migration: {e}")
        raise
    finally:
        cursor.close()


@register_migration("052", "add_auto_download_delete_columns", "Add auto-delete-downloads retention columns to Users and Podcasts tables (#655)", requires=["001", "005"])
def add_auto_download_delete_columns(conn, db_type: str) -> None:
    """Add server-download retention columns for existing installations (#655).

    A per-user global default (Users.AutoDownloadDeleteDays) plus a per-podcast override
    (Podcasts.AutoDownloadDeleteDays + AutoDownloadDeleteCustomized), mirroring the existing
    PlaybackSpeed/PlaybackSpeedCustomized pattern. A value of 0 means "never auto-delete";
    N means "delete server downloads older than N days". The AutoDownloadDeleteCustomized
    boolean (not a NULL check) is the discriminator for whether the podcast overrides the
    user default."""
    logger.info("Starting migration 052: add auto-download-delete columns")
    cursor = conn.cursor()

    try:
        # Add AutoDownloadDeleteDays to Users table if it doesn't exist
        try:
            if db_type == "postgresql":
                cursor.execute("""
                    ALTER TABLE "Users"
                    ADD COLUMN IF NOT EXISTS AutoDownloadDeleteDays INT DEFAULT 0
                """)
            else:  # MySQL/MariaDB
                cursor.execute("""
                    SELECT COUNT(*)
                    FROM INFORMATION_SCHEMA.COLUMNS
                    WHERE TABLE_SCHEMA = DATABASE()
                    AND TABLE_NAME = 'Users'
                    AND COLUMN_NAME = 'AutoDownloadDeleteDays'
                """)
                if cursor.fetchone()[0] == 0:
                    cursor.execute("""
                        ALTER TABLE Users
                        ADD COLUMN AutoDownloadDeleteDays INT DEFAULT 0
                    """)
                    logger.info("Added AutoDownloadDeleteDays column to Users table")
                else:
                    logger.info("AutoDownloadDeleteDays column already exists in Users table")
        except Exception as e:
            logger.error(f"Error adding AutoDownloadDeleteDays to Users table: {e}")

        # Add AutoDownloadDeleteDays + AutoDownloadDeleteCustomized to Podcasts table
        try:
            if db_type == "postgresql":
                cursor.execute("""
                    ALTER TABLE "Podcasts"
                    ADD COLUMN IF NOT EXISTS AutoDownloadDeleteDays INT DEFAULT 0,
                    ADD COLUMN IF NOT EXISTS AutoDownloadDeleteCustomized BOOLEAN DEFAULT FALSE
                """)
            else:  # MySQL/MariaDB
                cursor.execute("""
                    SELECT COUNT(*)
                    FROM INFORMATION_SCHEMA.COLUMNS
                    WHERE TABLE_SCHEMA = DATABASE()
                    AND TABLE_NAME = 'Podcasts'
                    AND COLUMN_NAME = 'AutoDownloadDeleteDays'
                """)
                if cursor.fetchone()[0] == 0:
                    cursor.execute("""
                        ALTER TABLE Podcasts
                        ADD COLUMN AutoDownloadDeleteDays INT DEFAULT 0
                    """)
                    logger.info("Added AutoDownloadDeleteDays column to Podcasts table")
                else:
                    logger.info("AutoDownloadDeleteDays column already exists in Podcasts table")

                cursor.execute("""
                    SELECT COUNT(*)
                    FROM INFORMATION_SCHEMA.COLUMNS
                    WHERE TABLE_SCHEMA = DATABASE()
                    AND TABLE_NAME = 'Podcasts'
                    AND COLUMN_NAME = 'AutoDownloadDeleteCustomized'
                """)
                if cursor.fetchone()[0] == 0:
                    cursor.execute("""
                        ALTER TABLE Podcasts
                        ADD COLUMN AutoDownloadDeleteCustomized TINYINT(1) DEFAULT 0
                    """)
                    logger.info("Added AutoDownloadDeleteCustomized column to Podcasts table")
                else:
                    logger.info("AutoDownloadDeleteCustomized column already exists in Podcasts table")
        except Exception as e:
            logger.error(f"Error adding auto-delete columns to Podcasts table: {e}")

        logger.info("Auto-download-delete columns migration completed")

    finally:
        cursor.close()


@register_migration("053", "add_auto_queue_to_podcasts", "Add AutoQueue column to Podcasts table for auto-adding new episodes to the play queue (#648)", requires=["005"])
def add_auto_queue_to_podcasts(conn, db_type: str) -> None:
    """Add the per-podcast AutoQueue flag for existing installations (#648).

    When enabled, newly-discovered episodes for the podcast are automatically appended to
    the owning user's play queue during refresh. Mirrors the existing per-podcast
    AutoDownload flag. Default FALSE (opt-in)."""
    logger.info("Starting migration 053: add AutoQueue column to Podcasts")
    cursor = conn.cursor()

    try:
        if db_type == "postgresql":
            cursor.execute("""
                ALTER TABLE "Podcasts"
                ADD COLUMN IF NOT EXISTS AutoQueue BOOLEAN DEFAULT FALSE
            """)
        else:  # MySQL/MariaDB
            cursor.execute("""
                SELECT COUNT(*)
                FROM INFORMATION_SCHEMA.COLUMNS
                WHERE TABLE_SCHEMA = DATABASE()
                AND TABLE_NAME = 'Podcasts'
                AND COLUMN_NAME = 'AutoQueue'
            """)
            if cursor.fetchone()[0] == 0:
                cursor.execute("""
                    ALTER TABLE Podcasts
                    ADD COLUMN AutoQueue TINYINT(1) DEFAULT 0
                """)
                logger.info("Added AutoQueue column to Podcasts table")
            else:
                logger.info("AutoQueue column already exists in Podcasts table")

        logger.info("AutoQueue column migration completed")

    finally:
        cursor.close()


@register_migration("054", "add_default_volume_to_users", "Add DefaultVolume column to Users table for per-user default playback volume (#828)", requires=["001"])
def add_default_volume_to_users(conn, db_type: str) -> None:
    """Add the per-user default playback volume column for existing installations (#828).

    Volume is stored as an integer percentage 0-100 on the Users table, mirroring the
    existing per-user PlaybackSpeed setting. Default 100 preserves today's behavior
    (episodes start at full volume) for existing users."""
    logger.info("Starting migration 054: add DefaultVolume column to Users")
    cursor = conn.cursor()

    try:
        if db_type == "postgresql":
            cursor.execute("""
                ALTER TABLE "Users"
                ADD COLUMN IF NOT EXISTS DefaultVolume INT DEFAULT 100
            """)
        else:  # MySQL/MariaDB
            cursor.execute("""
                SELECT COUNT(*)
                FROM INFORMATION_SCHEMA.COLUMNS
                WHERE TABLE_SCHEMA = DATABASE()
                AND TABLE_NAME = 'Users'
                AND COLUMN_NAME = 'DefaultVolume'
            """)
            if cursor.fetchone()[0] == 0:
                cursor.execute("""
                    ALTER TABLE Users
                    ADD COLUMN DefaultVolume INT DEFAULT 100
                """)
                logger.info("Added DefaultVolume column to Users table")
            else:
                logger.info("DefaultVolume column already exists in Users table")

        logger.info("DefaultVolume column migration completed")

    finally:
        cursor.close()


@register_migration("055", "add_download_metadata_columns", "Add server-download sidecar/metadata columns to AppSettings (#451, #533, #658)", requires=["001"])
def add_download_metadata_columns(conn, db_type: str) -> None:
    """Add admin-controlled download-metadata options for existing installations.

    These global AppSettings toggles control the extra files written to the server
    download tree: the podcast cover as folder.jpg (#658), an episode cover sidecar
    image, and an episode metadata sidecar (#451). All file-writing options default
    OFF so existing installs are unchanged; the metadata format defaults to 'both'
    (JSON + XML) and sidecars default to a metadata/ subfolder. The always-on ID3
    feed-URL/description enrichment (#533) needs no column."""
    logger.info("Starting migration 055: add download-metadata columns to AppSettings")
    cursor = conn.cursor()

    # (column_name, postgres_type_default, mysql_type_default)
    columns = [
        ("DownloadFolderCover", "BOOLEAN DEFAULT FALSE", "TINYINT(1) DEFAULT 0"),
        ("DownloadEpisodeCover", "BOOLEAN DEFAULT FALSE", "TINYINT(1) DEFAULT 0"),
        ("DownloadMetadataSidecar", "BOOLEAN DEFAULT FALSE", "TINYINT(1) DEFAULT 0"),
        ("DownloadMetadataFormat", "VARCHAR(20) DEFAULT 'both'", "VARCHAR(20) DEFAULT 'both'"),
        ("DownloadMetadataSubfolder", "BOOLEAN DEFAULT TRUE", "TINYINT(1) DEFAULT 1"),
    ]

    try:
        for name, pg_def, my_def in columns:
            try:
                if db_type == "postgresql":
                    cursor.execute(
                        f'ALTER TABLE "AppSettings" ADD COLUMN IF NOT EXISTS {name} {pg_def}'
                    )
                else:  # MySQL/MariaDB
                    cursor.execute(
                        """
                        SELECT COUNT(*)
                        FROM INFORMATION_SCHEMA.COLUMNS
                        WHERE TABLE_SCHEMA = DATABASE()
                        AND TABLE_NAME = 'AppSettings'
                        AND COLUMN_NAME = %s
                        """,
                        (name,),
                    )
                    if cursor.fetchone()[0] == 0:
                        cursor.execute(f"ALTER TABLE AppSettings ADD COLUMN {name} {my_def}")
                        logger.info(f"Added {name} column to AppSettings table")
                    else:
                        logger.info(f"{name} column already exists in AppSettings table")
            except Exception as e:
                logger.error(f"Error adding {name} to AppSettings table: {e}")

        logger.info("Download-metadata columns migration completed")

    finally:
        cursor.close()


@register_migration("056", "create_ad_detection_and_ai_settings", "Ad-detection: EpisodeAdSkipReview + per-podcast ad toggles + Episodes.AdsDetected + global AISettings (#790)", requires=["001", "050", "051"])
def migration_056_create_ad_detection_and_ai_settings(conn, db_type: str):
    """AI ad-detection over transcripts (#790), built on the #726/#727 foundation.

    Detected ads reuse the existing content-level EpisodeSkipSegments table with
    Kind='ad'/Source='auto-ad' (shared across subscribers — detect once). Because this
    is a multi-user app, the *review/skip* decision is per-user, so EpisodeAdSkipReview
    stores each user's per-segment override ('confirmed'|'rejected'); absence of a row
    falls back to the podcast's AdSkipAutoActivate default.

    Per-podcast controls (per-user, like AutoTranscribe/TrimSilence):
      AutoAdDetect       - opt-in to auto-detect ads on new episodes (only meaningful
                           when AutoTranscribe is also on — ads need the transcript)
      AdSkipAutoActivate - are detected ads skipped immediately (TRUE) or held pending
                           user confirmation on the episode page (FALSE); default TRUE

    Episodes.AdsDetected marks that ad detection has already run for an episode (its own
    guard, since Episodes.SilenceDetected is silence-specific).

    AISettings is a singleton (AISettingsID=1, like EmailSettings) holding admin-level
    global AI config resolved per-request into sidecar calls: the whisper transcription
    model, and the LLM backend used for ad detection (local bundled GGUF vs a remote
    OpenAI-compatible endpoint). LlmApiKey is stored encrypted like other secrets."""
    logger.info("Starting migration 056: ad-detection tables/columns + AISettings")
    cursor = conn.cursor()

    try:
        if db_type == "postgresql":
            # Per-user review override of a shared ad segment
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "EpisodeAdSkipReview" (
                    ReviewID SERIAL PRIMARY KEY,
                    UserID INT NOT NULL,
                    SegmentID INT NOT NULL,
                    Status VARCHAR(20) NOT NULL,
                    CreatedAt TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE,
                    FOREIGN KEY (SegmentID) REFERENCES "EpisodeSkipSegments"(SegmentID) ON DELETE CASCADE,
                    UNIQUE (UserID, SegmentID)
                )
            """)
            cursor.execute("""
                CREATE INDEX IF NOT EXISTS idx_ad_skip_review_segmentid ON "EpisodeAdSkipReview"(SegmentID);
                CREATE INDEX IF NOT EXISTS idx_ad_skip_review_userid ON "EpisodeAdSkipReview"(UserID);
            """)

            # Per-podcast ad controls
            for col_name, col_def in (
                ("autoaddetect", "BOOLEAN DEFAULT FALSE"),
                ("adskipautoactivate", "BOOLEAN DEFAULT TRUE"),
            ):
                cursor.execute(f'ALTER TABLE "Podcasts" ADD COLUMN IF NOT EXISTS {col_name} {col_def}')

            # Detection-complete marker (ad-specific, separate from silencedetected)
            cursor.execute('ALTER TABLE "Episodes" ADD COLUMN IF NOT EXISTS adsdetected BOOLEAN DEFAULT FALSE')

            # Global AI config singleton
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "AISettings" (
                    AISettingsID SERIAL PRIMARY KEY,
                    TranscriptionModel VARCHAR(100) NOT NULL DEFAULT 'base',
                    LlmBackend VARCHAR(20) NOT NULL DEFAULT 'local',
                    LlmModel VARCHAR(200),
                    LlmUrl VARCHAR(500),
                    LlmApiKey TEXT,
                    WhisperDevice VARCHAR(20) NOT NULL DEFAULT 'cpu',
                    WhisperComputeType VARCHAR(20) NOT NULL DEFAULT 'int8',
                    UpdatedAt TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
                )
            """)
            cursor.execute('SELECT COUNT(*) FROM "AISettings" WHERE AISettingsID = 1')
            if cursor.fetchone()[0] == 0:
                cursor.execute("""
                    INSERT INTO "AISettings" (AISettingsID, TranscriptionModel, LlmBackend)
                    VALUES (1, 'base', 'local')
                """)
                logger.info("Seeded default AISettings row (PostgreSQL)")

        else:  # MySQL / MariaDB
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS EpisodeAdSkipReview (
                    ReviewID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT NOT NULL,
                    SegmentID INT NOT NULL,
                    Status VARCHAR(20) NOT NULL,
                    CreatedAt TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE,
                    FOREIGN KEY (SegmentID) REFERENCES EpisodeSkipSegments(SegmentID) ON DELETE CASCADE,
                    UNIQUE KEY uq_ad_skip_review_user_segment (UserID, SegmentID)
                )
            """)
            for idx_sql in (
                "CREATE INDEX idx_ad_skip_review_segmentid ON EpisodeAdSkipReview(SegmentID)",
                "CREATE INDEX idx_ad_skip_review_userid ON EpisodeAdSkipReview(UserID)",
            ):
                try:
                    cursor.execute(idx_sql)
                except Exception:
                    pass  # Index may already exist

            for table, col_name, col_def in (
                ("Podcasts", "AutoAdDetect", "BOOLEAN DEFAULT FALSE"),
                ("Podcasts", "AdSkipAutoActivate", "BOOLEAN DEFAULT TRUE"),
                ("Episodes", "AdsDetected", "BOOLEAN DEFAULT FALSE"),
            ):
                cursor.execute(
                    """
                    SELECT COLUMN_NAME FROM INFORMATION_SCHEMA.COLUMNS
                    WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = %s AND COLUMN_NAME = %s
                    """,
                    (table, col_name),
                )
                if not cursor.fetchone():
                    cursor.execute(f"ALTER TABLE {table} ADD COLUMN {col_name} {col_def}")
                    logger.info(f"Added column {col_name} to {table} (MySQL)")

            cursor.execute("""
                CREATE TABLE IF NOT EXISTS AISettings (
                    AISettingsID INT AUTO_INCREMENT PRIMARY KEY,
                    TranscriptionModel VARCHAR(100) NOT NULL DEFAULT 'base',
                    LlmBackend VARCHAR(20) NOT NULL DEFAULT 'local',
                    LlmModel VARCHAR(200),
                    LlmUrl VARCHAR(500),
                    LlmApiKey TEXT,
                    WhisperDevice VARCHAR(20) NOT NULL DEFAULT 'cpu',
                    WhisperComputeType VARCHAR(20) NOT NULL DEFAULT 'int8',
                    UpdatedAt TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
                )
            """)
            cursor.execute("SELECT COUNT(*) FROM AISettings WHERE AISettingsID = 1")
            if cursor.fetchone()[0] == 0:
                cursor.execute(
                    "INSERT INTO AISettings (AISettingsID, TranscriptionModel, LlmBackend) VALUES (1, 'base', 'local')"
                )
                logger.info("Seeded default AISettings row (MySQL)")

        logger.info("Ad-detection + AISettings migration completed successfully")

    except Exception as e:
        logger.error(f"Error in ad-detection/AISettings migration: {e}")
        raise
    finally:
        cursor.close()


@register_migration("057", "add_collection_auto_add_categories", "Add AutoAddCategories column to Collections for auto-adding episodes by podcast category", requires=["048"])
def add_collection_auto_add_categories(conn, db_type: str) -> None:
    """Add the per-collection AutoAddCategories rule.

    Stores a JSON array of podcast category names (e.g. ["Technology","News"]). When set,
    episodes from podcasts in any of those categories are auto-added to the collection during
    the scheduled refresh (and optionally backfilled on save). NULL/empty means no auto-add.
    Intentionally limited to category matching — anything richer overlaps smart playlists."""
    logger.info("Starting migration 057: add AutoAddCategories column to Collections")
    cursor = conn.cursor()

    try:
        if db_type == "postgresql":
            cursor.execute("""
                ALTER TABLE "Collections"
                ADD COLUMN IF NOT EXISTS AutoAddCategories TEXT
            """)
        else:  # MySQL/MariaDB
            cursor.execute("""
                SELECT COUNT(*)
                FROM INFORMATION_SCHEMA.COLUMNS
                WHERE TABLE_SCHEMA = DATABASE()
                AND TABLE_NAME = 'Collections'
                AND COLUMN_NAME = 'AutoAddCategories'
            """)
            if cursor.fetchone()[0] == 0:
                cursor.execute("""
                    ALTER TABLE Collections
                    ADD COLUMN AutoAddCategories TEXT
                """)
                logger.info("Added AutoAddCategories column to Collections table")
            else:
                logger.info("AutoAddCategories column already exists in Collections table")

        logger.info("AutoAddCategories column migration completed")

    finally:
        cursor.close()


@register_migration("058", "create_recommendation_cache", "Create RecommendationCache table for the Discover page's per-user podcast recommendations (#103)", requires=["001"])
def migration_058_create_recommendation_cache(conn, db_type: str) -> None:
    """Per-user cache of generated podcast recommendations for the Discover page (#103).

    Recommendations are expensive (external PodcastIndex trending calls + cosine ranking), so
    they're computed at most daily per user and stored here as a JSON array of RecommendedPodcast
    objects. One row per user (UserID is the PK); refreshed by the scheduler nightly or on demand
    via /api/data/recommendations?refresh=1. Rows are disposable — dropping the table just forces a
    recompute — so no migration backfill is needed."""
    logger.info("Starting migration 058: Create RecommendationCache table")
    cursor = conn.cursor()

    try:
        if db_type == "postgresql":
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "RecommendationCache" (
                    UserID INT PRIMARY KEY,
                    GeneratedAt TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    ResultsJSON TEXT NOT NULL,
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE
                )
            """)
        else:  # MySQL / MariaDB
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS RecommendationCache (
                    UserID INT PRIMARY KEY,
                    GeneratedAt TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    ResultsJSON LONGTEXT NOT NULL,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE
                )
            """)

        logger.info("RecommendationCache migration completed successfully")

    except Exception as e:
        logger.error(f"Error in RecommendationCache migration: {e}")
        raise
    finally:
        cursor.close()
