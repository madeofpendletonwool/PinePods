import mysql.connector
from mysql.connector import errorcode
import mysql.connector.pooling
import sys
import os
import requests
import datetime
import time
import appdirs
import base64

def add_podcast(cnx, podcast_values, user_id):
    cursor = cnx.cursor()

    # check if the podcast already exists for the user
    query = ("SELECT PodcastID FROM Podcasts "
             "WHERE FeedURL = %s AND UserID = %s")

    cursor.execute(query, (podcast_values[6], user_id))
    result = cursor.fetchone()

    if result is not None:
        # podcast already exists for the user, return False
        cursor.close()
        # cnx.close()
        return False

    # insert the podcast into the database
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

    # add episodes to database
    add_episodes(cnx, podcast_id, podcast_values[6], podcast_values[1])

    cursor.close()
    # cnx.close()

    # return True to indicate success
    return True


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
    # cnx.close()


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
                parsed_description = entry.get('content', [{}])[0].get('value', entry.summary)

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



                def parse_duration(duration_string: str) -> int:
                    match = re.match(r'(\d+):(\d+)(?::(\d+))?', duration_string)  # Regex to optionally match HH:MM:SS
                    if match:
                        if match.group(3):  # If there is an HH part
                            parsed_duration = int(match.group(1)) * 3600 + int(match.group(2)) * 60 + int(match.group(3))
                        else:  # It's only MM:SS
                            parsed_duration = int(match.group(1)) * 60 + int(match.group(2))
                    else:
                        try:
                            parsed_duration = int(duration_string)
                        except ValueError:
                            print(f'Error parsing duration from duration_string: {duration_string}')
                            parsed_duration = 0
                    return parsed_duration

                parsed_duration = 0
                if entry.itunes_duration:
                    parsed_duration = parse_duration(entry.itunes_duration)
                elif entry.itunes_duration_seconds:
                    parsed_duration = entry.itunes_duration_seconds
                elif entry.duration:
                    parsed_duration = parse_duration(entry.duration)
                elif entry.length:
                    parsed_duration = parse_duration(entry.length)


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
    # cnx.close()



def remove_user(cnx, user_name):
    pass

def return_episodes(cnx, user_id):
    cursor = cnx.cursor(dictionary=True)

    query = (f"SELECT Podcasts.PodcastName, Episodes.EpisodeTitle, Episodes.EpisodePubDate, "
             f"Episodes.EpisodeDescription, Episodes.EpisodeArtwork, Episodes.EpisodeURL, Episodes.EpisodeDuration, "
             f"UserEpisodeHistory.ListenDuration "
             f"FROM Episodes "
             f"INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
             f"LEFT JOIN UserEpisodeHistory ON Episodes.EpisodeID = UserEpisodeHistory.EpisodeID AND UserEpisodeHistory.UserID = %s "
             f"WHERE Episodes.EpisodePubDate >= DATE_SUB(NOW(), INTERVAL 30 DAY) "
             f"AND Podcasts.UserID = %s "
             f"ORDER BY Episodes.EpisodePubDate DESC")

    cursor.execute(query, (user_id, user_id))
    rows = cursor.fetchall()

    cursor.close()

    if not rows:
        return None

    return rows

def return_selected_episode(cnx, user_id, title, url):
    cursor = cnx.cursor()
    query = ("SELECT Episodes.EpisodeTitle, Episodes.EpisodeDescription, Episodes.EpisodeURL, "
            "Episodes.EpisodeArtwork, Episodes.EpisodePubDate, Episodes.EpisodeDuration, "
            "Podcasts.PodcastName, Podcasts.WebsiteURL, Podcasts.FeedURL "
            "FROM Episodes "
            "INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
            "WHERE Episodes.EpisodeTitle = %s AND Episodes.EpisodeURL = %s")

    cursor.execute(query, (title, url))
    result = cursor.fetchall()

    cursor.close()
    # cnx.close()

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
    # cnx.close()

    if not rows:
        return None

    return rows


def refresh_pods(cnx):
    import concurrent.futures

    print('refresh begin')
    cursor = cnx.cursor()

    select_podcasts = "SELECT PodcastID, FeedURL, ArtworkURL FROM Podcasts"

    cursor.execute(select_podcasts)
    result_set = cursor.fetchall() # fetch the result set

    cursor.nextset()  # move to the next result set

    for (podcast_id, feed_url, artwork_url) in result_set:
        print(f'Running for :{podcast_id}')
        add_episodes(cnx, podcast_id, feed_url, artwork_url)

    cursor.close()
    # cnx.close()

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
            # check if episode URL is still valid
            response = requests.head(episode_url)
            if response.status_code == 404:
                # remove episode from database
                delete_episode = "DELETE FROM Episodes WHERE EpisodeID=%s"
                cursor.execute(delete_episode, (episode_id,))
                cnx.commit()

        except Exception as e:
            print(f"Error checking episode {episode_id}: {e}")

    cursor.close()
    # cnx.close()




def get_podcast_id_by_title(cnx, podcast_title):
    cursor = cnx.cursor()

    # get the podcast ID for the specified title
    cursor.execute("SELECT PodcastID FROM Podcasts WHERE Title = %s", (podcast_title,))
    result = cursor.fetchone()

    if result:
        return result[0]
    else:
        return None

    cursor.close()
    # cnx.close()


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
    # cnx.close()

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
    # cnx.close()


