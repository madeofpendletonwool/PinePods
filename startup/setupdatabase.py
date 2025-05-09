import mysql.connector
import os
import sys
from cryptography.fernet import Fernet
import string
import secrets
import logging
import random
from argon2 import PasswordHasher
from argon2.exceptions import HashingError
from passlib.hash import argon2

# Set up basic configuration for logging
logging.basicConfig(level=logging.ERROR, format='%(asctime)s - %(levelname)s - %(message)s')

# Append the pinepods directory to sys.path for module import
sys.path.append('/pinepods')

try:
    # Attempt to import additional modules
    import database_functions.functions
    # import Auth.Passfunctions

    def hash_password(password: str):
        # Hash the password
        hashed_password = argon2.hash(password)
        # Argon2 includes the salt in the hashed output
        return hashed_password

    # Retrieve database connection details from environment variables
    db_host = os.environ.get("DB_HOST", "127.0.0.1")
    db_port = os.environ.get("DB_PORT", "3306")
    db_user = os.environ.get("DB_USER", "root")
    db_password = os.environ.get("DB_PASSWORD", "password")
    db_name = os.environ.get("DB_NAME", "pypods_database")

    # Attempt to create a database connector
    cnx = mysql.connector.connect(
        host=db_host,
        port=db_port,
        user=db_user,
        password=db_password,
        database=db_name,
        charset='utf8mb4',
        collation="utf8mb4_general_ci"
    )

    # Create a cursor to execute SQL statements
    cursor = cnx.cursor()

    # Function to ensure all usernames are lowercase
    def ensure_usernames_lowercase(cnx):
        cursor = cnx.cursor()
        cursor.execute('SELECT UserID, Username FROM Users')
        users = cursor.fetchall()
        for user_id, username in users:
            if username != username.lower():
                cursor.execute('UPDATE Users SET Username = %s WHERE UserID = %s', (username.lower(), user_id))
                print(f"Updated Username for UserID {user_id} to lowercase")
        cnx.commit()
        cursor.close()

    # Function to check and add columns if they don't exist
    def add_column_if_not_exists(cursor, table_name, column_name, column_definition):
        cursor.execute(f"""
            SELECT COUNT(*)
            FROM information_schema.columns
            WHERE table_name='{table_name}'
            AND column_name='{column_name}'
            AND table_schema=DATABASE();
        """)
        if cursor.fetchone()[0] == 0:
            cursor.execute(f"""
                ALTER TABLE {table_name}
                ADD COLUMN {column_name} {column_definition};
            """)
            print(f"Column '{column_name}' added to table '{table_name}'")
        else:
            return

    # Create Users table if it doesn't exist (your existing code)
    cursor.execute("""
        CREATE TABLE IF NOT EXISTS Users (
            UserID INT AUTO_INCREMENT PRIMARY KEY,
            Fullname VARCHAR(255),
            Username VARCHAR(255),
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
            UNIQUE (Username)
        )
    """)

    # Create OIDCProviders table if it doesn't exist (MySQL version)
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
    cnx.commit()

    # Add new columns to Users table if they don't exist
    add_column_if_not_exists(cursor, 'Users', 'auth_type', 'VARCHAR(50) DEFAULT \'standard\'')
    add_column_if_not_exists(cursor, 'Users', 'oidc_provider_id', 'INT')
    add_column_if_not_exists(cursor, 'Users', 'oidc_subject', 'VARCHAR(255)')

    # Check if foreign key exists before adding it
    cursor.execute("""
        SELECT COUNT(*)
        FROM information_schema.table_constraints
        WHERE constraint_name = 'fk_oidc_provider'
        AND table_name = 'Users'
        AND table_schema = DATABASE();
    """)
    if cursor.fetchone()[0] == 0:
        cursor.execute("""
            ALTER TABLE Users
            ADD CONSTRAINT fk_oidc_provider
            FOREIGN KEY (oidc_provider_id)
            REFERENCES OIDCProviders(ProviderID);
        """)
        print("Foreign key constraint 'fk_oidc_provider' added")

    # Add EnableRSSFeeds column if it doesn't exist
    cursor.execute("""
        SELECT COUNT(*)
        FROM information_schema.columns
        WHERE table_name = 'Users'
        AND column_name = 'EnableRSSFeeds'
    """)
    if cursor.fetchone()[0] == 0:
        cursor.execute("ALTER TABLE Users ADD COLUMN EnableRSSFeeds TINYINT(1) DEFAULT 0")


    ensure_usernames_lowercase(cnx)

    def add_pod_sync_if_not_exists(cursor, table_name, column_name, column_definition):
        cursor.execute(f"""
            SELECT COUNT(*)
            FROM information_schema.columns
            WHERE table_name='{table_name}'
            AND column_name='{column_name}';
        """)
        if cursor.fetchone()[0] == 0:
            cursor.execute(f"""
                ALTER TABLE {table_name}
                ADD COLUMN {column_name} {column_definition};
            """)
            print(f"Column '{column_name}' added to table '{table_name}'")
        else:
            return

    add_pod_sync_if_not_exists(cursor, 'Users', 'Pod_Sync_Type', 'VARCHAR(50) DEFAULT \'None\'')


    cursor.execute("""CREATE TABLE IF NOT EXISTS APIKeys (
                        APIKeyID INT AUTO_INCREMENT PRIMARY KEY,
                        UserID INT,
                        APIKey TEXT,
                        RssOnly TINYINT DEFAULT 0,
                        Created TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                        FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE
                    )""")


    try:
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
        cnx.commit()
        print("Created GpodderDevices table")

        # Create index for faster lookups
        cursor.execute("""
            CREATE INDEX idx_gpodder_devices_userid
            ON GpodderDevices(UserID)
        """)
        cnx.commit()

        # Create a table for subscription history/sync state
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
        cnx.commit()
        print("Created GpodderSyncState table")
    except Exception as e:
        print(f"Error creating GPodder tables: {e}")

    cursor.execute("""CREATE TABLE IF NOT EXISTS UserStats (
                        UserStatsID INT AUTO_INCREMENT PRIMARY KEY,
                        UserID INT,
                        UserCreated TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                        PodcastsPlayed INT DEFAULT 0,
                        TimeListened INT DEFAULT 0,
                        PodcastsAdded INT DEFAULT 0,
                        EpisodesSaved INT DEFAULT 0,
                        EpisodesDownloaded INT DEFAULT 0,
                        FOREIGN KEY (UserID) REFERENCES Users(UserID)
                    )""")

    # Generate a key
    key = Fernet.generate_key()

    # Create the AppSettings table
    cursor.execute("""
        CREATE TABLE IF NOT EXISTS AppSettings (
            AppSettingsID INT AUTO_INCREMENT PRIMARY KEY,
            SelfServiceUser TINYINT(1) DEFAULT 0,
            DownloadEnabled TINYINT(1) DEFAULT 1,
            EncryptionKey BINARY(44),  -- Set the data type to BINARY(32) to hold the 32-byte key
            NewsFeedSubscribed TINYINT(1) DEFAULT 0
        )
    """)

    cursor.execute("SELECT COUNT(*) FROM AppSettings WHERE AppSettingsID = 1")
    count = cursor.fetchone()[0]

    if count == 0:
        cursor.execute("""
            INSERT INTO AppSettings (SelfServiceUser, DownloadEnabled, EncryptionKey)
            VALUES (0, 1, %s)
        """, (key,))

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

    cursor.execute("""
        SELECT COUNT(*) FROM EmailSettings
    """)
    rows = cursor.fetchone()

    if rows[0] == 0:
        cursor.execute("""
            INSERT INTO EmailSettings (Server_Name, Server_Port, From_Email, Send_Mode, Encryption, Auth_Required, Username, Password)
            VALUES ('default_server', 587, 'default_email@domain.com', 'default_mode', 'default_encryption', 1, 'default_username', 'default_password')
        """)

    # Generate a random password
    def generate_random_password(length=12):
        characters = string.ascii_letters + string.digits + string.punctuation
        return ''.join(random.choice(characters) for i in range(length))

    # Hash the password using Argon2
    def hash_password(password):
        ph = PasswordHasher()
        try:
            return ph.hash(password)
        except HashingError as e:
            print(f"Error hashing password: {e}")
            return None

    # Check if a user with the username 'guest' exists
    def user_exists(cursor, username):
        cursor.execute("""
            SELECT 1 FROM Users WHERE Username = %s
        """, (username,))
        return cursor.fetchone() is not None

    def insert_or_update_user(cursor, hashed_password):
        try:
            # First, check if 'background_tasks' user exists
            cursor.execute("SELECT * FROM Users WHERE Username = %s", ('background_tasks',))
            existing_user = cursor.fetchone()

            if existing_user:
                # Update existing 'background_tasks' user
                cursor.execute("""
                UPDATE Users
                SET Fullname = %s, Email = %s, Hashed_PW = %s, IsAdmin = %s
                WHERE Username = %s
                """, ('Background Tasks', 'inactive', hashed_password, False, 'background_tasks'))
                logging.info("Updated existing 'background_tasks' user.")
            else:
                # Check for 'guest' or 'bt' users to update
                cursor.execute("SELECT Username FROM Users WHERE Username IN ('guest', 'bt')")
                old_user = cursor.fetchone()

                if old_user:
                    # Update old user to 'background_tasks'
                    cursor.execute("""
                    UPDATE Users
                    SET Fullname = %s, Username = %s, Email = %s, Hashed_PW = %s, IsAdmin = %s
                    WHERE Username = %s
                    """, ('Background Tasks', 'background_tasks', 'inactive', hashed_password, False, old_user[0]))
                    logging.info(f"Updated existing '{old_user[0]}' user to 'background_tasks' user.")
                else:
                    # Insert new 'background_tasks' user
                    cursor.execute("""
                    INSERT INTO Users (Fullname, Username, Email, Hashed_PW, IsAdmin)
                    VALUES (%s, %s, %s, %s, %s)
                    """, ('Background Tasks', 'background_tasks', 'inactive', hashed_password, False))
                    logging.info("Inserted new 'background_tasks' user.")


        except Exception as e:
            print(f"Error inserting or updating user: {e}")
            logging.error("Error inserting or updating user: %s", e)
            # Rollback the transaction in case of error

    try:
        # Generate and hash the password
        random_password = generate_random_password()
        hashed_password = hash_password(random_password)

        if hashed_password:
            insert_or_update_user(cursor, hashed_password)

    except Exception as e:
        print(f"Error setting default Background Task User: {e}")
        logging.error("Error setting default Background Task User: %s", e)

    # Create the web Key
    def create_api_key(cnx, user_id=1):
        cursor_key = cnx.cursor()

        # Check if API key exists for user_id
        query = f"SELECT APIKey FROM APIKeys WHERE UserID = {user_id}"
        cursor_key.execute(query)

        result = cursor_key.fetchone()

        if result:
            api_key = result[0]
        else:
            import secrets
            import string
            alphabet = string.ascii_letters + string.digits
            api_key = ''.join(secrets.choice(alphabet) for _ in range(64))

            # Note the quotes around {api_key}
            query = f"INSERT INTO APIKeys (UserID, APIKey) VALUES ({user_id}, '{api_key}')"
            cursor_key.execute(query)

            cnx.commit()

        cursor_key.close()
        return api_key

    web_api_key = create_api_key(cnx)
    with open("/tmp/web_api_key.txt", "w") as f:
        f.write(web_api_key)

    # Check if admin environment variables are set
    admin_fullname = os.environ.get("FULLNAME")
    admin_username = os.environ.get("USERNAME")
    admin_email = os.environ.get("EMAIL")
    admin_pw = os.environ.get("PASSWORD")

    admin_created = False
    if all([admin_fullname, admin_username, admin_email, admin_pw]):
        # Hash the admin password
        hashed_pw = hash_password(admin_pw)
        admin_insert_query = """INSERT IGNORE INTO Users (Fullname, Username, Email, Hashed_PW, IsAdmin)
                                VALUES (%s, %s, %s, %s, %s)"""
        # Execute the INSERT statement without a separate salt
        cursor.execute(admin_insert_query, (admin_fullname, admin_username, admin_email, hashed_pw, 1))
        admin_created = True

    # Always create stats for background_tasks user
    cursor.execute("""INSERT IGNORE INTO UserStats (UserID) VALUES (1)""")

    # Only create stats for admin if we created the admin user
    if admin_created:
        cursor.execute("""INSERT IGNORE INTO UserStats (UserID) VALUES (2)""")

    # Create the Podcasts table if it doesn't exist
    cursor.execute("""CREATE TABLE IF NOT EXISTS Podcasts (
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
        FOREIGN KEY (UserID) REFERENCES Users(UserID)
        )""")

    def add_youtube_column_if_not_exist(cursor, cnx):
        try:
            # Check if column exists in MySQL
            cursor.execute("""
                SELECT COLUMN_NAME
                FROM INFORMATION_SCHEMA.COLUMNS
                WHERE TABLE_NAME = 'Podcasts'
                AND COLUMN_NAME = 'IsYouTubeChannel'
                AND TABLE_SCHEMA = DATABASE()
            """)
            existing_column = cursor.fetchone()

            if not existing_column:
                cursor.execute("""
                    ALTER TABLE Podcasts
                    ADD COLUMN IsYouTubeChannel TINYINT(1) DEFAULT 0
                """)
                print("Added 'IsYouTubeChannel' column to 'Podcasts' table.")
                cnx.commit()
        except Exception as e:
            print(f"Error adding IsYouTubeChannel column to Podcasts table: {e}")

    add_youtube_column_if_not_exist(cursor, cnx)

    def add_feed_cutoff_column_if_not_exist(cursor, cnx):
        try:
            # Check if column exists in MySQL
            cursor.execute("""
                SELECT COLUMN_NAME
                FROM INFORMATION_SCHEMA.COLUMNS
                WHERE TABLE_NAME = 'Podcasts'
                AND COLUMN_NAME = 'FeedCutoffDays'
                AND TABLE_SCHEMA = DATABASE()
            """)
            existing_column = cursor.fetchone()

            if not existing_column:
                cursor.execute("""
                    ALTER TABLE Podcasts
                    ADD COLUMN FeedCutoffDays INT DEFAULT 0
                """)
                print("Added 'FeedCutoffDays' column to 'Podcasts' table.")
                cnx.commit()
        except Exception as e:
            print(f"Error adding FeedCutoffDays column to Podcasts table: {e}")

    add_feed_cutoff_column_if_not_exist(cursor, cnx)

    def add_user_pass_columns_if_not_exist(cursor, cnx):
        try:
            # Check if the columns exist
            cursor.execute("""
                SELECT column_name
                FROM information_schema.columns
                WHERE table_name='Podcasts'
                AND column_name IN ('Username', 'Password')
            """)
            existing_columns = cursor.fetchall()
            existing_columns = [col[0] for col in existing_columns]

            # Add Username column if it doesn't exist
            if 'Username' not in existing_columns:
                cursor.execute("""
                    ALTER TABLE Podcasts
                    ADD COLUMN Username TEXT
                """)
                print("Added 'Username' column to 'Podcasts' table.")

            # Add Password column if it doesn't exist
            if 'Password' not in existing_columns:
                cursor.execute("""
                    ALTER TABLE Podcasts
                    ADD COLUMN Password TEXT
                """)
                print("Added 'Password' column to 'Podcasts' table.")

            cnx.commit()  # Ensure changes are committed
        except Exception as e:
            print(f"Error adding columns to Podcasts table: {e}")

    # Usage
    add_user_pass_columns_if_not_exist(cursor, cnx)

    # Check if the new columns exist, and add them if they don't
    cursor.execute("SHOW COLUMNS FROM Podcasts LIKE 'AutoDownload'")
    result = cursor.fetchone()
    if not result:
        cursor.execute("""
            ALTER TABLE Podcasts
            ADD COLUMN AutoDownload TINYINT(1) DEFAULT 0,
            ADD COLUMN StartSkip INT DEFAULT 0,
            ADD COLUMN EndSkip INT DEFAULT 0
        """)
    cursor.execute("""CREATE TABLE IF NOT EXISTS Episodes (
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
                    )""")
    # Check if the Completed column exists, and add it if it doesn't
    cursor.execute("SHOW COLUMNS FROM Episodes LIKE 'Completed'")
    result = cursor.fetchone()
    if not result:
        cursor.execute("""
            ALTER TABLE Episodes
            ADD COLUMN Completed TINYINT(1) DEFAULT 0
        """)

    try:
        # YouTubeVideos table
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
        cnx.commit()

    except Exception as e:
        print(f"Error creating YoutubeVideos Table: {e}")


    def create_index_if_not_exists(cursor, index_name, table_name, column_name):
        cursor.execute(f"SELECT COUNT(1) IndexIsThere FROM INFORMATION_SCHEMA.STATISTICS WHERE table_schema = DATABASE() AND index_name = '{index_name}'")
        if cursor.fetchone()[0] == 0:
            cursor.execute(f"CREATE INDEX {index_name} ON {table_name}({column_name})")

    create_index_if_not_exists(cursor, "idx_podcasts_userid", "Podcasts", "UserID")
    create_index_if_not_exists(cursor, "idx_episodes_podcastid", "Episodes", "PodcastID")
    create_index_if_not_exists(cursor, "idx_episodes_episodepubdate", "Episodes", "EpisodePubDate")


    try:
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS People (
                PersonID INT AUTO_INCREMENT PRIMARY KEY,
                Name TEXT,
                PersonImg TEXT,
                PeopleDBID INT,
                AssociatedPodcasts TEXT,
                UserID INT,
                FOREIGN KEY (UserID) REFERENCES Users(UserID)
            );
        """)
        cnx.commit()
    except Exception as e:
        print(f"Error creating People table: {e}")

    try:
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS PeopleEpisodes (
                EpisodeID INT AUTO_INCREMENT PRIMARY KEY,
                PersonID INT,
                PodcastID INT,
                EpisodeTitle TEXT,
                EpisodeDescription TEXT,
                EpisodeURL TEXT,
                EpisodeArtwork TEXT,
                EpisodePubDate DATETIME,
                EpisodeDuration INT,
                AddedDate DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (PersonID) REFERENCES People(PersonID),
                FOREIGN KEY (PodcastID) REFERENCES Podcasts(PodcastID)
            );
        """)
        cnx.commit()
    except Exception as e:
        print(f"Error creating PeopleEpisodes table: {e}")

    create_index_if_not_exists(cursor, "idx_people_episodes_person", "PeopleEpisodes", "PersonID")
    create_index_if_not_exists(cursor, "idx_people_episodes_podcast", "PeopleEpisodes", "PodcastID")


    try:
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS SharedEpisodes (
                SharedEpisodeID INT AUTO_INCREMENT PRIMARY KEY,
                EpisodeID INT,
                UrlKey TEXT,
                ExpirationDate DATETIME,
                FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID)
            )
        """)
        cnx.commit()
    except Exception as e:
        print(f"Error creating SharedEpisodes table: {e}")



    try:
        cursor.execute("""CREATE TABLE IF NOT EXISTS UserSettings (
                            UserSettingID INT AUTO_INCREMENT PRIMARY KEY,
                            UserID INT UNIQUE,
                            Theme VARCHAR(255) DEFAULT 'Nordic',
                            StartPage VARCHAR(255) DEFAULT 'home',
                            FOREIGN KEY (UserID) REFERENCES Users(UserID)
                        )""")
    except Exception as e:
        print(f"Error adding UserSettings table: {e}")

    def add_startpage_column():
        try:
            # Check if the column exists
            cursor.execute("""
                SELECT COLUMN_NAME
                FROM INFORMATION_SCHEMA.COLUMNS
                WHERE TABLE_NAME='UserSettings'
                AND COLUMN_NAME='StartPage'
                AND TABLE_SCHEMA=DATABASE();
            """)

            # If the column doesn't exist (no rows returned), add it
            if not cursor.fetchone():
                cursor.execute("""
                    ALTER TABLE UserSettings
                    ADD COLUMN StartPage VARCHAR(255) DEFAULT 'home';
                """)
                print("Successfully added StartPage column to UserSettings table")
            else:
                print("StartPage column already exists in UserSettings table")

        except Exception as e:
            print(f"Error adding StartPage column: {e}")

    # Call the function to ensure the column exists
    add_startpage_column()

    cursor.execute("""INSERT IGNORE INTO UserSettings (UserID, Theme) VALUES ('1', 'Nordic')""")
    cursor.execute("""INSERT IGNORE INTO UserSettings (UserID, Theme) VALUES ('2', 'Nordic')""")

    cursor.execute("""CREATE TABLE IF NOT EXISTS UserEpisodeHistory (
                        UserEpisodeHistoryID INT AUTO_INCREMENT PRIMARY KEY,
                        UserID INT,
                        EpisodeID INT,
                        ListenDate DATETIME,
                        ListenDuration INT,
                        FOREIGN KEY (UserID) REFERENCES Users(UserID),
                        FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID)
                    )""")

    try:
        # UserVideoHistory table
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
        cnx.commit()

    except Exception as e:
        print(f"Error creating UserVideoHistory table: {e}")

    cursor.execute("""CREATE TABLE IF NOT EXISTS SavedEpisodes (
                        SaveID INT AUTO_INCREMENT PRIMARY KEY,
                        UserID INT,
                        EpisodeID INT,
                        SaveDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                        FOREIGN KEY (UserID) REFERENCES Users(UserID),
                        FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID)
                    )""")

    try:
        # SavedVideos table
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
        cnx.commit()

    except Exception as e:
        print(f"Error creating SavedVideos table: {e}")


    # Create the DownloadedEpisodes table
    cursor.execute("""CREATE TABLE IF NOT EXISTS DownloadedEpisodes (
                    DownloadID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT,
                    EpisodeID INT,
                    DownloadedDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    DownloadedSize INT,
                    DownloadedLocation VARCHAR(255),
                    FOREIGN KEY (UserID) REFERENCES Users(UserID),
                    FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID)
                    )""")

    try:
        # DownloadedVideos table
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
        cnx.commit()

    except Exception as e:
        print(f"Error creating DownloadedVideos table: {e}")

    # Create the EpisodeQueue table
    cursor.execute("""CREATE TABLE IF NOT EXISTS EpisodeQueue (
        QueueID INT AUTO_INCREMENT PRIMARY KEY,
        QueueDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        UserID INT,
        EpisodeID INT,
        QueuePosition INT NOT NULL DEFAULT 0,
        is_youtube TINYINT(1) DEFAULT 0,
        FOREIGN KEY (UserID) REFERENCES Users(UserID),
        FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID)
    )""")

    def add_queue_youtube_column_if_not_exist(cursor, cnx):
        try:
            # Check if column exists in MySQL
            cursor.execute("""
                SELECT COLUMN_NAME
                FROM INFORMATION_SCHEMA.COLUMNS
                WHERE TABLE_NAME = 'EpisodeQueue'
                AND COLUMN_NAME = 'is_youtube'
                AND TABLE_SCHEMA = DATABASE()
            """)
            existing_column = cursor.fetchone()

            if not existing_column:
                try:
                    # Add the is_youtube column
                    cursor.execute("""
                        ALTER TABLE EpisodeQueue
                        ADD COLUMN is_youtube TINYINT(1) DEFAULT 0
                    """)
                    cnx.commit()
                    print("Added 'is_youtube' column to 'EpisodeQueue' table.")
                except Exception as e:
                    cnx.rollback()
                    if 'Duplicate column name' not in str(e):  # MySQL specific error message
                        print(f"Error adding is_youtube column to EpisodeQueue table: {e}")
            else:
                cnx.commit()  # Commit transaction even if column exists

        except Exception as e:
            cnx.rollback()
            print(f"Error checking for is_youtube column: {e}")

    add_queue_youtube_column_if_not_exist(cursor, cnx)

    def add_rssonly_column_if_not_exists(cursor, cnx):
        try:
            # Check if column exists in MySQL
            cursor.execute("""
                SELECT COLUMN_NAME
                FROM INFORMATION_SCHEMA.COLUMNS
                WHERE TABLE_NAME = 'APIKeys'
                AND COLUMN_NAME = 'RssOnly'
                AND TABLE_SCHEMA = DATABASE()
            """)
            existing_column = cursor.fetchone()

            if not existing_column:
                try:
                    # Add the is_youtube column
                    cursor.execute("""
                        ALTER TABLE APIKeys
                        ADD COLUMN RssOnly TINYINT(1) DEFAULT 0
                    """)
                    cnx.commit()
                    print("Added 'RssOnly' column to 'APIKeys' table.")
                except Exception as e:
                    cnx.rollback()
                    if 'Duplicate column name' not in str(e):  # MySQL specific error message
                        print(f"Error adding RssOnly column to APIKeys table: {e}")
            else:
                cnx.commit()  # Commit transaction even if column exists

        except Exception as e:
            cnx.rollback()
            print(f"Error checking for is_youtube column: {e}")

    add_rssonly_column_if_not_exists(cursor, cnx)

    # Create the Sessions table
    cursor.execute("""CREATE TABLE IF NOT EXISTS Sessions (
                    SessionID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT,
                    value TEXT,
                    expire DATETIME NOT NULL,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID)
                    )""")

    def add_notification_column_if_not_exists(cursor, cnx):
        try:
            # First check if the column exists
            cursor.execute("""
                SELECT COUNT(*) as column_exists
                FROM INFORMATION_SCHEMA.COLUMNS
                WHERE TABLE_NAME = 'Podcasts'
                AND COLUMN_NAME = 'NotificationsEnabled'
                AND TABLE_SCHEMA = DATABASE()
            """)
            result = cursor.fetchone()
            column_exists = result[0] > 0 if isinstance(result, tuple) else result.get('column_exists', 0) > 0

            # Only attempt to add the column if it doesn't exist
            if not column_exists:
                try:
                    cursor.execute("""
                        ALTER TABLE Podcasts
                        ADD COLUMN NotificationsEnabled TINYINT(1) DEFAULT 0
                    """)
                    print("Added NotificationsEnabled column to Podcasts table.")
                    cnx.commit()
                except Exception as alter_err:
                    # Check if the error is because the column already exists
                    # (This can happen in race conditions or if the schema check was outdated)
                    if "Duplicate column name" in str(alter_err) or "column already exists" in str(alter_err).lower():
                        print("Column NotificationsEnabled already exists in Podcasts table.")
                    else:
                        # It's a different error, so re-raise it
                        raise alter_err
            else:
                print("Column NotificationsEnabled already exists in Podcasts table.")

        except Exception as e:
            print(f"Error checking/adding NotificationsEnabled column to Podcasts table: {e}")
            # Only rollback if we're in a transaction that needs rolling back
            try:
                cnx.rollback()
            except:
                pass  # If rollback fails, we're not in a transaction

    # Create the notification settings table
    try:
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS UserNotificationSettings (
                SettingID INT AUTO_INCREMENT PRIMARY KEY,
                UserID INT,
                Platform VARCHAR(50) NOT NULL,
                Enabled TINYINT(1) DEFAULT 1,
                NtfyTopic VARCHAR(255),
                NtfyServerUrl VARCHAR(255) DEFAULT 'https://ntfy.sh',
                GotifyUrl VARCHAR(255),
                GotifyToken VARCHAR(255),
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (UserID) REFERENCES Users(UserID),
                UNIQUE(UserID, platform)
            )
        """)
        print("Checked/Created UserNotificationSettings table")
        cnx.commit()
    except Exception as e:
        print(f"Error creating UserNotificationSettings table: {e}")
        cnx.rollback()

    # Call our function to add the new column
    add_notification_column_if_not_exists(cursor, cnx)

    try:
        # Create Playlists table with the unique constraint
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS Playlists (
                PlaylistID INT AUTO_INCREMENT PRIMARY KEY,
                UserID INT NOT NULL,
                Name VARCHAR(255) NOT NULL,
                Description TEXT,
                IsSystemPlaylist TINYINT(1) NOT NULL DEFAULT 0,
                PodcastIDs TEXT, -- Storing as JSON array in MySQL
                IncludeUnplayed TINYINT(1) NOT NULL DEFAULT 1,
                IncludePartiallyPlayed TINYINT(1) NOT NULL DEFAULT 1,
                IncludePlayed TINYINT(1) NOT NULL DEFAULT 0,
                MinDuration INT, -- NULL means no minimum
                MaxDuration INT, -- NULL means no maximum
                SortOrder VARCHAR(50) NOT NULL DEFAULT 'date_desc',
                GroupByPodcast TINYINT(1) NOT NULL DEFAULT 0,
                MaxEpisodes INT, -- NULL means no limit
                PlayProgressMin FLOAT, -- NULL means no minimum progress requirement
                PlayProgressMax FLOAT, -- NULL means no maximum progress limit
                TimeFilterHours INT, -- NULL means no time filter
                LastUpdated DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                Created DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
                IconName VARCHAR(50) NOT NULL DEFAULT 'ph-playlist',
                FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE,
                UNIQUE(UserID, Name),
                CHECK (PlayProgressMin IS NULL OR (PlayProgressMin >= 0 AND PlayProgressMin <= 100)),
                CHECK (PlayProgressMax IS NULL OR (PlayProgressMax >= 0 AND PlayProgressMax <= 100)),
                CHECK (PlayProgressMin IS NULL OR PlayProgressMax IS NULL OR PlayProgressMin <= PlayProgressMax),
                CHECK (MinDuration IS NULL OR MinDuration >= 0),
                CHECK (MaxDuration IS NULL OR MaxDuration >= 0),
                CHECK (MinDuration IS NULL OR MaxDuration IS NULL OR MinDuration <= MaxDuration),
                CHECK (TimeFilterHours IS NULL OR TimeFilterHours > 0),
                CHECK (MaxEpisodes IS NULL OR MaxEpisodes > 0),
                CHECK (SortOrder IN ('date_asc', 'date_desc',
                                'duration_asc', 'duration_desc',
                                'listen_progress', 'completion'))
            )
        """)
        cnx.commit()

        # Create PlaylistContents table
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS PlaylistContents (
                PlaylistContentID INT AUTO_INCREMENT PRIMARY KEY,
                PlaylistID INT,
                EpisodeID INT,
                VideoID INT,
                Position INT,
                DateAdded DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (PlaylistID) REFERENCES Playlists(PlaylistID) ON DELETE CASCADE,
                FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID) ON DELETE CASCADE,
                FOREIGN KEY (VideoID) REFERENCES YouTubeVideos(VideoID) ON DELETE CASCADE,
                CHECK ((EpisodeID IS NOT NULL AND VideoID IS NULL) OR (EpisodeID IS NULL AND VideoID IS NOT NULL))
            )
        """)
        cnx.commit()

        # Create indexes
        cursor.execute("CREATE INDEX idx_playlists_userid ON Playlists(UserID)")
        cnx.commit()

        cursor.execute("CREATE INDEX idx_playlist_contents_playlistid ON PlaylistContents(PlaylistID)")
        cnx.commit()

        cursor.execute("CREATE INDEX idx_playlist_contents_episodeid ON PlaylistContents(EpisodeID)")
        cnx.commit()

        cursor.execute("CREATE INDEX idx_playlist_contents_videoid ON PlaylistContents(VideoID)")
        cnx.commit()

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

        # Insert system playlists
        for playlist in system_playlists:
            try:
                # First check if this playlist already exists
                cursor.execute("""
                    SELECT COUNT(*)
                    FROM Playlists
                    WHERE UserID = 1 AND Name = %s AND IsSystemPlaylist = 1
                """, (playlist['name'],))

                if cursor.fetchone()[0] == 0:
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
                            PlayProgressMin,
                            PlayProgressMax,
                            TimeFilterHours
                        ) VALUES (
                            1,
                            %s,
                            %s,
                            1,
                            %s,
                            %s,
                            %s,
                            %s,
                            %s,
                            %s,
                            %s,
                            %s,
                            %s,
                            %s,
                            %s
                        )
                    """, (
                        playlist['name'],
                        playlist['description'],
                        playlist.get('min_duration'),
                        playlist.get('max_duration'),
                        playlist.get('sort_order', 'date_asc'),
                        1 if playlist.get('group_by_podcast', False) else 0,
                        1 if playlist.get('include_unplayed', True) else 0,
                        1 if playlist.get('include_partially_played', True) else 0,
                        1 if playlist.get('include_played', False) else 0,
                        playlist.get('icon_name', 'ph-playlist'),
                        playlist.get('play_progress_min'),
                        playlist.get('play_progress_max'),
                        playlist.get('time_filter_hours')
                    ))
                    cnx.commit()
                    print(f"Successfully added system playlist: {playlist['name']}")
                else:
                    print(f"System playlist already exists: {playlist['name']}")

            except Exception as e:
                print(f"Error handling system playlist {playlist['name']}: {e}")
                continue

    except Exception as e:
        print(f"Error setting up platlists: {e}")

    print("Checked/Created Playlist Tables")

except mysql.connector.Error as err:
    logging.error(f"Database error: {err}")
except Exception as e:
    logging.error(f"General error: {e}")

# Ensure to close the cursor and connection
finally:
    if 'cursor' in locals():
        cursor.close()
    if 'cnx' in locals():
        cnx.close()
