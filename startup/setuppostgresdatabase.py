import os
import sys
from cryptography.fernet import Fernet
import string
import secrets
from passlib.hash import argon2
import psycopg
from argon2 import PasswordHasher
from argon2.exceptions import HashingError
import logging
import random

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



    # Database variables
    db_host = os.environ.get("DB_HOST", "127.0.0.1")
    db_port = os.environ.get("DB_PORT", "5432")
    db_user = os.environ.get("DB_USER", "postgres")
    db_password = os.environ.get("DB_PASSWORD", "password")
    db_name = os.environ.get("DB_NAME", "pypods_database")

    # Create database connector
    cnx = psycopg.connect(
        host=db_host,
        port=db_port,
        user=db_user,
        password=db_password,
        dbname=db_name
    )

    # create a cursor to execute SQL statements
    cursor = cnx.cursor()

    # Execute SQL command to create tables
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
            TimeFormat INT  DEFAULT 24,
            DateFormat VARCHAR(3) DEFAULT 'ISO',
            FirstLogin BOOLEAN DEFAULT false,
            GpodderUrl VARCHAR(255) DEFAULT '',
            Pod_Sync_Type VARCHAR(50) DEFAULT 'None',
            GpodderLoginName VARCHAR(255) DEFAULT '',
            GpodderToken VARCHAR(255) DEFAULT ''
        )
    """)


    cursor.execute("""CREATE TABLE IF NOT EXISTS "APIKeys" (
                        APIKeyID SERIAL PRIMARY KEY,
                        UserID INT,
                        APIKey TEXT,
                        Created TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                        FOREIGN KEY (UserID) REFERENCES "Users"(UserID) ON DELETE CASCADE
                    )""")

    cursor.execute("""CREATE TABLE IF NOT EXISTS "UserStats" (
                        UserStatsID SERIAL PRIMARY KEY,
                        UserID INT UNIQUE,
                        UserCreated TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                        PodcastsPlayed INT DEFAULT 0,
                        TimeListened INT DEFAULT 0,
                        PodcastsAdded INT DEFAULT 0,
                        EpisodesSaved INT DEFAULT 0,
                        EpisodesDownloaded INT DEFAULT 0,
                        FOREIGN KEY (UserID) REFERENCES "Users"(UserID)
                    )""")


    # Generate a key
    key = Fernet.generate_key()

    # Create the AppSettings table
    cursor.execute("""
        CREATE TABLE IF NOT EXISTS "AppSettings" (
            AppSettingsID SERIAL PRIMARY KEY,
            SelfServiceUser BOOLEAN DEFAULT false,
            DownloadEnabled BOOLEAN DEFAULT true,
            EncryptionKey BYTEA,  -- Set the data type to BYTEA for binary data
            NewsFeedSubscribed BOOLEAN DEFAULT false
        )
    """)

    cursor.execute('SELECT COUNT(*) FROM "AppSettings" WHERE AppSettingsID = 1')
    count = cursor.fetchone()[0]

    if count == 0:
        cursor.execute("""
            INSERT INTO "AppSettings" (SelfServiceUser, DownloadEnabled, EncryptionKey)
            VALUES (false, true, %s)
        """, (key,))

    try:
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
    except Exception as e:
        logging.error(f"Failed to create EmailSettings table: {e}")

    try:
        cursor.execute("""
            SELECT COUNT(*) FROM "EmailSettings"
        """)
        rows = cursor.fetchone()


        if rows[0] == 0:
            cursor.execute("""
                INSERT INTO "EmailSettings" (Server_Name, Server_Port, From_Email, Send_Mode, Encryption, Auth_Required, Username, Password)
                VALUES ('default_server', 587, 'default_email@domain.com', 'default_mode', 'default_encryption', true, 'default_username', 'default_password')
            """)
    except Exception as e:
        print(f"Error setting default email data: {e}")

    def user_exists(cursor, username):
        cursor.execute("""
            SELECT 1 FROM "Users" WHERE Username = %s
        """, (username,))
        return cursor.fetchone() is not None

    # Insert or update the user in the database
    def insert_or_update_user(cursor, hashed_password):
        try:
            if user_exists(cursor, 'guest'):
                cursor.execute("""
                    UPDATE "Users"
                    SET Fullname = %s, Username = %s, Email = %s, Hashed_PW = %s, IsAdmin = %s
                    WHERE Username = %s
                """, ('Background Tasks', 'background_tasks', 'inactive', hashed_password, False, 'guest'))
                logging.info("Updated existing 'guest' user to 'background_tasks' user.")
            elif user_exists(cursor, 'bt'):
                cursor.execute("""
                    UPDATE "Users"
                    SET Fullname = %s, Username = %s, Email = %s, Hashed_PW = %s, IsAdmin = %s
                    WHERE Username = %s
                """, ('Background Tasks', 'background_tasks', 'inactive', hashed_password, False, 'bt'))
                logging.info("Updated existing 'guest' user to 'background_tasks' user.")
            else:
                cursor.execute("""
                    INSERT INTO "Users" (Fullname, Username, Email, Hashed_PW, IsAdmin)
                    VALUES (%s, %s, %s, %s, %s)
                    ON CONFLICT (Username) DO NOTHING
                """, ('Background Tasks', 'bt', 'inactive', hashed_password, False, 'guest'))
        except Exception as e:
            print(f"Error inserting or updating user: {e}")
            logging.error("Error inserting or updating user: %s", e)


    try:
        # Generate and hash the password
        random_password = generate_random_password()
        hashed_password = hash_password(random_password)

        if hashed_password:
            insert_or_update_user(cursor, hashed_password)



    except Exception as e:
        print(f"Error setting default Background Task User: {e}")
        logging.error("Error setting default Background Task User: %s", e)


    try:
        # Check if API key exists for user_id
        cursor.execute('SELECT apikey FROM "APIKeys" WHERE userid = %s', (1,))

        result = cursor.fetchone()

        if result:
            api_key = result[0]
        else:
            import secrets
            import string
            alphabet = string.ascii_letters + string.digits
            api_key = ''.join(secrets.choice(alphabet) for _ in range(64))

            # Insert the new API key into the database using a parameterized query
            cursor.execute('INSERT INTO "APIKeys" (UserID, APIKey) VALUES (%s, %s)', (1, api_key))

            cnx.commit()

        with open("/tmp/web_api_key.txt", "w") as f:
            f.write(api_key)
    except Exception as e:
        print(f"Error creating web key: {e}")

    try:
        # Your admin user variables
        admin_fullname = os.environ.get("FULLNAME", "Admin User")
        admin_username = os.environ.get("USERNAME", "admin")
        admin_email = os.environ.get("EMAIL", "admin@pinepods.online")

        alphabet = string.ascii_letters + string.digits + string.punctuation
        fallback_password = ''.join(secrets.choice(alphabet) for _ in range(15))

        admin_pw = os.environ.get("PASSWORD", fallback_password)

        # Hash the admin password
        hashed_pw = hash_password(admin_pw).strip()

        admin_insert_query = """
            INSERT INTO "Users" (Fullname, Username, Email, Hashed_PW, IsAdmin)
            VALUES (%s, %s, %s, %s, %s::boolean)
            ON CONFLICT (Username) DO NOTHING
        """


        # Execute the INSERT statement without a separate salt
        cursor.execute(admin_insert_query, (admin_fullname, admin_username, admin_email, hashed_pw, True))
    except Exception as e:
        print(f"Error creating default admin: {e}")

    try:
        cursor.execute("""
            INSERT INTO "UserStats" (UserID) VALUES (1)
            ON CONFLICT (UserID) DO NOTHING
        """)

        cursor.execute("""
            INSERT INTO "UserStats" (UserID) VALUES (2)
            ON CONFLICT (UserID) DO NOTHING
        """)
    except Exception as e:
        print(f"Error creating intial users in UserStats: {e}")

    try:
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS "Podcasts" (
                PodcastID SERIAL PRIMARY KEY,
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
                FOREIGN KEY (UserID) REFERENCES "Users"(UserID)
            )
        """)
        cnx.commit()  # Ensure changes are committed
    except Exception as e:
        print(f"Error adding Podcasts table: {e}")

    cursor.execute("SELECT to_regclass('public.\"Podcasts\"')")

    try:
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

        cnx.commit()  # Ensure changes are committed
    except Exception as e:
        print(f"Error adding Episodes table: {e}")

    def create_index_if_not_exists(cursor, index_name, table_name, column_name):
        cursor.execute(f"""
            SELECT 1
            FROM pg_indexes
            WHERE lower(indexname) = lower('{index_name}') AND tablename = '{table_name}'
        """)
        if not cursor.fetchone():
            cursor.execute(f'CREATE INDEX {index_name} ON "{table_name}"({column_name})')

    create_index_if_not_exists(cursor, "idx_podcasts_userid", "Podcasts", "UserID")
    create_index_if_not_exists(cursor, "idx_episodes_podcastid", "Episodes", "PodcastID")
    create_index_if_not_exists(cursor, "idx_episodes_episodepubdate", "Episodes", "EpisodePubDate")


    try:
        cursor.execute("""CREATE TABLE IF NOT EXISTS "UserSettings" (
                            UserSettingID SERIAL PRIMARY KEY,
                            UserID INT UNIQUE,
                            Theme VARCHAR(255) DEFAULT 'nordic',
                            FOREIGN KEY (UserID) REFERENCES "Users"(UserID)
                        )""")
    except Exception as e:
        print(f"Error adding UserSettings table: {e}")

    cursor.execute("""INSERT INTO "UserSettings" (UserID, Theme) VALUES ('1', 'nordic') ON CONFLICT (UserID) DO NOTHING""")
    cursor.execute("""INSERT INTO "UserSettings" (UserID, Theme) VALUES ('2', 'nordic') ON CONFLICT (UserID) DO NOTHING""")

    cursor.execute("""CREATE TABLE IF NOT EXISTS "UserEpisodeHistory" (
                        UserEpisodeHistoryID SERIAL PRIMARY KEY,
                        UserID INT,
                        EpisodeID INT,
                        ListenDate TIMESTAMP,
                        ListenDuration INT,
                        FOREIGN KEY (UserID) REFERENCES "Users"(UserID),
                        FOREIGN KEY (EpisodeID) REFERENCES "Episodes"(EpisodeID)
                    )""")

    cursor.execute("""CREATE TABLE IF NOT EXISTS "SavedEpisodes" (
                        SaveID SERIAL PRIMARY KEY,
                        UserID INT,
                        EpisodeID INT,
                        SaveDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                        FOREIGN KEY (UserID) REFERENCES "Users"(UserID),
                        FOREIGN KEY (EpisodeID) REFERENCES "Episodes"(EpisodeID)
                    )""")


    # Create the DownloadedEpisodes table
    cursor.execute("""CREATE TABLE IF NOT EXISTS "DownloadedEpisodes" (
                    DownloadID SERIAL PRIMARY KEY,
                    UserID INT,
                    EpisodeID INT,
                    DownloadedDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    DownloadedSize INT,
                    DownloadedLocation VARCHAR(255),
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID),
                    FOREIGN KEY (EpisodeID) REFERENCES "Episodes"(EpisodeID)
                    )""")

    # Create the EpisodeQueue table
    cursor.execute("""CREATE TABLE IF NOT EXISTS "EpisodeQueue" (
                    QueueID SERIAL PRIMARY KEY,
                    QueueDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    UserID INT,
                    EpisodeID INT,
                    QueuePosition INT NOT NULL DEFAULT 0,
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID),
                    FOREIGN KEY (EpisodeID) REFERENCES "Episodes"(EpisodeID)
                    )""")

    # Create the Sessions table
    cursor.execute("""CREATE TABLE IF NOT EXISTS "Sessions" (
                    SessionID SERIAL PRIMARY KEY,
                    UserID INT,
                    value TEXT,
                    expire TIMESTAMP NOT NULL,
                    FOREIGN KEY (UserID) REFERENCES "Users"(UserID)
                    )""")
    cnx.commit()


except psycopg.Error as err:
    logging.error(f"Database error: {err}")
except Exception as e:
    logging.error(f"General error: {e}")

# Ensure to close the cursor and connection
finally:
    if 'cursor' in locals():
        cursor.close()
    if 'cnx' in locals():
        cnx.close()