def get_user_id(cnx, username):
    cursor = cnx.cursor()
    query = "SELECT UserID FROM Users WHERE Username = %s"
    cursor.execute(query, (username,))
    result = cursor.fetchone()
    cursor.close()
    # cnx.close()

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
    # cnx.close()

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

def get_user_details_id(cnx, user_id):
    cursor = cnx.cursor()
    query = "SELECT * FROM Users WHERE UserID = %s"
    cursor.execute(query, (user_id,))
    result = cursor.fetchone()
    cursor.close()
    # cnx.close()

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
    # cnx.close()
    return results



def download_podcast(cnx, url, title, user_id):
    cursor = cnx.cursor()

    # First, get the EpisodeID and PodcastID from the Episodes table
    query = ("SELECT EpisodeID, PodcastID FROM Episodes "
             "WHERE EpisodeURL = %s AND EpisodeTitle = %s")
    cursor.execute(query, (url, title))
    result = cursor.fetchone()

    if result is None:
        # Episode not found
        return False

    episode_id, podcast_id = result


    # Next, using the PodcastID, get the PodcastName from the Podcasts table
    query = ("SELECT PodcastName FROM Podcasts WHERE PodcastID = %s")
    cursor.execute(query, (podcast_id,))
    podcast_name = cursor.fetchone()[0]

    # Create a directory named after the podcast, inside the main downloads directory
    download_dir = os.path.join("/opt/pypods/downloads", podcast_name)
    os.makedirs(download_dir, exist_ok=True)

    # Generate the episode filename based on episode ID and user ID
    filename = f"{user_id}-{episode_id}.mp3"
    file_path = os.path.join(download_dir, filename)

    response = requests.get(url, stream=True)
    response.raise_for_status()

    # Get the current date and time for DownloadedDate
    downloaded_date = datetime.datetime.now()

    # Get the file size from the Content-Length header
    file_size = int(response.headers.get("Content-Length", 0))

    # Write the file to disk
    with open(file_path, "wb") as f:
        for chunk in response.iter_content(chunk_size=1024):
            f.write(chunk)

    # Insert a new row into the DownloadedEpisodes table
    query = ("INSERT INTO DownloadedEpisodes "
             "(UserID, EpisodeID, DownloadedDate, DownloadedSize, DownloadedLocation) "
             "VALUES (%s, %s, %s, %s, %s)")
    cursor.execute(query, (user_id, episode_id, downloaded_date, file_size, file_path))

    # Update UserStats table to increment EpisodesDownloaded count
    query = ("UPDATE UserStats SET EpisodesDownloaded = EpisodesDownloaded + 1 "
             "WHERE UserID = %s")
    cursor.execute(query, (user_id,))

    cnx.commit()

    if cursor:
        cursor.close()
        # cnx.close()

    return True

def check_downloaded(cnx, user_id, title, url):
    cursor = None
    try:
        cursor = cnx.cursor()

        # Get the EpisodeID from the Episodes table
        query = "SELECT EpisodeID FROM Episodes WHERE EpisodeTitle = %s AND EpisodeURL = %s"
        cursor.execute(query, (title, url))
        result = cursor.fetchone()

        if result is None:   # add this check
            return False

        episode_id = result[0]

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


def download_episode_list(cnx, user_id):
    cursor = cnx.cursor(dictionary=True)

    query = """
    SELECT 
        Podcasts.PodcastID, 
        Podcasts.PodcastName, 
        Podcasts.ArtworkURL, 
        Episodes.EpisodeID, 
        Episodes.EpisodeTitle, 
        Episodes.EpisodePubDate, 
        Episodes.EpisodeDescription, 
        Episodes.EpisodeArtwork, 
        Episodes.EpisodeURL, 
        Episodes.EpisodeDuration, 
        Podcasts.WebsiteURL, 
        DownloadedEpisodes.DownloadedLocation,
        UserEpisodeHistory.ListenDuration
    FROM DownloadedEpisodes 
    INNER JOIN Episodes ON DownloadedEpisodes.EpisodeID = Episodes.EpisodeID 
    INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID 
    LEFT JOIN UserEpisodeHistory ON DownloadedEpisodes.EpisodeID = UserEpisodeHistory.EpisodeID AND DownloadedEpisodes.UserID = UserEpisodeHistory.UserID
    WHERE DownloadedEpisodes.UserID = %s 
    ORDER BY DownloadedEpisodes.DownloadedDate DESC
    """
    cursor.execute(query, (user_id,))
    rows = cursor.fetchall()

    cursor.close()
    # cnx.close()

    if not rows:
        return None

    return rows


def save_email_settings(cnx, email_settings):
    cursor = cnx.cursor()
    
    query = ("UPDATE EmailSettings SET Server_Name = %s, Server_Port = %s, From_Email = %s, Send_Mode = %s, Encryption = %s, Auth_Required = %s, Username = %s, Password = %s WHERE EmailSettingsID = 1")
    
    cursor.execute(query, (email_settings['server_name'], email_settings['server_port'], email_settings['from_email'], email_settings['send_mode'], email_settings['encryption'], int(email_settings['auth_required']), email_settings['email_username'], email_settings['email_password']))
    
    cnx.commit()
    cursor.close()
    # cnx.close()


