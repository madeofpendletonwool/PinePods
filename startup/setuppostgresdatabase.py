import os
import sys
from cryptography.fernet import Fernet
import string
import secrets
import bcrypt
import psycopg2

sys.path.append('/pinepods')

def hash_password(password: str):
    # Generate a random salt
    salt = bcrypt.gensalt()

    # Hash the password with the salt
    hashed_password = bcrypt.hashpw(password.encode('utf-8'), salt)

    # Return the salt and the hashed password
    return salt, hashed_password


# Database variables
db_host = os.environ.get("DB_HOST", "127.0.0.1")
db_port = os.environ.get("DB_PORT", "5432")
db_user = os.environ.get("DB_USER", "postgres")
db_password = os.environ.get("DB_PASSWORD", "password")
db_name = os.environ.get("DB_NAME", "pypods_database")

# Create database connector
cnx = psycopg2.connect(
    host=db_host,
    port=db_port,
    user=db_user,
    password=db_password,
    dbname=db_name
)

# create a cursor to execute SQL statements
cursor = cnx.cursor()

# create tables
cursor.execute("""
    CREATE TABLE IF NOT EXISTS Users (
        UserID SERIAL PRIMARY KEY,
        Fullname TEXT,
        Username TEXT UNIQUE,
        Email VARCHAR(255),
        Hashed_PW CHAR(128),
        Salt CHAR(128),
        IsAdmin BOOLEAN,
        Reset_Code TEXT,
        Reset_Expiry TIMESTAMP,
        MFA_Secret VARCHAR(50),
        TimeZone VARCHAR(50) DEFAULT 'UTC',
        TimeFormat INT  DEFAULT 24,
        FirstLogin BOOLEAN DEFAULT false
    )
""")

cursor.execute("""CREATE TABLE IF NOT EXISTS APIKeys (
                    APIKeyID SERIAL PRIMARY KEY,
                    UserID INT,
                    APIKey TEXT,
                    Created TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE
                )""")

cursor.execute("""CREATE TABLE IF NOT EXISTS UserStats (
                    UserStatsID SERIAL PRIMARY KEY,
                    UserID INT UNIQUE,
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
        AppSettingsID SERIAL PRIMARY KEY,
        SelfServiceUser BOOLEAN DEFAULT false,
        DownloadEnabled BOOLEAN DEFAULT true,
        EncryptionKey BYTEA
    )
""")

cursor.execute("SELECT COUNT(*) FROM AppSettings WHERE AppSettingsID = 1")
count = cursor.fetchone()[0]

if count == 0:
    cursor.execute("""
        INSERT INTO AppSettings (SelfServiceUser, DownloadEnabled, EncryptionKey) 
        VALUES (false, true, %s)
    """, (key,))

