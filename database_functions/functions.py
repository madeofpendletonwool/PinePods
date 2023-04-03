import mysql.connector
from mysql.connector import errorcode
import sys
import os
import requests
import datetime
import time

def add_podcast(cnx, podcast_values, user_id):
    cursor = cnx.cursor()

    add_podcast = ("INSERT INTO Podcasts "
                "(PodcastName, ArtworkURL, Author, Categories, Description, EpisodeCount, FeedURL, WebsiteURL, UserID) "
                "VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s)")

    cursor.execute(add_podcast, podcast_values)

    # get the ID of the newly-inserted podcast
    podcast_id = cursor.lastrowid

    # Update UserStats table to increment PodcastsAdded count
    query = ("UPDATE UserStats SET PodcastsAdded = PodcastsAdded + 1 "
             "WHERE UserID = %s")
    cursor.execute(query, (user_id,))

    cnx.commit()

    cursor.close()

    # add episodes to database
    add_episodes(cnx, podcast_id, podcast_values[6], podcast_values[1])

def add_user(cnx, user_values):
    cursor = cnx.cursor()
    
    add_user = ("INSERT INTO Users "
                "(Fullname, Username, Email, Hashed_PW, Salt, IsAdmin) "
                "VALUES (%s, %s, %s, %s, %s, 0)")
    
    cursor.execute(add_user, user_values)
    
    user_id = cursor.lastrowid
    
    add_user_settings = ("INSERT INTO UserSettings "
                         "(UserID, Theme) "
                         "VALUES (%s, %s)")
    
    cursor.execute(add_user_settings, (user_id, 'nordic'))
    
    add_user_stats = ("INSERT INTO UserStats "
                      "(UserID) "
                      "VALUES (%s)")
    
    cursor.execute(add_user_stats, (user_id,))
    
    cnx.commit()
    
    cursor.close()


def add_admin_user(cnx, user_values):
    cursor = cnx.cursor()
    
    add_user = ("INSERT INTO Users "
                "(Fullname, Username, Email, Hashed_PW, Salt, IsAdmin) "
                "VALUES (%s, %s, %s, %s, %s, 1)")
    
    cursor.execute(add_user, user_values)
    
    user_id = cursor.lastrowid
    
    add_user_settings = ("INSERT INTO UserSettings "
                         "(UserID, Theme) "
                         "VALUES (%s, %s)")
    
    cursor.execute(add_user_settings, (user_id, 'nordic'))

    add_user_stats = ("INSERT INTO UserStats "
                      "(UserID) "
                      "VALUES (%s)")
    
    cursor.execute(add_user_stats, (user_id,))
    
    cnx.commit()
    
    cursor.close()

def add_episodes(cnx, podcast_id, feed_url, artwork_url):
    import datetime
    import feedparser
    import dateutil.parser
    import re

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



                parsed_duration = 0
                if entry.itunes_duration:
                    print('itunes_duration:')
                    duration_string = entry.itunes_duration
                    match = re.match(r'(\d+):(\d+)', duration_string)
                    if match:
                        parsed_duration = int(match.group(1)) * 60 + int(match.group(2))
                        print('Found duration using itunes_duration')
                    else:
                        try:
                            parsed_duration = int(duration_string)
                            print('Found duration using itunes_duration')
                        except ValueError:
                            print(f'Error parsing duration from itunes_duration: {duration_string}')

                elif entry.itunes_duration_seconds:
                    parsed_duration = entry.itunes_duration
                elif entry.duration:
                    parsed_duration = entry.itunes_duration
                elif entry.length:
                    parsed_duration = entry.itunes_duration
                else:
                    parsed_duration = 0

                print(parsed_duration)

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
                values = (podcast_id, parsed_title, parsed_description, parsed_audio_url, parsed_artwork_url, parsed_release_date, parsed_duration)
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

