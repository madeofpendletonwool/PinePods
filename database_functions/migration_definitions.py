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
                    PlaybackSpeed NUMERIC(2,1) DEFAULT 1.0
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
                    PlaybackSpeed DECIMAL(2,1) UNSIGNED DEFAULT 1.0
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
                    NewsFeedSubscribed BOOLEAN DEFAULT false
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
                    NewsFeedSubscribed TINYINT(1) DEFAULT 0
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
        
        # Always write API key to temp file for web service (file may not exist after container restart)
        with open("/tmp/web_api_key.txt", "w") as f:
            f.write(api_key)
        
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
                    StartSkip INT DEFAULT 0,
                    EndSkip INT DEFAULT 0,
                    Username TEXT,
                    Password TEXT,
                    IsYouTubeChannel BOOLEAN DEFAULT FALSE,
                    NotificationsEnabled BOOLEAN DEFAULT FALSE,
                    FeedCutoffDays INT DEFAULT 0,
                    PlaybackSpeed NUMERIC(2,1) DEFAULT 1.0,
                    PlaybackSpeedCustomized BOOLEAN DEFAULT FALSE,
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
                    StartSkip INT DEFAULT 0,
                    EndSkip INT DEFAULT 0,
                    Username TEXT,
                    Password TEXT,
                    IsYouTubeChannel TINYINT(1) DEFAULT 0,
                    NotificationsEnabled TINYINT(1) DEFAULT 0,
                    FeedCutoffDays INT DEFAULT 0,
                    PlaybackSpeed DECIMAL(2,1) UNSIGNED DEFAULT 1.0,
                    PlaybackSpeedCustomized TINYINT(1) DEFAULT 0,
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
                    Name VARCHAR(255) NOT NULL,
                    UserID INT NOT NULL,
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE
                )
            """)
            
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "PeopleEpisodes" (
                    PeopleEpisodeID SERIAL PRIMARY KEY,
                    PersonID INT NOT NULL,
                    EpisodeID INT NOT NULL,
                    AddedDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (PersonID) REFERENCES "People"(PersonID) ON DELETE CASCADE,
                    FOREIGN KEY (EpisodeID) REFERENCES "Episodes"(EpisodeID) ON DELETE CASCADE
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
                    Name VARCHAR(255) NOT NULL,
                    UserID INT NOT NULL,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE
                )
            """)
            
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS PeopleEpisodes (
                    PeopleEpisodeID INT AUTO_INCREMENT PRIMARY KEY,
                    PersonID INT NOT NULL,
                    EpisodeID INT NOT NULL,
                    AddedDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (PersonID) REFERENCES People(PersonID) ON DELETE CASCADE,
                    FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID) ON DELETE CASCADE
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


def register_all_migrations():
    """Register all migrations with the migration manager"""
    # Migrations are auto-registered via decorators
    logger.info("All migrations registered")


if __name__ == "__main__":
    # Register all migrations and run them
    register_all_migrations()
    from database_functions.migrations import run_all_migrations
    success = run_all_migrations()
    sys.exit(0 if success else 1)