cursor.execute("""
    CREATE TABLE IF NOT EXISTS EmailSettings (
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

cursor.execute("""
    SELECT COUNT(*) FROM EmailSettings
""")
rows = cursor.fetchone()

if rows[0] == 0:
    cursor.execute("""
        INSERT INTO EmailSettings (Server_Name, Server_Port, From_Email, Send_Mode, Encryption, Auth_Required, Username, Password)
        VALUES ('default_server', 587, 'default_email@domain.com', 'default_mode', 'default_encryption', true, 'default_username', 'default_password')
    """)



cursor.execute("""
    INSERT INTO Users (Fullname, Username, Email, Hashed_PW, Salt, IsAdmin)
    VALUES ('Guest User', 'guest', 'inactive', 'Hmc7toxfqLssTdzaFGiKhigJ4VN3JeEy8VTkVHQ2FFrxAg74FrdoPRXowqgh', 'Hmc7toxfqLssTdzaFGiKhigJ4VN3JeEy8VTkVHQ2FFrxAg74FrdoPRXowqgh', false)
    ON CONFLICT (Username) DO NOTHING
""")

# Create the web API Key
def create_api_key(cnx, user_id=1):
    cursor = cnx.cursor()

    # Check if API key exists for user_id
    query = "SELECT APIKey FROM APIKeys WHERE UserID = %s"
    cursor.execute(query, (user_id,))
    result = cursor.fetchone()

    if result:
        api_key = result[0]
    else:
        import secrets
        import string
        alphabet = string.ascii_letters + string.digits
        api_key = ''.join(secrets.choice(alphabet) for _ in range(64))

        query = "INSERT INTO APIKeys (UserID, APIKey) VALUES (%s, %s)"
        cursor.execute(query, (user_id, api_key))
        cnx.commit()

    cursor.close()
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

salt, hash_pw = hash_password(admin_pw)

# Parameterized INSERT statement for the admin user
admin_insert_query = """
    INSERT INTO Users (Fullname, Username, Email, Hashed_PW, Salt, IsAdmin)
    VALUES (%s, %s, %s, %s, %s, %s)
    ON CONFLICT (Username) DO NOTHING
"""  # Assuming 'Username' is the unique column

# Execute the INSERT statement with the admin user variables
cursor.execute(admin_insert_query, (admin_fullname, admin_username, admin_email, hash_pw, salt, 'true'))

cursor.execute("""INSERT INTO UserStats (UserID) VALUES (1) ON CONFLICT (UserID) DO NOTHING""")

cursor.execute("""INSERT INTO UserStats (UserID) VALUES (2) ON CONFLICT (UserID) DO NOTHING""")


cursor.execute("""CREATE TABLE IF NOT EXISTS Podcasts (
                    PodcastID SERIAL PRIMARY KEY,
                    PodcastName TEXT,
                    ArtworkURL TEXT,
                    Author TEXT,
                    Categories TEXT,
                    Description TEXT,
                    EpisodeCount INT,
                    FeedURL TEXT,
                    WebsiteURL TEXT,
                    UserID INT,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID)
                )""")

cursor.execute("""CREATE TABLE IF NOT EXISTS Episodes (
                    EpisodeID SERIAL PRIMARY KEY,
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
    cursor.execute(f"""
        SELECT 1 
        FROM pg_indexes 
        WHERE lower(indexname) = lower('{index_name}') AND lower(tablename) = lower('{table_name}')
    """)
    if not cursor.fetchone():
        cursor.execute(f"CREATE INDEX {index_name} ON {table_name}({column_name})")


create_index_if_not_exists(cursor, "idx_podcasts_userid", "Podcasts", "UserID")
create_index_if_not_exists(cursor, "idx_episodes_podcastid", "Episodes", "PodcastID")
create_index_if_not_exists(cursor, "idx_episodes_episodepubdate", "Episodes", "EpisodePubDate")



cursor.execute("""CREATE TABLE IF NOT EXISTS UserSettings (
                    UserSettingID SERIAL PRIMARY KEY,
                    UserID INT UNIQUE,
                    Theme VARCHAR(255) DEFAULT 'nordic',
                    FOREIGN KEY (UserID) REFERENCES Users(UserID)
                )""")

cursor.execute("""INSERT INTO UserSettings (UserID, Theme) VALUES ('1', 'nordic') ON CONFLICT (UserID) DO NOTHING""")
cursor.execute("""INSERT INTO UserSettings (UserID, Theme) VALUES ('2', 'nordic') ON CONFLICT (UserID) DO NOTHING""")

cursor.execute("""CREATE TABLE IF NOT EXISTS UserEpisodeHistory (
                    UserEpisodeHistoryID SERIAL PRIMARY KEY,
                    UserID INT,
                    EpisodeID INT,
                    ListenDate TIMESTAMP,
                    ListenDuration INT,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID),
                    FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID)
                )""")

cursor.execute("""CREATE TABLE IF NOT EXISTS SavedEpisodes (
                    SaveID SERIAL PRIMARY KEY,
                    UserID INT,
                    EpisodeID INT,
                    SaveDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (UserID) REFERENCES Users(UserID),
                    FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID)
                )""")


# Create the DownloadedEpisodes table
cursor.execute("""CREATE TABLE IF NOT EXISTS DownloadedEpisodes (
                  DownloadID SERIAL PRIMARY KEY,
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
                  QueueID SERIAL PRIMARY KEY,
                  QueueDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                  UserID INT,
                  EpisodeID INT,
                  QueuePosition INT NOT NULL DEFAULT 0,
                  FOREIGN KEY (UserID) REFERENCES Users(UserID),
                  FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID)
                )""")

# Create the Sessions table
cursor.execute("""CREATE TABLE IF NOT EXISTS Sessions (
                  SessionID SERIAL PRIMARY KEY,
                  UserID INT,
                  value TEXT,
                  expire TIMESTAMP NOT NULL,
                  FOREIGN KEY (UserID) REFERENCES Users(UserID)
                )""")


# Close the cursor
cursor.close()

# Commit the changes
cnx.commit()

# Close the connection
cnx.close()