def remove_podcast(cnx, podcast_name, user_id):
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

    # Delete saved episodes associated with the podcast
    delete_saved = "DELETE FROM SavedEpisodes WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = %s)"
    cursor.execute(delete_saved, (podcast_id,))

    # Delete episode queue items associated with the podcast
    delete_queue = "DELETE FROM EpisodeQueue WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = %s)"
    cursor.execute(delete_queue, (podcast_id,))

    # Delete episodes associated with the podcast
    delete_episodes = "DELETE FROM Episodes WHERE PodcastID = %s"
    cursor.execute(delete_episodes, (podcast_id,))

    # Delete the podcast
    delete_podcast = "DELETE FROM Podcasts WHERE PodcastName = %s"
    cursor.execute(delete_podcast, (podcast_name,))

    # Update UserStats table to decrement PodcastsAdded count
    query = ("UPDATE UserStats SET PodcastsAdded = PodcastsAdded - 1 "
             "WHERE UserID = %s")
    cursor.execute(query, (user_id,))

    cnx.commit()

    cursor.close()



def remove_user(cnx, user_name):
    pass

def return_episodes(cnx, user_id):
    cursor = cnx.cursor(dictionary=True)

    query = (f"SELECT Podcasts.PodcastName, Episodes.EpisodeTitle, Episodes.EpisodePubDate, "
             f"Episodes.EpisodeDescription, Episodes.EpisodeArtwork, Episodes.EpisodeURL, Episodes.EpisodeDuration "
             f"FROM Episodes "
             f"INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
             f"WHERE Episodes.EpisodePubDate >= DATE_SUB(NOW(), INTERVAL 30 DAY) "
             f"AND Podcasts.UserID = %s "
             f"ORDER BY Episodes.EpisodePubDate DESC")

    cursor.execute(query, (user_id,))
    rows = cursor.fetchall()

    cursor.close()

    if not rows:
        return None

    return rows

def return_selected_episode(cnx, user_id, title, url):
    cursor = cnx.cursor()
    query = ("SELECT Episodes.EpisodeTitle, Episodes.EpisodeDescription, Episodes.EpisodeURL, "
            "Episodes.EpisodeArtwork, Episodes.EpisodePubDate, Episodes.EpisodeDuration, "
            "Podcasts.PodcastName, Podcasts.WebsiteURL "
            "FROM Episodes "
            "INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
            "WHERE Episodes.EpisodeTitle = %s AND Episodes.EpisodeURL = %s")

    cursor.execute(query, (title, url))
    result = cursor.fetchall()

    cursor.close()

    episodes = []
    for row in result:
        episode = {
            'EpisodeTitle': row[0],
            'EpisodeDescription': row[1],
            'EpisodeURL': row[2],
            'EpisodeArtwork': row[3],
            'EpisodePubDate': row[4],
            'EpisodeDuration': row[5],
            'PodcastName': row[6],
            'WebsiteURL': row[7]
        }
        episodes.append(episode)

    return episodes




def return_pods(cnx, user_id):
    cursor = cnx.cursor(dictionary=True)

    query = ("SELECT PodcastName, ArtworkURL, Description, EpisodeCount, WebsiteURL, FeedURL, Author, Categories "
            "FROM Podcasts "
            "WHERE UserID = %s")

    cursor.execute(query, (user_id,))
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

def remove_unavailable_episodes(cnx):
    cursor = cnx.cursor()

    # select all episodes
    select_episodes = "SELECT EpisodeID, PodcastID, EpisodeTitle, EpisodeURL, EpisodePubDate FROM Episodes"
    cursor.execute(select_episodes)
    episodes = cursor.fetchall()

    # iterate through all episodes
    for episode in episodes:
        episode_id, podcast_id, episode_title, episode_url, published_date = episode

        try:
            print('checking')
            # check if episode URL is still valid
            response = requests.head(episode_url)
            if response.status_code == 404:
                print('deleteing')
                # remove episode from database
                delete_episode = "DELETE FROM Episodes WHERE EpisodeID=%s"
                cursor.execute(delete_episode, (episode_id,))
                cnx.commit()

        except Exception as e:
            print(f"Error checking episode {episode_id}: {e}")

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

        # Fetch the result of the first query before executing the second query
        cursor.fetchone()

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

def get_user_details(cnx, username):
    cursor = cnx.cursor()
    query = "SELECT * FROM Users WHERE Username = %s"
    cursor.execute(query, (username,))
    result = cursor.fetchone()
    cursor.close()

    if result:
        return {
            'UserID': result[0],
            'Fullname': result[1],
            'Username': result[2],
            'Email': result[3],
            'Hashed_PW': result[4],
            'Salt': result[5]
        }
    else:
        return None

