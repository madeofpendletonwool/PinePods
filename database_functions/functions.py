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

def add_episodes(cnx, podcast_id, feed_url, artwork_url):
    import datetime
    import feedparser
    import dateutil.parser

    episode_dump = feedparser.parse(feed_url)

    cursor = cnx.cursor()

    try:
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

                # check if the episode already exists
                check_episode = ("SELECT * FROM Episodes "
                                "WHERE PodcastID = %s AND EpisodeTitle = %s")
                check_episode_values = (podcast_id, parsed_title)
                cursor.execute(check_episode, check_episode_values)
                if cursor.fetchone() is not None:
                    # episode already exists, skip it
                    continue

                # insert the episode into the database
                query = "INSERT INTO Episodes (PodcastID, EpisodeTitle, EpisodeDescription, EpisodeURL, EpisodeArtwork, EpisodePubDate, EpisodeDuration) VALUES (%s, %s, %s, %s, %s, %s, %s)"
                values = (podcast_id, parsed_title, parsed_description, parsed_audio_url, parsed_artwork_url, parsed_release_date, 0)
                cursor.execute(query, values)

                # check if any rows were affected by the insert operation
                if cursor.rowcount > 0:
                    print(f"Added episode '{parsed_title}'")

            else:
                print("Skipping entry without required attributes or enclosures")

        cnx.commit()

    except Exception as e:
        print(f"Error adding episodes: {e}")
        cnx.rollback()

    finally:
        cursor.close()

def remove_podcast(cnx, podcast_name):
    cursor = cnx.cursor()

    # Get the PodcastID for the given podcast name
    select_podcast_id = "SELECT PodcastID FROM Podcasts WHERE PodcastName = %s"
    cursor.execute(select_podcast_id, (podcast_name,))
    podcast_id = cursor.fetchone()[0]

    # Delete user episode history entries associated with the podcast
    delete_history = "DELETE FROM UserEpisodeHistory WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = %s)"
    cursor.execute(delete_history, (podcast_id,))

    # Delete downloaded episodes associated with the podcast
    delete_downloaded = "DELETE FROM DownloadedEpisodes WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = %s)"
    cursor.execute(delete_downloaded, (podcast_id,))

    # Delete episode queue items associated with the podcast
    delete_queue = "DELETE FROM EpisodeQueue WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = %s)"
    cursor.execute(delete_queue, (podcast_id,))

    # Delete episodes associated with the podcast
    delete_episodes = "DELETE FROM Episodes WHERE PodcastID = %s"
    cursor.execute(delete_episodes, (podcast_id,))

    # Delete the podcast
    delete_podcast = "DELETE FROM Podcasts WHERE PodcastName = %s"
    cursor.execute(delete_podcast, (podcast_name,))

    cnx.commit()

    cursor.close()



def remove_user(cnx, user_name):
    pass

def return_episodes(cnx):
    cursor = cnx.cursor(dictionary=True)

    query = ("SELECT Podcasts.PodcastName, Episodes.EpisodeTitle, Episodes.EpisodePubDate, "
             "Episodes.EpisodeDescription, Episodes.EpisodeArtwork, Episodes.EpisodeURL "
             "FROM Episodes "
             "INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
             "WHERE Episodes.EpisodePubDate >= DATE_SUB(NOW(), INTERVAL 30 DAY) "
             "ORDER BY Episodes.EpisodePubDate DESC")

    cursor.execute(query)
    rows = cursor.fetchall()

    cursor.close()

    if not rows:
        return None

    return rows

def return_pods(cnx):
    cursor = cnx.cursor(dictionary=True)

    query = ("SELECT PodcastName, ArtworkURL, Description, EpisodeCount, WebsiteURL, FeedURL, Author, Categories "
            "FROM Podcasts;")

    cursor.execute(query)
    rows = cursor.fetchall()

    cursor.close()

    if not rows:
        return None

    return rows

def refresh_pods(cnx):
    print('refresh begin')
    cursor = cnx.cursor()

    select_podcasts = "SELECT PodcastID, FeedURL, ArtworkURL FROM Podcasts"

    print('before query')

    cursor.execute(select_podcasts)
    result_set = cursor.fetchall() # fetch the result set

    cursor.nextset()  # move to the next result set

    print('after fetch')

    for (podcast_id, feed_url, artwork_url) in result_set:
        print(f'Running for :{podcast_id}')
        add_episodes(cnx, podcast_id, feed_url, artwork_url)

    cursor.close()

def get_podcast_id_by_title(cnx, podcast_title):
    cursor = cnx.cursor()

    # get the podcast ID for the specified title
    cursor.execute("SELECT PodcastID FROM Podcasts WHERE Title = %s", (podcast_title,))
    result = cursor.fetchone()

    if result:
        return result[0]
    else:
        return None