def get_encryption_key(cnx):
    cursor = cnx.cursor()
    query = ("SELECT EncryptionKey FROM AppSettings WHERE AppSettingsID = 1")
    cursor.execute(query)
    result = cursor.fetchone()

    if not result:
        cursor.close()
        # cnx.close()
        return None

    # Convert the result to a dictionary.
    result_dict = dict(zip([column[0] for column in cursor.description], result))

    cursor.close()
    # cnx.close()

    # Convert the bytearray to a base64 encoded string before returning.
    return base64.b64encode(result_dict['EncryptionKey']).decode()

def get_email_settings(cnx):
    cursor = cnx.cursor()
    
    query = "SELECT * FROM EmailSettings"
    cursor.execute(query)
    
    result = cursor.fetchone()
    cursor.close()
    # cnx.close()
    
    if result:
        keys = ["EmailSettingsID", "Server_Name", "Server_Port", "From_Email", "Send_Mode", "Encryption", "Auth_Required", "Username", "Password"]
        return dict(zip(keys, result))
    else:
        return None


def delete_selected_episodes(cnx, user_id, selected_episodes):
    pass
def delete_selected_podcasts(cnx, user_id, selected_episodes):
    pass

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
    # cnx.close()

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
        # cnx.close()

        return True
    else:
        # Episode not found in the database
        cursor.close()
        # cnx.close()
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
        # cnx.close()

        return True
    else:
        # Episode not found in the database
        cursor.close()
        # cnx.close()
        return False

def check_usernames(cnx, username):
    cursor = cnx.cursor()
    query = ("SELECT COUNT(*) FROM Users WHERE Username = %s")
    cursor.execute(query, (username,))
    count = cursor.fetchone()[0]
    cursor.close()
    # cnx.close()
    return count > 0

def record_listen_duration(cnx, url, title, user_id, listen_duration):
    listen_date = datetime.datetime.now()
    cursor = cnx.cursor()

    # Get EpisodeID from Episodes table by joining with Podcasts table
    query = """SELECT e.EpisodeID
               FROM Episodes e
               JOIN Podcasts p ON e.PodcastID = p.PodcastID
               WHERE e.EpisodeURL = %s AND e.EpisodeTitle = %s AND p.UserID = %s"""
    cursor.execute(query, (url, title, user_id))
    result = cursor.fetchone()
    if result is None:
        # Episode not found in database, handle this case
        cursor.close()
        # cnx.close()
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
    # cnx.close()

def check_episode_playback(cnx, user_id, episode_title, episode_url):
    cursor = None
    try:
        cursor = cnx.cursor()

        # Get the EpisodeID from the Episodes table
        query = """SELECT e.EpisodeID 
                   FROM Episodes e
                   JOIN Podcasts p ON e.PodcastID = p.PodcastID
                   WHERE e.EpisodeTitle = %s AND e.EpisodeURL = %s AND p.UserID = %s"""
        cursor.execute(query, (episode_title, episode_url, user_id))
        result = cursor.fetchone()

        # Check if the EpisodeID is None
        if result is None:
            return False, 0

        episode_id = result[0]

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
        if cnx:
            print('cnx open')
            # cnx.close()

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
            # cnx.close()

def get_theme(cnx, user_id):
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
            # cnx.close()

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
            # cnx.close()

def get_user_info(cnx):
    cursor = cnx.cursor(dictionary=True)

    query = (f"SELECT Users.UserID, Users.Fullname, Users.Username, "
             f"Users.Email, Users.IsAdmin "
             f"FROM Users ")


    cursor.execute(query)
    rows = cursor.fetchall()

    cursor.close()
    # cnx.close()

    if not rows:
        return None

    return rows

def get_api_info(cnx):
    cursor = cnx.cursor(dictionary=True)

    query = (f"SELECT APIKeys.APIKeyID, APIKeys.UserID, Users.Username, "
             f"RIGHT(APIKeys.APIKey, 4) as LastFourDigits, "
             f"APIKeys.Created "
             f"FROM APIKeys "
             f"JOIN Users ON APIKeys.UserID = Users.UserID ")

    cursor.execute(query)
    rows = cursor.fetchall()

    cursor.close()
    # cnx.close()

    if not rows:
        return []

    return rows

def create_api_key(cnx, user_id):
    import secrets
    import string
    alphabet = string.ascii_letters + string.digits
    api_key = ''.join(secrets.choice(alphabet) for _ in range(64))

    cursor = cnx.cursor()
    query = "INSERT INTO APIKeys (UserID, APIKey) VALUES (%s, %s)"
    cursor.execute(query, (user_id, api_key))
    cnx.commit()
    cursor.close()
    # cnx.close()

    return api_key

def delete_api(cnx, api_id):
    cursor = cnx.cursor()
    query = "DELETE FROM APIKeys WHERE APIKeyID = %s"
    cursor.execute(query, (api_id,))
    cnx.commit()
    cursor.close()
    # cnx.close()

def set_username(cnx, user_id, new_username):
    cursor = cnx.cursor()
    query = "UPDATE Users SET Username = %s WHERE UserID = %s"
    cursor.execute(query, (new_username, user_id))
    cnx.commit()
    cursor.close()
    # cnx.close()

def set_password(cnx, user_id, salt, hash_pw):
    cursor = cnx.cursor()
    update_query = "UPDATE Users SET Salt=%s, Hashed_PW=%s WHERE UserID=%s"
    cursor.execute(update_query, (salt, hash_pw, user_id))
    cnx.commit()
    cursor.close()
    # cnx.close()


