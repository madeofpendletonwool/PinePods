import mysql.connector

def add_podcast(cnx, podcast_values):
    cursor = cnx.cursor()
    
    add_podcast = ("INSERT INTO Podcasts "
                "(PodcastName, ArtworkURL, Author, Categories, Description, EpisodeCount, FeedURL, WebsiteURL, UserID) "
                "VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s)")
    
    cursor.execute(add_podcast, podcast_values)
    
    cnx.commit()
    
    cursor.close()
    cnx.close()

def add_user(cnx, user_values):
    cursor = cnx.cursor()
    
    add_user = ("INSERT INTO Users "
                "(Username, Email, Hashed_PW, Salt) "
                "VALUES (%s, %s, %s, %s)")
    
    cursor.execute(add_user, user_values)
    
    cnx.commit()
    
    cursor.close()
    cnx.close()

def add_episodes(cnx, episode_values):
    pass

def remove_podcast(cnx, podcast_name):
    pass

def remove_user(cnx, user_name):
    pass

def remove_episodes(cnx, podcast_id):
    pass