def user_history(cnx, user_id):
    cursor = cnx.cursor()
    query = ("SELECT UserEpisodeHistory.ListenDate, UserEpisodeHistory.ListenDuration, "
            "Episodes.EpisodeTitle, Episodes.EpisodeDescription, Episodes.EpisodeArtwork, "
            "Episodes.EpisodeURL, Episodes.EpisodeDuration, Podcasts.PodcastName, Episodes.EpisodePubDate "
            "FROM UserEpisodeHistory "
            "JOIN Episodes ON UserEpisodeHistory.EpisodeID = Episodes.EpisodeID "
            "JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
            "WHERE UserEpisodeHistory.UserID = %s "
            "ORDER BY UserEpisodeHistory.ListenDate")

    cursor.execute(query, (user_id,))
    # results = cursor.fetchall()
    results = [dict(zip([column[0] for column in cursor.description], row)) for row in cursor.fetchall()]

    cursor.close()
    return results


def download_podcast(cnx, url, title, user_id):
    # Get the episode ID from the Episodes table
    cursor = cnx.cursor()
    query = ("SELECT EpisodeID FROM Episodes "
             "WHERE EpisodeURL = %s AND EpisodeTitle = %s")
    cursor.execute(query, (url, title))
    episode_id = cursor.fetchone()

    if episode_id is None:
        # Episode not found
        return False

    episode_id = episode_id[0]
    print(episode_id)
    print(title)

    # Get the current date and time for DownloadedDate
    downloaded_date = datetime.datetime.now()

    # Make the request to download the file
    response = requests.get(url, stream=True)
    response.raise_for_status()

    # Get the file size from the Content-Length header
    file_size = int(response.headers.get("Content-Length", 0))

    # Set the download location to the user's downloads folder
    download_location = "/opt/pypods/downloads"

    # Generate a unique filename based on the current timestamp
    timestamp = time.time()
    filename = f"{user_id}-{episode_id}-{timestamp}.mp3"
    file_path = os.path.join(download_location, filename)
    print(file_path)

    # Write the file to disk
    with open(file_path, "wb") as f:
        for chunk in response.iter_content(chunk_size=1024):
            f.write(chunk)

    # Insert a new row into the DownloadedEpisodes table
    cursor = cnx.cursor()
    query = ("INSERT INTO DownloadedEpisodes "
             "(UserID, EpisodeID, DownloadedDate, DownloadedSize, DownloadedLocation) "
             "VALUES (%s, %s, %s, %s, %s)")
    cursor.execute(query, (user_id, episode_id, downloaded_date, file_size, file_path))

    # Update UserStats table to increment EpisodesDownloaded count
    query = ("UPDATE UserStats SET EpisodesDownloaded = EpisodesDownloaded + 1 "
             "WHERE UserID = %s")
    cursor.execute(query, (user_id,))

    cnx.commit()

    return True

def check_downloaded(cnx, user_id, title, url):
    cursor = None
    try:
        cursor = cnx.cursor()

        # Get the EpisodeID from the Episodes table
        query = "SELECT EpisodeID FROM Episodes WHERE EpisodeTitle = %s AND EpisodeURL = %s"
        cursor.execute(query, (title, url))
        episode_id = cursor.fetchone()[0]

        # Check if the episode is downloaded for the user
        query = "SELECT DownloadID FROM DownloadedEpisodes WHERE UserID = %s AND EpisodeID = %s"
        cursor.execute(query, (user_id, episode_id))
        result = cursor.fetchone()

        if result:
            return True
        else:
            return False

    except mysql.connector.errors.InterfaceError:
        return False
    finally:
        if cursor:
            cursor.close()
        cnx.commit()

def download_episode_list(cnx, user_id):
    cursor = cnx.cursor(dictionary=True)

    query = (f"SELECT Podcasts.PodcastName, Episodes.EpisodeTitle, Episodes.EpisodePubDate, "
             f"Episodes.EpisodeDescription, Episodes.EpisodeArtwork, Episodes.EpisodeURL, Episodes.EpisodeDuration, "
             f"Podcasts.WebsiteURL, DownloadedEpisodes.DownloadedLocation "
             f"FROM DownloadedEpisodes "
             f"INNER JOIN Episodes ON DownloadedEpisodes.EpisodeID = Episodes.EpisodeID "
             f"INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
             f"WHERE DownloadedEpisodes.UserID = %s "
             f"ORDER BY DownloadedEpisodes.DownloadedDate DESC")

    cursor.execute(query, (user_id,))
    rows = cursor.fetchall()

    cursor.close()

    if not rows:
        return None

    return rows

