CREATE TABLE Users (
  UserID INT AUTO_INCREMENT PRIMARY KEY,
  Username TEXT,
  Email VARCHAR(255),
  Password CHAR(60)
);

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

CREATE TABLE EpisodeProgress (
  EpisodeProgressID INT AUTO_INCREMENT PRIMARY KEY,
  EpisodeID INT,
  UserID INT,
  EpisodeProgress INT,
  FOREIGN KEY (EpisodeID) REFERENCES Episodes(EpisodeID),
  FOREIGN KEY (UserID) REFERENCES Users(UserID)
);