CREATE TABLE Users (
  UserID INT AUTO_INCREMENT PRIMARY KEY,
  Fullname TEXT,
  Username TEXT,
  Email VARCHAR(255),
  Hashed_PW CHAR(60),
  Salt CHAR(60),
  IsAdmin TINYINT(1)
);

INSERT INTO Users (Fullname, Username, Email, Hashed_PW, Salt)
VALUES ('Guest User', 'guest', 'guest@pypods.com', 'Hmc7toxfqLssTdzaFGiKhigJ4VN3JeEy8VTkVHQ2FFrxAg74FrdoPRXowqgh', 'Hmc7toxfqLssTdzaFGiKhigJ4VN3JeEy8VTkVHQ2FFrxAg74FrdoPRXowqgh');


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
  UserID INT,
  SettingName TEXT,
  SettingValue TEXT,
  FOREIGN KEY (UserID) REFERENCES Users(UserID)
);

CREATE TABLE UserEpisodeHistory (
  UserEpisodeHistoryID INT AUTO_INCREMENT PRIMARY KEY,
  UserID INT,
  EpisodeID INT,
  ListenDate DATETIME,
  ListenDuration INT,
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