def set_email(cnx, user_id, new_email):
    cursor = cnx.cursor()
    query = "UPDATE Users SET Email = %s WHERE UserID = %s"
    cursor.execute(query, (new_email, user_id))
    cnx.commit()
    cursor.close()
    # cnx.close()

def set_fullname(cnx, user_id, new_name):
    cursor = cnx.cursor()
    query = "UPDATE Users SET Fullname = %s WHERE UserID = %s"
    cursor.execute(query, (new_name, user_id))
    cnx.commit()
    cursor.close()
    # cnx.close()

def set_isadmin(cnx, user_id, isadmin):
    cursor = cnx.cursor()
    
    # Convert boolean isadmin value to integer (0 or 1)
    isadmin_int = int(isadmin)
    
    query = f"UPDATE Users SET IsAdmin = {isadmin_int} WHERE UserID = {user_id}"
    
    cursor.execute(query)
    cnx.commit()
    
    cursor.close()
    # cnx.close()


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
    # cnx.close()



def user_admin_check(cnx, user_id):
    cursor = cnx.cursor()
    query = f"SELECT IsAdmin FROM Users WHERE UserID = '{user_id}'"
    cursor.execute(query)
    result = cursor.fetchone()
    cursor.close()
    # cnx.close()

    
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
    # cnx.close()

    return False

def download_status(cnx):
    cursor = cnx.cursor()
    query = "SELECT DownloadEnabled FROM AppSettings"
    cursor.execute(query)
    result = cursor.fetchone()
    cursor.close()
    # cnx.close()

    if result and result[0] == 1:
        return True
    else:
        return False

def guest_status(cnx):
    cursor = cnx.cursor()
    query = "SELECT Email FROM Users WHERE Email = 'active'"
    cursor.execute(query)
    result = cursor.fetchone()
    cursor.close()
    # cnx.close()

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
    # cnx.close()

def enable_disable_downloads(cnx):
    cursor = cnx.cursor()
    query = "UPDATE AppSettings SET DownloadEnabled = CASE WHEN DownloadEnabled = 1 THEN 0 ELSE 1 END"
    cursor.execute(query)
    cnx.commit()
    cursor.close()
    # cnx.close()

def self_service_status(cnx):
    cursor = cnx.cursor()
    query = "SELECT SelfServiceUser FROM AppSettings WHERE SelfServiceUser = 1"
    cursor.execute(query)
    result = cursor.fetchone()
    cursor.close()
    # cnx.close()

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
    # cnx.close()


def get_stats(cnx, user_id):
    cursor = cnx.cursor()
    
    query = ("SELECT UserCreated, PodcastsPlayed, TimeListened, PodcastsAdded, EpisodesSaved, EpisodesDownloaded "
             "FROM UserStats "
             "WHERE UserID = %s")
    
    cursor.execute(query, (user_id,))
    
    results = cursor.fetchall()
    result = results[0] if results else None

    if result:
        stats = {
            "UserCreated": result[0],
            "PodcastsPlayed": result[1],
            "TimeListened": result[2],
            "PodcastsAdded": result[3],
            "EpisodesSaved": result[4],
            "EpisodesDownloaded": result[5]
        }
    else:
        stats = None
    
    cursor.close()
    # cnx.close()
    
    return stats


def saved_episode_list(cnx, user_id):
    cursor = cnx.cursor(dictionary=True)

    query = (f"SELECT Podcasts.PodcastName, Episodes.EpisodeTitle, Episodes.EpisodePubDate, "
             f"Episodes.EpisodeDescription, Episodes.EpisodeArtwork, Episodes.EpisodeURL, "
             f"Episodes.EpisodeDuration, Podcasts.WebsiteURL, UserEpisodeHistory.ListenDuration "
             f"FROM SavedEpisodes "
             f"INNER JOIN Episodes ON SavedEpisodes.EpisodeID = Episodes.EpisodeID "
             f"INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
             f"LEFT JOIN UserEpisodeHistory ON SavedEpisodes.EpisodeID = UserEpisodeHistory.EpisodeID AND UserEpisodeHistory.UserID = %s "
             f"WHERE SavedEpisodes.UserID = %s "
             f"ORDER BY SavedEpisodes.SaveDate DESC")

    cursor.execute(query, (user_id, user_id))
    rows = cursor.fetchall()

    cursor.close()
    # cnx.close()

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
    cursor.close()
    # cnx.close()

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
            # cnx.close()



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
    # cnx.close()


def increment_played(cnx, user_id):
    cursor = cnx.cursor()

    # Update UserStats table to increment PodcastsPlayed count
    query = ("UPDATE UserStats SET PodcastsPlayed = PodcastsPlayed + 1 "
             "WHERE UserID = %s")
    cursor.execute(query, (user_id,))
    cnx.commit()
    
    cursor.close()
    # cnx.close()


def increment_listen_time(cnx, user_id):
    cursor = cnx.cursor()

    # Update UserStats table to increment PodcastsPlayed count
    query = ("UPDATE UserStats SET TimeListened = TimeListened + 1 "
             "WHERE UserID = %s")
    cursor.execute(query, (user_id,))
    cnx.commit()
    
    cursor.close()
    # cnx.close()