def delete_podcast(cnx, url, title, user_id):

    cursor = cnx.cursor()

    # Get the download ID from the DownloadedEpisodes table
    query = ("SELECT DownloadID, DownloadedLocation "
             "FROM DownloadedEpisodes "
             "INNER JOIN Episodes ON DownloadedEpisodes.EpisodeID = Episodes.EpisodeID "
             "INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
             "WHERE Episodes.EpisodeTitle = %s AND Episodes.EpisodeURL = %s AND Podcasts.UserID = %s")
    cursor.execute(query, (title, url, user_id))
    result = cursor.fetchone()

    if not result:
        print("No matching download found.")
        return

    download_id, downloaded_location = result

    # Delete the downloaded file
    os.remove(downloaded_location)
    print(f"Deleted downloaded file at {downloaded_location}")

    # Remove the entry from the DownloadedEpisodes table
    query = "DELETE FROM DownloadedEpisodes WHERE DownloadID = %s"
    cursor.execute(query, (download_id,))
    cnx.commit()
    print(f"Removed {cursor.rowcount} entry from the DownloadedEpisodes table.")

    # Update UserStats table to increment EpisodesDownloaded count
    query = ("UPDATE UserStats SET EpisodesDownloaded = EpisodesDownloaded - 1 "
             "WHERE UserID = %s")
    cursor.execute(query, (user_id,))
    
    cursor.close()

def get_episode_id(cnx, podcast_id, episode_title, episode_url):
    cursor = cnx.cursor()

    query = "SELECT EpisodeID FROM Episodes WHERE PodcastID = %s AND EpisodeTitle = %s AND EpisodeURL = %s"
    params = (podcast_id, episode_title, episode_url)

    cursor.execute(query, params)
    result = cursor.fetchone()

    if result:
        episode_id = result[0]
    else:
        # Episode not found, insert a new episode into the Episodes table
        query = "INSERT INTO Episodes (PodcastID, EpisodeTitle, EpisodeURL) VALUES (%s, %s, %s)"
        params = (podcast_id, episode_title, episode_url)

        cursor.execute(query, params)
        episode_id = cursor.lastrowid

    cnx.commit()
    cursor.close()

    return episode_id

def queue_podcast_entry(cnx, user_id, episode_title, episode_url):
    cursor = cnx.cursor()

    # Get the episode ID using the episode title and URL
    query = "SELECT EpisodeID, PodcastID FROM Episodes WHERE EpisodeTitle = %s AND EpisodeURL = %s"
    cursor.execute(query, (episode_title, episode_url))
    result = cursor.fetchone()

    if result:
        episode_id, podcast_id = result

        # Check if the episode is already in the queue
        query = "SELECT COUNT(*) FROM EpisodeQueue WHERE UserID = %s AND EpisodeID = %s"
        cursor.execute(query, (user_id, episode_id))
        count = cursor.fetchone()[0]

        if count > 0:
            # Episode is already in the queue, move it to position 1 and update the QueueDate
            query = "UPDATE EpisodeQueue SET QueuePosition = 1, QueueDate = CURRENT_TIMESTAMP WHERE UserID = %s AND EpisodeID = %s"
            cursor.execute(query, (user_id, episode_id))
            cnx.commit()
        else:
            # Episode is not in the queue, insert it at position 1
            query = "INSERT INTO EpisodeQueue (UserID, EpisodeID, QueuePosition) VALUES (%s, %s, 1)"
            cursor.execute(query, (user_id, episode_id))
            cnx.commit()

        cursor.close()

        return True
    else:
        # Episode not found in the database
        cursor.close()
        return False

