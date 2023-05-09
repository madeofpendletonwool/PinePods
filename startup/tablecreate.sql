CREATE TABLE Users (
  UserID INT AUTO_INCREMENT PRIMARY KEY,
  Fullname TEXT,
  Username TEXT,
  Email VARCHAR(255),
  Hashed_PW CHAR(60),
  Salt CHAR(60),
  IsAdmin TINYINT(1)
);

CREATE TABLE APIKeys (
  APIKeyID INT AUTO_INCREMENT PRIMARY KEY,
  UserID INT,
  APIKey TEXT,
  Created TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (UserID) REFERENCES Users(UserID) ON DELETE CASCADE
);


CREATE TABLE UserStats (
  UserStatsID INT AUTO_INCREMENT PRIMARY KEY,
  UserID INT,
  UserCreated TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  PodcastsPlayed INT DEFAULT 0,
  TimeListened INT DEFAULT 0,
  PodcastsAdded INT DEFAULT 0,
  EpisodesSaved INT DEFAULT 0,
  EpisodesDownloaded INT DEFAULT 0,
  FOREIGN KEY (UserID) REFERENCES Users(UserID)
);

CREATE TABLE AppSettings (
  AppSettingsID INT AUTO_INCREMENT PRIMARY KEY,
  SelfServiceUser TINYINT(1) DEFAULT 0
);

INSERT INTO AppSettings (SelfServiceUser) VALUES (0);

INSERT INTO Users (Fullname, Username, Email, Hashed_PW, Salt, IsAdmin)
VALUES ('Guest User', 'guest', 'inactive', 'Hmc7toxfqLssTdzaFGiKhigJ4VN3JeEy8VTkVHQ2FFrxAg74FrdoPRXowqgh', 'Hmc7toxfqLssTdzaFGiKhigJ4VN3JeEy8VTkVHQ2FFrxAg74FrdoPRXowqgh', 0);
INSERT INTO UserStats (UserID) VALUES (1);


CREATE TABLE Podcasts (
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
);

CREATE TABLE Episodes (
  EpisodeID INT AUTO_INCREMENT PRIMARY KEY,
  PodcastID INT,
  EpisodeTitle TEXT,
  EpisodeDescription TEXT,
  EpisodeURL TEXT,
  EpisodeArtwork TEXT,
  EpisodePubDate DATE,
  EpisodeDuration INT,
  FOREIGN KEY (PodcastID) REFERENCES Podcasts(PodcastID)
);

CREATE TABLE UserSettings (
  UserSettingID INT AUTO_INCREMENT PRIMARY KEY,
  UserID INT UNIQUE,
  Theme VARCHAR(255) DEFAULT 'nordic',
  FOREIGN KEY (UserID) REFERENCES Users(UserID)
);


INSERT INTO UserSettings (UserID, Theme)
VALUES ('1', 'nordic');


CREATE TABLE UserEpisodeHistory (
  UserEpisodeHistoryID INT AUTO_INCREMENT PRIMARY KEY,
  UserID INT,
  EpisodeID INT,
  ListenDate DATETIME,
  ListenDuration INT,
  FOREIGN KEY (UserID) REFERENCES Users(UserID),
  FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID)
);

CREATE TABLE SavedEpisodes (
  SaveID INT AUTO_INCREMENT PRIMARY KEY,
  UserID INT,
  EpisodeID INT,
  SaveDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (UserID) REFERENCES Users(UserID),
  FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID)
);

CREATE TABLE DownloadedEpisodes (
  DownloadID INT AUTO_INCREMENT PRIMARY KEY,
  UserID INT,
  EpisodeID INT,
  DownloadedDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  DownloadedSize INT,
  DownloadedLocation VARCHAR(255),
  FOREIGN KEY (UserID) REFERENCES Users(UserID),
  FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID)
);

CREATE TABLE EpisodeQueue (
  QueueID INT AUTO_INCREMENT PRIMARY KEY,
  QueueDate TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
  UserID INT,
  EpisodeID INT,
  QueuePosition INT NOT NULL DEFAULT 0,
  FOREIGN KEY (UserID) REFERENCES Users(UserID),
  FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID)
);

CREATE TABLE Sessions (
    SessionID INT AUTO_INCREMENT PRIMARY KEY,
    UserID INT,
    value TEXT,
    expire DATETIME NOT NULL,
    FOREIGN KEY (UserID) REFERENCES Users(UserID)
);