def get_user_episode_count(cnx, user_id):
    cursor = cnx.cursor()
    
    query = ("SELECT COUNT(*) "
             "FROM Episodes "
             "INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
             "WHERE Podcasts.UserID = %s")
    
    cursor.execute(query, (user_id,))
    
    episode_count = cursor.fetchone()[0]
    
    cursor.close()
    # cnx.close()

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
    # cnx.close()


def get_session_file_path():
    app_name = 'pinepods'
    data_dir = appdirs.user_data_dir(app_name)
    os.makedirs(data_dir, exist_ok=True)
    session_file_path = os.path.join(data_dir, "session.txt")
    return session_file_path

def save_session_to_file(session_id):
    session_file_path = get_session_file_path()
    with open(session_file_path, "w") as file:
        file.write(session_id)

def get_saved_session_from_file():
    app_name = 'pinepods'
    session_file_path = get_session_file_path()
    try:
        with open(session_file_path, "r") as file:
            session_id = file.read()
            return session_id
    except FileNotFoundError:
        return None

def check_saved_session(cnx, session_value):
    cursor = cnx.cursor()

    # Get the session with the matching value and expiration time
    cursor.execute("""
    SELECT UserID, expire FROM Sessions WHERE value = %s;
    """, (session_value,))

    result = cursor.fetchone()

    if result:
        user_id, session_expire = result
        current_time = datetime.datetime.now()
        if current_time < session_expire:
            return user_id

    return False
    cursor.close()
    # cnx.close()


def check_saved_web_session(cnx, session_value):
    cursor = cnx.cursor()

    # Get the session with the matching value and expiration time
    cursor.execute("""
    SELECT UserID, expire FROM Sessions WHERE value = %s;
    """, (session_value,))

    result = cursor.fetchone()

    if result:
        user_id, session_expire = result
        current_time = datetime.datetime.now()
        if current_time < session_expire:
            return user_id

    return False
    cursor.close()
    # cnx.close()


def create_session(cnx, user_id, session_value):
    # Calculate the expiration date 30 days in the future
    expire_date = datetime.datetime.now() + datetime.timedelta(days=30)

    # Insert the new session into the Sessions table
    cursor = cnx.cursor()
    cursor.execute("""
    INSERT INTO Sessions (UserID, value, expire) VALUES (%s, %s, %s);
    """, (user_id, session_value, expire_date))

    cnx.commit()
    cursor.close()
    # cnx.close()


def create_web_session(cnx, user_id, session_value):
    # Calculate the expiration date 30 days in the future
    expire_date = datetime.datetime.now() + datetime.timedelta(days=30)

    # Insert the new session into the Sessions table
    cursor = cnx.cursor()
    cursor.execute("""
    INSERT INTO Sessions (UserID, value, expire) VALUES (%s, %s, %s);
    """, (user_id, session_value, expire_date))

    cnx.commit()
    cursor.close()
    # cnx.close()

def clean_expired_sessions(cnx):
    current_time = datetime.datetime.now()
    cursor = cnx.cursor()

    cursor.execute("""
    DELETE FROM Sessions WHERE expire < %s;
    """, (current_time,))

    cnx.commit()
    cursor.close()
    # cnx.close()


def user_exists(cnx, username):
    cursor = cnx.cursor()
    query = "SELECT COUNT(*) FROM Users WHERE Username = %s"
    cursor.execute(query, (username,))
    count = cursor.fetchone()[0]
    cursor.close()
    # cnx.close()
    return count > 0

def reset_password_create_code(cnx, user_email, reset_code):
    cursor = cnx.cursor()
    
    # Check if a user with this email exists
    check_query = """
        SELECT UserID
        FROM Users
        WHERE Email = %s
    """
    cursor.execute(check_query, (user_email,))
    result = cursor.fetchone()
    if result is None:
        cursor.close()
        # cnx.close()
        return False
    
    # If the user exists, update the reset code and expiry
    reset_expiry = datetime.datetime.now() + datetime.timedelta(hours=1)

    update_query = """
        UPDATE Users
        SET Reset_Code = %s,
            Reset_Expiry = %s
        WHERE Email = %s
    """
    params = (reset_code, reset_expiry.strftime('%Y-%m-%d %H:%M:%S'), user_email)
    try:
        cursor.execute(update_query, params)
        cnx.commit()
    except Exception as e:
        print(f"Error when trying to update reset code: {e}")
        cursor.close()
        # cnx.close()
        return False

    cursor.close()
    # cnx.close()
    
    return True

def verify_reset_code(cnx, user_email, reset_code):
    cursor = cnx.cursor()

    select_query = """
        SELECT Reset_Code, Reset_Expiry
        FROM Users
        WHERE Email = %s
    """
    cursor.execute(select_query, (user_email,))
    result = cursor.fetchone()
    
    cursor.close()
    # cnx.close()

    # Check if a user with this email exists
    if result is None:
        return None
    
    # Check if the reset code is valid and not expired
    stored_code, expiry = result
    if stored_code == reset_code and datetime.datetime.now() < expiry:
        return True
    
    return False

def reset_password_prompt(cnx, user_email, salt, hashed_pw):
    cursor = cnx.cursor()

    update_query = """
        UPDATE Users
        SET Hashed_PW = %s,
            Salt = %s,
            Reset_Code = NULL,
            Reset_Expiry = NULL
        WHERE Email = %s
    """
    params = (hashed_pw, salt, user_email)
    cursor.execute(update_query, params)

    if cursor.rowcount == 0:
        return None

    cnx.commit()
    cursor.close()
    # cnx.close()

    return "Password Reset Successfully"