def episode_remove_queue(cnx, user_id, url, title):
    cursor = cnx.cursor()

    # Get the episode ID using the episode title and URL
    query = "SELECT EpisodeID FROM Episodes WHERE EpisodeTitle = %s AND EpisodeURL = %s"
    cursor.execute(query, (title, url))
    episode_id = cursor.fetchone()

    if episode_id:
        # Remove the episode from the user's queue
        query = "DELETE FROM EpisodeQueue WHERE UserID = %s AND EpisodeID = %s"
        cursor.execute(query, (user_id, episode_id[0])) # Extract the episode ID from the tuple
        cnx.commit()

        cursor.close()

        return True
    else:
        # Episode not found in the database
        cursor.close()
        return False



def get_queue_list(cnx, queue_urls):
    if not queue_urls:
        return None
    
    query_template = """
        SELECT Episodes.EpisodeTitle, Podcasts.PodcastName, Episodes.EpisodePubDate,
            Episodes.EpisodeDescription, Episodes.EpisodeArtwork, Episodes.EpisodeURL, Episodes.EpisodeDuration, 
            NOW() as QueueDate
        FROM Episodes
        INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID
        WHERE Episodes.EpisodeURL IN ({})
    """
    placeholders = ",".join(["%s"] * len(queue_urls))
    query = query_template.format(placeholders)

    cursor = cnx.cursor(dictionary=True)
    cursor.execute(query, queue_urls)

    episode_list = cursor.fetchall()
    cursor.close()
    return episode_list

def check_usernames(cnx, username):
    cursor = cnx.cursor()
    query = ("SELECT COUNT(*) FROM Users WHERE Username = %s")
    cursor.execute(query, (username,))
    count = cursor.fetchone()[0]
    cursor.close()
    return count > 0

def record_listen_duration(cnx, url, title, user_id, listen_duration):
    listen_date = datetime.datetime.now()
    cursor = cnx.cursor()

    # Get EpisodeID from Episodes table
    cursor.execute("SELECT EpisodeID FROM Episodes WHERE EpisodeURL=%s AND EpisodeTitle=%s", (url, title))
    result = cursor.fetchone()
    if result is None:
        # Episode not found in database, handle this case
        cursor.close()
        return
    episode_id = result[0]

    # Check if UserEpisodeHistory row already exists for the given user and episode
    cursor.execute("SELECT * FROM UserEpisodeHistory WHERE UserID=%s AND EpisodeID=%s", (user_id, episode_id))
    existing_row = cursor.fetchone()

    if existing_row:
        # UserEpisodeHistory row already exists, update ListenDuration
        listen_duration_data = (listen_duration, user_id, episode_id)
        update_listen_duration = ("UPDATE UserEpisodeHistory SET ListenDuration=%s WHERE UserID=%s AND EpisodeID=%s")
        cursor.execute(update_listen_duration, listen_duration_data)
    else:
        # UserEpisodeHistory row does not exist, insert new row
        add_listen_duration = ("INSERT INTO UserEpisodeHistory "
                            "(UserID, EpisodeID, ListenDate, ListenDuration) "
                            "VALUES (%s, %s, %s, %s)")
        listen_duration_data = (user_id, episode_id, listen_date, listen_duration)
        cursor.execute(add_listen_duration, listen_duration_data)

    cnx.commit()
    cursor.close()

def check_episode_playback(cnx, user_id, episode_title, episode_url):
    cursor = None
    try:
        cursor = cnx.cursor()

        # Get the EpisodeID from the Episodes table
        query = "SELECT EpisodeID FROM Episodes WHERE EpisodeTitle = %s AND EpisodeURL = %s"
        cursor.execute(query, (episode_title, episode_url))
        episode_id = cursor.fetchone()[0]

        # Check if the user has played the episode before
        query = "SELECT ListenDuration FROM UserEpisodeHistory WHERE UserID = %s AND EpisodeID = %s"
        cursor.execute(query, (user_id, episode_id))
        result = cursor.fetchone()

        if result:
            listen_duration = result[0]
            return True, listen_duration
        else:
            return False, 0
    except mysql.connector.errors.InterfaceError:
        return False, 0
    finally:
        if cursor:
            cursor.close()
        cnx.commit()




