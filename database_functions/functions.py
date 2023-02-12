import mysql.connector

def add_podcast(cnx, podcast_values):
    cursor = cnx.cursor()
    
    add_podcast = ("INSERT INTO Podcasts "
                "(PodcastID, PodcastName, ArtworkURL, Author, Categories, Description, EpisodeCount, FeedURL, WebsiteURL, UserID) "
                "VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s)")
    
    cursor.execute(add_podcast, podcast_values)
    
    cnx.commit()
    
    cursor.close()
    cnx.close()