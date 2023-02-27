import mysql.connector
import sys

def add_podcast(cnx, podcast_values):
    cursor = cnx.cursor()

    add_podcast = ("INSERT INTO Podcasts "
                "(PodcastName, ArtworkURL, Author, Categories, Description, EpisodeCount, FeedURL, WebsiteURL, UserID) "
                "VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s)")

    cursor.execute(add_podcast, podcast_values)

    # get the ID of the newly-inserted podcast
    podcast_id = cursor.lastrowid

    cnx.commit()

    cursor.close()

    # add episodes to database
    add_episodes(cnx, podcast_id, podcast_values[6], podcast_values[1])

def add_user(cnx, user_values):
    cursor = cnx.cursor()
    
    add_user = ("INSERT INTO Users "
                "(Fullname, Username, Email, Hashed_PW, Salt) "
                "VALUES (%s, %s, %s, %s, %s)")
    
    cursor.execute(add_user, user_values)
    
    cnx.commit()
    
    cursor.close()
    cnx.close()

def add_episodes(cnx, podcast_id, feed_url, artwork_url):
    import datetime
    import feedparser
    import dateutil.parser

    episode_dump = feedparser.parse(feed_url)    

    cursor = cnx.cursor()

    for entry in episode_dump.entries:
        if hasattr(entry, "title") and hasattr(entry, "summary") and hasattr(entry, "enclosures"):
            # get the episode title
            parsed_title = entry.title

            # get the episode description
            parsed_description = entry.summary

            # get the URL of the audio file for the episode
            if entry.enclosures:
                parsed_audio_url = entry.enclosures[0].href
            else:
                parsed_audio_url = ""

            # get the release date of the episode and convert it to a MySQL date format
            parsed_release_date = dateutil.parser.parse(entry.published).strftime("%Y-%m-%d")

            # get the URL of the episode artwork, or use the podcast image URL if not available
            parsed_artwork_url = entry.get('itunes_image', {}).get('href', None) or entry.get('image', {}).get('href', None)
            if parsed_artwork_url == None:
                parsed_artwork_url = artwork_url

            # insert the episode into the database
            add_episode = ("INSERT INTO Episodes "
                            "(PodcastID, EpisodeTitle, EpisodeDescription, EpisodeURL, EpisodePubDate, EpisodeDuration) "
                            "VALUES (%s, %s, %s, %s, %s, %s)")
            episode_values = (podcast_id, parsed_title, parsed_description, parsed_audio_url, parsed_release_date, 0)
            cursor.execute(add_episode, episode_values)

        else:
            print("Skipping entry without required attributes or enclosures")

    cnx.commit()

    cursor.close()

def remove_podcast(cnx, podcast_name):
    pass

def remove_user(cnx, user_name):
    pass

def remove_episodes(cnx, podcast_id):
    pass

if __name__ == '__main__':
    feed_url = "https://changelog.com/practicalai/feed"
    cnx = 'test'
    add_episodes(cnx, feed_url)