def get_episode_listen_time(cnx, user_id, title, url):
        cursor = None
        try:
            cursor = cnx.cursor()

            # Get the EpisodeID from the Episodes table
            query = "SELECT EpisodeID FROM Episodes WHERE EpisodeTitle = %s AND EpisodeURL = %s"
            cursor.execute(query, (title, url))
            episode_id = cursor.fetchone()[0]

            # Get the user's listen duration for this episode
            query = "SELECT ListenDuration FROM UserEpisodeHistory WHERE UserID = %s AND EpisodeID = %s"
            cursor.execute(query, (user_id, episode_id))
            listen_duration = cursor.fetchone()[0]

            return listen_duration

            # Seek to the user's last listen duration
            # current_episode.seek_to_second(listen_duration)

        finally:
            if cursor:
                cursor.close()

def get_theme(cnx, user_id):
    print(user_id)
    cursor = None
    try:
        cursor = cnx.cursor()

        # Get the EpisodeID from the Episodes table
        query = "SELECT Theme FROM UserSettings WHERE UserID = %s"
        cursor.execute(query, (user_id,))
        theme = cursor.fetchone()[0]

        return theme

    finally:
        if cursor:
            cursor.close()

def set_theme(cnx, user_id, theme):
    cursor = None
    try:
        cursor = cnx.cursor()

        # Update the UserSettings table with the new theme value
        query = "UPDATE UserSettings SET Theme = %s WHERE UserID = %s"
        cursor.execute(query, (theme, user_id))
        cnx.commit()

    finally:
        if cursor:
            cursor.close()

def get_user_info(cnx):
    cursor = cnx.cursor(dictionary=True)

    query = (f"SELECT Users.UserID, Users.Fullname, Users.Username, "
             f"Users.Email, Users.IsAdmin "
             f"FROM Users ")


    cursor.execute(query)
    rows = cursor.fetchall()

    cursor.close()

    if not rows:
        return None

    return rows

def set_username(cnx, user_id, new_username):
    cursor = cnx.cursor()
    query = "UPDATE Users SET Username = %s WHERE UserID = %s"
    cursor.execute(query, (new_username, user_id))
    cnx.commit()
    cursor.close()

def set_password(cnx, user_id, salt, hash_pw):
    cursor = cnx.cursor()
    update_query = "UPDATE Users SET Salt=%s, Hashed_PW=%s WHERE UserID=%s"
    cursor.execute(update_query, (salt, hash_pw, user_id))
    cnx.commit()
    cursor.close()


def set_email(cnx, user_id, new_email):
    cursor = cnx.cursor()
    query = "UPDATE Users SET Email = %s WHERE UserID = %s"
    cursor.execute(query, (new_email, user_id))
    cnx.commit()
    cursor.close()

def set_fullname(cnx, user_id, new_name):
    cursor = cnx.cursor()
    query = "UPDATE Users SET Fullname = %s WHERE UserID = %s"
    cursor.execute(query, (new_name, user_id))
    cnx.commit()
    cursor.close()

def set_isadmin(cnx, user_id, isadmin):
    cursor = cnx.cursor()
    
    # Convert boolean isadmin value to integer (0 or 1)
    isadmin_int = int(isadmin)
    
    query = f"UPDATE Users SET IsAdmin = {isadmin_int} WHERE UserID = {user_id}"
    
    cursor.execute(query)
    cnx.commit()
    
    cursor.close()

def delete_user(cnx, user_id):
    cursor = cnx.cursor()

    # Delete user from UserEpisodeHistory table
    try:
        query = "DELETE FROM UserEpisodeHistory WHERE UserID = %s"
        cursor.execute(query, (user_id,))
    except:
        pass

    # Delete user from DownloadedEpisodes table
    try:
        query = "DELETE FROM DownloadedEpisodes WHERE UserID = %s"
        cursor.execute(query, (user_id,))
    except:
        pass

    # Delete user from EpisodeQueue table
    try:
        query = "DELETE FROM EpisodeQueue WHERE UserID = %s"
        cursor.execute(query, (user_id,))
    except:
        pass

    # Delete user from Podcasts table
    try:
        query = "DELETE FROM Podcasts WHERE UserID = %s"
        cursor.execute(query, (user_id,))
    except:
        pass

    # Delete user from UserSettings table
    try:
        query = "DELETE FROM UserSettings WHERE UserID = %s"
        cursor.execute(query, (user_id,))
    except:
        pass

    # Delete user from UserStats table
    try:
        query = "DELETE FROM UserStats WHERE UserID = %s"
        cursor.execute(query, (user_id,))
    except:
        pass

    # Delete user from Users table
    query = "DELETE FROM Users WHERE UserID = %s"
    cursor.execute(query, (user_id,))
    cnx.commit()

    cursor.close()


