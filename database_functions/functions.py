import random
import string
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
import subprocess
import psycopg2
from psycopg2.extras import RealDictCursor
from requests.exceptions import RequestException

# # Get the application root directory from the environment variable
# app_root = os.environ.get('APP_ROOT')
sys.path.append('/pinepods/')
# Import the functions directly from app_functions.py located in the database_functions directory
from database_functions.app_functions import sync_subscription_change, get_podcast_values

def get_web_key(cnx):
    cursor = cnx.cursor()
    query = "SELECT APIKey FROM APIKeys WHERE UserID = 1"
    cursor.execute(query)
    result = cursor.fetchone()
    cursor.close()

    if result:
        return result[0]
    else:
        return None


def add_podcast(cnx, podcast_values, user_id):
    cursor = cnx.cursor()
    print(f"Podcast values '{podcast_values}'")

    # check if the podcast already exists for the user
    query = ("SELECT PodcastID FROM Podcasts "
             "WHERE FeedURL = %s AND UserID = %s")

    cursor.execute(query, (podcast_values['pod_feed_url'], user_id))
    result = cursor.fetchone()

    if result is not None:
        # podcast already exists for the user, return False
        cursor.close()
        # cnx.close()
        return False

    # insert the podcast into the database
    add_podcast = ("INSERT INTO Podcasts "
                   "(PodcastName, ArtworkURL, Author, Categories, Description, EpisodeCount, FeedURL, WebsiteURL, Explicit, UserID) "
                   "VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s)")
    cursor.execute(add_podcast, (
        podcast_values['pod_title'], 
        podcast_values['pod_artwork'], 
        podcast_values['pod_author'], 
        str(podcast_values['categories']), 
        podcast_values['pod_description'], 
        podcast_values['pod_episode_count'], 
        podcast_values['pod_feed_url'], 
        podcast_values['pod_website'],
        podcast_values['pod_explicit'], 
        user_id
    ))

    # get the ID of the newly-inserted podcast
    podcast_id = cursor.lastrowid

    # Update UserStats table to increment PodcastsAdded count
    query = ("UPDATE UserStats SET PodcastsAdded = PodcastsAdded + 1 "
             "WHERE UserID = %s")
    cursor.execute(query, (user_id,))

    cnx.commit()

    # add episodes to database
    add_episodes(cnx, podcast_id, podcast_values['pod_feed_url'], podcast_values['pod_artwork'])

    cursor.close()
    # cnx.close()

    # return True to indicate success
    return True


def add_user(cnx, user_values):
    cursor = cnx.cursor()

    add_user = ("INSERT INTO Users "
                "(Fullname, Username, Email, Hashed_PW, IsAdmin) "
                "VALUES (%s, %s, %s, %s, 0)")

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
                "(Fullname, Username, Email, Hashed_PW, IsAdmin) "
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

    def parse_duration(duration_string: str) -> int:
        # First, check if duration is in seconds (no colons)
        if ':' not in duration_string:
            try:
                # Directly return seconds if no colon is found
                return int(duration_string)
            except ValueError:
                print(f'Error parsing duration from pure seconds: {duration_string}')
                return 0  # Return 0 or some default value in case of error
        else:
            # Handle HH:MM:SS format
            parts = duration_string.split(':')
            if len(parts) == 1:
                # If there's only one part, it's in seconds
                return int(parts[0])
            else:
                while len(parts) < 3:
                    parts.insert(0, '0')  # Prepend zeros if any parts are missing (ensuring HH:MM:SS format)
                h, m, s = map(int, parts)
                return h * 3600 + m * 60 + s



    for entry in episode_dump.entries:
        # Check necessary fields are present
        if not all(hasattr(entry, attr) for attr in ["title", "summary", "enclosures"]):
            continue

        # Extract necessary information
        parsed_title = entry.title
        parsed_description = entry.get('content', [{}])[0].get('value', entry.summary)
        parsed_audio_url = entry.enclosures[0].href if entry.enclosures else ""
        parsed_release_datetime = dateutil.parser.parse(entry.published).strftime("%Y-%m-%d %H:%M:%S")
        
        # Artwork prioritizing episode-specific artwork, then falling back to the feed's artwork if necessary
        parsed_artwork_url = (entry.get('itunes_image', {}).get('href') or 
                            getattr(entry, 'image', {}).get('href') or
                            artwork_url)

        # Duration parsing
        parsed_duration = 0
        duration_str = getattr(entry, 'itunes_duration', '')
        if ':' in duration_str:
            # If duration contains ":", then process as HH:MM:SS or MM:SS
            time_parts = list(map(int, duration_str.split(':')))
            while len(time_parts) < 3:
                time_parts.insert(0, 0)  # Pad missing values with zeros
            h, m, s = time_parts
            parsed_duration = h * 3600 + m * 60 + s
        elif duration_str.isdigit():
            # If duration is all digits (no ":"), treat as seconds directly
            parsed_duration = int(duration_str)
        elif hasattr(entry, 'itunes_duration_seconds'):
            # Additional format as fallback, if explicitly provided as seconds
            parsed_duration = int(entry.itunes_duration_seconds)
        elif hasattr(entry, 'duration'):
            # Other specified duration formats (assume they are in correct format or seconds)
            parsed_duration = parse_duration(entry.duration)
        elif hasattr(entry, 'length'):
            # If duration not specified but length is, use length (assuming it's in seconds)
            parsed_duration = int(entry.length)


        # Check for existing episode
        cursor.execute("SELECT * FROM Episodes WHERE PodcastID = %s AND EpisodeTitle = %s", (podcast_id, parsed_title))
        if cursor.fetchone():
            continue  # Episode already exists

        # Insert the new episode
        cursor.execute("""
            INSERT INTO Episodes 
            (PodcastID, EpisodeTitle, EpisodeDescription, EpisodeURL, EpisodeArtwork, EpisodePubDate, EpisodeDuration) 
            VALUES (%s, %s, %s, %s, %s, %s, %s)
            """, (podcast_id, parsed_title, parsed_description, parsed_audio_url, parsed_artwork_url, parsed_release_datetime, parsed_duration))

        if cursor.rowcount > 0:
            print(f"Added episode '{parsed_title}'")

    cnx.commit()


