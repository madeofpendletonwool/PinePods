import mysql.connector
import os
import sys
from cryptography.fernet import Fernet
import string
import secrets
import logging
from passlib.hash import argon2

# Set up basic configuration for logging
logging.basicConfig(level=logging.DEBUG, format='%(asctime)s - %(levelname)s - %(message)s')

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
        charset='utf8mb4'
    )

    # Create a cursor to execute SQL statements
    cursor = cnx.cursor()

    # Execute SQL command to create tables
    cursor.execute("""
        CREATE TABLE IF NOT EXISTS Users (
            UserID INT AUTO_INCREMENT PRIMARY KEY,
            Fullname TEXT,
            Username TEXT UNIQUE,
            Email VARCHAR(255),
            Hashed_PW CHAR(255),
            IsAdmin TINYINT(1),
            Reset_Code TEXT,
            Reset_Expiry DATETIME,
            MFA_Secret VARCHAR(70),
            TimeZone VARCHAR(50) DEFAULT 'UTC',
            TimeFormat INT  DEFAULT 24,
            FirstLogin TINYINT(1) DEFAULT 0,
            GpodderUrl VARCHAR(255) DEFAULT '',
            GpodderToken TEXT DEFAULT ''
        )
    """)

    logging.info("Database tables created or verified successfully.")




    cursor.execute("""CREATE TABLE IF NOT EXISTS APIKeys (
                        APIKeyID INT AUTO_INCREMENT PRIMARY KEY,
                        UserID INT,
                        APIKey TEXT,
                        Created TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                        FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE
                    )""")

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
            EncryptionKey BINARY(44)  -- Set the data type to BINARY(32) to hold the 32-byte key
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



    cursor.execute("""INSERT IGNORE INTO Users (Fullname, Username, Email, Hashed_PW, IsAdmin)
                    VALUES ('Guest User', 'guest', 'inactive', '$argon2id$v=19$m=65536,t=3,p=4$nCy4H3qu2kJOJVa7dmdS5A$C5IkJLgalKIZGwAKw3V2KYKIWxzstLAmzoL41tdhDyw', 0)""")

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

    # Your admin user variables
    admin_fullname = os.environ.get("FULLNAME", "Admin User")
    admin_username = os.environ.get("USERNAME", "admin")
    admin_email = os.environ.get("EMAIL", "admin@pinepods.online")

    alphabet = string.ascii_letters + string.digits + string.punctuation
    fallback_password = ''.join(secrets.choice(alphabet) for _ in range(15))

    admin_pw = os.environ.get("PASSWORD", fallback_password)

    # Hash the admin password
    hashed_pw = hash_password(admin_pw)

    admin_insert_query = """INSERT IGNORE INTO Users (Fullname, Username, Email, Hashed_PW, IsAdmin)
                            VALUES (%s, %s, %s, %s, %s)"""

    # Execute the INSERT statement without a separate salt
    cursor.execute(admin_insert_query, (admin_fullname, admin_username, admin_email, hashed_pw, 1))



    cursor.execute("""INSERT IGNORE INTO UserStats (UserID) VALUES (1)""")

    cursor.execute("""INSERT IGNORE INTO UserStats (UserID) VALUES (2)""")

    cursor.execute("""CREATE TABLE IF NOT EXISTS Podcasts (
                        PodcastID INT AUTO_INCREMENT PRIMARY KEY,
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
                        FOREIGN KEY (UserID) REFERENCES Users(UserID)
                    )""")

    cursor.execute("""CREATE TABLE IF NOT EXISTS Episodes (
                        EpisodeID INT AUTO_INCREMENT PRIMARY KEY,
                        PodcastID INT,
                        EpisodeTitle TEXT,
                        EpisodeDescription TEXT,
                        EpisodeURL TEXT,
                        EpisodeArtwork TEXT,
                        EpisodePubDate DATE,
                        EpisodeDuration INT,
                        FOREIGN KEY (PodcastID) REFERENCES Podcasts(PodcastID)
                    )""")

    def create_index_if_not_exists(cursor, index_name, table_name, column_name):
        cursor.execute(f"SELECT COUNT(1) IndexIsThere FROM INFORMATION_SCHEMA.STATISTICS WHERE table_schema = DATABASE() AND index_name = '{index_name}'")
        if cursor.fetchone()[0] == 0:
            cursor.execute(f"CREATE INDEX {index_name} ON {table_name}({column_name})")

    create_index_if_not_exists(cursor, "idx_podcasts_userid", "Podcasts", "UserID")
    create_index_if_not_exists(cursor, "idx_episodes_podcastid", "Episodes", "PodcastID")
    create_index_if_not_exists(cursor, "idx_episodes_episodepubdate", "Episodes", "EpisodePubDate")



    cursor.execute("""CREATE TABLE IF NOT EXISTS UserSettings (
                        UserSettingID INT AUTO_INCREMENT PRIMARY KEY,
                        UserID INT UNIQUE,
                        Theme VARCHAR(255) DEFAULT 'nordic',
                        FOREIGN KEY (UserID) REFERENCES Users(UserID)
                    )""")

    cursor.execute("""INSERT IGNORE INTO UserSettings (UserID, Theme) VALUES ('1', 'nordic')""")
    cursor.execute("""INSERT IGNORE INTO UserSettings (UserID, Theme) VALUES ('2', 'nordic')""")

    cursor.execute("""CREATE TABLE IF NOT EXISTS UserEpisodeHistory (
                        UserEpisodeHistoryID INT AUTO_INCREMENT PRIMARY KEY,
                        UserID INT,
                        EpisodeID INT,
                        ListenDate DATETIME,
                        ListenDuration INT,
                        FOREIGN KEY (UserID) REFERENCES Users(UserID),
                        FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID)
                    )""")

    cursor.execute("""CREATE TABLE IF NOT EXISTS SavedEpisodes (
                        SaveID INT AUTO_INCREMENT PRIMARY KEY,
                        UserID INT,
                        EpisodeID INT,
                        SaveDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                        FOREIGN KEY (UserID) REFERENCES Users(UserID),
                        FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID)
                    )""")


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

    # Create the EpisodeQueue table
    cursor.execute("""CREATE TABLE IF NOT EXISTS EpisodeQueue (
                    QueueID INT AUTO_INCREMENT PRIMARY KEY,
                    QueueDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    UserID INT,
                    EpisodeID INT,
                    QueuePosition INT NOT NULL DEFAULT 0,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID),
                    FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID)
                    )""")

    # Create the Sessions table
    cursor.execute("""CREATE TABLE IF NOT EXISTS Sessions (
                    SessionID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT,
                    value TEXT,
                    expire DATETIME NOT NULL,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID)
                    )""")
    
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