def user_admin_check(cnx, user_id):
    cursor = cnx.cursor()
    query = f"SELECT IsAdmin FROM Users WHERE UserID = '{user_id}'"
    cursor.execute(query)
    result = cursor.fetchone()
    cursor.close()
    
    if result is None:
        return False
    
    return bool(result[0])

def final_admin(cnx, user_id):
    cursor = cnx.cursor()

    # Check if user being deleted is the final admin user
    query = "SELECT COUNT(*) FROM Users WHERE IsAdmin = 1"
    cursor.execute(query)
    admin_count = cursor.fetchone()[0]

    if admin_count == 1:
        query = "SELECT IsAdmin FROM Users WHERE UserID = %s"
        cursor.execute(query, (user_id,))
        is_admin = cursor.fetchone()[0]
        if is_admin == 1:
            return True

    cursor.close()
    return False

def guest_status(cnx):
    cursor = cnx.cursor()
    query = "SELECT Email FROM Users WHERE Email = 'active'"
    cursor.execute(query)
    result = cursor.fetchone()
    cursor.close()
    if result:
        return True
    else:
        return False

def enable_disable_guest(cnx):
    cursor = cnx.cursor()
    query = "UPDATE Users SET Email = CASE WHEN Email = 'inactive' THEN 'active' ELSE 'inactive' END WHERE Username = 'guest'"
    cursor.execute(query)
    cnx.commit()
    cursor.close()

def self_service_status(cnx):
    cursor = cnx.cursor()
    query = "SELECT SelfServiceUser FROM AppSettings WHERE SelfServiceUser = 1"
    cursor.execute(query)
    result = cursor.fetchone()
    cursor.close()
    if result:
        return True
    else:
        return False

def enable_disable_self_service(cnx):
    cursor = cnx.cursor()
    query = "UPDATE AppSettings SET SelfServiceUser = CASE WHEN SelfServiceUser = 0 THEN 1 ELSE 0 END"
    cursor.execute(query)
    cnx.commit()
    cursor.close()

def get_stats(cnx, user_id):
    cursor = cnx.cursor()
    
    query = ("SELECT UserCreated, PodcastsPlayed, TimeListened, PodcastsAdded, EpisodesSaved, EpisodesDownloaded "
             "FROM UserStats "
             "WHERE UserID = %s")
    
    cursor.execute(query, (user_id,))
    
    result = cursor.fetchone()
    stats = {
        "UserCreated": result[0],
        "PodcastsPlayed": result[1],
        "TimeListened": result[2],
        "PodcastsAdded": result[3],
        "EpisodesSaved": result[4],
        "EpisodesDownloaded": result[5]
    }
    
    cursor.close()
    
    return stats

def saved_episode_list(cnx, user_id):
    cursor = cnx.cursor(dictionary=True)

    query = (f"SELECT Podcasts.PodcastName, Episodes.EpisodeTitle, Episodes.EpisodePubDate, "
             f"Episodes.EpisodeDescription, Episodes.EpisodeArtwork, Episodes.EpisodeURL, Episodes.EpisodeDuration, "
             f"Podcasts.WebsiteURL "
             f"FROM SavedEpisodes "
             f"INNER JOIN Episodes ON SavedEpisodes.EpisodeID = Episodes.EpisodeID "
             f"INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
             f"WHERE SavedEpisodes.UserID = %s "
             f"ORDER BY SavedEpisodes.SaveDate DESC")

    cursor.execute(query, (user_id,))
    rows = cursor.fetchall()

    cursor.close()

    if not rows:
        return None

    return rows