def clear_guest_data(cnx):
    cursor = cnx.cursor()

    # First delete all the episodes associated with the guest user
    delete_episodes_query = """
        DELETE Episodes
        FROM Episodes
        INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID
        WHERE Podcasts.UserID = 1
    """
    cursor.execute(delete_episodes_query)

    # Then delete all the podcasts associated with the guest user
    delete_podcasts_query = """
        DELETE FROM Podcasts 
        WHERE UserID = 1
    """
    cursor.execute(delete_podcasts_query)

    # Commit the transaction
    cnx.commit()
    cursor.close()

    return "Guest user data cleared successfully"

def get_episode_metadata(cnx, url, title, user_id):
    cursor = cnx.cursor(dictionary=True)
    print(url, title, user_id)

    query = ("SELECT EpisodeID FROM Episodes "
             "WHERE EpisodeURL = %s AND EpisodeTitle = %s")
    cursor.execute(query, (url, title))
    episode_id = cursor.fetchone()

    if episode_id is None:
        # Episode not found
        return False

    print(episode_id)
    episode_id = episode_id['EpisodeID']

    query = (f"SELECT Podcasts.PodcastID, Podcasts.PodcastName, Podcasts.ArtworkURL, Episodes.EpisodeTitle, Episodes.EpisodePubDate, "
             f"Episodes.EpisodeDescription, Episodes.EpisodeArtwork, Episodes.EpisodeURL, Episodes.EpisodeDuration, Episodes.EpisodeID, "
             f"Podcasts.WebsiteURL "
             f"FROM Episodes "
             f"INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
             f"WHERE Episodes.EpisodeID = %s AND Podcasts.UserID = %s")

    cursor.execute(query, (episode_id, user_id,))
    row = cursor.fetchone()

    cursor.close()

    if not row:
        raise ValueError(f"No episode found with ID {episode_id} for user {user_id}")
        
    return row

def save_mfa_secret(cnx, user_id, mfa_secret):
    cursor = cnx.cursor(dictionary=True)

    query = (f"UPDATE Users "
             f"SET MFA_Secret = %s "
             f"WHERE UserID = %s")
    
    try:
        cursor.execute(query, (mfa_secret, user_id))
        cnx.commit()
        cursor.close()
        return True
    except Exception as e:
        print("Error saving MFA secret:", e)
        return False


def check_mfa_enabled(cnx, user_id):
    cursor = cnx.cursor(dictionary=True)

    query = (f"SELECT MFA_Secret FROM Users WHERE UserID = %s")

    try:
        cursor.execute(query, (user_id,))
        result = cursor.fetchone()
        cursor.close()

        # Check if MFA_Secret is NULL
        if result['MFA_Secret']:
            return True  # MFA is enabled
        else:
            return False  # MFA is disabled
    except Exception as e:
        print("Error checking MFA status:", e)
        return False

def get_mfa_secret(cnx, user_id):
    cursor = cnx.cursor(dictionary=True)

    query = (f"SELECT MFA_Secret FROM Users WHERE UserID = %s")

    try:
        cursor.execute(query, (user_id,))
        result = cursor.fetchone()
        cursor.close()

        return result['MFA_Secret']
    except Exception as e:
        print("Error retrieving MFA secret:", e)
        return None

def delete_mfa_secret(cnx, user_id):
    cursor = cnx.cursor(dictionary=True)

    query = (f"UPDATE Users SET MFA_Secret = NULL WHERE UserID = %s")

    try:
        cursor.execute(query, (user_id,))
        cnx.commit()
        cursor.close()

        return True
    except Exception as e:
        print("Error deleting MFA secret:", e)
        return False

def get_all_episodes(cnx, pod_feed):
    cursor = cnx.cursor(dictionary=True)

    query = (f"SELECT * FROM Episodes INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID WHERE Podcasts.FeedURL = %s")

    try:
        cursor.execute(query, (pod_feed,))
        result = cursor.fetchall()
        cursor.close()

        return result
    except Exception as e:
        print("Error retrieving Podcast Episodes:", e)
        return None

def remove_episode_history(cnx, url, title, user_id):
    cursor = cnx.cursor(dictionary=True)

    query = (f"""DELETE FROM UserEpisodeHistory 
                WHERE UserID = %s AND EpisodeID IN (
                    SELECT EpisodeID FROM Episodes 
                    WHERE EpisodeURL = %s AND EpisodeTitle = %s
                )""")

    try:
        cursor.execute(query, (user_id, url, title))
        cnx.commit()
        cursor.close()

        return True
    except Exception as e:
        print("Error removing episode from history:", e)
        return False

def setup_timezone_info(cnx, user_id, timezone, hour_pref):
    cursor = cnx.cursor(dictionary=True)

    query = f"""UPDATE Users SET Timezone = %s, TimeFormat = %s, FirstLogin = %s WHERE UserID = %s"""

    try:
        cursor.execute(query, (timezone, hour_pref, 1, user_id))
        cnx.commit()
        cursor.close()

        return True
    except Exception as e:
        print("Error setting up time info:", e)
        return False

def get_time_info(cnx, user_id):
    cursor = cnx.cursor(dictionary=True)
    query = (f"""SELECT Timezone, TimeFormat FROM Users WHERE UserID = %s""")

    cursor.execute(query, (user_id,))
    result = cursor.fetchone()
    cursor.close()

    if result:
        return result['Timezone'], result['TimeFormat']
    else:
        return None

