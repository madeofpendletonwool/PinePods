import mysql.connector
import os

# Database variables
db_host = os.environ.get("DB_HOST", "127.0.0.1")
db_port = os.environ.get("DB_PORT", "3306")
db_user = os.environ.get("DB_USER", "root")
db_password = os.environ.get("DB_PASSWORD", "password")
db_name = os.environ.get("DB_NAME", "pypods_database")

# Create database connector
cnx = mysql.connector.connect(
    host=db_host,
    port=db_port,
    user=db_user,
    password=db_password,
    database=db_name,
    charset='utf8mb4'
)

# create a cursor to execute SQL statements
cursor = cnx.cursor()

# create tables
cursor.execute("""CREATE TABLE IF NOT EXISTS Users (
                    UserID INT AUTO_INCREMENT PRIMARY KEY,
                    Fullname TEXT,
                    Username TEXT,
                    Email VARCHAR(255),
                    Hashed_PW CHAR(60),
                    Salt CHAR(60),
                    IsAdmin TINYINT(1)
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

cursor.execute("""CREATE TABLE IF NOT EXISTS AppSettings (
                    AppSettingsID INT AUTO_INCREMENT PRIMARY KEY,
                    SelfServiceUser TINYINT(1) DEFAULT 0
                )""")

cursor.execute("""INSERT INTO AppSettings (SelfServiceUser)
                  SELECT 0 FROM DUAL WHERE NOT EXISTS (SELECT * FROM AppSettings)""")

cursor.execute("""INSERT INTO Users (Fullname, Username, Email, Hashed_PW, Salt, IsAdmin)
                    VALUES ('Guest User', 'guest', 'inactive', 'Hmc7toxfqLssTdzaFGiKhigJ4VN3JeEy8VTkVHQ2FFrxAg74FrdoPRXowqgh', 'Hmc7toxfqLssTdzaFGiKhigJ4VN3JeEy8VTkVHQ2FFrxAg74FrdoPRXowqgh', 0)
                    ON DUPLICATE KEY UPDATE UserID=UserID""")

cursor.execute("""INSERT IGNORE INTO UserStats (UserID) VALUES (1)""")

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

cursor.execute("""CREATE TABLE IF NOT EXISTS UserSettings (
                    UserSettingID INT AUTO_INCREMENT PRIMARY KEY,
                    UserID INT,
                    Theme VARCHAR(255) DEFAULT 'nordic',
                    FOREIGN KEY (UserID) REFERENCES Users(UserID)
                )""")

cursor.execute("""INSERT INTO UserSettings (UserID, Theme) VALUES ('1', 'nordic')""")

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


# Close the cursor
cursor.close()

# Commit the changes
cnx.commit()

# Close the connection
cnx.close()