def save_episode(cnx, url, title, user_id):
    # Get the episode ID from the Episodes table
    cursor = cnx.cursor()
    query = ("SELECT EpisodeID FROM Episodes "
             "WHERE EpisodeURL = %s AND EpisodeTitle = %s")
    cursor.execute(query, (url, title))
    episode_id = cursor.fetchone()

    if episode_id is None:
        # Episode not found
        return False

    episode_id = episode_id[0]
    print(episode_id)
    print(title)

    # Insert a new row into the DownloadedEpisodes table
    cursor = cnx.cursor()
    query = ("INSERT INTO SavedEpisodes "
             "(UserID, EpisodeID) "
             "VALUES (%s, %s)")
    cursor.execute(query, (user_id, episode_id))

    # Update UserStats table to increment EpisodesSaved count
    query = ("UPDATE UserStats SET EpisodesSaved = EpisodesSaved + 1 "
             "WHERE UserID = %s")
    cursor.execute(query, (user_id,))

    cnx.commit()

    return True

def check_saved(cnx, user_id, title, url):
    cursor = None
    try:
        cursor = cnx.cursor()

        # Get the EpisodeID from the Episodes table
        query = "SELECT EpisodeID FROM Episodes WHERE EpisodeTitle = %s AND EpisodeURL = %s"
        cursor.execute(query, (title, url))
        episode_id = cursor.fetchone()[0]

        # Check if the episode is saved for the user
        query = "SELECT * FROM SavedEpisodes WHERE UserID = %s AND EpisodeID = %s"
        cursor.execute(query, (user_id, episode_id))
        result = cursor.fetchone()

        if result:
            return True
        else:
            return False
    except mysql.connector.Error as err:
        print("Error checking saved episode: {}".format(err))
        return False
    finally:
        if cursor:
            cursor.close()


def remove_saved_episode(cnx, url, title, user_id):

    cursor = cnx.cursor()

    # Get the Save ID from the SavedEpisodes table
    query = ("SELECT SaveID "
             "FROM SavedEpisodes "
             "INNER JOIN Episodes ON SavedEpisodes.EpisodeID = Episodes.EpisodeID "
             "INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
             "WHERE Episodes.EpisodeTitle = %s AND Episodes.EpisodeURL = %s AND Podcasts.UserID = %s")
    cursor.execute(query, (title, url, user_id))
    save_id = cursor.fetchone()

    if not save_id:
        print("No matching episode found.")
        return

    # Remove the entry from the SavedEpisodes table
    query = "DELETE FROM SavedEpisodes WHERE SaveID = %s"
    cursor.execute(query, (save_id[0],))

    # Update UserStats table to increment EpisodesSaved count
    query = ("UPDATE UserStats SET EpisodesSaved = EpisodesSaved - 1 "
             "WHERE UserID = %s")
    cursor.execute(query, (user_id,))

    cnx.commit()
    print(f"Removed {cursor.rowcount} entry from the SavedEpisodes table.")
    
    cursor.close()

def increment_played(cnx, user_id):
    cursor = cnx.cursor()

    # Update UserStats table to increment PodcastsPlayed count
    query = ("UPDATE UserStats SET PodcastsPlayed = PodcastsPlayed + 1 "
             "WHERE UserID = %s")
    cursor.execute(query, (user_id,))
    cnx.commit()
    
    cursor.close()

def increment_listen_time(cnx, user_id):
    cursor = cnx.cursor()

    # Update UserStats table to increment PodcastsPlayed count
    query = ("UPDATE UserStats SET TimeListened = TimeListened + 1 "
             "WHERE UserID = %s")
    cursor.execute(query, (user_id,))
    cnx.commit()
    
    cursor.close()

def get_user_episode_count(cnx, user_id):
    cursor = cnx.cursor()
    
    query = ("SELECT COUNT(*) "
             "FROM Episodes "
             "INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
             "WHERE Podcasts.UserID = %s")
    
    cursor.execute(query, (user_id,))
    
    episode_count = cursor.fetchone()[0]
    
    cursor.close()
    
    return episode_count

def check_podcast(cnx, user_id, podcast_name):
    cursor = None
    try:
        cursor = cnx.cursor()

        query = "SELECT PodcastID FROM Podcasts WHERE UserID = %s AND PodcastName = %s"
        cursor.execute(query, (user_id, podcast_name))

        if cursor.fetchone() is not None:
            return True
        else:
            return False
    except mysql.connector.errors.InterfaceError:
        return False
    finally:
        if cursor:
            cursor.close()
        cnx.commit()