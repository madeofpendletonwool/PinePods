import mysql.connector
import os

print(os.getcwd())

# Connect to the database
cnx = mysql.connector.connect(
    host="127.0.0.1",
    port="3306",
    user="root",
    password="password",
    database="pypods_database"
)

# Create a cursor object
cursor = cnx.cursor()

# Read the SQL script file into a string
with open("startup/tablecreate.sql", "r") as file:
    table_setup = file.read()

# Execute the SQL script
# Create the Users table
cursor.execute("""CREATE TABLE Users (
                  UserID INT AUTO_INCREMENT PRIMARY KEY,
                  Username TEXT,
                  Email VARCHAR(255),
                  Hashed_PW CHAR(60),
                  Salt CHAR(60)
                )""")

# Create the Podcasts table
cursor.execute("""CREATE TABLE Podcasts (
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

# Create the Episodes table
cursor.execute("""CREATE TABLE Episodes (
                  EpisodeID INT AUTO_INCREMENT PRIMARY KEY,
                  PodcastID INT,
                  EpisodeTitle TEXT,
                  EpisodeDescription TEXT,
                  EpisodeURL TEXT,
                  EpisodePubDate DATE,
                  EpisodeDuration INT,
                  FOREIGN KEY (PodcastID) REFERENCES Podcasts(PodcastID)
                )""")

# Create the UserSettings table
cursor.execute("""CREATE TABLE UserSettings (
                  UserSettingID INT AUTO_INCREMENT PRIMARY KEY,
                  UserID INT,
                  SettingName TEXT,
                  SettingValue TEXT,
                  FOREIGN KEY (UserID) REFERENCES Users(UserID)
                )""")

# Create the EpisodeProgress table
cursor.execute("""CREATE TABLE EpisodeProgress (
                  EpisodeProgressID INT AUTO_INCREMENT PRIMARY KEY,
                  EpisodeID INT,
                  UserID INT,
                  EpisodeProgress INT,
                  FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID),
                  FOREIGN KEY (UserID) REFERENCES Users(UserID)
                )""")

# Close the cursor
cursor.close()

# Commit the changes
cnx.commit()

# Close the cursor and connection
cursor.close()
cnx.close()