def remove_podcast(cnx, podcast_name, podcast_url, user_id):
    cursor = cnx.cursor()

    try:
        # Get the PodcastID for the given podcast name
        select_podcast_id = "SELECT PodcastID FROM Podcasts WHERE PodcastName = %s AND FeedURL = %s"
        cursor.execute(select_podcast_id, (podcast_name, podcast_url))
        result = cursor.fetchall()  # fetch all results
        podcast_id = result[0][0] if result else None

        # If there's no podcast ID found, raise an error or exit the function early
        if podcast_id is None:
            raise ValueError("No podcast found with name {}".format(podcast_name))

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
    except mysql.connector.Error as err:
        print("Error: {}".format(err))
        cnx.rollback()
    finally:
        cursor.close()
        # cnx.close()

def remove_podcast_id(cnx, podcast_id, user_id):
    cursor = cnx.cursor()

    try:
        podcast_id = podcast_id

        # If there's no podcast ID found, raise an error or exit the function early
        if podcast_id is None:
            raise ValueError("No podcast found with name {}".format(podcast_id))

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
        delete_podcast = "DELETE FROM Podcasts WHERE PodcastID = %s"
        cursor.execute(delete_podcast, (podcast_id,))

        # Update UserStats table to decrement PodcastsAdded count
        query = ("UPDATE UserStats SET PodcastsAdded = PodcastsAdded - 1 "
                 "WHERE UserID = %s")
        cursor.execute(query, (user_id,))

        cnx.commit()
    except mysql.connector.Error as err:
        print("Error: {}".format(err))
        cnx.rollback()
    finally:
        cursor.close()
        # cnx.close()


def remove_user(cnx, user_name):
    pass


def return_episodes(database_type, cnx, user_id):
    if database_type == "postgresql":
        cursor = cnx.cursor(cursor_factory=RealDictCursor)
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
        cursor = cnx.cursor(dictionary=True)

    query = (f"SELECT Podcasts.PodcastName, Episodes.EpisodeTitle, Episodes.EpisodePubDate, "
             f"Episodes.EpisodeDescription, Episodes.EpisodeArtwork, Episodes.EpisodeURL, Episodes.EpisodeDuration, "
             f"UserEpisodeHistory.ListenDuration, Episodes.EpisodeID "
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


def return_podcast_episodes(database_type, cnx, user_id, podcast_id):
    if database_type == "postgresql":
        cursor = cnx.cursor(cursor_factory=RealDictCursor)
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
        cursor = cnx.cursor(dictionary=True)

    query = (
        f"SELECT Podcasts.PodcastID, Podcasts.PodcastName, Episodes.EpisodeID, Episodes.EpisodeTitle, Episodes.EpisodePubDate, "
        f"Episodes.EpisodeDescription, Episodes.EpisodeArtwork, Episodes.EpisodeURL, Episodes.EpisodeDuration, "
        f"UserEpisodeHistory.ListenDuration "
        f"FROM Episodes "
        f"INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
        f"LEFT JOIN UserEpisodeHistory ON Episodes.EpisodeID = UserEpisodeHistory.EpisodeID AND UserEpisodeHistory.UserID = %s "
        f"WHERE Podcasts.PodcastID = %s "
        f"AND Podcasts.UserID = %s "
        f"ORDER BY Episodes.EpisodePubDate DESC")

    cursor.execute(query, (user_id, podcast_id, user_id))
    rows = cursor.fetchall()

    cursor.close()

    if not rows:
        return None

    return rows


def get_podcast_id(database_type, cnx, user_id, podcast_feed):
    if database_type == "postgresql":
        cursor = cnx.cursor(cursor_factory=RealDictCursor)
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
        cursor = cnx.cursor(dictionary=True)

    # Adjusted query to select only the PodcastID based on FeedURL and UserID
    query = (f"SELECT PodcastID "
             f"FROM Podcasts "
             f"WHERE FeedURL = %s AND UserID = %s")

    cursor.execute(query, (podcast_feed, user_id))
    row = cursor.fetchone()  # Fetching only one row as we expect a single result

    cursor.close()

    if not row:
        return None

    return row['PodcastID']  # Assuming the column name is 'PodcastID'


def delete_episode(cnx, episode_id, user_id):
    cursor = cnx.cursor()

    # Get the download ID from the DownloadedEpisodes table
    query = ("SELECT DownloadID, DownloadedLocation "
             "FROM DownloadedEpisodes "
             "INNER JOIN Episodes ON DownloadedEpisodes.EpisodeID = Episodes.EpisodeID "
             "INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
             "WHERE Episodes.EpisodeID = %s AND Podcasts.UserID = %s")
    cursor.execute(query, (episode_id, user_id))
    result = cursor.fetchone()

    if not result:
        print("No matching download found.")
        return

    download_id, downloaded_location = result

    # Delete the downloaded file
    os.remove(downloaded_location)

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
    # cnx.close()


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


def return_pods(database_type, cnx, user_id):
    if database_type == "postgresql":
        cursor = cnx.cursor(cursor_factory=RealDictCursor)
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
        cursor = cnx.cursor(dictionary=True)

    query = (
        "SELECT PodcastID, PodcastName, ArtworkURL, Description, EpisodeCount, WebsiteURL, FeedURL, Author, Categories, Explicit "
        "FROM Podcasts "
        "WHERE UserID = %s")

    cursor.execute(query, (user_id,))
    rows = cursor.fetchall()

    cursor.close()
    # cnx.close()

    if not rows:
        return None

    return rows

def check_self_service(cnx):
    cursor = cnx.cursor()
    query = "SELECT SelfServiceUser FROM AppSettings"
    cursor.execute(query)
    result = cursor.fetchone()
    cursor.close()

    if result and result[0] == 1:
        return True
    elif result and result[0] == 0:
        return False
    else:
        return None

def refresh_pods(cnx):
    import concurrent.futures

    print('refresh begin')
    cursor = cnx.cursor()

    select_podcasts = "SELECT PodcastID, FeedURL, ArtworkURL FROM Podcasts"

    cursor.execute(select_podcasts)
    result_set = cursor.fetchall()  # fetch the result set

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
            artwork_url = entry.get('itunes_image', {}).get('href', None) or entry.get('image', {}).get('href',
                                                                                                        None) or artwork_url

            # insert the episode into the database
            add_episode = ("INSERT INTO Episodes "
                           "(PodcastID, EpisodeTitle, EpisodeDescription, EpisodeURL, EpisodeArtwork, EpisodePubDate, EpisodeDuration) "
                           "VALUES (%s, %s, %s, %s, %s, %s, %s)")
            episode_values = (podcast_id, title, description, audio_url, artwork_url, release_date, 0)
            cursor.execute(add_episode, episode_values)

    cnx.commit()

    cursor.close()
    # cnx.close()


def record_podcast_history(cnx, episode_id, user_id, episode_pos):
    from datetime import datetime
    cursor = cnx.cursor()

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
            'Hashed_PW': result[4]
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
            'Hashed_PW': result[4]
        }
    else:
        return None


