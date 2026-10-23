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
                    DefaultVolume INT DEFAULT 100,
                    GpodderServerEnabled BOOLEAN NOT NULL DEFAULT FALSE,
                    SyncNotice VARCHAR(64)
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
                    DefaultVolume INT DEFAULT 100,
                    GpodderServerEnabled TINYINT(1) NOT NULL DEFAULT 0,
                    SyncNotice VARCHAR(64)
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
                    DownloadMetadataSubfolder BOOLEAN DEFAULT true,
                    PodpingEnabled BOOLEAN NOT NULL DEFAULT false
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
                    DownloadMetadataSubfolder TINYINT(1) DEFAULT 1,
                    PodpingEnabled TINYINT(1) NOT NULL DEFAULT 0
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
                    FeedContentHash TEXT,
                    RefreshNotBefore TIMESTAMP,
                    FailingSince TIMESTAMP,
                    LastErrorKind VARCHAR(32),
                    ProblemNotified BOOLEAN NOT NULL DEFAULT FALSE,
                    NextRefreshAt TIMESTAMP DEFAULT (CURRENT_TIMESTAMP + INTERVAL '5 minutes'),
                    RefreshIntervalMinutes INT,
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
                    FeedContentHash TEXT,
                    RefreshNotBefore DATETIME NULL DEFAULT NULL,
                    FailingSince DATETIME NULL DEFAULT NULL,
                    LastErrorKind VARCHAR(32),
                    ProblemNotified TINYINT(1) NOT NULL DEFAULT 0,
                    NextRefreshAt DATETIME NULL DEFAULT (CURRENT_TIMESTAMP + INTERVAL 5 MINUTE),
                    RefreshIntervalMinutes INT,
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
                    FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID),
                    CONSTRAINT user_episode_unique UNIQUE (UserID, EpisodeID)
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
                    FOREIGN KEY (VideoID) REFERENCES YouTubeVideos(VideoID),
                    CONSTRAINT user_video_unique UNIQUE (UserID, VideoID)
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
                    UserCaption VARCHAR(255),
                    IsDefault BOOLEAN DEFAULT FALSE,
                    LastSync TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    IsActive BOOLEAN DEFAULT TRUE,
                    SystemRole VARCHAR(16),
                    SubsDownloadCursor BIGINT,
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
                    UserCaption VARCHAR(255),
                    IsDefault BOOLEAN DEFAULT FALSE,
                    LastSync TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    IsActive BOOLEAN DEFAULT TRUE,
                    SystemRole VARCHAR(16),
                    SubsDownloadCursor BIGINT,
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
# The gpodder sync tables (the gpodder server in rust-api serves them)
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
                    DeviceID INT,
                    PodcastURL TEXT NOT NULL,
                    Action VARCHAR(10) NOT NULL,
                    Timestamp BIGINT NOT NULL,
                    ServerTs BIGINT NOT NULL,
                    Origin VARCHAR(32) NOT NULL DEFAULT 'client',
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE,
                    CONSTRAINT fk_gpodder_subscriptions_device FOREIGN KEY (DeviceID) REFERENCES "GpodderDevices"(DeviceID) ON DELETE SET NULL
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
                    ServerTs BIGINT NOT NULL,
                    Guid TEXT,
                    EpisodeID INT,
                    Origin VARCHAR(32) NOT NULL DEFAULT 'client',
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE,
                    CONSTRAINT fk_gpodder_episode_actions_device FOREIGN KEY (DeviceID) REFERENCES "GpodderDevices"(DeviceID) ON DELETE SET NULL,
                    CONSTRAINT fk_gpodder_episode_actions_episode FOREIGN KEY (EpisodeID) REFERENCES "Episodes"(EpisodeID) ON DELETE SET NULL
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
                    DeviceID INT,
                    PodcastURL TEXT NOT NULL,
                    Action VARCHAR(10) NOT NULL,
                    Timestamp BIGINT NOT NULL,
                    ServerTs BIGINT NOT NULL,
                    Origin VARCHAR(32) NOT NULL DEFAULT 'client',
                    FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE,
                    CONSTRAINT fk_gpodder_subscriptions_device FOREIGN KEY (DeviceID) REFERENCES GpodderDevices(DeviceID) ON DELETE SET NULL
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
                    ServerTs BIGINT NOT NULL,
                    Guid TEXT,
                    EpisodeID INT,
                    Origin VARCHAR(32) NOT NULL DEFAULT 'client',
                    FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE,
                    CONSTRAINT fk_gpodder_episode_actions_device FOREIGN KEY (DeviceID) REFERENCES GpodderDevices(DeviceID) ON DELETE SET NULL,
                    CONSTRAINT fk_gpodder_episode_actions_episode FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID) ON DELETE SET NULL
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
                    EpisodesDownloadCursor BIGINT,
                    AppPasswordID INT,
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
                    EpisodesDownloadCursor BIGINT,
                    AppPasswordID INT,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE
                )
            ''', conn=conn)
            
            # Create indexes
            indexes_sql = [
                'CREATE INDEX idx_gpodder_sessions_token ON GpodderSessions(SessionToken(64))',
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


@register_migration("109", "host_feed_cache", "Skip HostFeedCache - replaced by migration 062", requires=["009"])
def migration_109_host_feed_cache(conn, db_type: str):
    """No-op. This used to create HostFeedCache for the old host feed, which migration 062
    replaced and drops. Fresh installs run 109 after 062 (versions run in order), so creating
    the table here would leave an unused table that upgraded installs don't have.
    """
    logger.info("Skipping migration 109: HostFeedCache was replaced by migration 062")


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
                WHERE TABLE_SCHEMA = DATABASE()
                AND TABLE_NAME = 'Episodes'
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
                WHERE TABLE_SCHEMA = DATABASE()
                AND TABLE_NAME = 'Podcasts'
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
                WHERE TABLE_SCHEMA = DATABASE()
                AND TABLE_NAME = 'Podcasts'
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


@register_migration("059", "add_ytdlp_management_columns", "Add self-updating yt-dlp management columns to AppSettings (#793)", requires=["001"])
def add_ytdlp_management_columns(conn, db_type: str) -> None:
    """Admin controls for the locally-run, self-updating yt-dlp binary (#793).

    YouTube support breaks whenever Google changes YouTube internals; the fix is a newer
    yt-dlp. Rather than rebuild/release the image for every yt-dlp bump, the binary now
    updates itself on a schedule (and on demand) via `yt-dlp --update-to <channel>`.

      YtDlpAutoUpdate  - run the daily + startup auto-update (default ON, so most installs
                         self-heal without intervention)
      YtDlpChannel     - update channel: 'stable' or 'nightly'. Nightly is the escape hatch:
                         YouTube fixes usually land in nightly the same day, before a stable
                         release.
      YtDlpLastUpdated - timestamp of the last update attempt (for the settings UI)
      YtDlpLastResult  - human-readable result of the last update attempt (version or error)

    Defaults keep existing installs auto-updating on the stable channel."""
    logger.info("Starting migration 059: add yt-dlp management columns to AppSettings")
    cursor = conn.cursor()

    # (column_name, postgres_type_default, mysql_type_default)
    columns = [
        ("YtDlpAutoUpdate", "BOOLEAN DEFAULT TRUE", "TINYINT(1) DEFAULT 1"),
        ("YtDlpChannel", "VARCHAR(20) DEFAULT 'stable'", "VARCHAR(20) DEFAULT 'stable'"),
        ("YtDlpLastUpdated", "TIMESTAMP NULL", "TIMESTAMP NULL"),
        ("YtDlpLastResult", "TEXT", "TEXT"),
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

        logger.info("yt-dlp management columns migration completed")

    finally:
        cursor.close()


@register_migration("060", "backfill_missing_user_stats", "Create the missing UserStats row for users added via add_user / self-service sign-up", requires=["003"])
def backfill_missing_user_stats(conn, db_type: str) -> None:
    """Users created through /api/data/add_user (admin "add user" and self-service sign-up)
    never got a UserStats row, so their stats page 404'd and every stats counter
    (UPDATE "UserStats" ... WHERE userid = ...) silently updated nothing. The API now creates
    the row; this backfills it for accounts created before the fix. Idempotent."""
    logger.info("Starting migration 060: backfill missing UserStats rows")
    cursor = conn.cursor()
    try:
        if db_type == "postgresql":
            cursor.execute("""
                INSERT INTO "UserStats" (UserID)
                SELECT u.UserID FROM "Users" u
                WHERE NOT EXISTS (SELECT 1 FROM "UserStats" s WHERE s.UserID = u.UserID)
            """)
        else:  # MySQL/MariaDB
            cursor.execute("""
                INSERT INTO UserStats (UserID)
                SELECT u.UserID FROM Users u
                WHERE NOT EXISTS (SELECT 1 FROM UserStats s WHERE s.UserID = u.UserID)
            """)
        logger.info(f"Backfilled UserStats for {cursor.rowcount} user(s)")
    finally:
        cursor.close()


@register_migration("061", "repair_mysql_columns_skipped_by_cross_schema_check", "Re-add Episodes.IsVideo / Podcasts.AutoPlayNext / Podcasts.IsFavorite where 038-040 were skipped (MySQL/MariaDB)", requires=["038", "039", "040"])
def repair_mysql_columns_skipped_by_cross_schema_check(conn, db_type: str) -> None:
    """Migrations 038-040 checked INFORMATION_SCHEMA.COLUMNS for their column without
    `TABLE_SCHEMA = DATABASE()`. On a MySQL/MariaDB server hosting another database that
    already had the column (a second PinePods instance, a test copy, ...), they concluded
    "already exists", were recorded as applied, and added nothing, after which episode
    listing fails with "Unknown column 'Episodes.IsVideo'". Those checks are now schema-
    scoped; this repairs databases that were already skipped. PostgreSQL's
    information_schema is per-database, so it was never affected."""
    if db_type == "postgresql":
        return
    logger.info("Starting migration 061: repair columns skipped by cross-schema checks")
    cursor = conn.cursor()
    try:
        for table, column in [("Episodes", "IsVideo"), ("Podcasts", "AutoPlayNext"), ("Podcasts", "IsFavorite")]:
            cursor.execute(
                """
                SELECT COUNT(*) FROM INFORMATION_SCHEMA.COLUMNS
                WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = %s AND COLUMN_NAME = %s
                """,
                (table, column),
            )
            if cursor.fetchone()[0] == 0:
                cursor.execute(f"ALTER TABLE {table} ADD COLUMN {column} BOOLEAN DEFAULT FALSE")
                logger.info(f"Repaired missing {table}.{column}")
    finally:
        cursor.close()


def _normalize_person_name(name) -> str:
    """Name key for matching people. Must stay identical to normalize_name in
    rust-api/src/services/podcasting2.rs (and PodPeopleDB's)."""
    import unicodedata
    s = unicodedata.normalize("NFKD", name or "")
    s = "".join(c for c in s if not unicodedata.combining(c)).lower()
    s = s.translate(str.maketrans({"ß": "s", "ø": "o", "æ": "a", "đ": "d", "ð": "d", "ł": "l"}))
    out = []
    for c in s:
        if c.isalnum():
            out.append(c)
        elif c.isspace() or c in "-_":
            out.append(" ")
    return " ".join("".join(out).split())


def _column_exists(cursor, db_type: str, table: str, column: str) -> bool:
    if db_type == "postgresql":
        cursor.execute(
            "SELECT COUNT(*) FROM information_schema.columns WHERE table_name = %s AND column_name = %s",
            (table, column.lower()),
        )
    else:
        cursor.execute(
            "SELECT COUNT(*) FROM INFORMATION_SCHEMA.COLUMNS WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = %s AND COLUMN_NAME = %s",
            (table, column),
        )
    return cursor.fetchone()[0] > 0


def _table_exists(cursor, db_type: str, table: str) -> bool:
    if db_type == "postgresql":
        cursor.execute("SELECT COUNT(*) FROM information_schema.tables WHERE table_name = %s", (table,))
    else:
        cursor.execute(
            "SELECT COUNT(*) FROM INFORMATION_SCHEMA.TABLES WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = %s",
            (table,),
        )
    return cursor.fetchone()[0] > 0


@register_migration("062", "people_refactor", "Shared people identity, feed/PodPeopleDB credits, person follows, stored Podcasting 2.0 data", requires=["001", "005", "009"])
def migration_062_people_refactor(conn, db_type: str) -> None:
    """Replace per-user host subscriptions with a shared person identity.

    - Persons: one row per person across the server (name, image, link, PodPeopleDB id).
    - PodcastPeople / EpisodePeople: who is credited on a podcast or episode, from the feed's
      <podcast:person> tags (Source 'feed', written at refresh) or PodPeopleDB ('podpeople',
      written by the background sync). Rows cascade with their podcast/episode.
    - PersonFollows: a user following a person, everywhere they appear (#349).
    - PersonExternalEpisodes: episodes a followed/viewed person appears in on shows outside the
      library, cached from the Podcast Index person search and PodPeopleDB.
    - Podcasts.Podcasting2Data/Podcasting2Hash/PodcastGUID and Episodes.ChaptersURL/
      TranscriptsJSON: Podcasting 2.0 data stored at refresh so pages never fetch the feed.

    Existing People rows become Persons + PersonFollows; People, PeopleEpisodes and
    HostFeedCache are dropped. The user-1 placeholder podcasts the old code created for host
    feeds are removed by the API at startup (it owns the cascade logic for podcast removal).
    """
    logger.info("Starting migration 062: people refactor")
    cursor = conn.cursor()
    pg = db_type == "postgresql"
    try:
        if pg:
            statements = [
                '''CREATE TABLE IF NOT EXISTS "Persons" (
                    PersonID SERIAL PRIMARY KEY,
                    Name TEXT NOT NULL,
                    NameNormalized TEXT NOT NULL,
                    Img TEXT,
                    Href TEXT,
                    PodPeopleID INT UNIQUE,
                    ExternalRefreshedAt TIMESTAMP,
                    CreatedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )''',
                'CREATE INDEX IF NOT EXISTS idx_persons_name_normalized ON "Persons"(NameNormalized)',
                '''CREATE TABLE IF NOT EXISTS "PodcastPeople" (
                    PodcastID INT NOT NULL REFERENCES "Podcasts"(PodcastID) ON DELETE CASCADE,
                    PersonID INT NOT NULL REFERENCES "Persons"(PersonID) ON DELETE CASCADE,
                    Role VARCHAR(64) NOT NULL,
                    PersonGroup VARCHAR(64),
                    Source VARCHAR(16) NOT NULL,
                    Verified BOOLEAN NOT NULL DEFAULT TRUE,
                    PRIMARY KEY (PodcastID, PersonID, Role, Source)
                )''',
                'CREATE INDEX IF NOT EXISTS idx_podcastpeople_person ON "PodcastPeople"(PersonID)',
                '''CREATE TABLE IF NOT EXISTS "EpisodePeople" (
                    EpisodeID INT NOT NULL REFERENCES "Episodes"(EpisodeID) ON DELETE CASCADE,
                    PersonID INT NOT NULL REFERENCES "Persons"(PersonID) ON DELETE CASCADE,
                    Role VARCHAR(64) NOT NULL,
                    PersonGroup VARCHAR(64),
                    Source VARCHAR(16) NOT NULL,
                    Verified BOOLEAN NOT NULL DEFAULT TRUE,
                    PRIMARY KEY (EpisodeID, PersonID, Role, Source)
                )''',
                'CREATE INDEX IF NOT EXISTS idx_episodepeople_person ON "EpisodePeople"(PersonID)',
                '''CREATE TABLE IF NOT EXISTS "PersonFollows" (
                    UserID INT NOT NULL REFERENCES "Users"(UserID) ON DELETE CASCADE,
                    PersonID INT NOT NULL REFERENCES "Persons"(PersonID) ON DELETE CASCADE,
                    CreatedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    PRIMARY KEY (UserID, PersonID)
                )''',
                'CREATE INDEX IF NOT EXISTS idx_personfollows_person ON "PersonFollows"(PersonID)',
                '''CREATE TABLE IF NOT EXISTS "PersonExternalEpisodes" (
                    ID SERIAL PRIMARY KEY,
                    PersonID INT NOT NULL REFERENCES "Persons"(PersonID) ON DELETE CASCADE,
                    FeedURL TEXT NOT NULL,
                    PodcastTitle TEXT,
                    PodcastArtwork TEXT,
                    PodcastIndexID INT,
                    EpisodeGUID TEXT,
                    EpisodeTitle TEXT NOT NULL,
                    EpisodeDescription TEXT,
                    EpisodeURL TEXT NOT NULL,
                    EpisodeArtwork TEXT,
                    EpisodePubDate TIMESTAMP,
                    EpisodeDuration INT,
                    Role VARCHAR(64),
                    Source VARCHAR(16) NOT NULL,
                    FetchedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    UNIQUE (PersonID, EpisodeURL)
                )''',
                'CREATE INDEX IF NOT EXISTS idx_personexternal_date ON "PersonExternalEpisodes"(PersonID, EpisodePubDate DESC)',
            ]
        else:
            statements = [
                '''CREATE TABLE IF NOT EXISTS Persons (
                    PersonID INT AUTO_INCREMENT PRIMARY KEY,
                    Name VARCHAR(255) NOT NULL,
                    NameNormalized VARCHAR(255) NOT NULL,
                    Img TEXT,
                    Href TEXT,
                    PodPeopleID INT UNIQUE,
                    ExternalRefreshedAt TIMESTAMP NULL,
                    CreatedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    INDEX idx_persons_name_normalized (NameNormalized)
                )''',
                '''CREATE TABLE IF NOT EXISTS PodcastPeople (
                    PodcastID INT NOT NULL,
                    PersonID INT NOT NULL,
                    Role VARCHAR(64) NOT NULL,
                    PersonGroup VARCHAR(64),
                    Source VARCHAR(16) NOT NULL,
                    Verified BOOLEAN NOT NULL DEFAULT TRUE,
                    PRIMARY KEY (PodcastID, PersonID, Role, Source),
                    INDEX idx_podcastpeople_person (PersonID),
                    FOREIGN KEY (PodcastID) REFERENCES Podcasts(PodcastID) ON DELETE CASCADE,
                    FOREIGN KEY (PersonID) REFERENCES Persons(PersonID) ON DELETE CASCADE
                )''',
                '''CREATE TABLE IF NOT EXISTS EpisodePeople (
                    EpisodeID INT NOT NULL,
                    PersonID INT NOT NULL,
                    Role VARCHAR(64) NOT NULL,
                    PersonGroup VARCHAR(64),
                    Source VARCHAR(16) NOT NULL,
                    Verified BOOLEAN NOT NULL DEFAULT TRUE,
                    PRIMARY KEY (EpisodeID, PersonID, Role, Source),
                    INDEX idx_episodepeople_person (PersonID),
                    FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID) ON DELETE CASCADE,
                    FOREIGN KEY (PersonID) REFERENCES Persons(PersonID) ON DELETE CASCADE
                )''',
                '''CREATE TABLE IF NOT EXISTS PersonFollows (
                    UserID INT NOT NULL,
                    PersonID INT NOT NULL,
                    CreatedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    PRIMARY KEY (UserID, PersonID),
                    INDEX idx_personfollows_person (PersonID),
                    FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE,
                    FOREIGN KEY (PersonID) REFERENCES Persons(PersonID) ON DELETE CASCADE
                )''',
                '''CREATE TABLE IF NOT EXISTS PersonExternalEpisodes (
                    ID INT AUTO_INCREMENT PRIMARY KEY,
                    PersonID INT NOT NULL,
                    FeedURL TEXT NOT NULL,
                    PodcastTitle TEXT,
                    PodcastArtwork TEXT,
                    PodcastIndexID INT,
                    EpisodeGUID TEXT,
                    EpisodeTitle TEXT NOT NULL,
                    EpisodeDescription MEDIUMTEXT,
                    EpisodeURL TEXT NOT NULL,
                    EpisodeURLHash CHAR(64) NOT NULL,
                    EpisodeArtwork TEXT,
                    EpisodePubDate TIMESTAMP NULL,
                    EpisodeDuration INT,
                    Role VARCHAR(64),
                    Source VARCHAR(16) NOT NULL,
                    FetchedAt TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    UNIQUE KEY uniq_person_episode (PersonID, EpisodeURLHash),
                    INDEX idx_personexternal_date (PersonID, EpisodePubDate),
                    FOREIGN KEY (PersonID) REFERENCES Persons(PersonID) ON DELETE CASCADE
                )''',
            ]
        for stmt in statements:
            cursor.execute(stmt)

        podcasts = '"Podcasts"' if pg else "Podcasts"
        episodes = '"Episodes"' if pg else "Episodes"
        for table, quoted, column, definition in [
            ("Podcasts", podcasts, "Podcasting2Data", "TEXT"),
            ("Podcasts", podcasts, "Podcasting2Hash", "VARCHAR(32)"),
            ("Podcasts", podcasts, "PodcastGUID", "VARCHAR(255)"),
            ("Episodes", episodes, "ChaptersURL", "TEXT"),
            ("Episodes", episodes, "TranscriptsJSON", "TEXT"),
        ]:
            if not _column_exists(cursor, db_type, table, column):
                cursor.execute(f"ALTER TABLE {quoted} ADD COLUMN {column} {definition}")

        # Existing host subscriptions -> Persons + PersonFollows.
        people_table = '"People"' if pg else "People"
        if _table_exists(cursor, db_type, "People"):
            cursor.execute(f"SELECT UserID, Name, PersonImg, PeopleDBID FROM {people_table}")
            rows = cursor.fetchall()
            persons_table = '"Persons"' if pg else "Persons"
            follows_table = '"PersonFollows"' if pg else "PersonFollows"
            by_podpeople, by_name = {}, {}
            migrated = 0
            for user_id, name, img, podpeople_id in rows:
                name = " ".join((name or "").split())
                norm = _normalize_person_name(name)
                if not norm:
                    continue
                podpeople_id = podpeople_id if podpeople_id and podpeople_id > 0 else None
                person_id = (by_podpeople.get(podpeople_id) if podpeople_id else None) or by_name.get(norm)
                if person_id is None:
                    if pg:
                        cursor.execute(
                            f"INSERT INTO {persons_table} (Name, NameNormalized, Img, PodPeopleID) VALUES (%s, %s, %s, %s) RETURNING PersonID",
                            (name, norm, img or None, podpeople_id),
                        )
                        person_id = cursor.fetchone()[0]
                    else:
                        cursor.execute(
                            f"INSERT INTO {persons_table} (Name, NameNormalized, Img, PodPeopleID) VALUES (%s, %s, %s, %s)",
                            (name, norm, img or None, podpeople_id),
                        )
                        person_id = cursor.lastrowid
                elif podpeople_id and podpeople_id not in by_podpeople:
                    cursor.execute(
                        f"UPDATE {persons_table} SET PodPeopleID = %s WHERE PersonID = %s AND PodPeopleID IS NULL",
                        (podpeople_id, person_id),
                    )
                by_name.setdefault(norm, person_id)
                if podpeople_id:
                    by_podpeople.setdefault(podpeople_id, person_id)
                if pg:
                    cursor.execute(
                        f"INSERT INTO {follows_table} (UserID, PersonID) SELECT %s, %s WHERE EXISTS (SELECT 1 FROM \"Users\" WHERE UserID = %s) ON CONFLICT DO NOTHING",
                        (user_id, person_id, user_id),
                    )
                else:
                    cursor.execute(
                        f"INSERT IGNORE INTO {follows_table} (UserID, PersonID) SELECT %s, %s FROM DUAL WHERE EXISTS (SELECT 1 FROM Users WHERE UserID = %s)",
                        (user_id, person_id, user_id),
                    )
                migrated += 1
            logger.info(f"Migrated {migrated} host subscriptions into {len(by_name)} people")

        for old in ["PeopleEpisodes", "People", "HostFeedCache"]:
            if _table_exists(cursor, db_type, old):
                cursor.execute(f'DROP TABLE {chr(34) + old + chr(34) if pg else old}')
        logger.info("Migration 062 completed")
    finally:
        cursor.close()


@register_migration("063", "add_feed_content_hash_and_refresh_not_before", "Add Podcasts.FeedContentHash (unchanged-body skip) and Podcasts.RefreshNotBefore (Retry-After backoff)", requires=["005"])
def migration_063_add_feed_content_hash_and_refresh_not_before(conn, db_type: str) -> None:
    """Columns for skipping unchanged feeds and honouring Retry-After.

    - FeedContentHash: SHA-256 hex of the last feed body that was applied. Many hosts send no
      ETag/Last-Modified (or change them every request), so the refresher compares the body hash
      and skips parse/apply when the feed hasn't changed.
    - RefreshNotBefore: set from a 429/503 response's Retry-After; refreshes skip the feed until
      then. Cleared on the next success.
    """
    logger.info("Starting migration 063: Podcasts.FeedContentHash / RefreshNotBefore")
    cursor = conn.cursor()
    pg = db_type == "postgresql"
    table = '"Podcasts"' if pg else "Podcasts"
    try:
        columns = [
            ("FeedContentHash", "TEXT"),
            ("RefreshNotBefore", "TIMESTAMP" if pg else "DATETIME NULL DEFAULT NULL"),
        ]
        for name, definition in columns:
            if _column_exists(cursor, db_type, "Podcasts", name):
                logger.info(f"Column {name} already exists in Podcasts")
                continue
            cursor.execute(f"ALTER TABLE {table} ADD COLUMN {name} {definition}")
            logger.info(f"Added column {name} to Podcasts")
        logger.info("Migration 063 completed")
    finally:
        cursor.close()


def _in_list(values) -> str:
    return ", ".join(["%s"] * len(values))


def _new_merge_counts() -> dict:
    return {
        "history": 0, "saved": 0, "downloads": 0, "queue": 0, "playlists": 0,
        "collections": 0, "shares": 0, "segments": 0, "transcripts": 0, "people": 0, "sync_actions": 0,
    }


def _merge_episodes(cursor, db_type: str, survivor: int, losers: list, moved: dict, label: str) -> None:
    """Merge the episode rows `losers` into `survivor` and delete them. Every table keyed by
    EpisodeID is re-pointed at the survivor:

    - UserEpisodeHistory: one row per user, with the highest listen position and latest date.
    - SavedEpisodes / DownloadedEpisodes: one row per user, the most recent one kept.
      DownloadedEpisodes rows that are dropped are logged with their file location.
    - EpisodeQueue (podcast rows only; is_youtube rows hold video ids): one row per user, the
      earliest queue position kept.
    - PlaylistContents / CollectionEpisodes: one row per playlist / collection.
    - SharedEpisodes and the gpodder episode action log: every row is kept.
    - EpisodeSkipSegments / EpisodeTranscripts: per-episode results, taken from the survivor if
      it has any, otherwise from the first duplicate that does (review rows follow the segments).
    - EpisodePeople: credits are unioned.
    - Episodes: Completed / SilenceDetected / AdsDetected are OR-ed, the longest duration and any
      chapters/transcript links are kept (the survivor's first).
    """
    pg = db_type == "postgresql"

    def q(table: str) -> str:
        return f'"{table}"' if pg else table

    ids = [survivor] + list(losers)

    def keep_one_per_owner(table, id_col, owner_col, order_key, extra_where=""):
        """Re-point `table` rows for `ids` at `survivor`, keeping one row per owner.

        `order_key` sorts (row id, owner, sort value) tuples; the first row per owner stays.
        Returns (rows touched, rows dropped)."""
        cursor.execute(
            f"SELECT {id_col}, {owner_col}, {order_key[0]} FROM {q(table)} "
            f"WHERE EpisodeID IN ({_in_list(ids)}){extra_where}",
            ids,
        )
        rows = sorted(cursor.fetchall(), key=order_key[1])
        kept, dropped = {}, []
        for row in rows:
            if row[1] in kept:
                dropped.append(row)
            else:
                kept[row[1]] = row
        if dropped:
            cursor.execute(
                f"DELETE FROM {q(table)} WHERE {id_col} IN ({_in_list(dropped)})",
                [r[0] for r in dropped],
            )
        repoint = [r[0] for r in kept.values()]
        if repoint:
            cursor.execute(
                f"UPDATE {q(table)} SET EpisodeID = %s WHERE {id_col} IN ({_in_list(repoint)})",
                [survivor] + repoint,
            )
        return len(rows), dropped

    # Sort keys over (row id, owner, value) rows: newest value first (NULL last), else lowest id.
    newest = lambda r: (r[2] is None, -(r[2].timestamp()) if r[2] is not None else 0, r[0])
    lowest_id = lambda r: r[0]

    # Episode-level flags.
    cursor.execute(
        f"""SELECT EpisodeID, Completed, EpisodeDuration, SilenceDetected, AdsDetected,
                   ChaptersURL, TranscriptsJSON
            FROM {q('Episodes')} WHERE EpisodeID IN ({_in_list(ids)})""",
        ids,
    )
    rows = sorted(cursor.fetchall(), key=lambda r: (r[0] != survivor, r[0]))
    completed = any(bool(r[1]) for r in rows)
    duration = max((r[2] or 0) for r in rows)
    silence = any(bool(r[3]) for r in rows)
    ads = any(bool(r[4]) for r in rows)
    chapters = next((r[5] for r in rows if r[5]), None)
    transcripts = next((r[6] for r in rows if r[6]), None)
    cursor.execute(
        f"""UPDATE {q('Episodes')}
            SET Completed = %s, EpisodeDuration = %s, SilenceDetected = %s, AdsDetected = %s,
                ChaptersURL = %s, TranscriptsJSON = %s
            WHERE EpisodeID = %s""",
        (completed, duration, silence, ads, chapters, transcripts, survivor),
    )

    # Listen history: one row per user with the furthest position and latest listen date.
    cursor.execute(
        f"""SELECT UserID, MAX(ListenDuration), MAX(ListenDate), COUNT(*)
            FROM {q('UserEpisodeHistory')} WHERE EpisodeID IN ({_in_list(ids)})
            GROUP BY UserID""",
        ids,
    )
    history = cursor.fetchall()
    cursor.execute(
        f"DELETE FROM {q('UserEpisodeHistory')} WHERE EpisodeID IN ({_in_list(losers)})",
        losers,
    )
    for user_id, position, listened_at, count in history:
        cursor.execute(
            f"SELECT COUNT(*) FROM {q('UserEpisodeHistory')} WHERE UserID = %s AND EpisodeID = %s",
            (user_id, survivor),
        )
        if cursor.fetchone()[0]:
            cursor.execute(
                f"""UPDATE {q('UserEpisodeHistory')} SET ListenDuration = %s, ListenDate = %s
                    WHERE UserID = %s AND EpisodeID = %s""",
                (position, listened_at, user_id, survivor),
            )
        else:
            cursor.execute(
                f"""INSERT INTO {q('UserEpisodeHistory')} (UserID, EpisodeID, ListenDate, ListenDuration)
                    VALUES (%s, %s, %s, %s)""",
                (user_id, survivor, listened_at, position),
            )
        moved["history"] += count

    c, _ = keep_one_per_owner("SavedEpisodes", "SaveID", "UserID", ("SaveDate", newest))
    moved["saved"] += c
    cursor.execute(
        f"SELECT DownloadID, DownloadedLocation FROM {q('DownloadedEpisodes')} "
        f"WHERE EpisodeID IN ({_in_list(ids)})",
        ids,
    )
    locations = dict(cursor.fetchall())
    c, dropped = keep_one_per_owner("DownloadedEpisodes", "DownloadID", "UserID", ("DownloadedDate", newest))
    moved["downloads"] += c
    for row in dropped:
        logger.info(
            f"{label}: dropped duplicate download {row[0]} of episode {survivor} for user {row[1]}; "
            f"file left on disk: {locations.get(row[0])}"
        )
    c, _ = keep_one_per_owner(
        "EpisodeQueue", "QueueID", "UserID",
        ("QueuePosition", lambda r: (r[2] is None, r[2] if r[2] is not None else 0, r[0])),
        extra_where=" AND is_youtube = FALSE" if pg else " AND COALESCE(is_youtube, 0) = 0",
    )
    moved["queue"] += c
    c, _ = keep_one_per_owner("PlaylistContents", "PlaylistContentID", "PlaylistID", ("Position", lowest_id))
    moved["playlists"] += c
    c, _ = keep_one_per_owner("CollectionEpisodes", "CollectionEpisodeID", "CollectionID", ("AddedAt", lowest_id))
    moved["collections"] += c

    cursor.execute(
        f"UPDATE {q('SharedEpisodes')} SET EpisodeID = %s WHERE EpisodeID IN ({_in_list(losers)})",
        [survivor] + list(losers),
    )
    moved["shares"] += cursor.rowcount or 0
    if _column_exists(cursor, db_type, "GpodderSyncEpisodeActions", "EpisodeID"):
        cursor.execute(
            f"UPDATE {q('GpodderSyncEpisodeActions')} SET EpisodeID = %s WHERE EpisodeID IN ({_in_list(losers)})",
            [survivor] + list(losers),
        )
        moved["sync_actions"] += cursor.rowcount or 0

    # Per-episode processing results: take one episode's set whole, never mix two.
    for table, key in (("EpisodeSkipSegments", "segments"), ("EpisodeTranscripts", "transcripts")):
        cursor.execute(
            f"SELECT DISTINCT EpisodeID FROM {q(table)} WHERE EpisodeID IN ({_in_list(ids)})",
            ids,
        )
        having = sorted(r[0] for r in cursor.fetchall())
        if having and survivor not in having:
            cursor.execute(
                f"UPDATE {q(table)} SET EpisodeID = %s WHERE EpisodeID = %s",
                (survivor, having[0]),
            )
            moved[key] += cursor.rowcount or 0

    insert_ignore = "INSERT INTO" if pg else "INSERT IGNORE INTO"
    on_conflict = " ON CONFLICT DO NOTHING" if pg else ""
    cursor.execute(
        f"""{insert_ignore} {q('EpisodePeople')} (EpisodeID, PersonID, Role, PersonGroup, Source, Verified)
            SELECT %s, PersonID, Role, PersonGroup, Source, Verified
            FROM {q('EpisodePeople')} WHERE EpisodeID IN ({_in_list(losers)}){on_conflict}""",
        [survivor] + list(losers),
    )
    moved["people"] += cursor.rowcount or 0

    cursor.execute(
        f"DELETE FROM {q('Episodes')} WHERE EpisodeID IN ({_in_list(losers)})",
        losers,
    )


@register_migration("064", "dedupe_episodes_by_guid", "Merge duplicate (PodcastID, EpisodeGUID) episode rows left by racing refreshes", requires=["045", "048", "050", "051", "062"])
def migration_064_dedupe_episodes_by_guid(conn, db_type: str) -> None:
    """Merge episodes that share (PodcastID, EpisodeGUID), so 065 can make the pair unique.

    Overlapping refreshes (scheduled + manual, or two manual ones) could both insert the same new
    episode because nothing enforced uniqueness. For each duplicate set the lowest EpisodeID
    survives (see _merge_episodes for how each table is merged), and EpisodeCount is recounted
    for the affected podcasts. Runs in one transaction.
    """
    logger.info("Starting migration 064: merge duplicate (PodcastID, EpisodeGUID) episodes")
    cursor = conn.cursor()
    pg = db_type == "postgresql"

    def q(table: str) -> str:
        return f'"{table}"' if pg else table

    try:
        cursor.execute(
            f"""
            SELECT e.EpisodeID, e.PodcastID, e.EpisodeGUID
            FROM {q('Episodes')} e
            JOIN (
                SELECT PodcastID, EpisodeGUID
                FROM {q('Episodes')}
                WHERE EpisodeGUID IS NOT NULL
                GROUP BY PodcastID, EpisodeGUID
                HAVING COUNT(*) > 1
            ) d ON d.PodcastID = e.PodcastID AND d.EpisodeGUID = e.EpisodeGUID
            ORDER BY e.PodcastID, e.EpisodeGUID, e.EpisodeID
            """
        )
        groups = {}
        for episode_id, podcast_id, guid in cursor.fetchall():
            groups.setdefault((podcast_id, guid), []).append(episode_id)

        if not groups:
            logger.info("Migration 064: no duplicate episodes found")
            return

        extra = sum(len(ids) - 1 for ids in groups.values())
        podcasts = sorted({podcast_id for podcast_id, _ in groups})
        logger.info(
            f"Migration 064: {len(groups)} duplicated episodes ({extra} extra rows) "
            f"across {len(podcasts)} podcasts; merging"
        )

        moved = _new_merge_counts()
        detail_limit = 50
        for n, ((podcast_id, guid), ids) in enumerate(sorted(groups.items())):
            survivor, losers = ids[0], ids[1:]
            if n < detail_limit:
                logger.info(f"Migration 064: podcast {podcast_id} guid {guid!r}: keeping {survivor}, merging {losers}")
            _merge_episodes(cursor, db_type, survivor, losers, moved, "Migration 064")

        if len(groups) > detail_limit:
            logger.info(f"Migration 064: ... and {len(groups) - detail_limit} more")

        for podcast_id in podcasts:
            cursor.execute(
                f"""UPDATE {q('Podcasts')}
                    SET EpisodeCount = (SELECT COUNT(*) FROM {q('Episodes')} WHERE PodcastID = %s)
                    WHERE PodcastID = %s""",
                (podcast_id, podcast_id),
            )

        logger.info(
            f"Migration 064 completed: removed {extra} duplicate episodes; rows re-pointed or merged: "
            + ", ".join(f"{k}={v}" for k, v in moved.items())
        )
    finally:
        cursor.close()


@register_migration("065", "unique_episode_guid_per_podcast", "Make (PodcastID, EpisodeGUID) unique so racing refreshes can't insert an episode twice", requires=["064"])
def migration_065_unique_episode_guid_per_podcast(conn, db_type: str) -> None:
    """Enforce one episode per GUID per podcast; inserts use ON CONFLICT DO NOTHING / INSERT IGNORE.

    PostgreSQL: a partial unique index (episodes without a GUID are unconstrained).
    MySQL/MariaDB can't put a unique index on a TEXT column, so a stored generated
    EpisodeGUIDHash = SHA2(EpisodeGUID, 256) carries it; NULL GUIDs hash to NULL, which a unique
    index allows any number of times. The old non-unique idx_episodes_podcastid_guid is replaced.
    """
    logger.info("Starting migration 065: unique (PodcastID, EpisodeGUID)")
    cursor = conn.cursor()
    try:
        if db_type == "postgresql":
            safe_add_index(
                cursor, db_type,
                'CREATE UNIQUE INDEX idx_episodes_podcastid_guid_unique ON "Episodes"(PodcastID, EpisodeGUID) '
                'WHERE EpisodeGUID IS NOT NULL',
                "idx_episodes_podcastid_guid_unique",
            )
            if not check_index_exists(cursor, db_type, "idx_episodes_podcastid_guid_unique"):
                raise RuntimeError("idx_episodes_podcastid_guid_unique was not created")
            cursor.execute("DROP INDEX IF EXISTS idx_episodes_podcastid_guid")
        else:
            if not _column_exists(cursor, db_type, "Episodes", "EpisodeGUIDHash"):
                cursor.execute(
                    "ALTER TABLE Episodes ADD COLUMN EpisodeGUIDHash CHAR(64) "
                    "AS (SHA2(EpisodeGUID, 256)) STORED"
                )
                logger.info("Added generated column Episodes.EpisodeGUIDHash")
            if not check_index_exists(cursor, db_type, "idx_episodes_podcastid_guidhash_unique"):
                cursor.execute(
                    "CREATE UNIQUE INDEX idx_episodes_podcastid_guidhash_unique ON Episodes(PodcastID, EpisodeGUIDHash)"
                )
                logger.info("Added index idx_episodes_podcastid_guidhash_unique")
            if check_index_exists(cursor, db_type, "idx_episodes_podcastid_guid"):
                cursor.execute("DROP INDEX idx_episodes_podcastid_guid ON Episodes")
        logger.info("Migration 065 completed")
    finally:
        cursor.close()


@register_migration("066", "add_episode_url_lookup_index", "Index Episodes.EpisodeURL for sync episode matching", requires=["001"])
def migration_066_add_episode_url_lookup_index(conn, db_type: str) -> None:
    """Sync matches episodes by URL, which was a sequential scan of Episodes.

    PostgreSQL: a hash index (equality only, any length). MySQL/MariaDB: a stored generated
    EpisodeURLHash = SHA2(EpisodeURL, 256) with a plain index; lookups compare
    `EpisodeURLHash = SHA2(?, 256)`.
    """
    logger.info("Starting migration 066: EpisodeURL lookup index")
    cursor = conn.cursor()
    try:
        if db_type == "postgresql":
            safe_add_index(
                cursor, db_type,
                'CREATE INDEX idx_episodes_episodeurl_hash ON "Episodes" USING hash (EpisodeURL)',
                "idx_episodes_episodeurl_hash",
            )
        else:
            if not _column_exists(cursor, db_type, "Episodes", "EpisodeURLHash"):
                cursor.execute(
                    "ALTER TABLE Episodes ADD COLUMN EpisodeURLHash CHAR(64) "
                    "AS (SHA2(EpisodeURL, 256)) STORED"
                )
                logger.info("Added generated column Episodes.EpisodeURLHash")
            safe_add_index(
                cursor, db_type,
                "CREATE INDEX idx_episodes_episodeurlhash ON Episodes(EpisodeURLHash)",
                "idx_episodes_episodeurlhash",
            )
        logger.info("Migration 066 completed")
    finally:
        cursor.close()


def _feed_url_key(url) -> str:
    """How feed URLs are compared to find a subscription: no scheme, no "www.", no trailing
    slash, any case. Must match feed_url_key in rust-api/src/services/sync/feeds.rs."""
    u = (url or "").strip().lower()
    for prefix in ("https://", "http://"):
        if u.startswith(prefix):
            u = u[len(prefix):]
            break
    if u.startswith("www."):
        u = u[4:]
    return u.rstrip("/")


def _merge_duplicate_podcasts(cursor, db_type: str) -> int:
    """Merge podcasts a user is subscribed to twice under two addresses (a sync app that followed
    a feed move uploaded the new address). Two of a user's podcasts are the same show when their
    addresses compare equal, they share a Podcasting 2.0 GUID, or they have the same title and
    share episodes (by GUID or audio URL). The older podcast stays; the copy's episodes are merged
    into the matching ones (or moved over when the original doesn't have them), the address of the
    feed that published last is kept and the other one is remembered as an alias. Nothing is sent
    to sync apps. Returns how many podcasts were merged."""
    pg = db_type == "postgresql"

    def q(table: str) -> str:
        return f'"{table}"' if pg else table

    cursor.execute(
        f"SELECT PodcastID, UserID, PodcastName, FeedURL, PodcastGUID, COALESCE(IsYouTubeChannel, {'FALSE' if pg else '0'}) "
        f"FROM {q('Podcasts')} ORDER BY UserID, PodcastID"
    )
    by_user = {}
    for podcast_id, user_id, name, url, guid, youtube in cursor.fetchall():
        if youtube or not url or url.startswith("local://"):
            continue
        by_user.setdefault(user_id, []).append((podcast_id, " ".join((name or "").lower().split()), url, (guid or "").strip().lower()))

    def episode_count(podcast_id):
        cursor.execute(f"SELECT COUNT(*) FROM {q('Episodes')} WHERE PodcastID = %s", (podcast_id,))
        return cursor.fetchone()[0]

    def shared_episodes(a, b):
        cursor.execute(
            f"""SELECT COUNT(DISTINCT x.EpisodeID) FROM {q('Episodes')} x
                JOIN {q('Episodes')} y ON y.PodcastID = %s AND (y.EpisodeGUID = x.EpisodeGUID OR y.EpisodeURL = x.EpisodeURL)
                WHERE x.PodcastID = %s""",
            (b, a),
        )
        return cursor.fetchone()[0]

    moved = _new_merge_counts()
    merged = 0
    for user_id, podcasts in by_user.items():
        gone = set()
        for i, (a, a_name, a_url, a_guid) in enumerate(podcasts):
            for b, b_name, b_url, b_guid in podcasts[i + 1:]:
                if a in gone or b in gone:
                    continue
                same = _feed_url_key(a_url) == _feed_url_key(b_url) or bool(a_guid and a_guid == b_guid)
                if not same and a_name and a_name == b_name:
                    smaller = min(episode_count(a), episode_count(b))
                    same = smaller > 0 and shared_episodes(a, b) >= min(3, smaller)
                if not same:
                    continue
                _merge_podcast_into(cursor, db_type, user_id, a, b, moved)
                gone.add(b)
                merged += 1
                logger.info(f"Merged podcast {b} ({b_url}) into {a} ({a_url}) for user {user_id}: the same show under two addresses")
    if merged:
        logger.info("Duplicate podcasts merged; episode rows re-pointed or merged: " + ", ".join(f"{k}={v}" for k, v in moved.items()))
    return merged


def _merge_podcast_into(cursor, db_type: str, user_id: int, kept: int, copy: int, moved: dict) -> None:
    pg = db_type == "postgresql"

    def q(table: str) -> str:
        return f'"{table}"' if pg else table

    cursor.execute(f"SELECT PodcastID, FeedURL FROM {q('Podcasts')} WHERE PodcastID IN (%s, %s)", (kept, copy))
    urls = dict(cursor.fetchall())
    cursor.execute(
        f"SELECT PodcastID, MAX(EpisodePubDate) FROM {q('Episodes')} WHERE PodcastID IN (%s, %s) GROUP BY PodcastID",
        (kept, copy),
    )
    latest = dict(cursor.fetchall())
    # The address that published last is where the feed lives now.
    copy_is_newer = latest.get(copy) is not None and (latest.get(kept) is None or latest[copy] > latest[kept])

    cursor.execute(f"SELECT EpisodeID, EpisodeGUID, EpisodeURL FROM {q('Episodes')} WHERE PodcastID = %s", (kept,))
    by_guid, by_url = {}, {}
    for episode_id, guid, url in cursor.fetchall():
        if guid:
            by_guid.setdefault(guid, episode_id)
        if url:
            by_url.setdefault(url, episode_id)
    cursor.execute(f"SELECT EpisodeID, EpisodeGUID, EpisodeURL FROM {q('Episodes')} WHERE PodcastID = %s", (copy,))
    unmatched = []
    for episode_id, guid, url in cursor.fetchall():
        target = (by_guid.get(guid) if guid else None) or (by_url.get(url) if url else None)
        if target:
            _merge_episodes(cursor, db_type, target, [episode_id], moved, "Merging duplicate podcasts")
        else:
            unmatched.append(episode_id)
    if unmatched:
        cursor.execute(
            f"UPDATE {q('Episodes')} SET PodcastID = %s WHERE EpisodeID IN ({_in_list(unmatched)})",
            [kept] + unmatched,
        )

    cursor.execute(f"DELETE FROM {q('Podcasts')} WHERE PodcastID = %s", (copy,))
    cursor.execute(f"UPDATE {q('UserStats')} SET PodcastsAdded = PodcastsAdded - 1 WHERE UserID = %s AND PodcastsAdded > 0", (user_id,))
    if copy_is_newer:
        cursor.execute(
            f"""UPDATE {q('Podcasts')} SET FeedURL = %s, FeedETag = NULL, FeedLastModified = NULL, FeedContentHash = NULL
                WHERE PodcastID = %s""",
            (urls[copy], kept),
        )
    alias = urls[kept] if copy_is_newer else urls[copy]
    insert = "INSERT INTO" if pg else "INSERT IGNORE INTO"
    conflict = " ON CONFLICT DO NOTHING" if pg else ""
    cursor.execute(
        f"{insert} {q('PodcastFeedAliases')} (PodcastID, UserID, FeedURL) VALUES (%s, %s, %s){conflict}",
        (kept, user_id, alias),
    )
    cursor.execute(
        f"UPDATE {q('Podcasts')} SET EpisodeCount = (SELECT COUNT(*) FROM {q('Episodes')} WHERE PodcastID = %s) WHERE PodcastID = %s",
        (kept, kept),
    )


@register_migration("077", "podcast_feed_aliases", "Other addresses of a subscribed feed; merge podcasts subscribed twice under two addresses", requires=["044", "062", "063", "064", "065"])
def migration_077_podcast_feed_aliases(conn, db_type: str) -> None:
    """PodcastFeedAliases: other addresses of a subscribed feed (where it lived before it moved, or
    how a sync app writes it), so sync adds and removes under those addresses find the podcast.
    Then podcasts already subscribed twice under two addresses are merged."""
    cursor = conn.cursor()
    pg = db_type == "postgresql"
    try:
        if pg:
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "PodcastFeedAliases" (
                    AliasID SERIAL PRIMARY KEY,
                    PodcastID INT NOT NULL REFERENCES "Podcasts"(PodcastID) ON DELETE CASCADE,
                    UserID INT NOT NULL REFERENCES "Users"(UserID) ON DELETE CASCADE,
                    FeedURL TEXT NOT NULL,
                    CreatedAt TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    CONSTRAINT uq_podcast_feed_aliases UNIQUE (UserID, FeedURL)
                )
            """)
            cursor.execute('CREATE INDEX IF NOT EXISTS idx_podcast_feed_aliases_podcast ON "PodcastFeedAliases"(PodcastID)')
        else:
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS PodcastFeedAliases (
                    AliasID INT AUTO_INCREMENT PRIMARY KEY,
                    PodcastID INT NOT NULL,
                    UserID INT NOT NULL,
                    FeedURL TEXT NOT NULL,
                    FeedURLHash CHAR(64) AS (SHA2(FeedURL, 256)) STORED,
                    CreatedAt TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    UNIQUE KEY uq_podcast_feed_aliases (UserID, FeedURLHash),
                    KEY idx_podcast_feed_aliases_podcast (PodcastID),
                    FOREIGN KEY (PodcastID) REFERENCES Podcasts(PodcastID) ON DELETE CASCADE,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE
                )
            """)
        merged = _merge_duplicate_podcasts(cursor, db_type)
        logger.info(f"Migration 077: merged {merged} podcasts subscribed twice under two addresses")
        conn.commit()
    except Exception:
        conn.rollback()
        raise
    finally:
        cursor.close()


def _compact_old_episode_actions(cursor, db_type: str) -> None:
    """Before the episode action log is rewritten: delete the rows the nightly sync log compaction
    would delete anyway (older than PINEPODS_SYNC_LOG_RETENTION_DAYS, default 90, and replaced by a
    newer action for the same episode), so large old logs don't make the upgrade take minutes.
    Users with an external sync target are left alone: their rows may not have been pushed yet."""
    try:
        days = int(os.environ.get("PINEPODS_SYNC_LOG_RETENTION_DAYS", "90").strip())
    except ValueError:
        days = 90
    if days <= 0 or not _column_exists(cursor, db_type, "GpodderSyncEpisodeActions", "Timestamp"):
        return
    pg = db_type == "postgresql"
    table = '"GpodderSyncEpisodeActions"' if pg else "GpodderSyncEpisodeActions"
    has_targets = _table_exists(cursor, db_type, "SyncTargets")
    targets = ('"SyncTargets"' if pg else "SyncTargets") if has_targets else None
    skip_targets = f"AND UserID NOT IN (SELECT UserID FROM {targets})" if targets else ""
    cutoff_sql = f"EXTRACT(EPOCH FROM NOW())::BIGINT - {days * 86400}" if pg else f"UNIX_TIMESTAMP() - {days * 86400}"
    superseded = f"""
        SELECT ActionID FROM (
            SELECT ActionID, Timestamp, ROW_NUMBER() OVER (
                PARTITION BY UserID, PodcastURL, EpisodeURL, LOWER(Action) ORDER BY ActionID DESC) AS newer
            FROM {table} WHERE 1 = 1 {skip_targets}
        ) ranked WHERE newer > 1 AND Timestamp < {cutoff_sql}
    """
    if pg:
        cursor.execute(f"DELETE FROM {table} WHERE ActionID IN ({superseded})")
    else:
        cursor.execute(f"DELETE a FROM {table} a JOIN ({superseded}) old ON old.ActionID = a.ActionID")
    if cursor.rowcount:
        logger.info(f"Removed {cursor.rowcount} episode actions older than {days} days that newer ones replaced")


@register_migration("110", "gpodder_action_server_ts", "Add server receive time (ServerTs) to GpodderSyncEpisodeActions for sync cursors", requires=["100"])
def migration_110_gpodder_action_server_ts(conn, db_type: str) -> None:
    """Separate when the server received an episode action from when the client says it happened.

    `Timestamp` is the client's action time; clients use it for last-write-wins, so it must be
    stored as uploaded. Download cursors (`since`) need the server's receive time instead, or an
    action uploaded late (offline play) is never delivered to devices that already synced past
    its client time. Existing rows were stamped with their upload time, so ServerTs = Timestamp
    is the correct backfill.
    """
    logger.info("Starting migration 110: GpodderSyncEpisodeActions.ServerTs")
    cursor = conn.cursor()
    pg = db_type == "postgresql"
    table = '"GpodderSyncEpisodeActions"' if pg else "GpodderSyncEpisodeActions"
    try:
        _compact_old_episode_actions(cursor, db_type)
        if not _column_exists(cursor, db_type, "GpodderSyncEpisodeActions", "ServerTs"):
            cursor.execute(f"ALTER TABLE {table} ADD COLUMN ServerTs BIGINT NULL")
        cursor.execute(f"SELECT COUNT(*) FROM {table} WHERE ServerTs IS NULL")
        logger.info(f"Migration 110: setting ServerTs on {cursor.fetchone()[0]} episode actions (a few minutes for millions)")
        cursor.execute(f"UPDATE {table} SET ServerTs = Timestamp WHERE ServerTs IS NULL")
        if pg:
            cursor.execute(f"ALTER TABLE {table} ALTER COLUMN ServerTs SET NOT NULL")
        else:
            cursor.execute(f"ALTER TABLE {table} MODIFY COLUMN ServerTs BIGINT NOT NULL")
        safe_add_index(
            cursor,
            db_type,
            f"CREATE INDEX idx_gpodder_episode_actions_user_serverts ON {table}(UserID, ServerTs)",
            "idx_gpodder_episode_actions_user_serverts",
        )
        logger.info("Migration 110 completed")
    finally:
        cursor.close()


@register_migration("067", "create_sync_targets_and_app_passwords", "Add SyncTargets, GpodderAppPasswords and the Users.GpodderServerEnabled / SyncNotice columns", requires=["001"])
def migration_067_create_sync_targets_and_app_passwords(conn, db_type: str) -> None:
    """Schema for the per-user sync settings that replace the shared Users.Gpodder* columns.

    - Users.GpodderServerEnabled: the internal gpodder server is a per-user on/off flag.
    - Users.SyncNotice: a one-off message for the settings UI, e.g. 'reenter_external' when an
      external server couldn't be carried over by migration 068. Cleared by the UI.
    - SyncTargets: one external gpodder or Nextcloud server per user (UNIQUE(UserID) for now).
      EncryptedSecret is Fernet-encrypted with AppSettings.EncryptionKey. The pull cursors are
      server-returned timestamps; the push cursors are local log IDs
      (GpodderSyncSubscriptions.SubscriptionID / GpodderSyncEpisodeActions.ActionID).
    - GpodderAppPasswords: argon2 hashes accepted as the HTTP Basic password by the internal
      gpodder server. A table rather than one Users column so a user can have several (one per
      app, revocable separately) and so the migrated legacy token is just one of them.

    The old Users.GpodderUrl/GpodderLoginName/GpodderToken/Pod_Sync_Type columns stay until the
    old sync code is removed.
    """
    logger.info("Starting migration 067: SyncTargets + GpodderAppPasswords")
    cursor = conn.cursor()
    pg = db_type == "postgresql"
    users = '"Users"' if pg else "Users"
    try:
        if not _column_exists(cursor, db_type, "Users", "GpodderServerEnabled"):
            if pg:
                cursor.execute(f"ALTER TABLE {users} ADD COLUMN GpodderServerEnabled BOOLEAN NOT NULL DEFAULT FALSE")
            else:
                cursor.execute(f"ALTER TABLE {users} ADD COLUMN GpodderServerEnabled TINYINT(1) NOT NULL DEFAULT 0")
            logger.info("Added Users.GpodderServerEnabled")
        if not _column_exists(cursor, db_type, "Users", "SyncNotice"):
            cursor.execute(f"ALTER TABLE {users} ADD COLUMN SyncNotice VARCHAR(64)")
            logger.info("Added Users.SyncNotice")

        if pg:
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "SyncTargets" (
                    TargetID SERIAL PRIMARY KEY,
                    UserID INT NOT NULL REFERENCES "Users"(UserID) ON DELETE CASCADE,
                    Kind VARCHAR(16) NOT NULL CHECK (Kind IN ('gpodder', 'nextcloud')),
                    Url TEXT NOT NULL,
                    Username VARCHAR(255) NOT NULL DEFAULT '',
                    EncryptedSecret TEXT NOT NULL,
                    DeviceName VARCHAR(255),
                    Enabled BOOLEAN NOT NULL DEFAULT TRUE,
                    SubsPullCursor BIGINT NOT NULL DEFAULT 0,
                    ActionsPullCursor BIGINT NOT NULL DEFAULT 0,
                    SubsPushCursor BIGINT NOT NULL DEFAULT 0,
                    ActionsPushCursor BIGINT NOT NULL DEFAULT 0,
                    NeedsFullSync BOOLEAN NOT NULL DEFAULT TRUE,
                    LastAttemptAt TIMESTAMP,
                    LastSuccessAt TIMESTAMP,
                    LastError TEXT,
                    ConsecutiveFailures INT NOT NULL DEFAULT 0,
                    CreatedAt TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    CONSTRAINT uq_sync_targets_user UNIQUE (UserID)
                )
            """)
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "GpodderAppPasswords" (
                    AppPasswordID SERIAL PRIMARY KEY,
                    UserID INT NOT NULL REFERENCES "Users"(UserID) ON DELETE CASCADE,
                    Label VARCHAR(255) NOT NULL,
                    PasswordHash VARCHAR(500) NOT NULL,
                    CreatedAt TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    LastUsedAt TIMESTAMP
                )
            """)
            safe_add_index(
                cursor, db_type,
                'CREATE INDEX idx_gpodder_app_passwords_user ON "GpodderAppPasswords"(UserID)',
                "idx_gpodder_app_passwords_user",
            )
        else:
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS SyncTargets (
                    TargetID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT NOT NULL,
                    Kind VARCHAR(16) NOT NULL CHECK (Kind IN ('gpodder', 'nextcloud')),
                    Url TEXT NOT NULL,
                    Username VARCHAR(255) NOT NULL DEFAULT '',
                    EncryptedSecret TEXT NOT NULL,
                    DeviceName VARCHAR(255),
                    Enabled TINYINT(1) NOT NULL DEFAULT 1,
                    SubsPullCursor BIGINT NOT NULL DEFAULT 0,
                    ActionsPullCursor BIGINT NOT NULL DEFAULT 0,
                    SubsPushCursor BIGINT NOT NULL DEFAULT 0,
                    ActionsPushCursor BIGINT NOT NULL DEFAULT 0,
                    NeedsFullSync TINYINT(1) NOT NULL DEFAULT 1,
                    LastAttemptAt DATETIME,
                    LastSuccessAt DATETIME,
                    LastError TEXT,
                    ConsecutiveFailures INT NOT NULL DEFAULT 0,
                    CreatedAt DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    CONSTRAINT uq_sync_targets_user UNIQUE (UserID),
                    CONSTRAINT fk_sync_targets_user FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE
                )
            """)
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS GpodderAppPasswords (
                    AppPasswordID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT NOT NULL,
                    Label VARCHAR(255) NOT NULL,
                    PasswordHash VARCHAR(500) NOT NULL,
                    CreatedAt DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    LastUsedAt DATETIME,
                    INDEX idx_gpodder_app_passwords_user (UserID),
                    CONSTRAINT fk_gpodder_app_passwords_user FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE
                )
            """)
        logger.info("Migration 067 completed")
    finally:
        cursor.close()


INTERNAL_GPODDER_URL = "http://localhost:8042"
LEGACY_TOKEN_LABEL = "legacy token"


@register_migration("068", "migrate_pod_sync_type", "Move Users.Pod_Sync_Type settings to GpodderServerEnabled / SyncTargets / app passwords", requires=["002", "018", "019", "020", "067"])
def migration_068_migrate_pod_sync_type(conn, db_type: str) -> None:
    """Carry each user's sync setup over to the new model. The old columns are left as they are.

    - 'gpodder' (internal server): GpodderServerEnabled = true. The internal token (stored in
      plaintext; the Go server compares it as-is with the Basic password or X-GPodder-Token) is
      stored argon2-hashed as a permanent app password labelled 'legacy token', so connected
      clients keep working.
    - 'external': SyncTargets(kind='gpodder') with the already-encrypted token and the default
      device as Pinepods' device name.
    - 'nextcloud': SyncTargets(kind='nextcloud').
    - 'both': enabling the internal server overwrote the external credentials with the internal
      token, so this is 'gpodder' plus SyncNotice = 'reenter_external' for the UI banner.

    An external/Nextcloud row whose secret is missing or doesn't decrypt with the current key
    gets SyncNotice = 'reenter_external' instead of a target. Only counts are logged, never
    credentials. Safe to re-run: existing targets and legacy app passwords are kept.
    """
    logger.info("Starting migration 068: Pod_Sync_Type -> SyncTargets")
    cursor = conn.cursor()
    pg = db_type == "postgresql"
    t = (lambda name: f'"{name}"') if pg else (lambda name: name)
    try:
        cursor.execute(f"SELECT EncryptionKey FROM {t('AppSettings')} LIMIT 1")
        row = cursor.fetchone()
        key = row[0] if row else None
        if isinstance(key, memoryview):
            key = key.tobytes()
        if isinstance(key, str):
            key = key.encode()
        fernet = None
        if key:
            try:
                fernet = Fernet(key.rstrip(b"\x00"))
            except Exception:
                logger.warning("Migration 068: AppSettings.EncryptionKey is not a valid Fernet key; external sync settings can't be carried over")

        cursor.execute(
            f"SELECT UserID, Pod_Sync_Type, GpodderUrl, GpodderLoginName, GpodderToken, DefaultGpodderDevice FROM {t('Users')}"
        )
        rows = cursor.fetchall()

        counts = {k: 0 for k in (
            "gpodder", "both", "external", "nextcloud", "legacy_app_passwords", "no_token",
            "reenter_external", "unreadable_secret", "already_migrated", "unknown", "internal_with_other_url",
        )}
        ph = PasswordHasher()

        def has_target(user_id):
            cursor.execute(f"SELECT COUNT(*) FROM {t('SyncTargets')} WHERE UserID = %s", (user_id,))
            return cursor.fetchone()[0] > 0

        def enable_internal(user_id, url, token):
            cursor.execute(f"UPDATE {t('Users')} SET GpodderServerEnabled = %s WHERE UserID = %s", (True, user_id))
            if (url or "") not in ("", INTERNAL_GPODDER_URL):
                counts["internal_with_other_url"] += 1
            if not token:
                counts["no_token"] += 1
                return
            cursor.execute(
                f"SELECT COUNT(*) FROM {t('GpodderAppPasswords')} WHERE UserID = %s AND Label = %s",
                (user_id, LEGACY_TOKEN_LABEL),
            )
            if cursor.fetchone()[0] > 0:
                counts["already_migrated"] += 1
                return
            cursor.execute(
                f"INSERT INTO {t('GpodderAppPasswords')} (UserID, Label, PasswordHash) VALUES (%s, %s, %s)",
                (user_id, LEGACY_TOKEN_LABEL, ph.hash(token)),
            )
            counts["legacy_app_passwords"] += 1

        def flag_reenter(user_id):
            cursor.execute(f"UPDATE {t('Users')} SET SyncNotice = %s WHERE UserID = %s", ("reenter_external", user_id))
            counts["reenter_external"] += 1

        def add_target(user_id, kind, url, username, token, device):
            if has_target(user_id):
                counts["already_migrated"] += 1
                return
            if not url or not token:
                flag_reenter(user_id)
                return
            try:
                if fernet is None:
                    raise ValueError("no key")
                fernet.decrypt(token.encode())
            except Exception:
                counts["unreadable_secret"] += 1
                flag_reenter(user_id)
                return
            cursor.execute(
                f"INSERT INTO {t('SyncTargets')} (UserID, Kind, Url, Username, EncryptedSecret, DeviceName, Enabled, NeedsFullSync) "
                "VALUES (%s, %s, %s, %s, %s, %s, %s, %s)",
                (user_id, kind, url, username or "", token, device or None, True, True),
            )

        for user_id, sync_type, url, login, token, device in rows:
            kind = (sync_type or "").strip().lower()
            if kind in ("", "none"):
                continue
            if kind in ("gpodder", "both", "external", "nextcloud"):
                counts[kind] += 1
            if kind == "gpodder":
                enable_internal(user_id, url, token)
            elif kind == "both":
                enable_internal(user_id, url, token)
                flag_reenter(user_id)
            elif kind == "external":
                add_target(user_id, "gpodder", url, login, token, device)
            elif kind == "nextcloud":
                add_target(user_id, "nextcloud", url, login, token, None)
            else:
                counts["unknown"] += 1

        logger.info(
            "Migration 068: gpodder=%d both=%d external=%d nextcloud=%d legacy_app_passwords=%d "
            "no_token=%d reenter_external=%d unreadable_secret=%d already_migrated=%d unknown_type=%d "
            "internal_with_other_url=%d",
            counts["gpodder"], counts["both"], counts["external"], counts["nextcloud"],
            counts["legacy_app_passwords"], counts["no_token"], counts["reenter_external"],
            counts["unreadable_secret"], counts["already_migrated"], counts["unknown"],
            counts["internal_with_other_url"],
        )
        logger.info("Migration 068 completed")
    finally:
        cursor.close()


_PG_FK_RULES = {"a": "NO ACTION", "r": "RESTRICT", "c": "CASCADE", "n": "SET NULL", "d": "SET DEFAULT"}


def _foreign_keys(cursor, db_type: str, table: str, column: str, ref_table: str):
    """(constraint name, ON DELETE rule) for each FK on table.column that references ref_table."""
    if db_type == "postgresql":
        cursor.execute(
            """
            SELECT c.conname, c.confdeltype
            FROM pg_constraint c
            JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = ANY(c.conkey)
            WHERE c.contype = 'f' AND c.conrelid = %s::regclass AND c.confrelid = %s::regclass
              AND a.attname = %s
            """,
            (f'"{table}"', f'"{ref_table}"', column.lower()),
        )
        return [(name, _PG_FK_RULES.get(rule, rule)) for name, rule in cursor.fetchall()]
    cursor.execute(
        """
        SELECT rc.CONSTRAINT_NAME, rc.DELETE_RULE
        FROM INFORMATION_SCHEMA.REFERENTIAL_CONSTRAINTS rc
        JOIN INFORMATION_SCHEMA.KEY_COLUMN_USAGE k
          ON k.CONSTRAINT_SCHEMA = rc.CONSTRAINT_SCHEMA AND k.CONSTRAINT_NAME = rc.CONSTRAINT_NAME
         AND k.TABLE_NAME = rc.TABLE_NAME
        WHERE rc.CONSTRAINT_SCHEMA = DATABASE() AND rc.TABLE_NAME = %s
          AND rc.REFERENCED_TABLE_NAME = %s AND k.COLUMN_NAME = %s
        """,
        (table, ref_table, column),
    )
    return list(cursor.fetchall())


def _ensure_fk_set_null(cursor, db_type: str, table: str, column: str, ref_table: str, ref_column: str,
                        name: str, make_nullable: bool = False) -> None:
    """Make table.column's FK to ref_table ON DELETE SET NULL (looked up, not assumed by name)."""
    existing = _foreign_keys(cursor, db_type, table, column, ref_table)
    if existing and all(rule == "SET NULL" for _, rule in existing):
        return
    pg = db_type == "postgresql"
    qt = f'"{table}"' if pg else table
    for fk_name, rule in existing:
        if pg:
            cursor.execute(f'ALTER TABLE {qt} DROP CONSTRAINT "{fk_name}"')
        else:
            cursor.execute(f"ALTER TABLE {qt} DROP FOREIGN KEY `{fk_name}`")
        logger.info(f"Dropped FK {fk_name} on {table}.{column} (ON DELETE {rule})")
    if make_nullable:
        if pg:
            cursor.execute(f"ALTER TABLE {qt} ALTER COLUMN {column} DROP NOT NULL")
        else:
            cursor.execute(f"ALTER TABLE {qt} MODIFY COLUMN {column} INT NULL")
    qr = f'"{ref_table}"' if pg else ref_table
    cursor.execute(
        f"ALTER TABLE {qt} ADD CONSTRAINT {name} FOREIGN KEY ({column}) "
        f"REFERENCES {qr}({ref_column}) ON DELETE SET NULL"
    )
    logger.info(f"Added FK {name} on {table}.{column} ON DELETE SET NULL")


@register_migration("069", "unique_listen_history_per_episode", "Make UserEpisodeHistory (UserID, EpisodeID) and UserVideoHistory (UserID, VideoID) unique on MySQL, merging duplicates", requires=["006"])
def migration_069_unique_listen_history_per_episode(conn, db_type: str) -> None:
    """One history row per user and episode/video.

    Migration 006 added these UNIQUE constraints on PostgreSQL only, so on MySQL/MariaDB every
    `INSERT ... ON DUPLICATE KEY UPDATE` on the history tables (marking an episode completed, for
    one) inserted another row instead of updating, and readers saw whichever row came first.
    The sync core compares remote actions with ListenDate, so it needs the one true row.
    Duplicates are merged by keeping the most recent row (latest ListenDate, then highest id).
    Also covers a PostgreSQL install where 006's constraint couldn't be added.
    """
    logger.info("Starting migration 069: unique listen history per episode")
    cursor = conn.cursor()
    pg = db_type == "postgresql"
    try:
        for table, id_col, item_col, constraint in (
            ("UserEpisodeHistory", "UserEpisodeHistoryID", "EpisodeID", "user_episode_unique"),
            ("UserVideoHistory", "UserVideoHistoryID", "VideoID", "user_video_unique"),
        ):
            if check_constraint_exists(cursor, db_type, table, constraint):
                logger.info(f"Migration 069: {table} already has {constraint}")
                continue
            qt = f'"{table}"' if pg else table
            oldest = "'-infinity'::timestamp" if pg else "'1000-01-01 00:00:00'"
            newer = f"""
                k.UserID = h.UserID AND k.{item_col} = h.{item_col}
                AND (COALESCE(k.ListenDate, {oldest}) > COALESCE(h.ListenDate, {oldest})
                     OR (COALESCE(k.ListenDate, {oldest}) = COALESCE(h.ListenDate, {oldest})
                         AND k.{id_col} > h.{id_col}))
            """
            if pg:
                cursor.execute(f"DELETE FROM {qt} h USING {qt} k WHERE {newer}")
            else:
                cursor.execute(f"DELETE h FROM {qt} h JOIN {qt} k ON {newer}")
            logger.info(f"Migration 069: removed {cursor.rowcount} duplicate {table} rows")
            cursor.execute(f"ALTER TABLE {qt} ADD CONSTRAINT {constraint} UNIQUE (UserID, {item_col})")
            logger.info(f"Migration 069: added {constraint}")
        logger.info("Migration 069 completed")
    finally:
        cursor.close()


@register_migration("070", "gpodder_device_user_caption", "GpodderDevices.UserCaption: a name set in Pinepods that apps don't overwrite", requires=["008"])
def migration_070_gpodder_device_user_caption(conn, db_type: str) -> None:
    """Apps send their own caption whenever they configure their device, which replaced a name the
    user had given the device in Pinepods. The user's name now has its own column and wins."""
    cursor = conn.cursor()
    table = '"GpodderDevices"' if db_type == "postgresql" else "GpodderDevices"
    try:
        if not _column_exists(cursor, db_type, "GpodderDevices", "UserCaption"):
            cursor.execute(f"ALTER TABLE {table} ADD COLUMN UserCaption VARCHAR(255) NULL")
        conn.commit()
    except Exception:
        conn.rollback()
        raise
    finally:
        cursor.close()


@register_migration("071", "feed_health", "Podcasts.FailingSince/LastErrorKind/ProblemNotified and SyncImportFailures, for showing feeds that keep failing", requires=["001", "005", "044", "063"])
def migration_071_feed_health(conn, db_type: str) -> None:
    """Feed problems become visible.

    - Podcasts.FailingSince: when the current run of failed refreshes began (NULL when the last
      refresh worked); LastErrorKind: gone/auth/not_feed/unreachable/server/throttled/other;
      ProblemNotified: the user has been told about this run of failures.
    - SyncImportFailures: feeds a sync app subscribed to that Pinepods couldn't import, retried
      with backoff (NextAttemptAt NULL = gave up) until the user retries, removes or dismisses them.
    """
    cursor = conn.cursor()
    pg = db_type == "postgresql"
    podcasts = '"Podcasts"' if pg else "Podcasts"
    try:
        for column, ddl in (
            ("FailingSince", "TIMESTAMP" if pg else "DATETIME NULL DEFAULT NULL"),
            ("LastErrorKind", "VARCHAR(32)"),
            ("ProblemNotified", "BOOLEAN NOT NULL DEFAULT FALSE" if pg else "TINYINT(1) NOT NULL DEFAULT 0"),
        ):
            if not _column_exists(cursor, db_type, "Podcasts", column):
                cursor.execute(f"ALTER TABLE {podcasts} ADD COLUMN {column} {ddl}")
        # Feeds already failing count from their last attempt.
        cursor.execute(
            f"UPDATE {podcasts} SET FailingSince = LastRefreshAttempt "
            f"WHERE COALESCE(ConsecutiveFailures, 0) > 0 AND FailingSince IS NULL"
        )
        if pg:
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "SyncImportFailures" (
                    FailureID SERIAL PRIMARY KEY,
                    UserID INT NOT NULL REFERENCES "Users"(UserID) ON DELETE CASCADE,
                    FeedURL TEXT NOT NULL,
                    Origin VARCHAR(32) NOT NULL,
                    ErrorKind VARCHAR(32) NOT NULL,
                    LastError TEXT,
                    Attempts INT NOT NULL DEFAULT 1,
                    FirstFailedAt TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    LastAttemptAt TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    NextAttemptAt TIMESTAMP,
                    CONSTRAINT uq_sync_import_failures UNIQUE (UserID, FeedURL)
                )
            """)
            cursor.execute('CREATE INDEX IF NOT EXISTS idx_sync_import_failures_next ON "SyncImportFailures"(NextAttemptAt)')
        else:
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS SyncImportFailures (
                    FailureID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT NOT NULL,
                    FeedURL TEXT NOT NULL,
                    FeedURLHash CHAR(64) AS (SHA2(FeedURL, 256)) STORED,
                    Origin VARCHAR(32) NOT NULL,
                    ErrorKind VARCHAR(32) NOT NULL,
                    LastError TEXT,
                    Attempts INT NOT NULL DEFAULT 1,
                    FirstFailedAt DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    LastAttemptAt DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    NextAttemptAt DATETIME NULL DEFAULT NULL,
                    UNIQUE KEY uq_sync_import_failures (UserID, FeedURLHash),
                    KEY idx_sync_import_failures_next (NextAttemptAt),
                    CONSTRAINT fk_sync_import_failures_user FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE
                )
            """)
        conn.commit()
    except Exception:
        conn.rollback()
        raise
    finally:
        cursor.close()


@register_migration("072", "podping_setting", "AppSettings.PodpingEnabled: refresh feeds when Podping announces an update (off by default)", requires=["002"])
def migration_072_podping_setting(conn, db_type: str) -> None:
    """Podping is opt-in per server: when on, Pinepods asks the search service which feeds Podping
    announced and refreshes the ones it has right away."""
    cursor = conn.cursor()
    pg = db_type == "postgresql"
    table = '"AppSettings"' if pg else "AppSettings"
    try:
        if not _column_exists(cursor, db_type, "AppSettings", "PodpingEnabled"):
            ddl = "BOOLEAN NOT NULL DEFAULT false" if pg else "TINYINT(1) NOT NULL DEFAULT 0"
            cursor.execute(f"ALTER TABLE {table} ADD COLUMN PodpingEnabled {ddl}")
        conn.commit()
    except Exception:
        conn.rollback()
        raise
    finally:
        cursor.close()


@register_migration("073", "podcasts_feedurl_index", "Index Podcasts.FeedURL, for finding a feed's subscriptions across users", requires=["001"])
def migration_073_podcasts_feedurl_index(conn, db_type: str) -> None:
    """Podping and the shared refresh look podcasts up by feed URL for every user. The
    (UserID, FeedURL) unique key can't serve that on Postgres and MySQL had no index at all.
    Postgres gets a hash index, which takes URLs of any length."""
    cursor = conn.cursor()
    try:
        if db_type == "postgresql":
            safe_add_index(cursor, db_type, 'CREATE INDEX idx_podcasts_feedurl ON "Podcasts" USING hash (feedurl)', "idx_podcasts_feedurl")
        else:
            safe_add_index(cursor, db_type, "CREATE INDEX idx_podcasts_feedurl ON Podcasts (FeedURL(255))", "idx_podcasts_feedurl")
        conn.commit()
    except Exception:
        conn.rollback()
        raise
    finally:
        cursor.close()


@register_migration("075", "adaptive_refresh_schedule", "Podcasts.NextRefreshAt/RefreshIntervalMinutes: each feed is refreshed on its own schedule", requires=["001", "044"])
def migration_075_adaptive_refresh_schedule(conn, db_type: str) -> None:
    """Feeds are refreshed when they're due instead of all at once.

    - NextRefreshAt: when the feed is refreshed next. New rows default to a few minutes out, so
      the import that adds a podcast finishes (and schedules it) before a refresh picks it up.
    - RefreshIntervalMinutes: the interval its publishing pattern earned at the last refresh.

    Existing feeds get an interval from their newest ten episodes (the same rule the refresh
    uses, without the feed's own hints) and their first refresh is spread over the next 30
    minutes, the old refresh cycle.
    """
    import datetime

    cursor = conn.cursor()
    pg = db_type == "postgresql"
    podcasts = '"Podcasts"' if pg else "Podcasts"
    try:
        added = not _column_exists(cursor, db_type, "Podcasts", "NextRefreshAt")
        if added:
            ddl = ("TIMESTAMP DEFAULT (CURRENT_TIMESTAMP + INTERVAL '5 minutes')" if pg
                   else "DATETIME NULL DEFAULT (CURRENT_TIMESTAMP + INTERVAL 5 MINUTE)")
            cursor.execute(f"ALTER TABLE {podcasts} ADD COLUMN NextRefreshAt {ddl}")
        if not _column_exists(cursor, db_type, "Podcasts", "RefreshIntervalMinutes"):
            cursor.execute(f"ALTER TABLE {podcasts} ADD COLUMN RefreshIntervalMinutes INT")
        safe_add_index(cursor, db_type, f"CREATE INDEX idx_podcasts_nextrefreshat ON {podcasts} (NextRefreshAt)", "idx_podcasts_nextrefreshat")

        # Newest ten publishing dates per podcast, episodes or YouTube videos.
        dates = {}
        for table, date_col in (("Episodes", "EpisodePubDate"), ("YouTubeVideos", "PublishedAt")):
            if not _table_exists(cursor, db_type, table):
                continue
            name = f'"{table}"' if pg else table
            cursor.execute(
                f"SELECT PodcastID, {date_col} FROM ("
                f" SELECT PodcastID, {date_col}, ROW_NUMBER() OVER (PARTITION BY PodcastID ORDER BY {date_col} DESC) AS rn"
                f" FROM {name} WHERE {date_col} IS NOT NULL) recent WHERE rn <= 10"
            )
            for podcast_id, published in cursor.fetchall():
                dates.setdefault(podcast_id, []).append(published.replace(tzinfo=None))

        now = datetime.datetime.utcnow()
        cursor.execute(f"SELECT PodcastID FROM {podcasts} WHERE RefreshIntervalMinutes IS NULL")
        by_interval = {}
        for (podcast_id,) in cursor.fetchall():
            by_interval.setdefault(_refresh_interval_minutes(dates.get(podcast_id, []), now), []).append(podcast_id)
        for minutes, ids in by_interval.items():
            for i in range(0, len(ids), 500):
                chunk = ids[i:i + 500]
                marks = ", ".join(["%s"] * len(chunk))
                cursor.execute(
                    f"UPDATE {podcasts} SET RefreshIntervalMinutes = %s WHERE PodcastID IN ({marks})",
                    (minutes, *chunk),
                )
        if added:
            spread = "(PodcastID % 30) * INTERVAL '1 minute'" if pg else "INTERVAL (PodcastID % 30) MINUTE"
            cursor.execute(f"UPDATE {podcasts} SET NextRefreshAt = CURRENT_TIMESTAMP + {spread}")
        conn.commit()
    except Exception:
        conn.rollback()
        raise
    finally:
        cursor.close()


def _refresh_interval_minutes(published, now) -> int:
    """A feed's refresh interval from its newest publishing dates: an eighth of the median gap,
    15 minutes to 6 hours, 24 hours when nothing came out in 90 days. Matches the refresh."""
    import datetime

    dates = sorted(set(published), reverse=True)[:10]
    if not dates or now - dates[0] > datetime.timedelta(days=90):
        return 24 * 60
    gaps = sorted((a - b).total_seconds() for a, b in zip(dates, dates[1:]))
    if not gaps:
        return 60
    mid = len(gaps) // 2
    median = gaps[mid] if len(gaps) % 2 else (gaps[mid - 1] + gaps[mid]) / 2
    return int(min(max(median / 8 / 60, 15), 6 * 60))


@register_migration("080", "create_feeds", "Feeds: one row per feed address and credentials, holding the refresh state its subscriptions share", requires=["044", "063", "071", "075"])
def migration_080_create_feeds(conn, db_type: str) -> None:
    """Feeds: one row per feed, shared by every user subscribed to it, with the refresh state
    (validators, schedule, failures). Podcasts.FeedID links a subscription to its feed.

    A feed is its address (FeedURLHash, the SHA-256 of FeedURL, carries the unique key; the server
    fills it in) plus a credential key: '' for public feeds, the SHA-256 of
    username + NUL + password for private ones (same address, other credentials = another feed),
    and the SHA-256 of 'local' + NUL + user id for anything that isn't http(s), so uploads are
    never shared. The refresh state of existing subscriptions is merged per feed: the earliest
    next refresh and shortest interval, the stored validators, the body hash only when every
    subscription agrees, and the healthiest failure state.
    """
    cursor = conn.cursor()
    pg = db_type == "postgresql"
    podcasts = '"Podcasts"' if pg else "Podcasts"
    feeds = '"Feeds"' if pg else "Feeds"
    try:
        if pg:
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "Feeds" (
                    FeedID SERIAL PRIMARY KEY,
                    FeedURL TEXT NOT NULL,
                    FeedURLHash CHAR(64) NOT NULL,
                    CredentialKey VARCHAR(64) NOT NULL DEFAULT '',
                    IsYouTubeChannel BOOLEAN NOT NULL DEFAULT FALSE,
                    FeedETag TEXT,
                    FeedLastModified TEXT,
                    FeedContentHash TEXT,
                    RefreshNotBefore TIMESTAMP,
                    NextRefreshAt TIMESTAMP DEFAULT (CURRENT_TIMESTAMP + INTERVAL '5 minutes'),
                    RefreshIntervalMinutes INT,
                    LastRefreshAttempt TIMESTAMP,
                    LastRefreshSuccess TIMESTAMP,
                    ConsecutiveFailures INT NOT NULL DEFAULT 0,
                    LastErrorMessage TEXT,
                    FailingSince TIMESTAMP,
                    LastErrorKind VARCHAR(32),
                    CreatedAt TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    CONSTRAINT uq_feeds_url_credentials UNIQUE (FeedURLHash, CredentialKey)
                )
            """)
        else:
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS Feeds (
                    FeedID INT AUTO_INCREMENT PRIMARY KEY,
                    FeedURL TEXT NOT NULL,
                    FeedURLHash CHAR(64) NOT NULL,
                    CredentialKey VARCHAR(64) NOT NULL DEFAULT '',
                    IsYouTubeChannel TINYINT(1) NOT NULL DEFAULT 0,
                    FeedETag TEXT,
                    FeedLastModified TEXT,
                    FeedContentHash TEXT,
                    RefreshNotBefore DATETIME NULL DEFAULT NULL,
                    NextRefreshAt DATETIME NULL DEFAULT (CURRENT_TIMESTAMP + INTERVAL 5 MINUTE),
                    RefreshIntervalMinutes INT,
                    LastRefreshAttempt DATETIME NULL DEFAULT NULL,
                    LastRefreshSuccess DATETIME NULL DEFAULT NULL,
                    ConsecutiveFailures INT NOT NULL DEFAULT 0,
                    LastErrorMessage TEXT,
                    FailingSince DATETIME NULL DEFAULT NULL,
                    LastErrorKind VARCHAR(32),
                    CreatedAt DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    UNIQUE KEY uq_feeds_url_credentials (FeedURLHash, CredentialKey)
                )
            """)
        safe_add_index(cursor, db_type, f"CREATE INDEX idx_feeds_nextrefreshat ON {feeds} (NextRefreshAt)", "idx_feeds_nextrefreshat")

        if not _column_exists(cursor, db_type, "Podcasts", "FeedID"):
            cursor.execute(f"ALTER TABLE {podcasts} ADD COLUMN FeedID INT NULL")
        safe_add_index(cursor, db_type, f"CREATE INDEX idx_podcasts_feedid ON {podcasts} (FeedID)", "idx_podcasts_feedid")
        if pg:
            cursor.execute("""
                SELECT COUNT(*) FROM information_schema.table_constraints
                WHERE table_name = 'Podcasts' AND constraint_name = 'fk_podcasts_feed'
            """)
        else:
            cursor.execute("""
                SELECT COUNT(*) FROM INFORMATION_SCHEMA.TABLE_CONSTRAINTS
                WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'Podcasts' AND CONSTRAINT_NAME = 'fk_podcasts_feed'
            """)
        if not cursor.fetchone()[0]:
            cursor.execute(
                f"ALTER TABLE {podcasts} ADD CONSTRAINT fk_podcasts_feed FOREIGN KEY (FeedID) "
                f"REFERENCES {feeds}(FeedID) ON DELETE SET NULL"
            )

        linked = _link_podcasts_to_feeds(cursor, db_type)
        logger.info(f"Migration 080: linked {linked} podcasts to their feeds")
        conn.commit()
    except Exception:
        conn.rollback()
        raise
    finally:
        cursor.close()


def _feed_credential_key(feed_url, username, password, user_id) -> str:
    """Matches services::feeds::credential_key in rust-api."""
    import hashlib

    url = (feed_url or "").strip().lower()
    if not (url.startswith("http://") or url.startswith("https://")):
        return hashlib.sha256(f"local\0{user_id}".encode()).hexdigest()
    if not username and not password:
        return ""
    return hashlib.sha256(f"{username or ''}\0{password or ''}".encode()).hexdigest()


def _link_podcasts_to_feeds(cursor, db_type: str) -> int:
    """Create the feed of every podcast without one and link it. Returns how many were linked."""
    import hashlib

    pg = db_type == "postgresql"
    podcasts = '"Podcasts"' if pg else "Podcasts"
    feeds = '"Feeds"' if pg else "Feeds"
    state_cols = ["FeedETag", "FeedLastModified", "FeedContentHash", "RefreshNotBefore", "NextRefreshAt",
                  "RefreshIntervalMinutes", "LastRefreshAttempt", "LastRefreshSuccess", "ConsecutiveFailures",
                  "LastErrorMessage", "FailingSince", "LastErrorKind"]
    cursor.execute(
        f"SELECT PodcastID, FeedURL, Username, Password, UserID, COALESCE(IsYouTubeChannel, FALSE), "
        f"{', '.join(state_cols)} FROM {podcasts} WHERE FeedID IS NULL AND FeedURL IS NOT NULL AND FeedURL <> ''"
    )
    groups = {}
    for row in cursor.fetchall():
        podcast_id, url, username, password, user_id, youtube = row[:6]
        state = dict(zip(state_cols, row[6:]))
        key = (url, _feed_credential_key(url, username, password, user_id))
        groups.setdefault(key, {"youtube": bool(youtube), "rows": []})["rows"].append((podcast_id, state))
    if not groups:
        return 0

    def earliest(values):
        values = [v for v in values if v is not None]
        return min(values) if values else None

    def latest(values):
        values = [v for v in values if v is not None]
        return max(values) if values else None

    linked = 0
    for (url, cred_key), group in groups.items():
        states = [s for _, s in group["rows"]]
        hashes = {s["FeedContentHash"] for s in states}
        healthiest = min(states, key=lambda s: s["ConsecutiveFailures"] or 0)
        merged = {
            "FeedETag": next((s["FeedETag"] for s in states if s["FeedETag"]), None),
            "FeedLastModified": next((s["FeedLastModified"] for s in states if s["FeedLastModified"]), None),
            "FeedContentHash": hashes.pop() if len(hashes) == 1 else None,
            "RefreshNotBefore": latest(s["RefreshNotBefore"] for s in states),
            "NextRefreshAt": earliest(s["NextRefreshAt"] for s in states),
            "RefreshIntervalMinutes": earliest(s["RefreshIntervalMinutes"] for s in states),
            "LastRefreshAttempt": latest(s["LastRefreshAttempt"] for s in states),
            "LastRefreshSuccess": latest(s["LastRefreshSuccess"] for s in states),
            "ConsecutiveFailures": healthiest["ConsecutiveFailures"] or 0,
            "LastErrorMessage": healthiest["LastErrorMessage"],
            "FailingSince": healthiest["FailingSince"],
            "LastErrorKind": healthiest["LastErrorKind"],
        }
        url_hash = hashlib.sha256(url.encode()).hexdigest()
        cursor.execute(
            f"SELECT FeedID FROM {feeds} WHERE FeedURLHash = %s AND CredentialKey = %s",
            (url_hash, cred_key),
        )
        found = cursor.fetchone()
        if found:
            feed_id = found[0]
        else:
            cols = ["FeedURL", "FeedURLHash", "CredentialKey", "IsYouTubeChannel"] + state_cols
            values = [url, url_hash, cred_key, group["youtube"]] + [merged[c] for c in state_cols]
            marks = ", ".join(["%s"] * len(cols))
            if pg:
                cursor.execute(f"INSERT INTO {feeds} ({', '.join(cols)}) VALUES ({marks}) RETURNING FeedID", values)
                feed_id = cursor.fetchone()[0]
            else:
                cursor.execute(f"INSERT INTO {feeds} ({', '.join(cols)}) VALUES ({marks})", values)
                feed_id = cursor.lastrowid
        ids = [podcast_id for podcast_id, _ in group["rows"]]
        for i in range(0, len(ids), 500):
            chunk = ids[i:i + 500]
            cursor.execute(
                f"UPDATE {podcasts} SET FeedID = %s WHERE PodcastID IN ({', '.join(['%s'] * len(chunk))})",
                (feed_id, *chunk),
            )
        linked += len(ids)
    return linked


@register_migration("111", "gpodder_episode_action_log", "Episode action log: Guid, resolved EpisodeID, Origin, indexes; keep actions when a device is removed", requires=["066", "110"])
def migration_111_gpodder_episode_action_log(conn, db_type: str) -> None:
    """Turn GpodderSyncEpisodeActions into the sync log/outbox (sync overhaul 3.2).

    - Guid: the episode GUID when the client sends one.
    - EpisodeID: the resolved local episode (NULL = unmatched). Backfilled here by exact
      EpisodeURL within the user's podcasts; GUID and URL-base matching is left to the
      sync resolver. FK ON DELETE SET NULL, so removing a podcast leaves its actions unmatched.
    - Origin: 'client' (gpodder clients; the default, so the Go server's INSERTs keep working),
      'pinepods', or 'target:<TargetID>'.
    - DeviceID FK CASCADE -> SET NULL: removing a device no longer erases its history.
    """
    logger.info("Starting migration 111: episode action log")
    cursor = conn.cursor()
    pg = db_type == "postgresql"
    name = "GpodderSyncEpisodeActions"
    table = f'"{name}"' if pg else name
    try:
        if not _column_exists(cursor, db_type, name, "Guid"):
            cursor.execute(f"ALTER TABLE {table} ADD COLUMN Guid TEXT NULL")
        if not _column_exists(cursor, db_type, name, "EpisodeID"):
            cursor.execute(f"ALTER TABLE {table} ADD COLUMN EpisodeID INT NULL")
        if not _column_exists(cursor, db_type, name, "Origin"):
            cursor.execute(f"ALTER TABLE {table} ADD COLUMN Origin VARCHAR(32) NOT NULL DEFAULT 'client'")

        cursor.execute(f"SELECT COUNT(*) FROM {table} WHERE EpisodeID IS NULL")
        logger.info(f"Migration 111: matching {cursor.fetchone()[0]} episode actions to episodes (a few minutes for millions)")
        if pg:
            cursor.execute(f"""
                UPDATE {table} a SET EpisodeID = (
                    SELECT MIN(e.EpisodeID) FROM "Episodes" e
                    JOIN "Podcasts" p ON p.PodcastID = e.PodcastID
                    WHERE p.UserID = a.UserID AND e.EpisodeURL = a.EpisodeURL
                ) WHERE a.EpisodeID IS NULL
            """)
        else:
            cursor.execute(f"""
                UPDATE {table} a SET a.EpisodeID = (
                    SELECT MIN(e.EpisodeID) FROM Episodes e
                    JOIN Podcasts p ON p.PodcastID = e.PodcastID
                    WHERE p.UserID = a.UserID AND e.EpisodeURLHash = SHA2(a.EpisodeURL, 256)
                      AND e.EpisodeURL = a.EpisodeURL
                ) WHERE a.EpisodeID IS NULL
            """)
        cursor.execute(f"SELECT COUNT(*), COUNT(EpisodeID) FROM {table}")
        total, matched = cursor.fetchone()
        logger.info(f"Migration 111: {matched} of {total} episode actions matched to an episode by URL")

        # Indexes before the FKs, so MySQL reuses them instead of adding its own. (UserID,
        # ActionID) is PG only: InnoDB secondary indexes carry the primary key, so the existing
        # (UserID) index already is that index there.
        if pg:
            safe_add_index(cursor, db_type,
                           f"CREATE INDEX idx_gpodder_episode_actions_user_actionid ON {table}(UserID, ActionID)",
                           "idx_gpodder_episode_actions_user_actionid")
        safe_add_index(cursor, db_type,
                       f"CREATE INDEX idx_gpodder_episode_actions_episodeid ON {table}(EpisodeID, UserID)",
                       "idx_gpodder_episode_actions_episodeid")
        if pg:
            safe_add_index(cursor, db_type,
                           f"CREATE INDEX idx_gpodder_episode_actions_unmatched ON {table}(UserID) WHERE EpisodeID IS NULL",
                           "idx_gpodder_episode_actions_unmatched")
        else:
            safe_add_index(cursor, db_type,
                           f"CREATE INDEX idx_gpodder_episode_actions_unmatched ON {table}(UserID, EpisodeID)",
                           "idx_gpodder_episode_actions_unmatched")

        _ensure_fk_set_null(cursor, db_type, name, "DeviceID", "GpodderDevices", "DeviceID",
                            "fk_gpodder_episode_actions_device")
        _ensure_fk_set_null(cursor, db_type, name, "EpisodeID", "Episodes", "EpisodeID",
                            "fk_gpodder_episode_actions_episode")
        logger.info("Migration 111 completed")
    finally:
        cursor.close()


@register_migration("112", "gpodder_subscription_log", "Subscription log: ServerTs, Origin, indexes, SET NULL device FK; GpodderDeviceSubscriptions", requires=["100"])
def migration_112_gpodder_subscription_log(conn, db_type: str) -> None:
    """Turn GpodderSyncSubscriptions into the sync log/outbox and add per-device lists.

    - ServerTs: server receive time, the download cursor. Backfilled from Timestamp, which
      the Go server already stamps with its own clock. NULLable for now because the Go server
      doesn't write it; migration 114 makes it NOT NULL.
    - Origin: as in migration 111.
    - DeviceID becomes NULLable with FK ON DELETE SET NULL, so removing a device keeps its
      (possibly not yet pushed) log rows.
    - GpodderDeviceSubscriptions: each device's last known list, so a PUT only changes what that device reported.
      Backfilled by replaying the log per device: the last action per URL wins.
    """
    logger.info("Starting migration 112: subscription log + GpodderDeviceSubscriptions")
    cursor = conn.cursor()
    pg = db_type == "postgresql"
    name = "GpodderSyncSubscriptions"
    table = f'"{name}"' if pg else name
    try:
        if not _column_exists(cursor, db_type, name, "ServerTs"):
            cursor.execute(f"ALTER TABLE {table} ADD COLUMN ServerTs BIGINT NULL")
        cursor.execute(f"UPDATE {table} SET ServerTs = Timestamp WHERE ServerTs IS NULL")
        if not _column_exists(cursor, db_type, name, "Origin"):
            cursor.execute(f"ALTER TABLE {table} ADD COLUMN Origin VARCHAR(32) NOT NULL DEFAULT 'client'")

        safe_add_index(cursor, db_type,
                       f"CREATE INDEX idx_gpodder_subscriptions_user_serverts ON {table}(UserID, ServerTs)",
                       "idx_gpodder_subscriptions_user_serverts")
        if pg:  # InnoDB: the existing (UserID) index already ends in the primary key
            safe_add_index(cursor, db_type,
                           f"CREATE INDEX idx_gpodder_subscriptions_user_subid ON {table}(UserID, SubscriptionID)",
                           "idx_gpodder_subscriptions_user_subid")
        _ensure_fk_set_null(cursor, db_type, name, "DeviceID", "GpodderDevices", "DeviceID",
                            "fk_gpodder_subscriptions_device", make_nullable=True)

        if pg:
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS "GpodderDeviceSubscriptions" (
                    DeviceID INT NOT NULL REFERENCES "GpodderDevices"(DeviceID) ON DELETE CASCADE,
                    PodcastURL TEXT NOT NULL,
                    PRIMARY KEY (DeviceID, PodcastURL)
                )
            """)
            cursor.execute("""
                INSERT INTO "GpodderDeviceSubscriptions" (DeviceID, PodcastURL)
                SELECT DeviceID, PodcastURL FROM (
                    SELECT DeviceID, PodcastURL, Action,
                           ROW_NUMBER() OVER (PARTITION BY DeviceID, PodcastURL
                                              ORDER BY Timestamp DESC, SubscriptionID DESC) AS rn
                    FROM "GpodderSyncSubscriptions" WHERE DeviceID IS NOT NULL
                ) latest
                WHERE rn = 1 AND LOWER(Action) = 'add'
                ON CONFLICT DO NOTHING
            """)
        else:
            # TEXT can't be a key on MySQL: unique on a generated SHA2 of the URL instead.
            cursor.execute("""
                CREATE TABLE IF NOT EXISTS GpodderDeviceSubscriptions (
                    DeviceSubscriptionID INT AUTO_INCREMENT PRIMARY KEY,
                    DeviceID INT NOT NULL,
                    PodcastURL TEXT NOT NULL,
                    PodcastURLHash CHAR(64) AS (SHA2(PodcastURL, 256)) STORED,
                    UNIQUE KEY uq_gpodder_device_subscriptions (DeviceID, PodcastURLHash),
                    CONSTRAINT fk_gpodder_device_subscriptions_device FOREIGN KEY (DeviceID)
                        REFERENCES GpodderDevices(DeviceID) ON DELETE CASCADE
                )
            """)
            cursor.execute("""
                INSERT IGNORE INTO GpodderDeviceSubscriptions (DeviceID, PodcastURL)
                SELECT DeviceID, PodcastURL FROM (
                    SELECT DeviceID, PodcastURL, Action,
                           ROW_NUMBER() OVER (PARTITION BY DeviceID, SHA2(PodcastURL, 256)
                                              ORDER BY Timestamp DESC, SubscriptionID DESC) AS rn
                    FROM GpodderSyncSubscriptions WHERE DeviceID IS NOT NULL
                ) latest
                WHERE rn = 1 AND LOWER(Action) = 'add'
            """)
        dsub = '"GpodderDeviceSubscriptions"' if pg else "GpodderDeviceSubscriptions"
        cursor.execute(f"SELECT COUNT(*), COUNT(DISTINCT DeviceID) FROM {dsub}")
        rows, devices = cursor.fetchone()
        logger.info(f"Migration 112: backfilled {rows} device subscriptions across {devices} devices")
        logger.info("Migration 112 completed")
    finally:
        cursor.close()


@register_migration("113", "gpodder_pinepods_device", "GpodderDevices.SystemRole marks Pinepods' own device per user", requires=["008"])
def migration_113_gpodder_pinepods_device(conn, db_type: str) -> None:
    """Identify Pinepods' own gpodder device by role rather than by name.

    DeviceName is chosen by clients and UNIQUE(UserID, DeviceName), so a name alone can't be
    trusted to mean "Pinepods". SystemRole = 'pinepods' marks it, unique per user (NULLs don't
    collide). The device itself is created on demand by the sync core. The existing
    internal device the current Rust sync uses ('pinepods-internal-<UserID>') is marked now, so
    Pinepods keeps its device history and no second Pinepods device appears.
    """
    logger.info("Starting migration 113: GpodderDevices.SystemRole")
    cursor = conn.cursor()
    pg = db_type == "postgresql"
    table = '"GpodderDevices"' if pg else "GpodderDevices"
    try:
        if not _column_exists(cursor, db_type, "GpodderDevices", "SystemRole"):
            cursor.execute(f"ALTER TABLE {table} ADD COLUMN SystemRole VARCHAR(16) NULL")
        safe_add_index(cursor, db_type,
                       f"CREATE UNIQUE INDEX idx_gpodder_devices_user_systemrole ON {table}(UserID, SystemRole)",
                       "idx_gpodder_devices_user_systemrole")
        name_expr = "'pinepods-internal-' || CAST(UserID AS TEXT)" if pg else "CONCAT('pinepods-internal-', UserID)"
        cursor.execute(
            f"UPDATE {table} SET SystemRole = 'pinepods' WHERE SystemRole IS NULL AND DeviceName = {name_expr}"
        )
        logger.info(f"Migration 113: marked {cursor.rowcount} existing internal devices as the Pinepods device")
        logger.info("Migration 113 completed")
    finally:
        cursor.close()


@register_migration("114", "gpodder_subscription_server_ts_not_null", "Subscription log: backfill ServerTs and make it NOT NULL", requires=["112"])
def migration_114_gpodder_subscription_server_ts_not_null(conn, db_type: str) -> None:
    """Backfill GpodderSyncSubscriptions.ServerTs from Timestamp and make it NOT NULL."""
    logger.info("Starting migration 114: GpodderSyncSubscriptions.ServerTs NOT NULL")
    cursor = conn.cursor()
    pg = db_type == "postgresql"
    table = '"GpodderSyncSubscriptions"' if pg else "GpodderSyncSubscriptions"
    try:
        cursor.execute(f"UPDATE {table} SET ServerTs = Timestamp WHERE ServerTs IS NULL")
        logger.info(f"Migration 114: backfilled ServerTs on {cursor.rowcount} subscription log rows")
        if pg:
            cursor.execute(f"ALTER TABLE {table} ALTER COLUMN ServerTs SET NOT NULL")
        else:
            cursor.execute(f"ALTER TABLE {table} MODIFY COLUMN ServerTs BIGINT NOT NULL")
        logger.info("Migration 114 completed")
    finally:
        cursor.close()


@register_migration("115", "gpodder_download_cursors", "Remember the cursor each gpodder device/session was last given by a download", requires=["008", "102"])
def migration_115_gpodder_download_cursors(conn, db_type: str) -> None:
    """Last download cursor per device (subscriptions) and per session (episode actions), returned
    by uploads so a client doesn't skip changes made between its download and upload. Also
    indexes GpodderSessions.SessionToken on MySQL, which had no index."""
    logger.info("Starting migration 115: gpodder download cursors")
    cursor = conn.cursor()
    pg = db_type == "postgresql"
    devices = '"GpodderDevices"' if pg else "GpodderDevices"
    sessions = '"GpodderSessions"' if pg else "GpodderSessions"
    try:
        if not _column_exists(cursor, db_type, "GpodderDevices", "SubsDownloadCursor"):
            cursor.execute(f"ALTER TABLE {devices} ADD COLUMN SubsDownloadCursor BIGINT NULL")
        if not _column_exists(cursor, db_type, "GpodderSessions", "EpisodesDownloadCursor"):
            cursor.execute(f"ALTER TABLE {sessions} ADD COLUMN EpisodesDownloadCursor BIGINT NULL")
        if not pg:
            safe_add_index(cursor, db_type,
                           "CREATE INDEX idx_gpodder_sessions_token ON GpodderSessions(SessionToken(64))",
                           "idx_gpodder_sessions_token")
        logger.info("Migration 115 completed")
    finally:
        cursor.close()


@register_migration("116", "gpodder_session_app_password", "Sessions remember the app password they were opened with, so revoking it ends them", requires=["067", "102"])
def migration_116_gpodder_session_app_password(conn, db_type: str) -> None:
    """GpodderSessions.AppPasswordID: the app password a session was opened with (NULL for the
    user's own password). Deleting the app password deletes its sessions. The column is in 102's
    CREATE TABLE; the FK is added here because GpodderAppPasswords comes from 067."""
    logger.info("Starting migration 116: gpodder session app password")
    cursor = conn.cursor()
    pg = db_type == "postgresql"
    sessions = '"GpodderSessions"' if pg else "GpodderSessions"
    passwords = '"GpodderAppPasswords"' if pg else "GpodderAppPasswords"
    try:
        if not _column_exists(cursor, db_type, "GpodderSessions", "AppPasswordID"):
            cursor.execute(f"ALTER TABLE {sessions} ADD COLUMN AppPasswordID INT NULL")
        if not _foreign_keys(cursor, db_type, "GpodderSessions", "AppPasswordID", "GpodderAppPasswords"):
            cursor.execute(
                f"ALTER TABLE {sessions} ADD CONSTRAINT fk_gpodder_sessions_app_password FOREIGN KEY (AppPasswordID) "
                f"REFERENCES {passwords}(AppPasswordID) ON DELETE CASCADE"
            )
        logger.info("Migration 116 completed")
    finally:
        cursor.close()