def first_login_done(cnx, user_id):
    cursor = cnx.cursor(dictionary=True)

    # Query to fetch the FirstLogin status
    query = "SELECT FirstLogin FROM Users WHERE UserID = %s"

    try:
        # Execute the query
        cursor.execute(query, (user_id,))

        # Fetch the result
        result = cursor.fetchone()
        cursor.close()

        # Check if the FirstLogin value is 1
        if result['FirstLogin'] == 1:
            return True
        else:
            return False

    except Exception as e:
        print("Error fetching first login status:", e)
        return False


def delete_selected_episodes(cnx, selected_episodes, user_id):
    cursor = cnx.cursor()
    for episode_id in selected_episodes:
        # Get the download ID and location from the DownloadedEpisodes table
        query = ("SELECT DownloadID, DownloadedLocation "
                 "FROM DownloadedEpisodes "
                 "WHERE EpisodeID = %s AND UserID = %s")
        cursor.execute(query, (episode_id, user_id))
        result = cursor.fetchone()

        if not result:
            print(f"No matching download found for episode ID {episode_id}")
            continue

        download_id, downloaded_location = result

        # Delete the downloaded file
        os.remove(downloaded_location)

        # Remove the entry from the DownloadedEpisodes table
        query = "DELETE FROM DownloadedEpisodes WHERE DownloadID = %s"
        cursor.execute(query, (download_id,))
        cnx.commit()
        print(f"Removed {cursor.rowcount} entry from the DownloadedEpisodes table.")

        # Update UserStats table to decrement EpisodesDownloaded count
        query = ("UPDATE UserStats SET EpisodesDownloaded = EpisodesDownloaded - 1 "
                 "WHERE UserID = %s")
        cursor.execute(query, (user_id,))

    cursor.close()

    return "success"

def delete_selected_podcasts(cnx, delete_list, user_id):
    cursor = cnx.cursor()
    for podcast_id in delete_list:
        # Get the download IDs and locations from the DownloadedEpisodes table
        query = ("SELECT DownloadedEpisodes.DownloadID, DownloadedEpisodes.DownloadedLocation "
                 "FROM DownloadedEpisodes "
                 "INNER JOIN Episodes ON DownloadedEpisodes.EpisodeID = Episodes.EpisodeID "
                 "WHERE Episodes.PodcastID = %s AND DownloadedEpisodes.UserID = %s")
        cursor.execute(query, (podcast_id, user_id))

        results = cursor.fetchall()

        if not results:
            print(f"No matching downloads found for podcast ID {podcast_id}")
            continue

        for result in results:
            download_id, downloaded_location = result

            # Delete the downloaded file
            os.remove(downloaded_location)

            # Remove the entry from the DownloadedEpisodes table
            query = "DELETE FROM DownloadedEpisodes WHERE DownloadID = %s"
            cursor.execute(query, (download_id,))
            cnx.commit()
            print(f"Removed {cursor.rowcount} entry from the DownloadedEpisodes table.")

            # Update UserStats table to decrement EpisodesDownloaded count
            query = ("UPDATE UserStats SET EpisodesDownloaded = EpisodesDownloaded - 1 "
                     "WHERE UserID = %s")
            cursor.execute(query, (user_id,))

    cursor.close()
    return "success"

import time

def search_data(cnx, search_term, user_id):
    cursor = cnx.cursor(dictionary=True)

    query = """
    SELECT * FROM Podcasts 
    INNER JOIN Episodes ON Podcasts.PodcastID = Episodes.PodcastID 
    WHERE Podcasts.UserID = %s AND 
    Episodes.EpisodeTitle LIKE %s
    """
    search_term = '%' + search_term + '%'

    try:
        start = time.time()
        cursor.execute(query, (user_id, search_term))
        result = cursor.fetchall()
        end = time.time()
        print(f"Query executed in {end - start} seconds.")
        cursor.close()

        return result
    except Exception as e:
        print("Error retrieving Podcast Episodes:", e)
        return None

def queue_pod(cnx, episode_title, ep_url, user_id):
    cursor = cnx.cursor(dictionary=True)

    # Fetch the EpisodeID using EpisodeTitle and EpisodeURL
    query_get_episode_id = """
    SELECT EpisodeID FROM Episodes 
    WHERE EpisodeTitle = %s AND EpisodeURL = %s
    """
    cursor.execute(query_get_episode_id, (episode_title, ep_url))
    result = cursor.fetchone()

    # If Episode not found, raise exception or handle it as per your requirement
    if not result:
        raise Exception("Episode not found")

    episode_id = result['EpisodeID']

    # Find the current maximum QueuePosition for the user
    query_get_max_pos = """
    SELECT MAX(QueuePosition) AS max_pos FROM EpisodeQueue
    WHERE UserID = %s
    """
    cursor.execute(query_get_max_pos, (user_id,))
    result = cursor.fetchone()
    max_pos = result['max_pos'] if result['max_pos'] else 0

    # Insert the new episode into the queue
    query_queue_pod = """
    INSERT INTO EpisodeQueue(UserID, EpisodeID, QueuePosition) 
    VALUES (%s, %s, %s)
    """
    new_pos = max_pos + 1  # New QueuePosition is one more than the current maximum
    try:
        start = time.time()
        cursor.execute(query_queue_pod, (user_id, episode_id, new_pos))
        cnx.commit()  # Don't forget to commit the changes
        end = time.time()
        print(f"Query executed in {end - start} seconds.")
    except Exception as e:
        print("Error queueing Podcast Episode:", e)
        return None

    return {"detail": "Podcast Episode queued successfully."}