def refresh_podcast_by_title(cnx, podcast_title):
    # get the podcast ID for the specified title
    podcast_id = get_podcast_id_by_title(cnx, podcast_title)

    if podcast_id is not None:
        # refresh the podcast with the specified ID
        refresh_single_pod(cnx, podcast_id)
    else:
        print("Error: Could not find podcast with title {}".format(podcast_title))


def refresh_single_pod(cnx, podcast_id):
    cursor = cnx.cursor()

    # get the feed URL and artwork URL for the specified podcast
    cursor.execute("SELECT FeedURL, ArtworkURL FROM Podcasts WHERE PodcastID = %s", (podcast_id,))
    feed_url, artwork_url = cursor.fetchone()

    # parse the podcast feed
    episode_dump = feedparser.parse(feed_url)

    # get the list of episode titles already in the database
    cursor.execute("SELECT EpisodeTitle FROM Episodes WHERE PodcastID = %s", (podcast_id,))
    existing_titles = set(row[0] for row in cursor.fetchall())

    # insert any new episodes into the database
    for entry in episode_dump.entries:
        if hasattr(entry, "title") and hasattr(entry, "summary") and hasattr(entry, "enclosures"):
            title = entry.title

            # skip episodes that are already in the database
            if title in existing_titles:
                continue

            description = entry.summary
            audio_url = entry.enclosures[0].href if entry.enclosures else ""
            release_date = dateutil.parser.parse(entry.published).strftime("%Y-%m-%d")

            # get the URL of the episode artwork, or use the podcast image URL if not available
            artwork_url = entry.get('itunes_image', {}).get('href', None) or entry.get('image', {}).get('href', None) or artwork_url

            # insert the episode into the database
            add_episode = ("INSERT INTO Episodes "
                            "(PodcastID, EpisodeTitle, EpisodeDescription, EpisodeURL, EpisodeArtwork, EpisodePubDate, EpisodeDuration) "
                            "VALUES (%s, %s, %s, %s, %s, %s, %s)")
            episode_values = (podcast_id, title, description, audio_url, artwork_url, release_date, 0)
            cursor.execute(add_episode, episode_values)

    cnx.commit()

    cursor.close()

def record_podcast_history(cnx, episode_title, user_id, episode_pos):
    from datetime import datetime
    cursor = cnx.cursor()
    
    # Check if the episode exists in the database
    check_episode = ("SELECT EpisodeID FROM Episodes WHERE EpisodeTitle = %s")
    cursor.execute(check_episode, (episode_title,))
    result = cursor.fetchone()
    
    if result is not None:
        episode_id = result[0]

        # Check if a record already exists in the UserEpisodeHistory table
        check_history = ("SELECT * FROM UserEpisodeHistory "
                            "WHERE EpisodeID = %s AND UserID = %s")
        cursor.execute(check_history, (episode_id, user_id))
        result = cursor.fetchone()

        if result is not None:
            # Update the existing record
            update_history = ("UPDATE UserEpisodeHistory "
                                "SET ListenDuration = %s, ListenDate = %s "
                                "WHERE UserEpisodeHistoryID = %s")
            progress_id = result[0]
            new_listen_duration = round(episode_pos)
            now = datetime.now()
            values = (new_listen_duration, now, progress_id)
            cursor.execute(update_history, values)
        else:
            # Add a new record
            add_history = ("INSERT INTO UserEpisodeHistory "
                            "(EpisodeID, UserID, ListenDuration, ListenDate) "
                            "VALUES (%s, %s, %s, %s)")
            new_listen_duration = round(episode_pos)
            now = datetime.now()
            values = (episode_id, user_id, new_listen_duration, now)
            cursor.execute(add_history, values)

        cnx.commit()
        
    cursor.close()

def get_user_id(cnx, username):
    cursor = cnx.cursor()
    query = "SELECT UserID FROM Users WHERE Username = %s"
    cursor.execute(query, (username,))
    result = cursor.fetchone()
    cursor.close()

    if result:
        return result[0]
    else:
        return 1

def user_history(cnx, user_id):
    cursor = cnx.cursor()
    query = ("SELECT UserEpisodeHistory.ListenDate, UserEpisodeHistory.ListenDuration, "
             "Episodes.EpisodeTitle, Episodes.EpisodeDescription, Episodes.EpisodeArtwork, "
             "Episodes.EpisodeURL, Podcasts.PodcastName, Episodes.EpisodePubDate "
             "FROM UserEpisodeHistory "
             "JOIN Episodes ON UserEpisodeHistory.EpisodeID = Episodes.EpisodeID "
             "JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
             "WHERE UserEpisodeHistory.UserID = %s")
    cursor.execute(query, (user_id,))
    # results = cursor.fetchall()
    results = [dict(zip([column[0] for column in cursor.description], row)) for row in cursor.fetchall()]

    cursor.close()
    return results




if __name__ == '__main__':
    feed_url = "https://changelog.com/practicalai/feed"
    cnx = 'test'
    add_episodes(cnx, feed_url)