def user_history(cnx, user_id):
    cursor = cnx.cursor()
    query = ("SELECT Episodes.EpisodeID, UserEpisodeHistory.ListenDate, UserEpisodeHistory.ListenDuration, "
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


def download_podcast(cnx, episode_id, user_id):
    cursor = cnx.cursor()

    # First, get the EpisodeID and PodcastID from the Episodes table
    query = ("SELECT PodcastID FROM Episodes "
             "WHERE EpisodeID = %s")
    cursor.execute(query, (episode_id,))
    result = cursor.fetchone()

    if result is None:
        # Episode not found
        return False

    podcast_id = result[0]

    # First, get the EpisodeID and PodcastID from the Episodes table
    query = ("SELECT EpisodeURL FROM Episodes "
             "WHERE EpisodeID = %s")
    cursor.execute(query, (episode_id,))
    result = cursor.fetchone()

    if result is None:
        # Episode not found
        return False

    episode_url = result[0]

    # Next, using the PodcastID, get the PodcastName from the Podcasts table
    query = ("SELECT PodcastName FROM Podcasts WHERE PodcastID = %s")
    cursor.execute(query, (podcast_id,))
    podcast_name = cursor.fetchone()[0]

    # Create a directory named after the podcast, inside the main downloads directory
    download_dir = os.path.join("/opt/pinepods/downloads", podcast_name)
    os.makedirs(download_dir, exist_ok=True)

    # Generate the episode filename based on episode ID and user ID
    filename = f"{user_id}-{episode_id}.mp3"
    file_path = os.path.join(download_dir, filename)

    response = requests.get(episode_url, stream=True)
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


def check_downloaded(cnx, user_id, episode_id):
    cursor = None
    try:
        cursor = cnx.cursor()

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

def get_download_location(cnx, episode_id, user_id):
    cursor = cnx.cursor()
    try:
        # Check if the episode has been downloaded by the user
        query = "SELECT DownloadedLocation FROM DownloadedEpisodes WHERE EpisodeID = %s AND UserID = %s"
        cursor.execute(query, (episode_id, user_id))
        result = cursor.fetchone()  # Assuming one entry per episode per user

        if result:
            return result[0]  # Returns the DownloadedLocation directly
        return None

    finally:
        cursor.close()



def download_episode_list(database_type, cnx, user_id):
    if database_type == "postgresql":
        cursor = cnx.cursor(cursor_factory=RealDictCursor)
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
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

    query = (
        "UPDATE EmailSettings SET Server_Name = %s, Server_Port = %s, From_Email = %s, Send_Mode = %s, Encryption = %s, Auth_Required = %s, Username = %s, Password = %s WHERE EmailSettingsID = 1")

    cursor.execute(query, (email_settings['server_name'], email_settings['server_port'], email_settings['from_email'],
                           email_settings['send_mode'], email_settings['encryption'],
                           int(email_settings['auth_required']), email_settings['email_username'],
                           email_settings['email_password']))

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
        keys = ["EmailSettingsID", "Server_Name", "Server_Port", "From_Email", "Send_Mode", "Encryption",
                "Auth_Required", "Username", "Password"]
        return dict(zip(keys, result))
    else:
        return None


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


def get_episode_id_by_url(cnx, episode_url):
    cursor = cnx.cursor()

    query = "SELECT EpisodeID FROM Episodes WHERE EpisodeURL = %s"
    params = (episode_url,)  # Ensure this is a tuple

    cursor.execute(query, params)
    result = cursor.fetchone()

    episode_id = None  # Initialize episode_id
    if result:
        episode_id = result[0]

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
        cursor.execute(query, (user_id, episode_id[0]))  # Extract the episode ID from the tuple
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


def record_listen_duration(cnx, episode_id, user_id, listen_duration):
    listen_date = datetime.datetime.now()
    cursor = cnx.cursor()

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

def get_local_episode_times(cnx, user_id):
    cursor = cnx.cursor()
    # Fetch all listen durations for the given user
    cursor.execute("SELECT EpisodeID, ListenDuration FROM UserEpisodeHistory WHERE UserID=%s", (user_id,))
    episode_times = [{"episode_id": row[0], "listen_duration": row[1]} for row in cursor.fetchall()]
    cursor.close()
    return episode_times



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


def get_user_info(database_type, cnx):
    if database_type == "postgresql":
        cursor = cnx.cursor(cursor_factory=RealDictCursor)
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
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


def get_api_info(database_type, cnx, user_id):
    # Check if the user is an admin
    is_admin_query = "SELECT IsAdmin FROM Users WHERE UserID = %s"
    cursor = cnx.cursor()
    cursor.execute(is_admin_query, (user_id,))
    is_admin_result = cursor.fetchone()
    cursor.close()

    # Adjusting access based on the result type
    if isinstance(is_admin_result, dict):  # Dictionary style
        is_admin = is_admin_result.get('IsAdmin', 0)
    elif isinstance(is_admin_result, tuple):  # Tuple style (fallback)
        # Assuming 'IsAdmin' is the first column in the SELECT statement
        is_admin = is_admin_result[0] if is_admin_result else 0
    else:
        is_admin = 0


    # Adjust the query based on whether the user is an admin
    if database_type == "postgresql":
        cursor = cnx.cursor(cursor_factory=RealDictCursor)
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
        cursor = cnx.cursor(dictionary=True)

    query = (f"SELECT APIKeys.APIKeyID, APIKeys.UserID, Users.Username, "
             f"RIGHT(APIKeys.APIKey, 4) as LastFourDigits, "
             f"APIKeys.Created "
             f"FROM APIKeys "
             f"JOIN Users ON APIKeys.UserID = Users.UserID ")

    # Append condition to query if the user is not an admin
    if not is_admin:
        query += f"WHERE APIKeys.UserID = {user_id} "

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


def set_password(cnx, user_id, hash_pw):
    cursor = cnx.cursor()
    update_query = "UPDATE Users SET Hashed_PW=%s WHERE UserID=%s"
    cursor.execute(update_query, (hash_pw, user_id))
    cnx.commit()
    cursor.close()



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


def verify_api_key(cnx, passed_key):
    cursor = cnx.cursor()
    query = "SELECT * FROM APIKeys WHERE APIKey = %s"
    cursor.execute(query, (passed_key,))
    result = cursor.fetchone()
    print(f"Result: {result}")
    cursor.close()
    return True if result else False


def get_api_key(cnx, username):
    try:
        with cnx.cursor() as cursor:
            # Get the UserID
            query = "SELECT UserID FROM Users WHERE username = %s"
            cursor.execute(query, (username,))
            result = cursor.fetchone()

            # Check if a result is returned. If not, return None
            if result is None:
                print("No user found with the provided username.")
                return None

            user_id = result[0]

            # Get the API Key using the fetched UserID, and limit the results to 1
            query = "SELECT APIKey FROM APIKeys WHERE UserID = %s LIMIT 1"
            cursor.execute(query, (user_id,))
            result = cursor.fetchone()

            # Check and return the API key or create a new one if not found
            if result:
                print(f"Result: {result}")
                return result[0]  # Adjust the index if the API key is in a different column
            else:
                print("No API key found for the provided user. Creating a new one...")
                return create_api_key(cnx, user_id)

    except Exception as e:
        print(f"An error occurred: {str(e)}")
        return f"An error occurred: {str(e)}"


def get_api_user(cnx, api_key):
    try:
        with cnx.cursor() as cursor:
            # Get the API Key using the fetched UserID, and limit the results to 1
            query = "SELECT UserID FROM APIKeys WHERE APIKey = %s LIMIT 1"
            cursor.execute(query, (api_key,))
            result = cursor.fetchone()

            # Check and return the API key or create a new one if not found
            if result:
                print(f"Result: {result}")
                return result[0]  # Adjust the index if the API key is in a different column
            else:
                print(f"ApiKey Not Found")
                return "ApiKey Not Found"

    except Exception as e:
        print(f"An error occurred: {str(e)}")
        return f"An error occurred: {str(e)}"


def id_from_api_key(cnx, passed_key):
    cursor = cnx.cursor()
    query = "SELECT UserID FROM APIKeys WHERE APIKey = %s"
    cursor.execute(query, (passed_key,))
    result = cursor.fetchone()
    print(f"Result: {result}")
    cursor.close()
    return result[0] if result else None


def check_api_permission(cnx, passed_key):
    import tempfile
    # Create a temporary file to store the content. This is because the mysql command reads from a file.
    with tempfile.NamedTemporaryFile(mode='w+', delete=True) as tempf:
        tempf.write(server_restore_data)
        tempf.flush()
        cmd = [
            "mysql",
            "-h", 'db',
            "-P", '3306',
            "-u", "root",
            "-p" + database_pass,
            "pypods_database"
        ]

        # Use the file's content as input for the mysql command
        with open(tempf.name, 'r') as file:
            process = subprocess.Popen(cmd, stdin=file, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            stdout, stderr = process.communicate()

            if process.returncode != 0:
                raise Exception(f"Restoration failed with error: {stderr.decode()}")

    return "Restoration completed successfully!"


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


def saved_episode_list(database_type, cnx, user_id):
    if database_type == "postgresql":
        cursor = cnx.cursor(cursor_factory=RealDictCursor)
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
        cursor = cnx.cursor(dictionary=True)

    query = (f"SELECT Podcasts.PodcastName, Episodes.EpisodeTitle, Episodes.EpisodePubDate, "
             f"Episodes.EpisodeDescription, Episodes.EpisodeID, Episodes.EpisodeArtwork, Episodes.EpisodeURL, "
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


def save_episode(cnx, episode_id, user_id):
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


def check_saved(cnx, user_id, episode_id):
    cursor = None
    try:
        cursor = cnx.cursor()

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


def remove_saved_episode(cnx, episode_id, user_id):
    cursor = cnx.cursor()

    # Get the Save ID from the SavedEpisodes table
    query = ("SELECT SaveID "
             "FROM SavedEpisodes "
             "INNER JOIN Episodes ON SavedEpisodes.EpisodeID = Episodes.EpisodeID "
             "INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
             "WHERE Episodes.EpisodeID = %s AND Podcasts.UserID = %s")
    cursor.execute(query, (episode_id, user_id))
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


def check_podcast(cnx, user_id, podcast_name, podcast_url):
    cursor = None
    try:
        cursor = cnx.cursor()

        query = "SELECT PodcastID FROM Podcasts WHERE UserID = %s AND PodcastName = %s AND FeedURL = %s"
        cursor.execute(query, (user_id, podcast_name, podcast_url))

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


def reset_password_create_code(cnx, user_email):
    reset_code = ''.join(random.choices(string.ascii_uppercase + string.digits, k=6))
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

    return reset_code

def reset_password_remove_code(cnx, email):
    cursor = cnx.cursor()
    query = "UPDATE Users SET Reset_Code = NULL, Reset_Expiry = NULL WHERE Email = %s"
    cursor.execute(query, (email,))
    cnx.commit()
    return cursor.rowcount > 0


def verify_password(cnx, username: str, password: str) -> bool:
    cursor = cnx.cursor()
    print('checking pw')
    cursor.execute("SELECT Hashed_PW FROM Users WHERE Username = %s", (username,))
    result = cursor.fetchone()
    cursor.close()

    if not result:
        return False  # User not found

    hashed_password = result[0]

    ph = PasswordHasher()
    try:
        # Attempt to verify the password
        ph.verify(hashed_password, password)
        # If verification does not raise an exception, password is correct
        # Optionally rehash the password if needed (argon2 can detect this)
        if ph.check_needs_rehash(hashed_password):
            new_hash = ph.hash(password)
            # Update database with new hash if necessary
            # You'll need to implement this part
            # update_hashed_password(cnx, username, new_hash)
        return True
    except VerifyMismatchError:
        # If verification fails, password is incorrect
        return False


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

def check_reset_user(cnx, username, email):
    cursor = cnx.cursor()
    query = "SELECT * FROM Users WHERE Username = %s AND Email = %s"
    cursor.execute(query, (username, email))
    result = cursor.fetchone()
    return result is not None


def reset_password_prompt(cnx, user_email, hashed_pw):
    cursor = cnx.cursor()

    update_query = """
        UPDATE Users
        SET Hashed_PW = %s,
            Reset_Code = NULL,
            Reset_Expiry = NULL
        WHERE Email = %s
    """
    params = (hashed_pw, user_email)
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


def get_episode_metadata(database_type, cnx, episode_id, user_id):
    if database_type == "postgresql":
        cursor = cnx.cursor(cursor_factory=RealDictCursor)
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
        cursor = cnx.cursor(dictionary=True)

    query = (
        f"SELECT Podcasts.PodcastID, Podcasts.PodcastName, Podcasts.ArtworkURL, Episodes.EpisodeTitle, Episodes.EpisodePubDate, "
        f"Episodes.EpisodeDescription, Episodes.EpisodeArtwork, Episodes.EpisodeURL, Episodes.EpisodeDuration, Episodes.EpisodeID, "
        f"Podcasts.WebsiteURL, UserEpisodeHistory.ListenDuration "
        f"FROM Episodes "
        f"INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
        f"LEFT JOIN UserEpisodeHistory ON Episodes.EpisodeID = UserEpisodeHistory.EpisodeID AND Podcasts.UserID = UserEpisodeHistory.UserID "
        f"WHERE Episodes.EpisodeID = %s AND Podcasts.UserID = %s")

    cursor.execute(query, (episode_id, user_id,))
    row = cursor.fetchone()

    cursor.close()

    if not row:
        raise ValueError(f"No episode found with ID {episode_id} for user {user_id}")

    return row

import logging

def save_mfa_secret(database_type, cnx, user_id, mfa_secret):
    if database_type == "postgresql":
        cursor = cnx.cursor(cursor_factory=RealDictCursor)
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
        cursor = cnx.cursor(dictionary=True)

    query = (f"UPDATE Users "
             f"SET MFA_Secret = %s "
             f"WHERE UserID = %s")

    try:
        cursor.execute(query, (mfa_secret, user_id))
        cnx.commit()
        cursor.close()
        logging.info(f"Successfully saved MFA secret for user {user_id}")
        return True
    except Exception as e:
        logging.error(f"Error saving MFA secret for user {user_id}: {e}")
        return False

def check_mfa_enabled(database_type, cnx, user_id):
    if database_type == "postgresql":
        cursor = cnx.cursor(cursor_factory=RealDictCursor)
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
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


def get_mfa_secret(database_type, cnx, user_id):
    if database_type == "postgresql":
        cursor = cnx.cursor(cursor_factory=RealDictCursor)
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
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


def delete_mfa_secret(database_type, cnx, user_id):
    if database_type == "postgresql":
        cursor = cnx.cursor(cursor_factory=RealDictCursor)
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
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


def get_all_episodes(database_type, cnx, pod_feed):
    if database_type == "postgresql":
        cursor = cnx.cursor(cursor_factory=RealDictCursor)
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
        cursor = cnx.cursor(dictionary=True)

    query = (
        f"SELECT * FROM Episodes INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID WHERE Podcasts.FeedURL = %s")

    try:
        cursor.execute(query, (pod_feed,))
        result = cursor.fetchall()
        cursor.close()

        return result
    except Exception as e:
        print("Error retrieving Podcast Episodes:", e)
        return None


def remove_episode_history(database_type, cnx, url, title, user_id):
    if database_type == "postgresql":
        cursor = cnx.cursor(cursor_factory=RealDictCursor)
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
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


def setup_timezone_info(database_type, cnx, user_id, timezone, hour_pref, date_format):
    if database_type == "postgresql":
        cursor = cnx.cursor(cursor_factory=RealDictCursor)
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
        cursor = cnx.cursor(dictionary=True)

    query = f"""UPDATE Users SET Timezone = %s, TimeFormat = %s, DateFormat = %s, FirstLogin = %s WHERE UserID = %s"""

    try:
        cursor.execute(query, (timezone, hour_pref, date_format, 1, user_id))
        cnx.commit()
        cursor.close()

        return True
    except Exception as e:
        print("Error setting up time info:", e)
        return False


def get_time_info(database_type, cnx, user_id):
    if database_type == "postgresql":
        cursor = cnx.cursor(cursor_factory=RealDictCursor)
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
        cursor = cnx.cursor(dictionary=True)
    query = (f"""SELECT Timezone, TimeFormat, DateFormat FROM Users WHERE UserID = %s""")

    cursor.execute(query, (user_id,))
    result = cursor.fetchone()
    cursor.close()

    if result:
        return result['Timezone'], result['TimeFormat'], result['DateFormat']
    else:
        return None, None, None


def first_login_done(database_type, cnx, user_id):
    if database_type == "postgresql":
        cursor = cnx.cursor(cursor_factory=RealDictCursor)
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
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


def search_data(database_type, cnx, search_term, user_id):
    if database_type == "postgresql":
        cursor = cnx.cursor(cursor_factory=RealDictCursor)
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
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


def queue_pod(database_type, cnx, episode_id, user_id):
    if database_type == "postgresql":
        cursor = cnx.cursor(cursor_factory=RealDictCursor)
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
        cursor = cnx.cursor(dictionary=True)

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

def check_queued(database_type, cnx, episode_id, user_id):
    if database_type == "postgresql":
        cursor = cnx.cursor(cursor_factory=RealDictCursor)
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
        cursor = cnx.cursor(dictionary=True)

    query = """
    SELECT * FROM EpisodeQueue 
    WHERE EpisodeID = %s AND UserID = %s
    """
    cursor.execute(query, (episode_id, user_id))
    result = cursor.fetchone()
    cursor.close()

    if result:
        return True
    else:
        return False

def remove_queued_pod(database_type, cnx, episode_id, user_id):
    if database_type == "postgresql":
        cursor = cnx.cursor(cursor_factory=RealDictCursor)
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
        cursor = cnx.cursor(dictionary=True)

    # First, retrieve the EpisodeID and QueuePosition of the episode to be removed
    get_queue_data_query = """
    SELECT EpisodeQueue.EpisodeID, EpisodeQueue.QueuePosition
    FROM EpisodeQueue 
    INNER JOIN Episodes ON EpisodeQueue.EpisodeID = Episodes.EpisodeID 
    WHERE Episodes.EpisodeID = %s AND EpisodeQueue.UserID = %s
    """
    cursor.execute(get_queue_data_query, (episode_id, user_id))
    queue_data = cursor.fetchone()
    if queue_data is None:
        print(f"No queued episode found with ID {episode_id}")
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

    print(f"Successfully removed episode from queue.")
    cursor.close()

    return {"status": "success"}


def get_queued_episodes(database_type, cnx, user_id):
    if database_type == "postgresql":
        cursor = cnx.cursor(cursor_factory=RealDictCursor)
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
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
        UserEpisodeHistory.ListenDuration,
        Episodes.EpisodeID
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

def check_episode_exists(cnx, user_id, episode_title, episode_url):
    cursor = cnx.cursor()
    query = """
        SELECT EXISTS(
            SELECT 1 FROM Episodes
            JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID
            WHERE Podcasts.UserID = %s AND Episodes.EpisodeTitle = %s AND Episodes.EpisodeURL = %s
        )
    """
    cursor.execute(query, (user_id, episode_title, episode_url))
    result = cursor.fetchone()
    cursor.close()
    # result[0] will be 1 if the episode exists, otherwise 0
    return result[0] == 1


def add_gpodder_settings(database_type, cnx, user_id, gpodder_url, gpodder_token, login_name):
    print("Adding gPodder settings")
    print(f"User ID: {user_id}, gPodder URL: {gpodder_url}, gPodder Token: {gpodder_token}, Login Name: {login_name}")
    the_key = get_encryption_key(cnx)

    cursor = cnx.cursor()
    from cryptography.fernet import Fernet

    encryption_key_bytes = base64.b64decode(the_key)

    cipher_suite = Fernet(encryption_key_bytes)

    # Only encrypt password if it's not None
    if gpodder_token is not None:
        encrypted_password = cipher_suite.encrypt(gpodder_token.encode())
        # Decode encrypted password back to string
        decoded_token = encrypted_password.decode()
    else:
        decoded_token = None

    cursor.execute(
        "UPDATE Users SET GpodderUrl = %s, GpodderLoginName = %s, GpodderToken = %s WHERE UserID = %s",
        (gpodder_url, login_name, decoded_token, user_id)
    )
    # Check if the update was successful
    if cursor.rowcount == 0:
        return None

    cnx.commit()  # Commit changes to the database
    cursor.close()

    return True


def get_gpodder_settings(database_type, cnx, user_id):
    cursor = cnx.cursor()

    # Check if the user already has gPodder settings
    cursor.execute(
        "SELECT GpodderUrl, GpodderToken FROM Users WHERE UserID = %s",
        (user_id,)
    )
    result = cursor.fetchone()

    cnx.commit()  # Commit changes to the database
    cursor.close()
    return result


def remove_gpodder_settings(database_type, cnx, user_id):
    cursor = cnx.cursor()

    # Reset gPodder settings to default for the specified user
    cursor.execute(
        "UPDATE Users SET GpodderUrl = %s, GpodderToken = %s WHERE UserID = %s",
        ('', '', user_id)
    )

    cnx.commit()  # Commit changes to the database
    cursor.close()


def check_gpodder_settings(database_type, cnx, user_id):
    cursor = cnx.cursor()

    # Query to check if gPodder settings exist for the specified user
    cursor.execute(
        "SELECT GpodderUrl, GpodderToken FROM Users WHERE UserID = %s",
        (user_id,)
    )

    result = cursor.fetchone()

    cursor.close()

    # Check if gPodder settings are not empty
    if result and result[0] and result[1]:
        return True  # gPodder is set up
    else:
        return False  # gPodder is not set up


def get_nextcloud_users(database_type, cnx):
    cursor = cnx.cursor()

    # Query to select users with set Nextcloud gPodder URLs and Tokens
    query = "SELECT UserID, GpodderUrl, GpodderToken, GpodderLoginName FROM Users WHERE GpodderUrl <> '' AND GpodderToken <> '' AND GpodderLoginName <> ''"
    cursor.execute(query)

    # Fetch all matching records
    users = cursor.fetchall()
    cursor.close()

    return users

def current_timestamp():
    # Return the current time in ISO 8601 format
    return datetime.utcnow().isoformat() + 'Z'  # Adding 'Z' indicates Zulu time, which is UTC

def refresh_nextcloud_subscription(database_type, cnx, user_id, gpodder_url, encrypted_gpodder_token, gpodder_login):
    from cryptography.fernet import Fernet
    from requests.auth import HTTPBasicAuth
    # Fetch encryption key
    encryption_key = get_encryption_key(cnx)
    encryption_key_bytes = base64.b64decode(encryption_key)

    cipher_suite = Fernet(encryption_key_bytes)

    # Decrypt the token
    if encrypted_gpodder_token is not None:
        decrypted_token_bytes = cipher_suite.decrypt(encrypted_gpodder_token.encode())
        gpodder_token = decrypted_token_bytes.decode()
    else:
        gpodder_token = None

    # Prepare for Basic Auth
    auth = HTTPBasicAuth(gpodder_login, gpodder_token)

    # Now, use the decrypted token in your API request
    print(f"Decrypted gPodder token: {gpodder_token}")
    response = requests.get(f"{gpodder_url}/index.php/apps/gpoddersync/subscriptions", auth=auth)
    response.raise_for_status()  # This will raise an exception for HTTP errors
    print(f"Response status: {response.status_code}, Content: {response.text}")

    nextcloud_podcasts = response.json().get("add", [])
    print(f"Nextcloud podcasts: {nextcloud_podcasts}")

    cursor = cnx.cursor()
    cursor.execute("SELECT FeedURL FROM Podcasts WHERE UserID = %s", (user_id,))
    local_podcasts = [row[0] for row in cursor.fetchall()]
    print(f"Local podcasts: {local_podcasts}")

    podcasts_to_add = set(nextcloud_podcasts) - set(local_podcasts)
    podcasts_to_remove = set(local_podcasts) - set(nextcloud_podcasts)
    print(f"Podcasts to add: {podcasts_to_add}, Podcasts to remove: {podcasts_to_remove}")

    # Update local database
    # Add new podcasts
    for feed_url in podcasts_to_add:
        podcast_values = get_podcast_values(feed_url, user_id)
        return_value = add_podcast(cnx, podcast_values, user_id)
        if return_value:
            print(f"{feed_url} added!")
        else:
            print(f"error adding {feed_url}")

    # Remove podcasts no longer in the subscription
    for feed_url in podcasts_to_remove:
        cursor.execute("SELECT PodcastName FROM Podcasts WHERE FeedURL = %s", feed_url)
        result = cursor.fetchone()
        remove_podcast(cnx, result, user_id)

    cnx.commit()
    cursor.close()

    # Notify Nextcloud of changes made locally (if any)
    if podcasts_to_add or podcasts_to_remove:
        sync_subscription_change(gpodder_url, {"Authorization": f"Bearer {gpodder_token}"}, list(podcasts_to_add),
                                 list(podcasts_to_remove))
        
    # from requests.exceptions import RequestException

    # Fetch episode actions from Nextcloud
    try:
        episode_actions_response = requests.get(
            f"{gpodder_url}/index.php/apps/gpoddersync/episode_action",
            headers={"Authorization": f"Bearer {gpodder_token}"}
        )
        episode_actions_response.raise_for_status()  # This will raise an exception for HTTP errors
        episode_actions = episode_actions_response.json()
    except RequestException as e:
        print(f"Error fetching Nextcloud episode actions: {e}")
        episode_actions = []

    # Process episode actions from Nextcloud
    for action in episode_actions.get('actions', []):  # Ensure default to empty list if 'actions' is not found
        try:
            # Ensure action is relevant, such as a 'play' or 'update_time' action with a valid position
            if action["action"] in ["play", "update_time"] and "position" in action and "episode" in action:
                episode_id = get_episode_id_by_url(cnx, action["episode"])
                if episode_id:
                    record_listen_duration(cnx, episode_id, user_id, int(action["position"]))
        except Exception as e:
            print(f"Error processing episode action {action}: {e}")

    # Collect local episode listen times and push to Nextcloud if necessary
    try:
        local_episode_times = get_local_episode_times(cnx, user_id)
    except Exception as e:
        print(f"Error fetching local episode times: {e}")
        local_episode_times = []

    # Send local episode listen times to Nextcloud
    update_actions = []
    for episode_time in local_episode_times:
        update_actions.append({
            "podcast": episode_time["podcast_url"],
            "episode": episode_time["episode_url"],
            "action": "update_time",  # This is a hypothetical action
            "timestamp": current_timestamp(),  # Your method to get the current timestamp
            "position": episode_time["listen_duration"]
        })

    if update_actions:
        try:
            response = requests.post(
                f"{gpodder_url}/index.php/apps/gpoddersync/episode_action/create",
                json=update_actions,
                auth=HTTPBasicAuth(gpodder_login, gpodder_token)  # Use Basic Auth here
            )
            response.raise_for_status()  # Check for HTTP errors
            print(f"Update episode times response: {response.status_code}")
        except RequestException as e:
            print(f"Error updating episode times in Nextcloud: {e}")

# database_functions.py

def queue_bump(database_type, cnx, ep_url, title, user_id):
    cursor = cnx.cursor()

    # check if the episode is already in the queue
    cursor.execute(
        "SELECT QueueID, QueuePosition FROM EpisodeQueue "
        "INNER JOIN Episodes ON EpisodeQueue.EpisodeID = Episodes.EpisodeID "
        "WHERE Episodes.EpisodeURL = %s AND Episodes.EpisodeTitle = %s AND EpisodeQueue.UserID = %s",
        (ep_url, title, user_id)
    )
    result = cursor.fetchone()
    print(result)

    if result is not None:
        try:
            cursor.execute(
                "DELETE FROM EpisodeQueue WHERE QueueID = %s", (result[0],)
            )
        except Exception as e:
            print(f"Error while deleting episode from queue: {e}")

    # decrease the QueuePosition of all other episodes in the queue
    cursor.execute(
        "UPDATE EpisodeQueue SET QueuePosition = QueuePosition - 1 WHERE UserID = %s", (user_id,)
    )

    # add the episode to the front of the queue
    queue_pod(database_type, cnx, title, ep_url, user_id)

    cnx.commit()
    cursor.close()

    return {"detail": f"{title} moved to the front of the queue."}


def backup_user(database_type, cnx, user_id):
    if database_type == "postgresql":
        cursor = cnx.cursor(cursor_factory=RealDictCursor)
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
        cursor = cnx.cursor(dictionary=True)

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


def backup_server(cnx, database_pass):
    # Replace with your database and authentication details
    print(f'pass: {database_pass}')
    cmd = [
        "mysqldump",
        "-h", 'db',
        "-P", '3306',
        "-u", "root",
        "-p" + database_pass,
        "pypods_database"
    ]

    process = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    stdout, stderr = process.communicate()
    print("STDOUT:", stdout.decode())
    print("STDERR:", stderr.decode())

    if process.returncode != 0:
        # Handle error
        raise Exception(f"Backup failed with error: {stderr.decode()}")

    return stdout.decode()


def restore_server(cnx, database_pass, server_restore_data):
    import tempfile
    # Create a temporary file to store the content. This is because the mysql command reads from a file.
    with tempfile.NamedTemporaryFile(mode='w+', delete=True) as tempf:
        tempf.write(server_restore_data)
        tempf.flush()
        cmd = [
            "mysql",
            "-h", 'db',
            "-P", '3306',
            "-u", "root",
            "-p" + database_pass,
            "pypods_database"
        ]

        # Use the file's content as input for the mysql command
        with open(tempf.name, 'r') as file:
            process = subprocess.Popen(cmd, stdin=file, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            stdout, stderr = process.communicate()

            if process.returncode != 0:
                raise Exception(f"Restoration failed with error: {stderr.decode()}")

    return "Restoration completed successfully!"