def remove_queued_pod(cnx, episode_title, ep_url, user_id):
    cursor = cnx.cursor(dictionary=True)

    # First, retrieve the EpisodeID and QueuePosition of the episode to be removed
    get_queue_data_query = """
    SELECT EpisodeQueue.EpisodeID, EpisodeQueue.QueuePosition
    FROM EpisodeQueue 
    INNER JOIN Episodes ON EpisodeQueue.EpisodeID = Episodes.EpisodeID 
    WHERE Episodes.EpisodeTitle = %s AND Episodes.EpisodeURL = %s AND EpisodeQueue.UserID = %s
    """
    cursor.execute(get_queue_data_query, (episode_title, ep_url, user_id))
    queue_data = cursor.fetchone()
    if queue_data is None:
        print(f"No queued episode found for the title: {episode_title} and URL: {ep_url}")
        cursor.close()
        return None

    episode_id = queue_data['EpisodeID']
    removed_queue_position = queue_data['QueuePosition']

    # Then, delete the queued episode
    delete_query = """
    DELETE FROM EpisodeQueue
    WHERE UserID = %s AND EpisodeID = %s
    """
    cursor.execute(delete_query, (user_id, episode_id))
    cnx.commit()

    # After that, decrease the QueuePosition of all episodes that were after the removed one
    update_queue_query = """
    UPDATE EpisodeQueue 
    SET QueuePosition = QueuePosition - 1 
    WHERE UserID = %s AND QueuePosition > %s
    """
    cursor.execute(update_queue_query, (user_id, removed_queue_position))
    cnx.commit()

    print(f"Successfully removed episode: {episode_title} from queue.")
    cursor.close()

    return {"status": "success"}

def get_queued_episodes(cnx, user_id):
    cursor = cnx.cursor(dictionary=True)

    get_queued_episodes_query = """
    SELECT 
        Episodes.EpisodeTitle, 
        Podcasts.PodcastName, 
        Episodes.EpisodePubDate, 
        Episodes.EpisodeDescription, 
        Episodes.EpisodeArtwork, 
        Episodes.EpisodeURL, 
        EpisodeQueue.QueuePosition, 
        Episodes.EpisodeDuration, 
        EpisodeQueue.QueueDate,
        UserEpisodeHistory.ListenDuration
    FROM EpisodeQueue 
    INNER JOIN Episodes ON EpisodeQueue.EpisodeID = Episodes.EpisodeID 
    INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID 
    LEFT JOIN UserEpisodeHistory ON EpisodeQueue.EpisodeID = UserEpisodeHistory.EpisodeID AND EpisodeQueue.UserID = UserEpisodeHistory.UserID
    WHERE EpisodeQueue.UserID = %s 
    ORDER BY EpisodeQueue.QueuePosition ASC
    """
    cursor.execute(get_queued_episodes_query, (user_id,))
    queued_episodes = cursor.fetchall()

    cursor.close()

    return queued_episodes


# database_functions.py

def queue_bump(cnx, ep_url, title, user_id):
    cursor = cnx.cursor()

    # check if the episode is already in the queue
    cursor.execute(
        "SELECT QueueID, QueuePosition FROM EpisodeQueue "
        "INNER JOIN Episodes ON EpisodeQueue.EpisodeID = Episodes.EpisodeID "
        "WHERE Episodes.EpisodeURL = %s AND Episodes.EpisodeTitle = %s AND EpisodeQueue.UserID = %s",
        (ep_url, title, user_id)
    )
    result = cursor.fetchone()

    # if the episode is in the queue, remove it
    if result is not None:
        cursor.execute(
            "DELETE FROM EpisodeQueue WHERE QueueID = %s", (result['QueueID'],)
        )

    # decrease the QueuePosition of all other episodes in the queue
    cursor.execute(
        "UPDATE EpisodeQueue SET QueuePosition = QueuePosition - 1 WHERE UserID = %s", (user_id,)
    )

    # add the episode to the front of the queue
    queue_pod(cnx, title, ep_url, user_id)

    cnx.commit()
    cursor.close()

    return {"detail": f"{title} moved to the front of the queue."}


def backup_user(cnx, user_id):
    cursor = cnx.cursor(dictionary=True)  # We use dictionary=True to fetch results as dictionaries

    # Fetch podcasts for the user
    cursor.execute(
        "SELECT PodcastName, FeedURL FROM Podcasts WHERE UserID = %s", (user_id,)
    )
    podcasts = cursor.fetchall()
    cursor.close()

    # Construct the OPML content
    opml_content = '<?xml version="1.0" encoding="UTF-8"?>\n<opml version="2.0">\n  <head>\n    <title>Podcast Subscriptions</title>\n  </head>\n  <body>\n'

    for podcast in podcasts:
        opml_content += f'    <outline text="{podcast["PodcastName"]}" title="{podcast["PodcastName"]}" type="rss" xmlUrl="{podcast["FeedURL"]}" />\n'

    opml_content += '  </body>\n</opml>'

    return opml_content