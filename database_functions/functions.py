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
import psycopg
from psycopg.rows import dict_row
from requests.exceptions import RequestException
from fastapi import HTTPException
from mysql.connector import ProgrammingError

# # Get the application root directory from the environment variable
# app_root = os.environ.get('APP_ROOT')
sys.path.append('/pinepods/'),
# Import the functions directly from app_functions.py located in the database_functions directory
from database_functions.app_functions import sync_subscription_change, get_podcast_values, check_valid_feed


def pascal_case(snake_str):
    return ''.join(word.title() for word in snake_str.split('_'))

def lowercase_keys(data):
    if isinstance(data, dict):
        return {k.lower(): (bool(v) if k.lower() == 'completed' else v) for k, v in data.items()}
    elif isinstance(data, list):
        return [lowercase_keys(item) for item in data]
    return data



def capitalize_keys(data):
    if isinstance(data, dict):
        return {pascal_case(k): v for k, v in data.items()}
    elif isinstance(data, list):
        return [capitalize_keys(item) for item in data]
    return data

def normalize_keys(data, database_type):
    if database_type == "postgresql":
        # Convert keys to PascalCase
        return {pascal_case(k): v for k, v in data.items()}
    return data

def get_value(result, key, default=None):
    """
    Helper function to extract value from result set.
    It handles both dictionaries and tuples.
    """
    key_lower = key.lower()
    if isinstance(result, dict):
        # Handles keys returned as lowercase in PostgreSQL
        return result.get(key_lower, default)
    elif isinstance(result, tuple):
        # Handles keys with tuple index mapping
        key_map = {
            "podcastid": 0,
            "episodeurl": 0,
            "podcastname": 0
        }
        index = key_map.get(key_lower)
        return result[index] if index is not None else default
    return default



def get_web_key(cnx, database_type):
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'SELECT APIKey FROM "APIKeys" WHERE UserID = 1'
    else:
        query = "SELECT APIKey FROM APIKeys WHERE UserID = 1"
    cursor.execute(query)
    result = cursor.fetchone()
    cursor.close()

    if result:
        return result[0]
    else:
        return None

def add_custom_podcast(database_type, cnx, feed_url, user_id):
    # Proceed to extract and use podcast details if the feed is valid
    podcast_values = get_podcast_values(feed_url, user_id)
    print("Adding podcast custom")

    try:
        return_value = add_podcast(cnx, database_type, podcast_values, user_id)
        if not return_value:
            raise Exception("Failed to add the podcast.")
        return return_value
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))

def add_news_feed_if_not_added(database_type, cnx):
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            cursor.execute('SELECT NewsFeedSubscribed FROM "AppSettings"')
        else:  # MySQL or MariaDB
            cursor.execute("SELECT NewsFeedSubscribed FROM AppSettings")

        result = cursor.fetchone()
        if result is None or result[0] == 0:
            # The news feed has not been added before, so add it
            feed_url = "https://news.pinepods.online/feed.xml"
            user_id = 2
            add_custom_podcast(database_type, cnx, feed_url, user_id)

            # Update the AppSettings table to indicate that the news feed has been added
            if database_type == "postgresql":
                cursor.execute('UPDATE "AppSettings" SET NewsFeedSubscribed = TRUE')
            else:  # MySQL or MariaDB
                cursor.execute("UPDATE AppSettings SET NewsFeedSubscribed = 1")

            cnx.commit()
    except (psycopg.ProgrammingError, mysql.connector.ProgrammingError) as e:
        print(f"Error in add_news_feed_if_not_added: {e}")
        cnx.rollback()
    finally:
        cursor.close()

def add_podcast(cnx, database_type, podcast_values, user_id):
    cursor = cnx.cursor()
    print(f"Podcast values '{podcast_values}'")

    try:
        # Check if the podcast already exists for the user
        if database_type == "postgresql":
            query = 'SELECT PodcastID FROM "Podcasts" WHERE FeedURL = %s AND UserID = %s'
        else:  # MySQL or MariaDB
            query = "SELECT PodcastID FROM Podcasts WHERE FeedURL = %s AND UserID = %s"

        cursor.execute(query, (podcast_values['pod_feed_url'], user_id))
        result = cursor.fetchone()
        print(f"Result: {result}")
        print("Checked for existing podcast")

        if result is not None:
            # Podcast already exists for the user, return False
            cursor.close()
            return False

        # Insert the podcast into the database
        if database_type == "postgresql":
            add_podcast_query = """
                INSERT INTO "Podcasts"
                (PodcastName, ArtworkURL, Author, Categories, Description, EpisodeCount, FeedURL, WebsiteURL, Explicit, UserID)
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s) RETURNING PodcastID
            """
            explicit = podcast_values['pod_explicit']
        else:  # MySQL or MariaDB
            add_podcast_query = """
                INSERT INTO Podcasts
                (PodcastName, ArtworkURL, Author, Categories, Description, EpisodeCount, FeedURL, WebsiteURL, Explicit, UserID)
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
            """
            explicit = 1 if podcast_values['pod_explicit'] else 0

        print("Inserting into db")
        print(podcast_values['pod_title'])
        print(podcast_values['pod_artwork'])
        print(podcast_values['pod_author'])
        print(str(podcast_values['categories']))
        print(podcast_values['pod_description'])
        print(podcast_values['pod_episode_count'])
        print(podcast_values['pod_feed_url'])
        print(podcast_values['pod_website'])
        print(explicit)
        print(user_id)
        try:
            cursor.execute(add_podcast_query, (
                podcast_values['pod_title'],
                podcast_values['pod_artwork'],
                podcast_values['pod_author'],
                str(podcast_values['categories']),
                podcast_values['pod_description'],
                podcast_values['pod_episode_count'],
                podcast_values['pod_feed_url'],
                podcast_values['pod_website'],
                explicit,
                user_id
            ))

            if database_type == "postgresql":
                podcast_id = cursor.fetchone()
                if isinstance(podcast_id, tuple):
                    podcast_id = podcast_id[0]
                elif isinstance(podcast_id, dict):
                    podcast_id = podcast_id['podcastid']
            else:  # MySQL or MariaDB
                cnx.commit()
                podcast_id = cursor.lastrowid

            print('pre-id')
            if podcast_id is None:
                logging.error("No row was inserted.")
                print("No row was inserted.")
                cursor.close()
                return False

            print("Got id")
            print("Inserted into db")

            # Update UserStats table to increment PodcastsAdded count
            if database_type == "postgresql":
                query = 'UPDATE "UserStats" SET PodcastsAdded = PodcastsAdded + 1 WHERE UserID = %s'
            else:  # MySQL or MariaDB
                query = "UPDATE UserStats SET PodcastsAdded = PodcastsAdded + 1 WHERE UserID = %s"

            cursor.execute(query, (user_id,))
            cnx.commit()
            print("stats table updated")

            # Add episodes to database
            add_episodes(cnx, database_type, podcast_id, podcast_values['pod_feed_url'], podcast_values['pod_artwork'], False)
            print("episodes added")

        except Exception as e:
            logging.error(f"Failed to add podcast: {e}")
            print(f"Failed to add podcast: {e}")
            cnx.rollback()
            cursor.close()
            return False

    except Exception as e:
        print(f"Error during podcast insertion or UserStats update: {e}")
        logging.error(f"Error during podcast insertion or UserStats update: {e}")
        cnx.rollback()
        raise

    finally:
        cursor.close()

    # Return True to indicate success
    return True



def add_user(cnx, database_type, user_values):
    cursor = cnx.cursor()

    if database_type == "postgresql":
        add_user_query = """
            INSERT INTO "Users"
            (Fullname, Username, Email, Hashed_PW, IsAdmin)
            VALUES (%s, %s, %s, %s, false)
            RETURNING UserID
        """
    else:  # MySQL or MariaDB
        add_user_query = """
            INSERT INTO Users
            (Fullname, Username, Email, Hashed_PW, IsAdmin)
            VALUES (%s, %s, %s, %s, 0)
        """

    cursor.execute(add_user_query, user_values)
    # result = cursor.fetchone()
    # if isinstance(result, dict):
    #     user_id = result['userid']
    # else:
    #     user_id = result[0]
    if database_type == "postgresql":
        result = cursor.fetchone()
        user_id = result['userid'] if isinstance(result, dict) else result[0]
    else:  # MySQL or MariaDB
        user_id = cursor.lastrowid

    if database_type == "postgresql":
        add_user_settings_query = """
            INSERT INTO "UserSettings"
            (UserID, Theme)
            VALUES (%s, %s)
        """
    else:  # MySQL or MariaDB
        add_user_settings_query = """
            INSERT INTO UserSettings
            (UserID, Theme)
            VALUES (%s, %s)
        """
    cursor.execute(add_user_settings_query, (user_id, 'nordic'))

    if database_type == "postgresql":
        add_user_stats_query = """
            INSERT INTO "UserStats"
            (UserID)
            VALUES (%s)
        """
    else:  # MySQL or MariaDB
        add_user_stats_query = """
            INSERT INTO UserStats
            (UserID)
            VALUES (%s)
        """
    cursor.execute(add_user_stats_query, (user_id,))

    cnx.commit()
    cursor.close()


def add_admin_user(cnx, database_type, user_values):
    cursor = cnx.cursor()

    if database_type == "postgresql":
        add_user_query = """
            INSERT INTO "Users"
            (Fullname, Username, Email, Hashed_PW, IsAdmin)
            VALUES (%s, %s, %s, %s, 1)
        """
    else:  # MySQL or MariaDB
        add_user_query = """
            INSERT INTO Users
            (Fullname, Username, Email, Hashed_PW, IsAdmin)
            VALUES (%s, %s, %s, %s, 1)
        """

    cursor.execute(add_user_query, user_values)
    user_id = cursor.lastrowid if database_type != "postgresql" else cursor.fetchone()[0]

    if database_type == "postgresql":
        add_user_settings_query = """
            INSERT INTO "UserSettings"
            (UserID, Theme)
            VALUES (%s, %s)
        """
    else:  # MySQL or MariaDB
        add_user_settings_query = """
            INSERT INTO UserSettings
            (UserID, Theme)
            VALUES (%s, %s)
        """
    cursor.execute(add_user_settings_query, (user_id, 'nordic'))

    if database_type == "postgresql":
        add_user_stats_query = """
            INSERT INTO "UserStats"
            (UserID)
            VALUES (%s)
        """
    else:  # MySQL or MariaDB
        add_user_stats_query = """
            INSERT INTO UserStats
            (UserID)
            VALUES (%s)
        """
    cursor.execute(add_user_stats_query, (user_id,))

    cnx.commit()
    cursor.close()

def get_first_episode_id(cnx, database_type, podcast_id, user_id):
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            query = 'SELECT "EpisodeID" FROM "Episodes" WHERE "PodcastID" = %s LIMIT 1'
        else:  # MySQL or MariaDB
            query = "SELECT EpisodeID FROM Episodes WHERE PodcastID = %s LIMIT 1"

        cursor.execute(query, (podcast_id,))
        result = cursor.fetchone()
        return result[0] if result else None
    finally:
        cursor.close()



def add_episodes(cnx, database_type, podcast_id, feed_url, artwork_url, auto_download):
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
        if database_type == "postgresql":
            episode_check_query = 'SELECT * FROM "Episodes" WHERE PodcastID = %s AND EpisodeTitle = %s'
        else:  # MySQL or MariaDB
            episode_check_query = "SELECT * FROM Episodes WHERE PodcastID = %s AND EpisodeTitle = %s"

        cursor.execute(episode_check_query, (podcast_id, parsed_title))
        if cursor.fetchone():
            continue  # Episode already exists
        print("inserting now")
        # Insert the new episode
        if database_type == "postgresql":
            episode_insert_query = """
                INSERT INTO "Episodes"
                (PodcastID, EpisodeTitle, EpisodeDescription, EpisodeURL, EpisodeArtwork, EpisodePubDate, EpisodeDuration)
                VALUES (%s, %s, %s, %s, %s, %s, %s)
            """
        else:  # MySQL or MariaDB
            episode_insert_query = """
                INSERT INTO Episodes
                (PodcastID, EpisodeTitle, EpisodeDescription, EpisodeURL, EpisodeArtwork, EpisodePubDate, EpisodeDuration)
                VALUES (%s, %s, %s, %s, %s, %s, %s)
            """

        cursor.execute(episode_insert_query, (podcast_id, parsed_title, parsed_description, parsed_audio_url, parsed_artwork_url, parsed_release_datetime, parsed_duration))
        print('episodes inserted')
        # Get the EpisodeID for the newly added episode
        if cursor.rowcount > 0:
            print(f"Added episode '{parsed_title}'")
            if auto_download:  # Check if auto-download is enabled
                episode_id = get_episode_id(cnx, database_type, podcast_id, parsed_title, parsed_audio_url)
                user_id = get_user_id_from_pod_id(cnx, database_type, podcast_id)
                # Call your download function here
                download_podcast(cnx, database_type, episode_id, user_id)

    cnx.commit()


def remove_podcast(cnx, database_type, podcast_name, podcast_url, user_id):
    cursor = cnx.cursor()
    print('got to remove')

    try:
        print('getting id')
        # Get the PodcastID for the given podcast name
        if database_type == "postgresql":
            select_podcast_id = 'SELECT PodcastID FROM "Podcasts" WHERE PodcastName = %s AND FeedURL = %s'
        else:  # MySQL or MariaDB
            select_podcast_id = "SELECT PodcastID FROM Podcasts WHERE PodcastName = %s AND FeedURL = %s"
        cursor.execute(select_podcast_id, (podcast_name, podcast_url))
        result = cursor.fetchone()  # fetch one result

        if result:
            if isinstance(result, dict):
                podcast_id = result.get('podcastid')
            else:
                podcast_id = result[0]
        else:
            podcast_id = None

        print(podcast_id)

        # If there's no podcast ID found, raise an error or exit the function early
        if podcast_id is None:
            raise ValueError("No podcast found with name {}".format(podcast_name))

        # Delete user episode history entries associated with the podcast
        if database_type == "postgresql":
            delete_history = 'DELETE FROM "UserEpisodeHistory" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s)'
            delete_downloaded = 'DELETE FROM "DownloadedEpisodes" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s)'
            delete_saved = 'DELETE FROM "SavedEpisodes" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s)'
            delete_queue = 'DELETE FROM "EpisodeQueue" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s)'
            delete_episodes = 'DELETE FROM "Episodes" WHERE PodcastID = %s'
            delete_podcast = 'DELETE FROM "Podcasts" WHERE PodcastName = %s'
        else:  # MySQL or MariaDB
            delete_history = "DELETE FROM UserEpisodeHistory WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = %s)"
            delete_downloaded = "DELETE FROM DownloadedEpisodes WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = %s)"
            delete_saved = "DELETE FROM SavedEpisodes WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = %s)"
            delete_queue = "DELETE FROM EpisodeQueue WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = %s)"
            delete_episodes = "DELETE FROM Episodes WHERE PodcastID = %s"
            delete_podcast = "DELETE FROM Podcasts WHERE PodcastName = %s"

        cursor.execute(delete_history, (podcast_id,))
        cursor.execute(delete_downloaded, (podcast_id,))
        cursor.execute(delete_saved, (podcast_id,))
        cursor.execute(delete_queue, (podcast_id,))
        cursor.execute(delete_episodes, (podcast_id,))
        cursor.execute(delete_podcast, (podcast_name,))

        # Update UserStats table to decrement PodcastsAdded count
        if database_type == "postgresql":
            query = 'UPDATE "UserStats" SET PodcastsAdded = PodcastsAdded - 1 WHERE UserID = %s'
        else:  # MySQL or MariaDB
            query = "UPDATE UserStats SET PodcastsAdded = PodcastsAdded - 1 WHERE UserID = %s"
        cursor.execute(query, (user_id,))

        cnx.commit()
    except psycopg.Error as err:
        print("PostgreSQL Error: {}".format(err))
        cnx.rollback()
    except mysql.connector.Error as err:
        print("MySQL Error: {}".format(err))
        cnx.rollback()
    except Exception as e:
        print("General Error in remove_podcast: {}".format(e))
        cnx.rollback()
    finally:
        cursor.close()



def remove_podcast_id(cnx, database_type, podcast_id, user_id):
    cursor = cnx.cursor()

    try:
        # If there's no podcast ID found, raise an error or exit the function early
        if podcast_id is None:
            raise ValueError("No podcast found with ID {}".format(podcast_id))

        # Delete user episode history entries associated with the podcast
        if database_type == "postgresql":
            delete_history = 'DELETE FROM "UserEpisodeHistory" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s)'
            delete_downloaded = 'DELETE FROM "DownloadedEpisodes" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s)'
            delete_saved = 'DELETE FROM "SavedEpisodes" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s)'
            delete_queue = 'DELETE FROM "EpisodeQueue" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s)'
            delete_episodes = 'DELETE FROM "Episodes" WHERE PodcastID = %s'
            delete_podcast = 'DELETE FROM "Podcasts" WHERE PodcastID = %s'
            update_user_stats = 'UPDATE "UserStats" SET PodcastsAdded = PodcastsAdded - 1 WHERE UserID = %s'
        else:  # MySQL or MariaDB
            delete_history = "DELETE FROM UserEpisodeHistory WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = %s)"
            delete_downloaded = "DELETE FROM DownloadedEpisodes WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = %s)"
            delete_saved = "DELETE FROM SavedEpisodes WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = %s)"
            delete_queue = "DELETE FROM EpisodeQueue WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = %s)"
            delete_episodes = "DELETE FROM Episodes WHERE PodcastID = %s"
            delete_podcast = "DELETE FROM Podcasts WHERE PodcastID = %s"
            update_user_stats = "UPDATE UserStats SET PodcastsAdded = PodcastsAdded - 1 WHERE UserID = %s"

        cursor.execute(delete_history, (podcast_id,))
        cursor.execute(delete_downloaded, (podcast_id,))
        cursor.execute(delete_saved, (podcast_id,))
        cursor.execute(delete_queue, (podcast_id,))
        cursor.execute(delete_episodes, (podcast_id,))
        cursor.execute(delete_podcast, (podcast_id,))
        cursor.execute(update_user_stats, (user_id,))

        cnx.commit()
    except (psycopg.Error, mysql.connector.Error) as err:
        print("Error: {}".format(err))
        cnx.rollback()
    finally:
        cursor.close()

def return_episodes(database_type, cnx, user_id):
    if database_type == "postgresql":
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
        cursor = cnx.cursor(dictionary=True)

    if database_type == "postgresql":
        query = (
            'SELECT "Podcasts".PodcastName, "Episodes".EpisodeTitle, "Episodes".EpisodePubDate, '
            '"Episodes".EpisodeDescription, "Episodes".EpisodeArtwork, "Episodes".EpisodeURL, "Episodes".EpisodeDuration, '
            '"UserEpisodeHistory".ListenDuration, "Episodes".EpisodeID, "Episodes".Completed '
            'FROM "Episodes" '
            'INNER JOIN "Podcasts" ON "Episodes".PodcastID = "Podcasts".PodcastID '
            'LEFT JOIN "UserEpisodeHistory" ON "Episodes".EpisodeID = "UserEpisodeHistory".EpisodeID AND "UserEpisodeHistory".UserID = %s '
            'WHERE "Episodes".EpisodePubDate >= NOW() - INTERVAL \'30 days\' '
            'AND "Podcasts".UserID = %s '
            'ORDER BY "Episodes".EpisodePubDate DESC'
        )
    else:  # MySQL or MariaDB
        query = (
            "SELECT Podcasts.PodcastName, Episodes.EpisodeTitle, Episodes.EpisodePubDate, "
            "Episodes.EpisodeDescription, Episodes.EpisodeArtwork, Episodes.EpisodeURL, Episodes.EpisodeDuration, "
            "UserEpisodeHistory.ListenDuration, Episodes.EpisodeID, Episodes.Completed "
            "FROM Episodes "
            "INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
            "LEFT JOIN UserEpisodeHistory ON Episodes.EpisodeID = UserEpisodeHistory.EpisodeID AND UserEpisodeHistory.UserID = %s "
            "WHERE Episodes.EpisodePubDate >= DATE_SUB(NOW(), INTERVAL 30 DAY) "
            "AND Podcasts.UserID = %s "
            "ORDER BY Episodes.EpisodePubDate DESC"
        )

    cursor.execute(query, (user_id, user_id))
    rows = cursor.fetchall()

    cursor.close()

    if not rows:
        return []

    if database_type != "postgresql":
        # Convert column names to lowercase for MySQL and ensure `Completed` is a boolean
        rows = [{k.lower(): (bool(v) if k.lower() == 'completed' else v) for k, v in row.items()} for row in rows]

    return rows



def return_podcast_episodes(database_type, cnx, user_id, podcast_id):
    if database_type == "postgresql":
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
        cursor = cnx.cursor(dictionary=True)

    if database_type == "postgresql":
        query = (
            'SELECT "Podcasts".PodcastID, "Podcasts".PodcastName, "Episodes".EpisodeID, '
            '"Episodes".EpisodeTitle, "Episodes".EpisodePubDate, "Episodes".EpisodeDescription, '
            '"Episodes".EpisodeArtwork, "Episodes".EpisodeURL, "Episodes".EpisodeDuration, '
            '"UserEpisodeHistory".ListenDuration, CAST("Episodes".EpisodeID AS VARCHAR) AS guid '
            'FROM "Episodes" '
            'INNER JOIN "Podcasts" ON "Episodes".PodcastID = "Podcasts".PodcastID '
            'LEFT JOIN "UserEpisodeHistory" ON "Episodes".EpisodeID = "UserEpisodeHistory".EpisodeID AND "UserEpisodeHistory".UserID = %s '
            'WHERE "Podcasts".PodcastID = %s AND "Podcasts".UserID = %s '
            'ORDER BY "Episodes".EpisodePubDate DESC'
        )
    else:  # MySQL or MariaDB
        query = (
            "SELECT Podcasts.PodcastID, Podcasts.PodcastName, Episodes.EpisodeID, "
            "Episodes.EpisodeTitle, Episodes.EpisodePubDate, Episodes.EpisodeDescription, "
            "Episodes.EpisodeArtwork, Episodes.EpisodeURL, Episodes.EpisodeDuration, "
            "UserEpisodeHistory.ListenDuration, CAST(Episodes.EpisodeID AS CHAR) AS guid "
            "FROM Episodes "
            "INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
            "LEFT JOIN UserEpisodeHistory ON Episodes.EpisodeID = UserEpisodeHistory.EpisodeID AND UserEpisodeHistory.UserID = %s "
            "WHERE Podcasts.PodcastID = %s AND Podcasts.UserID = %s "
            "ORDER BY Episodes.EpisodePubDate DESC"
        )

    cursor.execute(query, (user_id, podcast_id, user_id))
    rows = cursor.fetchall()
    cursor.close()

    logging.error(f"Raw rows before normalization: {rows}")

    # Normalize keys
    rows = capitalize_keys(rows)

    logging.error(f"Raw rows after normalization: {rows}")

    logging.debug(f"Rows after normalization: {rows}")

    return rows or None

def get_podcast_details(database_type, cnx, user_id, podcast_id):
    if database_type == "postgresql":
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
        cursor = cnx.cursor(dictionary=True)

    if database_type == "postgresql":
        query = """
            SELECT *
            FROM "Podcasts"
            WHERE PodcastID = %s AND UserID = %s
        """
    else:  # MySQL or MariaDB
        query = """
            SELECT *
            FROM Podcasts
            WHERE PodcastID = %s AND UserID = %s
        """

    cursor.execute(query, (podcast_id, user_id))
    details = cursor.fetchone()
    cursor.close()

    return details


def get_podcast_id(database_type, cnx, user_id, podcast_feed, podcast_name):
    if database_type == "postgresql":
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
        cursor = cnx.cursor(dictionary=True)

    if database_type == "postgresql":
        query = (
            'SELECT PodcastID '
            'FROM "Podcasts" '
            'WHERE FeedURL = %s AND PodcastName = %s AND UserID = %s'
        )
    else:  # MySQL or MariaDB
        query = (
            "SELECT PodcastID "
            "FROM Podcasts "
            "WHERE FeedURL = %s AND PodcastName = %s AND UserID = %s"
        )

    cursor.execute(query, (podcast_feed, podcast_name, user_id))
    row = cursor.fetchone()  # Fetching only one row as we expect a single result

    cursor.close()

    if not row:
        return None

    if database_type == "postgresql":
        return row['podcastid']  # Assuming the column name is 'PodcastID'
    else:
        return row['PodcastID']  # Assuming the column name is 'PodcastID'

def get_location_value(result, key, default=None):
    """
    Helper function to extract value from result set.
    It handles both dictionaries and tuples.
    """
    key_lower = key.lower()
    if isinstance(result, dict):
        return result.get(key_lower, default)
    elif isinstance(result, tuple):
        # Define a mapping of field names to their tuple indices for your specific queries
        key_map = {
            "downloadid": 0,
            "downloadedlocation": 1
        }
        index = key_map.get(key_lower)
        return result[index] if index is not None else default
    return default

def delete_episode(database_type, cnx, episode_id, user_id):
    cursor = cnx.cursor()

    try:
        # Get the download ID from the DownloadedEpisodes table
        if database_type == "postgresql":
            query = (
                'SELECT DownloadID, DownloadedLocation '
                'FROM "DownloadedEpisodes" '
                'INNER JOIN "Episodes" ON "DownloadedEpisodes".EpisodeID = "Episodes".EpisodeID '
                'INNER JOIN "Podcasts" ON "Episodes".PodcastID = "Podcasts".PodcastID '
                'WHERE "Episodes".EpisodeID = %s AND "Podcasts".UserID = %s'
            )
        else:  # MySQL or MariaDB
            query = (
                "SELECT DownloadID, DownloadedLocation "
                "FROM DownloadedEpisodes "
                "INNER JOIN Episodes ON DownloadedEpisodes.EpisodeID = Episodes.EpisodeID "
                "INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
                "WHERE Episodes.EpisodeID = %s AND Podcasts.UserID = %s"
            )

        logging.debug(f"Executing query: {query} with EpisodeID: {episode_id} and UserID: {user_id}")
        cursor.execute(query, (episode_id, user_id))
        result = cursor.fetchone()

        logging.debug(f"Query result: {result}")

        if not result:
            logging.warning("No matching download found.")
            cursor.close()
            return

        download_id = get_location_value(result, "DownloadID")
        downloaded_location = get_location_value(result, "DownloadedLocation")

        logging.debug(f"DownloadID: {download_id}, DownloadedLocation: {downloaded_location}")

        # Delete the downloaded file
        if downloaded_location and os.path.exists(downloaded_location):
            os.remove(downloaded_location)
        else:
            logging.warning(f"Downloaded file not found: {downloaded_location}")

        # Remove the entry from the DownloadedEpisodes table
        if database_type == "postgresql":
            query = 'DELETE FROM "DownloadedEpisodes" WHERE DownloadID = %s'
        else:  # MySQL or MariaDB
            query = "DELETE FROM DownloadedEpisodes WHERE DownloadID = %s"
        cursor.execute(query, (download_id,))
        cnx.commit()
        logging.info(f"Removed {cursor.rowcount} entry from the DownloadedEpisodes table.")

        # Update UserStats table to decrement EpisodesDownloaded count
        if database_type == "postgresql":
            query = 'UPDATE "UserStats" SET EpisodesDownloaded = EpisodesDownloaded - 1 WHERE UserID = %s'
        else:  # MySQL or MariaDB
            query = "UPDATE UserStats SET EpisodesDownloaded = EpisodesDownloaded - 1 WHERE UserID = %s"
        cursor.execute(query, (user_id,))
        cnx.commit()

    except Exception as e:
        logging.error(f"Error during episode deletion: {e}")
        cnx.rollback()
    finally:
        cursor.close()


def return_selected_episode(database_type, cnx, user_id, title, url):
    if database_type == "postgresql":
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
        cursor = cnx.cursor(dictionary=True)

    if database_type == "postgresql":
        query = (
            'SELECT "Episodes".EpisodeTitle, "Episodes".EpisodeDescription, "Episodes".EpisodeURL, '
            '"Episodes".EpisodeArtwork, "Episodes".EpisodePubDate, "Episodes".EpisodeDuration, '
            '"Podcasts".PodcastName, "Podcasts".WebsiteURL, "Podcasts".FeedURL '
            'FROM "Episodes" '
            'INNER JOIN "Podcasts" ON "Episodes".PodcastID = "Podcasts".PodcastID '
            'WHERE "Episodes".EpisodeTitle = %s AND "Episodes".EpisodeURL = %s'
        )
    else:  # MySQL or MariaDB
        query = (
            "SELECT Episodes.EpisodeTitle, Episodes.EpisodeDescription, Episodes.EpisodeURL, "
            "Episodes.EpisodeArtwork, Episodes.EpisodePubDate, Episodes.EpisodeDuration, "
            "Podcasts.PodcastName, Podcasts.WebsiteURL, Podcasts.FeedURL "
            "FROM Episodes "
            "INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
            "WHERE Episodes.EpisodeTitle = %s AND Episodes.EpisodeURL = %s"
        )

    cursor.execute(query, (title, url))
    result = cursor.fetchall()

    cursor.close()

    episodes = []
    for row in result:
        episode = {
            'EpisodeTitle': row['EpisodeTitle'],
            'EpisodeDescription': row['EpisodeDescription'],
            'EpisodeURL': row['EpisodeURL'],
            'EpisodeArtwork': row['EpisodeArtwork'],
            'EpisodePubDate': row['EpisodePubDate'],
            'EpisodeDuration': row['EpisodeDuration'],
            'PodcastName': row['PodcastName'],
            'WebsiteURL': row['WebsiteURL']
        }
        episodes.append(episode)

    return episodes



def return_pods(database_type, cnx, user_id):
    if database_type == "postgresql":
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
        cursor = cnx.cursor(dictionary=True)

    if database_type == "postgresql":
        query = (
            'SELECT PodcastID, PodcastName, ArtworkURL, Description, EpisodeCount, WebsiteURL, FeedURL, Author, Categories, Explicit '
            'FROM "Podcasts" '
            'WHERE UserID = %s'
        )
    else:  # MySQL or MariaDB
        query = (
            "SELECT PodcastID, PodcastName, ArtworkURL, Description, EpisodeCount, WebsiteURL, FeedURL, Author, Categories, Explicit "
            "FROM Podcasts "
            "WHERE UserID = %s"
        )

    cursor.execute(query, (user_id,))
    rows = cursor.fetchall()

    cursor.close()

    if not rows:
        return None

    if database_type != "postgresql":
        # Convert column names to lowercase for MySQL
        rows = [{k.lower(): v for k, v in row.items()} for row in rows]

    return rows


def check_self_service(cnx, database_type):
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'SELECT SelfServiceUser FROM "AppSettings"'
    else:  # MySQL or MariaDB
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

def refresh_pods(cnx, database_type):
    import concurrent.futures

    print('refresh begin')
    cursor = cnx.cursor()

    if database_type == "postgresql":
        select_podcasts = 'SELECT PodcastID, FeedURL, ArtworkURL, AutoDownload FROM "Podcasts"'
    else:  # MySQL or MariaDB
        select_podcasts = "SELECT PodcastID, FeedURL, ArtworkURL, AutoDownload FROM Podcasts"


    cursor.execute(select_podcasts)
    result_set = cursor.fetchall()  # fetch the result set

    for (podcast_id, feed_url, artwork_url, auto_download) in result_set:
        print(f'Running for :{podcast_id}')
        add_episodes(cnx, database_type, podcast_id, feed_url, artwork_url, auto_download)

    cursor.close()
    # cnx.close()


def remove_unavailable_episodes(cnx, database_type):
    cursor = cnx.cursor()

    # select all episodes
    # select all episodes
    if database_type == "postgresql":
        select_episodes = 'SELECT EpisodeID, PodcastID, EpisodeTitle, EpisodeURL, EpisodePubDate FROM "Episodes"'
    else:  # MySQL or MariaDB
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
                if database_type == "postgresql":
                    delete_episode = 'DELETE FROM "Episodes" WHERE "EpisodeID"=%s'
                else:  # MySQL or MariaDB
                    delete_episode = "DELETE FROM Episodes WHERE EpisodeID=%s"
                cursor.execute(delete_episode, (episode_id,))
                cnx.commit()

        except Exception as e:
            print(f"Error checking episode {episode_id}: {e}")

    cursor.close()
    # cnx.close()


def get_podcast_id_by_title(cnx, database_type, podcast_title):
    cursor = cnx.cursor()

    # get the podcast ID for the specified title
    # get the podcast ID for the specified title
    if database_type == "postgresql":
        cursor.execute('SELECT PodcastID FROM "Podcasts" WHERE Title = %s', (podcast_title,))
    else:  # MySQL or MariaDB
        cursor.execute("SELECT PodcastID FROM Podcasts WHERE Title = %s", (podcast_title,))

    result = cursor.fetchone()

    if result:
        return result[0]
    else:
        return None

    cursor.close()
    # cnx.close()


def refresh_podcast_by_title(cnx, database_type, podcast_title):
    # get the podcast ID for the specified title
    podcast_id = get_podcast_id_by_title(cnx, database_type, podcast_title)

    if podcast_id is not None:
        # refresh the podcast with the specified ID
        refresh_single_pod(cnx, database_type, podcast_id)
    else:
        print("Error: Could not find podcast with title {}".format(podcast_title))


def refresh_single_pod(cnx, database_type, podcast_id):
    cursor = cnx.cursor()

    # get the feed URL and artwork URL for the specified podcast
    if database_type == "postgresql":
        cursor.execute('SELECT FeedURL, ArtworkURL FROM "Podcasts" WHERE PodcastID = %s', (podcast_id,))
    else:  # MySQL or MariaDB
        cursor.execute("SELECT FeedURL, ArtworkURL FROM Podcasts WHERE PodcastID = %s", (podcast_id,))
    feed_url, artwork_url = cursor.fetchone()

    # parse the podcast feed
    episode_dump = feedparser.parse(feed_url)

    # get the list of episode titles already in the database
    if database_type == "postgresql":
        cursor.execute('SELECT EpisodeTitle FROM "Episodes" WHERE PodcastID = %s', (podcast_id,))
    else:  # MySQL or MariaDB
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
            if database_type == "postgresql":
                add_episode = ('INSERT INTO "Episodes" '
                               '(PodcastID, EpisodeTitle, EpisodeDescription, EpisodeURL, EpisodeArtwork, EpisodePubDate, EpisodeDuration) '
                               'VALUES (%s, %s, %s, %s, %s, %s, %s)')
            else:  # MySQL or MariaDB
                add_episode = ("INSERT INTO Episodes "
                               "(PodcastID, EpisodeTitle, EpisodeDescription, EpisodeURL, EpisodeArtwork, EpisodePubDate, EpisodeDuration) "
                               "VALUES (%s, %s, %s, %s, %s, %s, %s)")
            episode_values = (podcast_id, title, description, audio_url, artwork_url, release_date, 0)
            cursor.execute(add_episode, episode_values)

    cnx.commit()

    cursor.close()
    # cnx.close()


def get_hist_value(result, key, default=None):
    """
    Helper function to extract value from result set.
    It handles both dictionaries and tuples.
    """
    if isinstance(result, dict):
        return result.get(key, default)
    elif isinstance(result, tuple):
        key_map = {
            "UserEpisodeHistoryID": 0,
        }
        index = key_map.get(key)
        return result[index] if index is not None else default
    return default

def record_podcast_history(cnx, database_type, episode_id, user_id, episode_pos):
    from datetime import datetime
    cursor = cnx.cursor()

    # Check if a record already exists in the UserEpisodeHistory table
    if database_type == "postgresql":
        check_history = 'SELECT UserEpisodeHistoryID FROM "UserEpisodeHistory" WHERE EpisodeID = %s AND UserID = %s'
    else:  # MySQL or MariaDB
        check_history = "SELECT UserEpisodeHistoryID FROM UserEpisodeHistory WHERE EpisodeID = %s AND UserID = %s"
    cursor.execute(check_history, (episode_id, user_id))
    result = cursor.fetchone()

    if result is not None:
        # Extract progress_id regardless of result type
        progress_id = get_hist_value(result, "UserEpisodeHistoryID")

        if progress_id is not None:
            # Update the existing record
            if database_type == "postgresql":
                update_history = 'UPDATE "UserEpisodeHistory" SET ListenDuration = %s, ListenDate = %s WHERE UserEpisodeHistoryID = %s'
            else:  # MySQL or MariaDB
                update_history = "UPDATE UserEpisodeHistory SET ListenDuration = %s, ListenDate = %s WHERE UserEpisodeHistoryID = %s"
            new_listen_duration = round(episode_pos)
            now = datetime.now()
            values = (new_listen_duration, now, progress_id)
            cursor.execute(update_history, values)
    else:
        # Add a new record
        if database_type == "postgresql":
            add_history = 'INSERT INTO "UserEpisodeHistory" (EpisodeID, UserID, ListenDuration, ListenDate) VALUES (%s, %s, %s, %s)'
        else:  # MySQL or MariaDB
            add_history = "INSERT INTO UserEpisodeHistory (EpisodeID, UserID, ListenDuration, ListenDate) VALUES (%s, %s, %s, %s)"
        new_listen_duration = round(episode_pos)
        now = datetime.now()
        values = (episode_id, user_id, new_listen_duration, now)
        cursor.execute(add_history, values)

    cnx.commit()
    cursor.close()
    # cnx.close()





def get_user_id(cnx, database_type, username):
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'SELECT UserID FROM "Users" WHERE Username = %s'
    else:
        query = "SELECT UserID FROM Users WHERE Username = %s"
    cursor.execute(query, (username,))
    result = cursor.fetchone()
    cursor.close()
    # cnx.close()

    if result:
        return result[0]
    else:
        return 1

def get_user_id_from_pod_id(cnx, database_type, podcast_id):
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'SELECT UserID FROM "Podcasts" WHERE PodcastID = %s'
    else:
        query = "SELECT UserID FROM Podcasts WHERE PodcastID = %s"

    cursor.execute(query, (podcast_id,))
    result = cursor.fetchone()

    if result:
        # Check if the result is a dictionary or tuple
        if isinstance(result, dict):
            user_id = result.get("userid")
        elif isinstance(result, tuple):
            user_id = result[0]
        else:
            user_id = None
    else:
        user_id = None

    cursor.close()
    return user_id


def get_user_details(cnx, database_type, username):
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'SELECT * FROM "Users" WHERE Username = %s'
    else:
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


def get_user_details_id(cnx, database_type, user_id):
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'SELECT * FROM "Users" WHERE UserID = %s'
    else:
        query = "SELECT * FROM Users WHERE UserID = %s"
    cursor.execute(query, (user_id,))
    result = cursor.fetchone()
    cursor.close()
    # cnx.close()

    if result:
        if isinstance(result, dict):
            return {
                'UserID': result['userid'],
                'Fullname': result['fullname'],
                'Username': result['username'],
                'Email': result['email'],
                'Hashed_PW': result['hashed_pw']
            }
        elif isinstance(result, tuple):
            return {
                'UserID': result[0],
                'Fullname': result[1],
                'Username': result[2],
                'Email': result[3],
                'Hashed_PW': result[4]
            }
    else:
        return None


def user_history(cnx, database_type, user_id):
    if not cnx:
        logging.error("Database connection is None.")
        return []

    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            query = ('SELECT "Episodes".EpisodeID, "UserEpisodeHistory".ListenDate, "UserEpisodeHistory".ListenDuration, '
                        '"Episodes".EpisodeTitle, "Episodes".EpisodeDescription, "Episodes".EpisodeArtwork, '
                        '"Episodes".EpisodeURL, "Episodes".EpisodeDuration, "Podcasts".PodcastName, "Episodes".EpisodePubDate, "Episodes".Completed '
                        'FROM "UserEpisodeHistory" '
                        'JOIN "Episodes" ON "UserEpisodeHistory".EpisodeID = "Episodes".EpisodeID '
                        'JOIN "Podcasts" ON "Episodes".PodcastID = "Podcasts".PodcastID '
                        'WHERE "UserEpisodeHistory".UserID = %s '
                        'ORDER BY "UserEpisodeHistory".ListenDate DESC')
        else:  # MySQL or MariaDB
            cursor = cnx.cursor(dictionary=True)  # Ensure dictionary mode
            query = ("SELECT Episodes.EpisodeID, UserEpisodeHistory.ListenDate, UserEpisodeHistory.ListenDuration, "
                        "Episodes.EpisodeTitle, Episodes.EpisodeDescription, Episodes.EpisodeArtwork, "
                        "Episodes.EpisodeURL, Episodes.EpisodeDuration, Podcasts.PodcastName, Episodes.EpisodePubDate, Episodes.Completed "
                        "FROM UserEpisodeHistory "
                        "JOIN Episodes ON UserEpisodeHistory.EpisodeID = Episodes.EpisodeID "
                        "JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
                        "WHERE UserEpisodeHistory.UserID = %s "
                        "ORDER BY UserEpisodeHistory.ListenDate DESC")

        cursor.execute(query, (user_id,))
        results = cursor.fetchall()

        if not results:
            logging.info("No results found for user history.")
            return []

    except Exception as e:
        logging.error(f"Error executing user_history query: {e}")
        raise
    finally:
        cursor.close()

    print('histing now')

    # Convert results to a list of dictionaries
    history_episodes = []
    for row in results:
        episode = {}
        if isinstance(row, tuple):
            for idx, col in enumerate(cursor.description):
                column_name = col[0].lower()
                value = row[idx]
                if column_name == 'completed':
                    value = bool(value)
                episode[column_name] = value
        elif isinstance(row, dict):
            for k, v in row.items():
                column_name = k.lower()
                value = v
                if column_name == 'completed':
                    value = bool(value)
                episode[column_name] = value
        else:
            logging.error(f"Unexpected row type: {type(row)}")
        history_episodes.append(episode)

    lower_hist = lowercase_keys(history_episodes)
    print(lower_hist)
    return lower_hist





def download_podcast(cnx, database_type, episode_id, user_id):
    cursor = cnx.cursor()
    print('download pod for print')
    logging.error('download pod for log')

    # Check if the episode is already downloaded
    if database_type == "postgresql":
        query = 'SELECT 1 FROM "DownloadedEpisodes" WHERE EpisodeID = %s AND UserID = %s'
    else:  # MySQL or MariaDB
        query = "SELECT 1 FROM DownloadedEpisodes WHERE EpisodeID = %s AND UserID = %s"
    cursor.execute(query, (episode_id, user_id))
    result = cursor.fetchone()
    if result:
        # Episode already downloaded
        cursor.close()
        return True

    print('getting id')
    # Get the EpisodeID and PodcastID from the Episodes table
    if database_type == "postgresql":
        query = 'SELECT PodcastID FROM "Episodes" WHERE EpisodeID = %s'
    else:  # MySQL or MariaDB
        query = "SELECT PodcastID FROM Episodes WHERE EpisodeID = %s"
    cursor.execute(query, (episode_id,))
    result = cursor.fetchone()
    print(f'here it is {result}')
    logging.error(f'here it is {result}')
    if result is None:
        # Episode not found
        return False

    podcast_id = get_value(result, "PodcastID")
    print('getting url')
    # Get the EpisodeURL from the Episodes table
    if database_type == "postgresql":
        query = 'SELECT EpisodeURL FROM "Episodes" WHERE EpisodeID = %s'
    else:  # MySQL or MariaDB
        query = "SELECT EpisodeURL FROM Episodes WHERE EpisodeID = %s"
    cursor.execute(query, (episode_id,))
    result = cursor.fetchone()
    print(f'here it is {result}')
    logging.error(f'here it is {result}')
    if result is None:
        # Episode not found
        return False

    episode_url = get_value(result, "EpisodeURL")
    print('getting name')
    # Get the PodcastName from the Podcasts table
    if database_type == "postgresql":
        query = 'SELECT PodcastName FROM "Podcasts" WHERE PodcastID = %s'
    else:  # MySQL or MariaDB
        query = "SELECT PodcastName FROM Podcasts WHERE PodcastID = %s"
    cursor.execute(query, (podcast_id,))
    result = cursor.fetchone()
    print(f'here it is name {result}')
    logging.error(f'here it is name {result}')
    if result is None:
        # Podcast not found
        return False

    podcast_name = get_value(result, "PodcastName")
    print('doing dir work')
    print(f'doing dirs')
    logging.error(f'doing dirs')
    # Create a directory named after the podcast, inside the main downloads directory
    download_dir = os.path.join("/opt/pinepods/downloads", podcast_name)
    os.makedirs(download_dir, exist_ok=True)
    print(f'generate name')
    logging.error(f'generate name')
    # Generate the episode filename based on episode ID and user ID
    filename = f"{user_id}-{episode_id}.mp3"
    print(filename)
    file_path = os.path.join(download_dir, filename)
    print(file_path)
    response = requests.get(episode_url, stream=True)
    response.raise_for_status()
    print(f'get date')
    logging.error(f'get date')
    # Get the current date and time for DownloadedDate
    downloaded_date = datetime.datetime.now()
    print(f'size')
    logging.error(f'size')
    # Get the file size from the Content-Length header
    file_size = int(response.headers.get("Content-Length", 0))
    print(f'file write')
    logging.error(f'file write')
    # Write the file to disk
    with open(file_path, "wb") as f:
        for chunk in response.iter_content(chunk_size=1024):
            f.write(chunk)
    print(f'insert')
    logging.error(f'insert')
    # Insert a new row into the DownloadedEpisodes table
    if database_type == "postgresql":
        query = ('INSERT INTO "DownloadedEpisodes" '
                 '(UserID, EpisodeID, DownloadedDate, DownloadedSize, DownloadedLocation) '
                 'VALUES (%s, %s, %s, %s, %s)')
    else:  # MySQL or MariaDB
        query = ("INSERT INTO DownloadedEpisodes "
                 "(UserID, EpisodeID, DownloadedDate, DownloadedSize, DownloadedLocation) "
                 "VALUES (%s, %s, %s, %s, %s)")
    cursor.execute(query, (user_id, episode_id, downloaded_date, file_size, file_path))
    print(f'download table write')
    logging.error(f'download table write')
    # Update UserStats table to increment EpisodesDownloaded count
    if database_type == "postgresql":
        query = ('UPDATE "UserStats" SET EpisodesDownloaded = EpisodesDownloaded + 1 WHERE UserID = %s')
    else:  # MySQL or MariaDB
        query = ("UPDATE UserStats SET EpisodesDownloaded = EpisodesDownloaded + 1 WHERE UserID = %s")
    cursor.execute(query, (user_id,))
    print(f'user stat')
    logging.error(f'user stat')
    cnx.commit()

    if cursor:
        cursor.close()

    return True

def get_episode_ids_for_podcast(cnx, database_type, podcast_id):
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s'
    else:  # MySQL or MariaDB
        query = "SELECT EpisodeID FROM Episodes WHERE PodcastID = %s"

    cursor.execute(query, (podcast_id,))
    results = cursor.fetchall()
    cursor.close()

    # Extract episode IDs from the results
    episode_ids = [row[0] if isinstance(row, tuple) else row.get('episodeid') for row in results]
    return episode_ids



def get_podcast_id_from_episode(cnx, database_type, episode_id, user_id):
    cursor = cnx.cursor()

    try:
        if database_type == "postgresql":
            query = (
                'SELECT "Episodes".PodcastID '
                'FROM "Episodes" '
                'INNER JOIN "Podcasts" ON "Episodes".PodcastID = "Podcasts".PodcastID '
                'WHERE "Episodes".EpisodeID = %s AND "Podcasts".UserID = %s'
            )
        else:  # MySQL or MariaDB
            query = (
                "SELECT Episodes.PodcastID "
                "FROM Episodes "
                "INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
                "WHERE Episodes.EpisodeID = %s AND Podcasts.UserID = %s"
            )
        cursor.execute(query, (episode_id, user_id))
        result = cursor.fetchone()

        if result:
            return result[0] if isinstance(result, tuple) else result.get("podcastid")
        return None

    finally:
        cursor.close()


def get_podcast_id_from_episode_name(cnx, database_type, episode_name, episode_url, user_id):
    cursor = cnx.cursor()

    try:
        if database_type == "postgresql":
            query = (
                'SELECT "Episodes".PodcastID '
                'FROM "Episodes" '
                'INNER JOIN "Podcasts" ON "Episodes".PodcastID = "Podcasts".PodcastID '
                'WHERE "Episodes".EpisodeTitle = %s AND "Episodes".EpisodeURL = %s AND "Podcasts".UserID = %s'
            )
        else:  # MySQL or MariaDB
            query = (
                "SELECT Episodes.PodcastID "
                "FROM Episodes "
                "INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
                "WHERE Episodes.EpisodeTitle = %s AND Episodes.EpisodeURL = %s AND Podcasts.UserID = %s"
            )
        cursor.execute(query, (episode_name, episode_url, user_id))
        result = cursor.fetchone()

        if result:
            return result[0] if isinstance(result, tuple) else result.get("podcastid")
        return None

    finally:
        cursor.close()


def mark_episode_completed(cnx, database_type, episode_id, user_id):
    cursor = cnx.cursor()
    print(f"episode_id: {episode_id}")
    print(f"user_id: {user_id}")
    logging.error(f"episode_id: {episode_id}")
    logging.error(f"user_id: {user_id}")
    try:
        if database_type == "postgresql":
            query = 'UPDATE "Episodes" SET Completed = TRUE WHERE EpisodeID = %s'
        else:  # MySQL or MariaDB
            query = "UPDATE Episodes SET Completed = 1 WHERE EpisodeID = %s"

        cursor.execute(query, (episode_id,))
        cnx.commit()
    finally:
        cursor.close()


def enable_auto_download(cnx, database_type, podcast_id, user_id, auto_download):
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            query = 'UPDATE "Podcasts" SET AutoDownload = %s WHERE PodcastID = %s AND UserID = %s'
        else:  # MySQL or MariaDB
            query = "UPDATE Podcasts SET AutoDownload = %s WHERE PodcastID = %s AND UserID = %s"
        cursor.execute(query, (auto_download, podcast_id, user_id))
        cnx.commit()
    except Exception as e:
        cnx.rollback()
        raise e
    finally:
        cursor.close()

def call_get_auto_download_status(cnx, database_type, podcast_id, user_id):
    cursor = cnx.cursor()
    print(f'podcast_id: {podcast_id}')
    try:
        if database_type == "postgresql":
            query = 'SELECT AutoDownload FROM "Podcasts" WHERE PodcastID = %s AND UserID = %s'
        else:  # MySQL or MariaDB
            query = "SELECT AutoDownload FROM Podcasts WHERE PodcastID = %s AND UserID = %s"

        cursor.execute(query, (podcast_id, user_id))
        result = cursor.fetchone()

        if result:
            return result[0] if isinstance(result, tuple) else result.get("autodownload")
        else:
            return None
    finally:
        cursor.close()



def adjust_skip_times(cnx, database_type, podcast_id, start_skip, end_skip):
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            query = 'UPDATE "Podcasts" SET StartSkip = %s, EndSkip = %s WHERE PodcastID = %s'
        else:  # MySQL or MariaDB
            query = "UPDATE Podcasts SET StartSkip = %s, EndSkip = %s WHERE PodcastID = %s"
        cursor.execute(query, (start_skip, end_skip, podcast_id))
        cnx.commit()
    except Exception as e:
        cnx.rollback()
        raise e
    finally:
        cursor.close()

def get_auto_skip_times(cnx, database_type, podcast_id, user_id):
    cursor = cnx.cursor()
    print(podcast_id, user_id)
    logging.error(f"Error retrieving DownloadedLocation: {podcast_id}, {user_id}")
    try:
        if database_type == "postgresql":
            query = """
                SELECT StartSkip, EndSkip
                FROM "Podcasts"
                WHERE PodcastID = %s AND UserID = %s
            """
        else:  # MySQL or MariaDB
            query = """
                SELECT StartSkip, EndSkip
                FROM Podcasts
                WHERE PodcastID = %s AND UserID = %s
            """
        cursor.execute(query, (podcast_id, user_id))
        result = cursor.fetchone()

        if result:
            if isinstance(result, dict):
                return result.get("startskip"), result.get("endskip")
            elif isinstance(result, tuple):
                return result[0], result[1]
        return None, None
    finally:
        cursor.close()


def check_downloaded(cnx, database_type, user_id, episode_id):
    cursor = None
    try:
        cursor = cnx.cursor()

        # Check if the episode is downloaded for the user
        if database_type == "postgresql":
            query = 'SELECT DownloadID FROM "DownloadedEpisodes" WHERE UserID = %s AND EpisodeID = %s'
        else:
            query = "SELECT DownloadID FROM DownloadedEpisodes WHERE UserID = %s AND EpisodeID = %s"
        cursor.execute(query, (user_id, episode_id))
        result = cursor.fetchone()

        if result:
            if isinstance(result, dict):
                return result.get("DownloadID") is not None
            elif isinstance(result, tuple):
                return result[0] is not None
        return False

    except mysql.connector.errors.InterfaceError:
        return False
    finally:
        if cursor:
            cursor.close()


def get_download_value(result, key, default=None):
    """
    Helper function to extract value from result set.
    It handles both dictionaries and tuples.
    """
    key_lower = key.lower()
    if isinstance(result, dict):
        return result.get(key_lower, default)
    elif isinstance(result, tuple):
        # Define a mapping of field names to their tuple indices for your specific queries
        key_map = {
            "downloadedlocation": 0
        }
        index = key_map.get(key_lower)
        return result[index] if index is not None else default
    return default

def get_download_location(cnx, database_type, episode_id, user_id):
    cursor = cnx.cursor()
    try:
        # Check if the episode has been downloaded by the user
        if database_type == "postgresql":
            query = 'SELECT DownloadedLocation FROM "DownloadedEpisodes" WHERE EpisodeID = %s AND UserID = %s'
        else:
            query = "SELECT DownloadedLocation FROM DownloadedEpisodes WHERE EpisodeID = %s AND UserID = %s"

        print(f"Executing query: {query} with EpisodeID: {episode_id} and UserID: {user_id}")
        cursor.execute(query, (episode_id, user_id))
        result = cursor.fetchone()

        print(f"Query result: {result}")

        if result:
            location = get_download_value(result, "DownloadedLocation")
            print(f"DownloadedLocation found: {location}")
            return location

        print("No DownloadedLocation found for the given EpisodeID and UserID")
        return None

    except Exception as e:
        logging.error(f"Error retrieving DownloadedLocation: {e}")
        return None

    finally:
        cursor.close()



def download_episode_list(database_type, cnx, user_id):
    if database_type == "postgresql":
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
        cursor = cnx.cursor(dictionary=True)

    if database_type == "postgresql":
        query = (
            'SELECT '
            '"Podcasts".PodcastID, '
            '"Podcasts".PodcastName, '
            '"Podcasts".ArtworkURL, '
            '"Episodes".EpisodeID, '
            '"Episodes".EpisodeTitle, '
            '"Episodes".EpisodePubDate, '
            '"Episodes".EpisodeDescription, '
            '"Episodes".EpisodeArtwork, '
            '"Episodes".EpisodeURL, '
            '"Episodes".EpisodeDuration, '
            '"Podcasts".WebsiteURL, '
            '"DownloadedEpisodes".DownloadedLocation, '
            '"UserEpisodeHistory".ListenDuration, '
            '"Episodes".Completed '
            'FROM "DownloadedEpisodes" '
            'INNER JOIN "Episodes" ON "DownloadedEpisodes".EpisodeID = "Episodes".EpisodeID '
            'INNER JOIN "Podcasts" ON "Episodes".PodcastID = "Podcasts".PodcastID '
            'LEFT JOIN "UserEpisodeHistory" ON "DownloadedEpisodes".EpisodeID = "UserEpisodeHistory".EpisodeID AND "DownloadedEpisodes".UserID = "UserEpisodeHistory".UserID '
            'WHERE "DownloadedEpisodes".UserID = %s '
            'ORDER BY "DownloadedEpisodes".DownloadedDate DESC'
        )
    else:  # MySQL or MariaDB
        query = (
            "SELECT "
            "Podcasts.PodcastID, "
            "Podcasts.PodcastName, "
            "Podcasts.ArtworkURL, "
            "Episodes.EpisodeID, "
            "Episodes.EpisodeTitle, "
            "Episodes.EpisodePubDate, "
            "Episodes.EpisodeDescription, "
            "Episodes.EpisodeArtwork, "
            "Episodes.EpisodeURL, "
            "Episodes.EpisodeDuration, "
            "Podcasts.WebsiteURL, "
            "DownloadedEpisodes.DownloadedLocation, "
            "UserEpisodeHistory.ListenDuration, "
            "Episodes.Completed "
            "FROM DownloadedEpisodes "
            "INNER JOIN Episodes ON DownloadedEpisodes.EpisodeID = Episodes.EpisodeID "
            "INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
            "LEFT JOIN UserEpisodeHistory ON DownloadedEpisodes.EpisodeID = UserEpisodeHistory.EpisodeID AND DownloadedEpisodes.UserID = UserEpisodeHistory.UserID "
            "WHERE DownloadedEpisodes.UserID = %s "
            "ORDER BY DownloadedEpisodes.DownloadedDate DESC"
        )

    cursor.execute(query, (user_id,))
    rows = cursor.fetchall()

    cursor.close()

    if not rows:
        return None

    downloaded_episodes = lowercase_keys(rows)

    return downloaded_episodes


def save_email_settings(cnx, database_type, email_settings):
    cursor = cnx.cursor()

    if database_type == "postgresql":
        # Convert auth_required to boolean for PostgreSQL
        auth_required = bool(int(email_settings['auth_required']))
        query = (
            'UPDATE "EmailSettings" SET Server_Name = %s, Server_Port = %s, From_Email = %s, Send_Mode = %s, Encryption = %s, Auth_Required = %s, Username = %s, Password = %s WHERE EmailSettingsID = 1')
    else:
        # Keep auth_required as integer for other databases
        auth_required = int(email_settings['auth_required'])
        query = (
            "UPDATE EmailSettings SET Server_Name = %s, Server_Port = %s, From_Email = %s, Send_Mode = %s, Encryption = %s, Auth_Required = %s, Username = %s, Password = %s WHERE EmailSettingsID = 1")

    cursor.execute(query, (email_settings['server_name'], email_settings['server_port'], email_settings['from_email'],
                           email_settings['send_mode'], email_settings['encryption'],
                           auth_required, email_settings['email_username'],
                           email_settings['email_password']))

    cnx.commit()
    cursor.close()
    # cnx.close()

def get_encryption_key(cnx, database_type):
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = ('SELECT EncryptionKey FROM "AppSettings" WHERE AppSettingsID = 1')
    else:
        query = ("SELECT EncryptionKey FROM AppSettings WHERE AppSettingsID = 1")
    cursor.execute(query)
    result = cursor.fetchone()

    if not result:
        cursor.close()
        # cnx.close()
        return None

    # Convert the result to a dictionary.
    result_dict = {}
    if isinstance(result, tuple):
        result_dict = {column[0].lower(): value for column, value in zip(cursor.description, result)}
    elif isinstance(result, dict):
        result_dict = {k.lower(): v for k, v in result.items()}

    cursor.close()
    # cnx.close()

    # Convert the bytearray to a base64 encoded string before returning.
    return base64.b64encode(result_dict['encryptionkey']).decode()

def get_email_settings(cnx, database_type):
    if database_type == "postgresql":
        cursor = cnx.cursor(row_factory=dict_row)
    else:
        cursor = cnx.cursor()

    if database_type == "postgresql":
        query = 'SELECT * FROM "EmailSettings"'
    else:
        query = "SELECT * FROM EmailSettings"

    cursor.execute(query)
    result = cursor.fetchone()
    cursor.close()

    if result:
        if database_type == "postgresql":
            # Normalize keys to PascalCase
            settings_dict = normalize_keys(result, database_type)
        else:
            # For MySQL or MariaDB, convert tuple result to dictionary and keep keys as is
            keys = ["Emailsettingsid", "ServerName", "ServerPort", "FromEmail", "SendMode", "Encryption",
                    "AuthRequired", "Username", "Password"]
            settings_dict = dict(zip(keys, result))

        # Convert AuthRequired to 0 or 1 if database is PostgreSQL
        if database_type == "postgresql":
            settings_dict["AuthRequired"] = 1 if settings_dict["AuthRequired"] else 0

        return settings_dict
    else:
        return None


def get_episode_id(cnx, database_type, podcast_id, episode_title, episode_url):
    if database_type == "postgresql":
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
    else:  # MySQL or MariaDB
        cursor = cnx.cursor()

    if database_type == "postgresql":
        query = 'SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s AND EpisodeTitle = %s AND EpisodeUrl = %s'
    else:  # MySQL or MariaDB
        query = "SELECT EpisodeID FROM Episodes WHERE PodcastID = %s AND EpisodeTitle = %s AND EpisodeUrl = %s"

    params = (podcast_id, episode_title, episode_url)

    cursor.execute(query, params)
    result = cursor.fetchone()

    if result:
        episode_id = result['episodeid'] if database_type == "postgresql" else result[0]
    else:
        # Episode not found, insert a new episode into the Episodes table
        if database_type == "postgresql":
            query = 'INSERT INTO "Episodes" (PodcastID, EpisodeTitle, EpisodeUrl) VALUES (%s, %s, %s) RETURNING EpisodeID'
        else:  # MySQL or MariaDB
            query = "INSERT INTO Episodes (PodcastID, EpisodeTitle, EpisodeUrl) VALUES (%s, %s, %s)"

        cursor.execute(query, params)
        if database_type == "postgresql":
            episode_id = cursor.fetchone()['EpisodeID']
        else:
            episode_id = cursor.lastrowid

    cnx.commit()
    cursor.close()

    return episode_id


def get_episode_id_by_url(cnx, database_type, episode_url):
    cursor = cnx.cursor()

    if database_type == "postgresql":
        query = 'SELECT EpisodeID FROM "Episodes" WHERE EpisodeURL = %s'
    else:
        query = "SELECT EpisodeID FROM Episodes WHERE EpisodeURL = %s"
    params = (episode_url,)  # Ensure this is a tuple

    cursor.execute(query, params)
    result = cursor.fetchone()

    episode_id = None  # Initialize episode_id
    if result:
        episode_id = result[0]

    cursor.close()
    return episode_id



def queue_podcast_entry(cnx, database_type, user_id, episode_title, episode_url):
    if database_type == "postgresql":
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
    else:  # MySQL or MariaDB
        cursor = cnx.cursor()

    # Get the episode ID using the episode title and URL
    if database_type == "postgresql":
        query = 'SELECT EpisodeID, PodcastID FROM "Episodes" WHERE EpisodeTitle = %s AND EpisodeURL = %s'
    else:  # MySQL or MariaDB
        query = "SELECT EpisodeID, PodcastID FROM Episodes WHERE EpisodeTitle = %s AND EpisodeURL = %s"
    cursor.execute(query, (episode_title, episode_url))
    result = cursor.fetchone()

    if result:
        episode_id, podcast_id = result['EpisodeID'] if database_type == "postgresql" else result

        # Check if the episode is already in the queue
        if database_type == "postgresql":
            query = 'SELECT COUNT(*) FROM "EpisodeQueue" WHERE UserID = %s AND EpisodeID = %s'
        else:  # MySQL or MariaDB
            query = "SELECT COUNT(*) FROM EpisodeQueue WHERE UserID = %s AND EpisodeID = %s"
        cursor.execute(query, (user_id, episode_id))
        count = cursor.fetchone()[0]

        if count > 0:
            # Episode is already in the queue, move it to position 1 and update the QueueDate
            if database_type == "postgresql":
                query = 'UPDATE "EpisodeQueue" SET QueuePosition = 1, QueueDate = CURRENT_TIMESTAMP WHERE UserID = %s AND EpisodeID = %s'
            else:  # MySQL or MariaDB
                query = "UPDATE EpisodeQueue SET QueuePosition = 1, QueueDate = CURRENT_TIMESTAMP WHERE UserID = %s AND EpisodeID = %s"
            cursor.execute(query, (user_id, episode_id))
            cnx.commit()
        else:
            # Episode is not in the queue, insert it at position 1
            if database_type == "postgresql":
                query = 'INSERT INTO "EpisodeQueue" (UserID, EpisodeID, QueuePosition) VALUES (%s, %s, 1)'
            else:  # MySQL or MariaDB
                query = "INSERT INTO EpisodeQueue (UserID, EpisodeID, QueuePosition) VALUES (%s, %s, 1)"
            cursor.execute(query, (user_id, episode_id))
            cnx.commit()

        cursor.close()
        return True
    else:
        # Episode not found in the database
        cursor.close()
        return False


def episode_remove_queue(cnx, database_type, user_id, url, title):
    cursor = cnx.cursor()

    # Get the episode ID using the episode title and URL
    if database_type == "postgresql":
        query = 'SELECT EpisodeID FROM "Episodes" WHERE EpisodeTitle = %s AND EpisodeURL = %s'
    else:
        query = "SELECT EpisodeID FROM Episodes WHERE EpisodeTitle = %s AND EpisodeURL = %s"
    cursor.execute(query, (title, url))
    episode_id = cursor.fetchone()

    if episode_id:
        # Remove the episode from the user's queue

        if database_type == "postgresql":
            query = 'DELETE FROM "EpisodeQueue" WHERE UserID = %s AND EpisodeID = %s'
        else:
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


def check_usernames(cnx, database_type, username):
    cursor = cnx.cursor()
    if database_type == 'postgresql':
        query = ('SELECT COUNT(*) FROM "Users" WHERE Username = %s')
    else:
        query = ("SELECT COUNT(*) FROM Users WHERE Username = %s")
    cursor.execute(query, (username,))
    count = cursor.fetchone()[0]
    cursor.close()
    # cnx.close()
    return count > 0


def record_listen_duration(cnx, database_type, episode_id, user_id, listen_duration):
    if listen_duration < 0:
        logging.info(f"Skipped updating listen duration for user {user_id} and episode {episode_id} due to invalid duration: {listen_duration}")
        return
    print(database_type)
    print(listen_duration)
    listen_date = datetime.datetime.now()
    cursor = cnx.cursor()

    try:
        # Check if UserEpisodeHistory row already exists for the given user and episode
        if database_type == "postgresql":
            cursor.execute('SELECT ListenDuration FROM "UserEpisodeHistory" WHERE UserID=%s AND EpisodeID=%s', (user_id, episode_id))
        else:
            cursor.execute("SELECT ListenDuration FROM UserEpisodeHistory WHERE UserID=%s AND EpisodeID=%s", (user_id, episode_id))
        result = cursor.fetchone()
        print("run result check")
        if result is not None:
            existing_duration = result[0] if isinstance(result, tuple) else result.get("ListenDuration")
            # Ensure existing_duration is not None
            existing_duration = existing_duration if existing_duration is not None else 0
            # Update only if the new duration is greater than the existing duration
            print('post rescd check')
            if listen_duration > existing_duration:
                if database_type == "postgresql":
                    update_listen_duration = 'UPDATE "UserEpisodeHistory" SET ListenDuration=%s, ListenDate=%s WHERE UserID=%s AND EpisodeID=%s'
                else:
                    update_listen_duration = "UPDATE UserEpisodeHistory SET ListenDuration=%s, ListenDate=%s WHERE UserID=%s AND EpisodeID=%s"
                cursor.execute(update_listen_duration, (listen_duration, listen_date, user_id, episode_id))
                print(f"Updated listen duration for user {user_id} and episode {episode_id} to {listen_duration}")
            else:
                print(f"No update required for user {user_id} and episode {episode_id} as existing duration {existing_duration} is greater than or equal to new duration {listen_duration}")
        else:
            # Insert new row
            if database_type == "postgresql":
                add_listen_duration = 'INSERT INTO "UserEpisodeHistory" (UserID, EpisodeID, ListenDate, ListenDuration) VALUES (%s, %s, %s, %s)'
            else:
                add_listen_duration = "INSERT INTO UserEpisodeHistory (UserID, EpisodeID, ListenDate, ListenDuration) VALUES (%s, %s, %s, %s)"
            cursor.execute(add_listen_duration, (user_id, episode_id, listen_date, listen_duration))
            print(f"Inserted new listen duration for user {user_id} and episode {episode_id}: {listen_duration}")

        cnx.commit()
    except Exception as e:
        logging.error(f"Failed to record listen duration due to: {e}")
        cnx.rollback()
    finally:
        cursor.close()
    # cnx.close()


def get_local_episode_times(cnx, database_type, user_id):
    if database_type == "postgresql":
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(dictionary=True)

    # Correct SQL query to fetch all listen durations along with necessary URLs for the given user
    if database_type == "postgresql":
        cursor.execute("""
        SELECT
            e.EpisodeURL,
            p.FeedURL,
            ueh.ListenDuration,
            e.EpisodeDuration
        FROM "UserEpisodeHistory" ueh
        JOIN "Episodes" e ON ueh.EpisodeID = e.EpisodeID
        JOIN "Podcasts" p ON e.PodcastID = p.PodcastID
        WHERE ueh.UserID = %s
        """, (user_id,))  # Ensuring the user_id is passed as a tuple
    else:  # MySQL or MariaDB
        cursor.execute("""
        SELECT
            e.EpisodeURL,
            p.FeedURL,
            ueh.ListenDuration,
            e.EpisodeDuration
        FROM UserEpisodeHistory ueh
        JOIN Episodes e ON ueh.EpisodeID = e.EpisodeID
        JOIN Podcasts p ON e.PodcastID = p.PodcastID
        WHERE ueh.UserID = %s
        """, (user_id,))  # Ensuring the user_id is passed as a tuple

    episode_times = [{
        "episode_url": row["EpisodeURL"] if database_type == "postgresql" else row["EpisodeURL"],
        "podcast_url": row["FeedURL"] if database_type == "postgresql" else row["FeedURL"],
        "listen_duration": row["ListenDuration"] if database_type == "postgresql" else row["ListenDuration"],
        "episode_duration": row["EpisodeDuration"] if database_type == "postgresql" else row["EpisodeDuration"]
    } for row in cursor.fetchall()]

    cursor.close()
    return episode_times



def generate_guid(episode_time):
    import uuid
    # Concatenate the podcast and episode URLs to form a unique string for each episode
    unique_string = episode_time["podcast_url"] + episode_time["episode_url"]
    # Generate a UUID based on the MD5 hash of the unique string
    guid = uuid.uuid3(uuid.NAMESPACE_URL, unique_string)
    return str(guid)


def check_episode_playback(cnx, database_type, user_id, episode_title, episode_url):
    if database_type == "postgresql":
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
    else:  # MySQL or MariaDB
        cursor = cnx.cursor()

    try:
        # Get the EpisodeID from the Episodes table
        if database_type == "postgresql":
            query = """
            SELECT e.EpisodeID
            FROM "Episodes" e
            JOIN "Podcasts" p ON e.PodcastID = p.PodcastID
            WHERE e.EpisodeTitle = %s AND e.EpisodeURL = %s AND p.UserID = %s
            """
        else:  # MySQL or MariaDB
            query = """
            SELECT e.EpisodeID
            FROM Episodes e
            JOIN Podcasts p ON e.PodcastID = p.PodcastID
            WHERE e.EpisodeTitle = %s AND e.EpisodeURL = %s AND p.UserID = %s
            """
        cursor.execute(query, (episode_title, episode_url, user_id))
        result = cursor.fetchone()

        # Check if the EpisodeID is None
        if result is None:
            return False, 0

        episode_id = result['EpisodeID'] if database_type == "postgresql" else result[0]

        # Check if the user has played the episode before
        if database_type == "postgresql":
            query = 'SELECT ListenDuration FROM "UserEpisodeHistory" WHERE UserID = %s AND EpisodeID = %s'
        else:  # MySQL or MariaDB
            query = "SELECT ListenDuration FROM UserEpisodeHistory WHERE UserID = %s AND EpisodeID = %s"
        cursor.execute(query, (user_id, episode_id))
        result = cursor.fetchone()

        if result:
            listen_duration = result['ListenDuration'] if database_type == "postgresql" else result[0]
            return True, listen_duration
        else:
            return False, 0
    except (psycopg.errors.InterfaceError, mysql.connector.errors.InterfaceError):
        return False, 0
    finally:
        if cursor:
            cursor.close()



# def get_episode_listen_time(cnx, user_id, title, url):
#     cursor = None
#     try:
#         cursor = cnx.cursor()

#         # Get the EpisodeID from the Episodes table
#         query = "SELECT EpisodeID FROM Episodes WHERE EpisodeTitle = %s AND EpisodeURL = %s"
#         cursor.execute(query, (title, url))
#         episode_id = cursor.fetchone()[0]

#         # Get the user's listen duration for this episode
#         query = "SELECT ListenDuration FROM UserEpisodeHistory WHERE UserID = %s AND EpisodeID = %s"
#         cursor.execute(query, (user_id, episode_id))
#         listen_duration = cursor.fetchone()[0]

#         return listen_duration

#         # Seek to the user's last listen duration
#         # current_episode.seek_to_second(listen_duration)

#     finally:
#         if cursor:
#             cursor.close()
#             # cnx.close()


def get_theme(cnx, database_type, user_id):
    cursor = None
    try:
        cursor = cnx.cursor()

        # Get the EpisodeID from the Episodes table
        if database_type == 'postgresql':
            query = 'SELECT Theme FROM "UserSettings" WHERE UserID = %s'
        else:
            query = "SELECT Theme FROM UserSettings WHERE UserID = %s"
        cursor.execute(query, (user_id,))
        result = cursor.fetchone()
        # Check the type of the result and access the theme accordingly
        if isinstance(result, dict):
            theme = result["theme"]
        else:
            theme = result[0]

        return theme

    finally:
        if cursor:
            cursor.close()
            # cnx.close()


def set_theme(cnx, database_type, user_id, theme):
    cursor = None
    try:
        cursor = cnx.cursor()

        # Update the UserSettings table with the new theme value
        if database_type == 'postgresql':
            query = 'UPDATE "UserSettings" SET Theme = %s WHERE UserID = %s'
        else:
            query = "UPDATE UserSettings SET Theme = %s WHERE UserID = %s"
        cursor.execute(query, (theme, user_id))
        cnx.commit()

    finally:
        if cursor:
            cursor.close()
            # cnx.close(


def get_user_info(database_type, cnx):
    try:
        if database_type == "postgresql":
            cnx.row_factory = dict_row
            cursor = cnx.cursor()
            query = 'SELECT UserID, Fullname, Username, Email, CASE WHEN IsAdmin THEN 1 ELSE 0 END AS IsAdmin FROM "Users"'
        else:  # MySQL or MariaDB
            cursor = cnx.cursor(dictionary=True)
            query = "SELECT UserID, Fullname, Username, Email, IsAdmin FROM Users"

        cursor.execute(query)
        rows = cursor.fetchall()

        if not rows:
            return None

        if database_type != "postgresql":
            # Convert column names to lowercase for MySQL
            rows = [{k.lower(): v for k, v in row.items()} for row in rows]

        return rows

    except Exception as e:
        print(f"Error getting user info: {e}")
        return None

    finally:
        if cursor:
            cursor.close()




def get_api_info(database_type, cnx, user_id):
    # Check if the user is an admin
    if database_type == "postgresql":
        cursor = cnx.cursor()
        is_admin_query = 'SELECT IsAdmin FROM "Users" WHERE UserID = %s'
    else:  # MySQL or MariaDB
        cursor = cnx.cursor()
        is_admin_query = "SELECT IsAdmin FROM Users WHERE UserID = %s"

    cursor.execute(is_admin_query, (user_id,))
    is_admin_result = cursor.fetchone()
    cursor.close()

    # Adjusting access based on the result type
    is_admin = is_admin_result[0] if isinstance(is_admin_result, tuple) else is_admin_result["isadmin"] if is_admin_result else 0

    # Adjust the query based on whether the user is an admin
    if database_type == "postgresql":
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
        query = (
            'SELECT APIKeyID, "APIKeys".UserID, Username, RIGHT(APIKey, 4) as LastFourDigits, Created '
            'FROM "APIKeys" '
            'JOIN "Users" ON "APIKeys".UserID = "Users".UserID '
        )
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(dictionary=True)
        query = (
            "SELECT APIKeyID, APIKeys.UserID, Username, RIGHT(APIKey, 4) as LastFourDigits, Created "
            "FROM APIKeys "
            "JOIN Users ON APIKeys.UserID = Users.UserID "
        )

    # Append condition to query if the user is not an admin
    if not is_admin:
        if database_type == 'postgresql':
            query += 'WHERE "APIKeys".UserID = %s'
        else:
            query += "WHERE APIKeys.UserID = %s"

    cursor.execute(query, (user_id,) if not is_admin else ())
    rows = cursor.fetchall()
    cursor.close()

    if not rows:
        return []

    if database_type != "postgresql":
        # Convert column names to lowercase for MySQL
        rows = [{k.lower(): v for k, v in row.items()} for row in rows]

    return rows



def create_api_key(cnx, database_type, user_id):
    import secrets
    import string
    alphabet = string.ascii_letters + string.digits
    api_key = ''.join(secrets.choice(alphabet) for _ in range(64))

    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'INSERT INTO "APIKeys" (UserID, APIKey) VALUES (%s, %s)'
    else:  # MySQL or MariaDB
        query = "INSERT INTO APIKeys (UserID, APIKey) VALUES (%s, %s)"

    cursor.execute(query, (user_id, api_key))
    cnx.commit()
    cursor.close()

    return api_key


def is_same_api_key(cnx, database_type, api_id, api_key):
    if database_type == "postgresql":
        cursor = cnx.cursor()
        query = 'SELECT APIKey FROM "APIKeys" WHERE APIKeyID = %s'
    else:  # MySQL or MariaDB
        cursor = cnx.cursor()
        query = "SELECT APIKey FROM APIKeys WHERE APIKeyID = %s"

    cursor.execute(query, (api_id,))
    result = cursor.fetchone()
    cursor.close()

    if result and result[0] == api_key:
        return True
    return False

def belongs_to_guest_user(cnx, database_type, api_id):
    if database_type == "postgresql":
        cursor = cnx.cursor()
        query = 'SELECT UserID FROM "APIKeys" WHERE APIKeyID = %s'
    else:  # MySQL or MariaDB
        cursor = cnx.cursor()
        query = "SELECT UserID FROM APIKeys WHERE APIKeyID = %s"

    cursor.execute(query, (api_id,))
    result = cursor.fetchone()
    cursor.close()
    # Check if the result exists and if the UserID is 1 (guest user)
    return result and result[0] == 1




def delete_api(cnx, database_type, api_id):
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'DELETE FROM "APIKeys" WHERE APIKeyID = %s'
    else:  # MySQL or MariaDB
        query = "DELETE FROM APIKeys WHERE APIKeyID = %s"

    cursor.execute(query, (api_id,))
    cnx.commit()
    cursor.close()



def set_username(cnx, database_type, user_id, new_username):
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'UPDATE "Users" SET Username = %s WHERE UserID = %s'
    else:  # MySQL or MariaDB
        query = "UPDATE Users SET Username = %s WHERE UserID = %s"

    cursor.execute(query, (new_username, user_id))
    cnx.commit()
    cursor.close()



def set_password(cnx, database_type, user_id, hash_pw):
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'UPDATE "Users" SET Hashed_PW = %s WHERE UserID = %s'
    else:  # MySQL or MariaDB
        query = "UPDATE Users SET Hashed_PW = %s WHERE UserID = %s"

    cursor.execute(query, (hash_pw, user_id))
    cnx.commit()
    cursor.close()




def set_email(cnx, database_type, user_id, new_email):
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'UPDATE "Users" SET Email = %s WHERE UserID = %s'
    else:  # MySQL or MariaDB
        query = "UPDATE Users SET Email = %s WHERE UserID = %s"

    cursor.execute(query, (new_email, user_id))
    cnx.commit()
    cursor.close()



def set_fullname(cnx, database_type, user_id, new_name):
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'UPDATE "Users" SET Fullname = %s WHERE UserID = %s'
    else:  # MySQL or MariaDB
        query = "UPDATE Users SET Fullname = %s WHERE UserID = %s"

    cursor.execute(query, (new_name, user_id))
    cnx.commit()
    cursor.close()



def set_isadmin(cnx, database_type, user_id, isadmin):
    cursor = cnx.cursor()

    # Convert boolean isadmin value to integer (0 or 1)
    isadmin_int = int(isadmin)

    if database_type == "postgresql":
        query = 'UPDATE "Users" SET IsAdmin = %s WHERE UserID = %s'
    else:  # MySQL or MariaDB
        query = "UPDATE Users SET IsAdmin = %s WHERE UserID = %s"

    cursor.execute(query, (isadmin_int, user_id))
    cnx.commit()
    cursor.close()



def delete_user(cnx, database_type, user_id):
    cursor = cnx.cursor()

    # Delete user from UserEpisodeHistory table
    try:
        if database_type == "postgresql":
            query = 'DELETE FROM "UserEpisodeHistory" WHERE UserID = %s'
        else:  # MySQL or MariaDB
            query = "DELETE FROM UserEpisodeHistory WHERE UserID = %s"
        cursor.execute(query, (user_id,))
    except Exception as e:
        print(f"Error deleting from UserEpisodeHistory: {e}")

    # Delete user from DownloadedEpisodes table
    try:
        if database_type == "postgresql":
            query = 'DELETE FROM "DownloadedEpisodes" WHERE UserID = %s'
        else:  # MySQL or MariaDB
            query = "DELETE FROM DownloadedEpisodes WHERE UserID = %s"
        cursor.execute(query, (user_id,))
    except Exception as e:
        print(f"Error deleting from DownloadedEpisodes: {e}")

    # Delete user from EpisodeQueue table
    try:
        if database_type == "postgresql":
            query = 'DELETE FROM "EpisodeQueue" WHERE UserID = %s'
        else:  # MySQL or MariaDB
            query = "DELETE FROM EpisodeQueue WHERE UserID = %s"
        cursor.execute(query, (user_id,))
    except Exception as e:
        print(f"Error deleting from EpisodeQueue: {e}")

    # Delete user from Podcasts table
    try:
        if database_type == "postgresql":
            query = 'DELETE FROM "Podcasts" WHERE UserID = %s'
        else:  # MySQL or MariaDB
            query = "DELETE FROM Podcasts WHERE UserID = %s"
        cursor.execute(query, (user_id,))
    except Exception as e:
        print(f"Error deleting from Podcasts: {e}")

    # Delete user from UserSettings table
    try:
        if database_type == "postgresql":
            query = 'DELETE FROM "UserSettings" WHERE UserID = %s'
        else:  # MySQL or MariaDB
            query = "DELETE FROM UserSettings WHERE UserID = %s"
        cursor.execute(query, (user_id,))
    except Exception as e:
        print(f"Error deleting from UserSettings: {e}")

    # Delete user from UserStats table
    try:
        if database_type == "postgresql":
            query = 'DELETE FROM "UserStats" WHERE UserID = %s'
        else:  # MySQL or MariaDB
            query = "DELETE FROM UserStats WHERE UserID = %s"
        cursor.execute(query, (user_id,))
    except Exception as e:
        print(f"Error deleting from UserStats: {e}")

    # Delete user from Users table
    if database_type == "postgresql":
        query = 'DELETE FROM "Users" WHERE UserID = %s'
    else:  # MySQL or MariaDB
        query = "DELETE FROM Users WHERE UserID = %s"
    cursor.execute(query, (user_id,))
    cnx.commit()

    cursor.close()



def user_admin_check(cnx, database_type, user_id):

    logging.info(f"Checking admin status for user ID: {user_id}, database type: {database_type}")
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'SELECT IsAdmin FROM "Users" WHERE UserID = %s'
    else:  # MySQL or MariaDB
        query = "SELECT IsAdmin FROM Users WHERE UserID = %s"

    cursor.execute(query, (user_id,))
    result = cursor.fetchone()
    cursor.close()

    logging.info(f"Query result: {result}")

    if result is None:
        logging.warning(f"No result found for user ID: {user_id}")
        return False

    try:
        return bool(result[0] if isinstance(result, tuple) else result['isadmin'])
    except KeyError as e:
        logging.error(f"KeyError: {e} - Result: {result}")
        return False


def final_admin(cnx, database_type, user_id):
    cursor = cnx.cursor()

    if database_type == "postgresql":
        query = 'SELECT COUNT(*) FROM "Users" WHERE IsAdmin = 1'
    else:  # MySQL or MariaDB
        query = "SELECT COUNT(*) FROM Users WHERE IsAdmin = 1"
    cursor.execute(query)
    admin_count = cursor.fetchone()[0]

    if admin_count == 1:
        if database_type == "postgresql":
            query = 'SELECT IsAdmin FROM "Users" WHERE UserID = %s'
        else:  # MySQL or MariaDB
            query = "SELECT IsAdmin FROM Users WHERE UserID = %s"
        cursor.execute(query, (user_id,))
        is_admin = cursor.fetchone()[0]
        if is_admin == 1:
            return True

    cursor.close()

    return False



def download_status(cnx, database_type):
    if database_type == "postgresql":
        from psycopg.rows import dict_row
        cursor = cnx.cursor(row_factory=dict_row)
        query = 'SELECT DownloadEnabled FROM "AppSettings"'
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(dictionary=True)
        query = "SELECT DownloadEnabled FROM AppSettings"

    cursor.execute(query)
    result = cursor.fetchone()
    cursor.close()

    if result:
        if isinstance(result, dict):
            download_enabled = result.get('DownloadEnabled') or result.get('downloadenabled')
        else:  # Handle the case where result is a tuple
            download_enabled = result[0]

        if download_enabled == 1:
            return True

    return False




def guest_status(cnx, database_type):
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'SELECT Email FROM "Users" WHERE Email = \'active\''
    else:  # MySQL or MariaDB
        query = "SELECT Email FROM Users WHERE Email = 'active'"

    cursor.execute(query)
    result = cursor.fetchone()
    cursor.close()

    if result:
        return True
    else:
        return False


def enable_disable_guest(cnx, database_type):
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'UPDATE "Users" SET Email = CASE WHEN Email = \'inactive\' THEN \'active\' ELSE \'inactive\' END WHERE Username = \'guest\''
    else:  # MySQL or MariaDB
        query = "UPDATE Users SET Email = CASE WHEN Email = 'inactive' THEN 'active' ELSE 'inactive' END WHERE Username = 'guest'"

    cursor.execute(query)
    cnx.commit()
    cursor.close()



def enable_disable_downloads(cnx, database_type):
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'UPDATE "AppSettings" SET DownloadEnabled = CASE WHEN DownloadEnabled = true THEN false ELSE true END'
    else:  # MySQL or MariaDB
        query = "UPDATE AppSettings SET DownloadEnabled = CASE WHEN DownloadEnabled = 1 THEN 0 ELSE 1 END"

    cursor.execute(query)
    cnx.commit()
    cursor.close()




def self_service_status(cnx, database_type):
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'SELECT SelfServiceUser FROM "AppSettings" WHERE SelfServiceUser = TRUE'
    else:  # MySQL or MariaDB
        query = "SELECT SelfServiceUser FROM AppSettings WHERE SelfServiceUser = 1"

    cursor.execute(query)
    result = cursor.fetchone()
    cursor.close()

    return bool(result)


def enable_disable_self_service(cnx, database_type):
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'UPDATE "AppSettings" SET SelfServiceUser = CASE WHEN SelfServiceUser = true THEN false ELSE true END'
    else:  # MySQL or MariaDB
        query = "UPDATE AppSettings SET SelfServiceUser = CASE WHEN SelfServiceUser = 1 THEN 0 ELSE 1 END"

    cursor.execute(query)
    cnx.commit()
    cursor.close()



def verify_api_key(cnx, database_type, passed_key):
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'SELECT * FROM "APIKeys" WHERE APIKey = %s'
    else:
        query = "SELECT * FROM APIKeys WHERE APIKey = %s"
    cursor.execute(query, (passed_key,))
    result = cursor.fetchone()
    cursor.close()
    return True if result else False


def get_api_key(cnx, database_type, username):
    try:
        cursor = cnx.cursor()
        # Get the UserID
        if database_type == "postgresql":
            query = 'SELECT UserID FROM "Users" WHERE username = %s'
        else:  # MySQL or MariaDB
            query = "SELECT UserID FROM Users WHERE username = %s"
        cursor.execute(query, (username,))
        result = cursor.fetchone()

        # Check if a result is returned. If not, return None
        if result is None:
            print("No user found with the provided username.")
            cursor.close()
            return None
        user_id = result[0] if isinstance(result, tuple) else result["userid"]

            # Check the type of the result and access the is_admin value accordingly
    # is_admin = is_admin_result[0] if isinstance(is_admin_result, tuple) else is_admin_result["IsAdmin"] if is_admin_result else 0


        # Get the API Key using the fetched UserID, and limit the results to 1
        if database_type == "postgresql":
            query = 'SELECT APIKey FROM "APIKeys" WHERE UserID = %s LIMIT 1'
        else:  # MySQL or MariaDB
            query = "SELECT APIKey FROM APIKeys WHERE UserID = %s LIMIT 1"
        cursor.execute(query, (user_id,))
        result = cursor.fetchone()

        cursor.close()

        # Check and return the API key or create a new one if not found
        if result:
            api_key = result[0] if isinstance(result, tuple) else result["apikey"]
            print(f"Result: {api_key}")
            return api_key # Adjust the index if the API key is in a different column
        else:
            print("No API key found for the provided user. Creating a new one...")
            return create_api_key(cnx, database_type, user_id)

    except Exception as e:
        print(f"An error occurred: {str(e)}")
        return f"An error occurred: {str(e)}"


def get_api_user(cnx, database_type, api_key):
    try:
        cursor = cnx.cursor()
        if database_type == "postgresql":
            query = 'SELECT UserID FROM "APIKeys" WHERE APIKey = %s LIMIT 1'
        else:  # MySQL or MariaDB
            query = "SELECT UserID FROM APIKeys WHERE APIKey = %s LIMIT 1"

        cursor.execute(query, (api_key,))
        result = cursor.fetchone()

        cursor.close()

        if result:
            user_id = result[0] if isinstance(result, tuple) else result['userid']
            print(f"Result: {user_id}")
            return user_id  # Adjust the index if the API key is in a different column
        else:
            print(f"ApiKey Not Found")
            return "ApiKey Not Found"

    except Exception as e:
        print(f"An error occurred: {str(e)}")
        return f"An error occurred: {str(e)}"



def id_from_api_key(cnx, database_type, passed_key):
    logging.info(f"Fetching user ID for API key: {passed_key}")

    if database_type == "postgresql":
        cursor = cnx.cursor()  # psycopg3 default cursor should be fine here
    else:
        cursor = cnx.cursor()

    try:
        if database_type == "postgresql":
            query = 'SELECT UserID FROM "APIKeys" WHERE APIKey = %s'
        else:
            query = "SELECT UserID FROM APIKeys WHERE APIKey = %s"

        cursor.execute(query, (passed_key,))
        result = cursor.fetchone()
        logging.info(f"Query result: {result}")

        if result:
            # Ensure accessing the first element of the tuple
            if database_type == "postgresql":
                user_id = result[0] if isinstance(result, tuple) else result['userid']
            else:
                user_id = result[0] if isinstance(result, tuple) else result['UserID']
            logging.info(f"Found user ID: {user_id} for API key: {passed_key}")
            return user_id
        else:
            logging.warning(f"No user ID found for API key: {passed_key}")
            return None
    except Exception as e:
        logging.error(f"Error fetching user ID for API key: {passed_key}, error: {e}")
        return None
    finally:
        cursor.close()




# def check_api_permission(cnx, passed_key):
#     import tempfile
#     # Create a temporary file to store the content. This is because the mysql command reads from a file.
#     with tempfile.NamedTemporaryFile(mode='w+', delete=True) as tempf:
#         tempf.write(server_restore_data)
#         tempf.flush()
#         cmd = [
#             "mysql",
#             "-h", 'db',
#             "-P", '3306',
#             "-u", "root",
#             "-p" + database_pass,
#             "pypods_database"
#         ]

#         # Use the file's content as input for the mysql command
#         with open(tempf.name, 'r') as file:
#             process = subprocess.Popen(cmd, stdin=file, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
#             stdout, stderr = process.communicate()

#             if process.returncode != 0:
#                 raise Exception(f"Restoration failed with error: {stderr.decode()}")

#     return "Restoration completed successfully!"


def get_stats(cnx, database_type, user_id):
    logging.info(f"Fetching stats for user ID: {user_id}, database type: {database_type}")
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'SELECT UserCreated, PodcastsPlayed, TimeListened, PodcastsAdded, EpisodesSaved, EpisodesDownloaded FROM "UserStats" WHERE UserID = %s'
    else:  # MySQL or MariaDB
        query = "SELECT UserCreated, PodcastsPlayed, TimeListened, PodcastsAdded, EpisodesSaved, EpisodesDownloaded FROM UserStats WHERE UserID = %s"
    print('gettings stats')
    cursor.execute(query, (user_id,))
    results = cursor.fetchall()
    cursor.close()
    print(f'stats {results}')
    logging.info(f"Query results: {results}")

    if not results:
        logging.warning(f"No stats found for user ID: {user_id}")
        return None

    result = results[0]
    if database_type == "postgresql":
        stats = {
            "UserCreated": result['usercreated'],
            "PodcastsPlayed": result['podcastsplayed'],
            "TimeListened": result['timelistened'],
            "PodcastsAdded": result['podcastsadded'],
            "EpisodesSaved": result['episodessaved'],
            "EpisodesDownloaded": result['episodesdownloaded']
        }
    else:  # MySQL or MariaDB
        stats = {
            "UserCreated": result[0],
            "PodcastsPlayed": result[1],
            "TimeListened": result[2],
            "PodcastsAdded": result[3],
            "EpisodesSaved": result[4],
            "EpisodesDownloaded": result[5]
        }
    logging.info(f"Fetched stats: {stats}")

    return stats



def saved_episode_list(database_type, cnx, user_id):
    if database_type == "postgresql":
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
        query = (
            'SELECT "Podcasts".PodcastName, "Episodes".EpisodeTitle, "Episodes".EpisodePubDate, '
            '"Episodes".EpisodeDescription, "Episodes".EpisodeID, "Episodes".EpisodeArtwork, "Episodes".EpisodeURL, '
            '"Episodes".EpisodeDuration, "Podcasts".WebsiteURL, "UserEpisodeHistory".ListenDuration, "Episodes".Completed '
            'FROM "SavedEpisodes" '
            'INNER JOIN "Episodes" ON "SavedEpisodes".EpisodeID = "Episodes".EpisodeID '
            'INNER JOIN "Podcasts" ON "Episodes".PodcastID = "Podcasts".PodcastID '
            'LEFT JOIN "UserEpisodeHistory" ON "SavedEpisodes".EpisodeID = "UserEpisodeHistory".EpisodeID AND "UserEpisodeHistory".UserID = %s '
            'WHERE "SavedEpisodes".UserID = %s '
            'ORDER BY "SavedEpisodes".SaveDate DESC'
        )
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(dictionary=True)
        query = (
            "SELECT Podcasts.PodcastName, Episodes.EpisodeTitle, Episodes.EpisodePubDate, "
            "Episodes.EpisodeDescription, Episodes.EpisodeID, Episodes.EpisodeArtwork, Episodes.EpisodeURL, "
            "Episodes.EpisodeDuration, Podcasts.WebsiteURL, UserEpisodeHistory.ListenDuration, Episodes.Completed "
            "FROM SavedEpisodes "
            "INNER JOIN Episodes ON SavedEpisodes.EpisodeID = Episodes.EpisodeID "
            "INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
            "LEFT JOIN UserEpisodeHistory ON SavedEpisodes.EpisodeID = UserEpisodeHistory.EpisodeID AND UserEpisodeHistory.UserID = %s "
            "WHERE SavedEpisodes.UserID = %s "
            "ORDER BY SavedEpisodes.SaveDate DESC"
        )

    cursor.execute(query, (user_id, user_id))
    rows = cursor.fetchall()

    cursor.close()

    if not rows:
        return None

    saved_episodes = lowercase_keys(rows)

    return saved_episodes


def save_episode(cnx, database_type, episode_id, user_id):
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'INSERT INTO "SavedEpisodes" (UserID, EpisodeID) VALUES (%s, %s)'
    else:  # MySQL or MariaDB
        query = "INSERT INTO SavedEpisodes (UserID, EpisodeID) VALUES (%s, %s)"
    cursor.execute(query, (user_id, episode_id))

    # Update UserStats table to increment EpisodesSaved count
    if database_type == "postgresql":
        query = 'UPDATE "UserStats" SET EpisodesSaved = EpisodesSaved + 1 WHERE UserID = %s'
    else:  # MySQL or MariaDB
        query = "UPDATE UserStats SET EpisodesSaved = EpisodesSaved + 1 WHERE UserID = %s"
    cursor.execute(query, (user_id,))

    cnx.commit()
    cursor.close()

    return True



def check_saved(cnx, database_type, user_id, episode_id):
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            query = 'SELECT * FROM "SavedEpisodes" WHERE UserID = %s AND EpisodeID = %s'
        else:  # MySQL or MariaDB
            query = "SELECT * FROM SavedEpisodes WHERE UserID = %s AND EpisodeID = %s"
        cursor.execute(query, (user_id, episode_id))
        result = cursor.fetchone()

        return bool(result)
    except Exception as err:
        print("Error checking saved episode: {}".format(err))
        return False
    finally:
        cursor.close()

            # cnx.close()


def get_saved_value(result, key, default=None):
    """
    Helper function to extract value from result set.
    It handles both dictionaries and tuples.
    """
    key_lower = key.lower()
    if isinstance(result, dict):
        return result.get(key_lower, default)
    elif isinstance(result, tuple):
        # Define a mapping of field names to their tuple indices for your specific queries
        key_map = {
            "saveid": 0
        }
        index = key_map.get(key_lower)
        return result[index] if index is not None else default
    return default


def remove_saved_episode(cnx, database_type, episode_id, user_id):
    cursor = cnx.cursor()

    try:
        if database_type == "postgresql":
            query = (
                'SELECT SaveID '
                'FROM "SavedEpisodes" '
                'INNER JOIN "Episodes" ON "SavedEpisodes".EpisodeID = "Episodes".EpisodeID '
                'INNER JOIN "Podcasts" ON "Episodes".PodcastID = "Podcasts".PodcastID '
                'WHERE "Episodes".EpisodeID = %s AND "Podcasts".UserID = %s'
            )
        else:  # MySQL or MariaDB
            query = (
                "SELECT SaveID "
                "FROM SavedEpisodes "
                "INNER JOIN Episodes ON SavedEpisodes.EpisodeID = Episodes.EpisodeID "
                "INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
                "WHERE Episodes.EpisodeID = %s AND Podcasts.UserID = %s"
            )
        logging.debug(f"Executing query: {query} with EpisodeID: {episode_id} and UserID: {user_id}")
        cursor.execute(query, (episode_id, user_id))
        result = cursor.fetchone()

        logging.debug(f"Query result: {result}")

        if not result:
            logging.warning("No matching episode found.")
            cursor.close()
            return

        save_id = get_saved_value(result, "SaveID")

        # Remove the entry from the SavedEpisodes table
        if database_type == "postgresql":
            query = 'DELETE FROM "SavedEpisodes" WHERE SaveID = %s'
        else:  # MySQL or MariaDB
            query = "DELETE FROM SavedEpisodes WHERE SaveID = %s"
        cursor.execute(query, (save_id,))

        # Update UserStats table to decrement EpisodesSaved count
        if database_type == "postgresql":
            query = 'UPDATE "UserStats" SET EpisodesSaved = EpisodesSaved - 1 WHERE UserID = %s'
        else:  # MySQL or MariaDB
            query = "UPDATE UserStats SET EpisodesSaved = EpisodesSaved - 1 WHERE UserID = %s"
        cursor.execute(query, (user_id,))

        cnx.commit()
        logging.info(f"Removed {cursor.rowcount} entry from the SavedEpisodes table.")

    except Exception as e:
        logging.error(f"Error during episode removal: {e}")
        cnx.rollback()
    finally:
        cursor.close()


def increment_played(cnx, database_type, user_id):
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'UPDATE "UserStats" SET PodcastsPlayed = PodcastsPlayed + 1 WHERE UserID = %s'
    else:  # MySQL or MariaDB
        query = "UPDATE UserStats SET PodcastsPlayed = PodcastsPlayed + 1 WHERE UserID = %s"
    cursor.execute(query, (user_id,))
    cnx.commit()
    cursor.close()

def increment_listen_time(cnx, database_type, user_id):
    cursor = cnx.cursor()

    # Update UserStats table to increment PodcastsPlayed count
    if database_type == "postgresql":
        query = ('UPDATE "UserStats" SET TimeListened = TimeListened + 1 '
                "WHERE UserID = %s")
    else:
        query = ("UPDATE UserStats SET TimeListened = TimeListened + 1 "
                "WHERE UserID = %s")
    cursor.execute(query, (user_id,))
    cnx.commit()

    cursor.close()
    # cnx.close()



def get_user_episode_count(cnx, database_type, user_id):
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = (
            'SELECT COUNT(*) '
            'FROM "Episodes" '
            'INNER JOIN "Podcasts" ON "Episodes".PodcastID = "Podcasts".PodcastID '
            'WHERE "Podcasts".UserID = %s'
        )
    else:  # MySQL or MariaDB
        query = (
            "SELECT COUNT(*) "
            "FROM Episodes "
            "INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
            "WHERE Podcasts.UserID = %s"
        )

    cursor.execute(query, (user_id,))
    episode_count = cursor.fetchone()[0]
    cursor.close()

    return episode_count



def get_user_episode_count(cnx, database_type, user_id):
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = (
            'SELECT COUNT(*) '
            'FROM "Episodes" '
            'INNER JOIN "Podcasts" ON "Episodes".PodcastID = "Podcasts".PodcastID '
            'WHERE "Podcasts".UserID = %s'
        )
    else:  # MySQL or MariaDB
        query = (
            "SELECT COUNT(*) "
            "FROM Episodes "
            "INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
            "WHERE Podcasts.UserID = %s"
        )

    cursor.execute(query, (user_id,))
    episode_count = cursor.fetchone()[0]
    cursor.close()

    return episode_count


def check_podcast(cnx, database_type, user_id, podcast_name, podcast_url):
    cursor = None
    try:
        cursor = cnx.cursor()
        if database_type == "postgresql":
            query = 'SELECT PodcastID FROM "Podcasts" WHERE UserID = %s AND PodcastName = %s AND FeedURL = %s'
        else:  # MySQL or MariaDB
            query = "SELECT PodcastID FROM Podcasts WHERE UserID = %s AND PodcastName = %s AND FeedURL = %s"

        cursor.execute(query, (user_id, podcast_name, podcast_url))

        return cursor.fetchone() is not None
    except Exception:
        return False
    finally:
        if cursor:
            cursor.close()


# def get_session_file_path():
#     app_name = 'pinepods'
#     data_dir = appdirs.user_data_dir(app_name)
#     os.makedirs(data_dir, exist_ok=True)
#     session_file_path = os.path.join(data_dir, "session.txt")
#     return session_file_path


# def save_session_to_file(session_id):
#     session_file_path = get_session_file_path()
#     with open(session_file_path, "w") as file:
#         file.write(session_id)


# def get_saved_session_from_file():
#     app_name = 'pinepods'
#     session_file_path = get_session_file_path()
#     try:
#         with open(session_file_path, "r") as file:
#             session_id = file.read()
#             return session_id
#     except FileNotFoundError:
#         return None


def check_saved_session(cnx, database_type, session_value):
    cursor = cnx.cursor()

    if database_type == "postgresql":
        query = 'SELECT UserID, expire FROM "Sessions" WHERE value = %s'
    else:  # MySQL or MariaDB
        query = "SELECT UserID, expire FROM Sessions WHERE value = %s"

    cursor.execute(query, (session_value,))
    result = cursor.fetchone()

    if result:
        user_id, session_expire = result
        current_time = datetime.datetime.now()
        if current_time < session_expire:
            cursor.close()
            return user_id

    cursor.close()
    return False



# def check_saved_web_session(cnx, session_value):
#     cursor = cnx.cursor()

#     # Get the session with the matching value and expiration time
#     cursor.execute("""
#     SELECT UserID, expire FROM Sessions WHERE value = %s;
#     """, (session_value,))

#     result = cursor.fetchone()

#     if result:
#         user_id, session_expire = result
#         current_time = datetime.datetime.now()
#         if current_time < session_expire:
#             return user_id

#     return False
#     cursor.close()
#     # cnx.close()


def create_session(cnx, database_type, user_id, session_value):
    expire_date = datetime.datetime.now() + datetime.timedelta(days=30)

    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'INSERT INTO "Sessions" (UserID, value, expire) VALUES (%s, %s, %s)'
    else:  # MySQL or MariaDB
        query = "INSERT INTO Sessions (UserID, value, expire) VALUES (%s, %s, %s)"

    cursor.execute(query, (user_id, session_value, expire_date))
    cnx.commit()
    cursor.close()



# def create_web_session(cnx, user_id, session_value):
#     # Calculate the expiration date 30 days in the future
#     expire_date = datetime.datetime.now() + datetime.timedelta(days=30)

#     # Insert the new session into the Sessions table
#     cursor = cnx.cursor()
#     cursor.execute("""
#     INSERT INTO Sessions (UserID, value, expire) VALUES (%s, %s, %s);
#     """, (user_id, session_value, expire_date))

#     cnx.commit()
#     cursor.close()
#     # cnx.close()


def clean_expired_sessions(cnx, database_type):
    current_time = datetime.datetime.now()
    cursor = cnx.cursor()

    if database_type == "postgresql":
        query = 'DELETE FROM "Sessions" WHERE "expire" < %s'
    else:  # MySQL or MariaDB
        query = "DELETE FROM Sessions WHERE expire < %s"

    cursor.execute(query, (current_time,))
    cnx.commit()
    cursor.close()



# def user_exists(cnx, username):
#     cursor = cnx.cursor()
#     query = "SELECT COUNT(*) FROM Users WHERE Username = %s"
#     cursor.execute(query, (username,))
#     count = cursor.fetchone()[0]
#     cursor.close()
#     # cnx.close()
#     return count > 0


def reset_password_create_code(cnx, database_type, user_email):
    reset_code = ''.join(random.choices(string.ascii_uppercase + string.digits, k=6))
    cursor = cnx.cursor()

    # Check if a user with this email exists
    if database_type == "postgresql":
        check_query = """
            SELECT UserID
            FROM "Users"
            WHERE Email = %s
        """
    else:
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

    if database_type == "postgresql":
        update_query = """
            UPDATE "Users"
            SET Reset_Code = %s,
                Reset_Expiry = %s
            WHERE Email = %s
        """
    else:
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

def reset_password_remove_code(cnx, database_type, email):
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'UPDATE "Users" SET Reset_Code = NULL, Reset_Expiry = NULL WHERE Email = %s'
    else:
        query = "UPDATE Users SET Reset_Code = NULL, Reset_Expiry = NULL WHERE Email = %s"
    cursor.execute(query, (email,))
    cnx.commit()
    return cursor.rowcount > 0


def verify_password(cnx, database_type, username: str, password: str) -> bool:
    cursor = cnx.cursor()
    if database_type == "postgresql":
        cursor.execute('SELECT Hashed_PW FROM "Users" WHERE Username = %s', (username,))
    else:
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


def verify_reset_code(cnx, database_type, user_email, reset_code):
    cursor = cnx.cursor()

    if database_type == "postgresql":
        select_query = """
            SELECT Reset_Code, Reset_Expiry
            FROM "Users"
            WHERE Email = %s
        """
    else:
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

def check_reset_user(cnx, database_type, username, email):
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'SELECT * FROM "Users" WHERE Username = %s AND Email = %s'
    else:
        query = "SELECT * FROM Users WHERE Username = %s AND Email = %s"
    cursor.execute(query, (username, email))
    result = cursor.fetchone()
    return result is not None


def reset_password_prompt(cnx, database_type, user_email, hashed_pw):
    cursor = cnx.cursor()
    if database_type == "postgresql":
        update_query = """
            UPDATE "Users"
            SET Hashed_PW = %s,
                Reset_Code = NULL,
                Reset_Expiry = NULL
            WHERE Email = %s
        """
    else:
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


def clear_guest_data(cnx, database_type):
    cursor = cnx.cursor()

    # First delete all the episodes associated with the guest user
    if database_type == "postgresql":
        delete_episodes_query = """
            DELETE Episodes
            FROM "Episodes"
            INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID
            WHERE Podcasts.UserID = 1
        """
    else:
        delete_episodes_query = """
            DELETE Episodes
            FROM Episodes
            INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID
            WHERE Podcasts.UserID = 1
        """
    cursor.execute(delete_episodes_query)

    # Then delete all the podcasts associated with the guest user
    if database_type == "postgresql":
        delete_podcasts_query = """
            DELETE FROM "Podcasts"
            WHERE UserID = 1
        """
    else:
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
        from psycopg.rows import dict_row
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
        query = (
            'SELECT "Podcasts".PodcastID, "Podcasts".PodcastName, "Podcasts".ArtworkURL, "Episodes".EpisodeTitle, "Episodes".EpisodePubDate, '
            '"Episodes".EpisodeDescription, "Episodes".EpisodeArtwork, "Episodes".EpisodeURL, "Episodes".EpisodeDuration, "Episodes".EpisodeID, '
            '"Podcasts".WebsiteURL, "UserEpisodeHistory".ListenDuration '
            'FROM "Episodes" '
            'INNER JOIN "Podcasts" ON "Episodes".PodcastID = "Podcasts".PodcastID '
            'LEFT JOIN "UserEpisodeHistory" ON "Episodes".EpisodeID = "UserEpisodeHistory".EpisodeID AND "Podcasts".UserID = "UserEpisodeHistory".UserID '
            'WHERE "Episodes".EpisodeID = %s AND "Podcasts".UserID = %s'
        )
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(dictionary=True)
        query = (
            "SELECT Podcasts.PodcastID, Podcasts.PodcastName, Podcasts.ArtworkURL, Episodes.EpisodeTitle, Episodes.EpisodePubDate, "
            "Episodes.EpisodeDescription, Episodes.EpisodeArtwork, Episodes.EpisodeURL, Episodes.EpisodeDuration, Episodes.EpisodeID, "
            "Podcasts.WebsiteURL, UserEpisodeHistory.ListenDuration "
            "FROM Episodes "
            "INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
            "LEFT JOIN UserEpisodeHistory ON Episodes.EpisodeID = UserEpisodeHistory.EpisodeID AND Podcasts.UserID = UserEpisodeHistory.UserID "
            "WHERE Episodes.EpisodeID = %s AND Podcasts.UserID = %s"
        )

    cursor.execute(query, (episode_id, user_id))
    row = cursor.fetchone()

    cursor.close()

    if not row:
        raise ValueError(f"No episode found with ID {episode_id} for user {user_id}")

    lower_row = lowercase_keys(row)

    return lower_row


import logging

def save_mfa_secret(database_type, cnx, user_id, mfa_secret):
    if database_type == "postgresql":
        cursor = cnx.cursor()
        query = 'UPDATE "Users" SET MFA_Secret = %s WHERE UserID = %s'
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(dictionary=True)
        query = "UPDATE Users SET MFA_Secret = %s WHERE UserID = %s"

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
        cursor = cnx.cursor()
        query = 'SELECT MFA_Secret FROM "Users" WHERE UserID = %s'
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(dictionary=True)
        query = "SELECT MFA_Secret FROM Users WHERE UserID = %s"

    try:
        cursor.execute(query, (user_id,))
        result = cursor.fetchone()
        cursor.close()

        if result is None:
            return False

        # For PostgreSQL, the column name will be 'mfa_secret' in lowercase
        # For MySQL, the column name might be 'MFA_Secret' so we access it using lowercase
        if database_type != "postgresql":
            result = {k.lower(): v for k, v in result.items()}

        mfa_secret = result[0] if isinstance(result, tuple) else result.get('mfa_secret')
        return bool(mfa_secret)
    except Exception as e:
        print("Error checking MFA status:", e)
        return False




def get_mfa_secret(database_type, cnx, user_id):
    if database_type == "postgresql":
        cursor = cnx.cursor()
        query = 'SELECT MFA_Secret FROM "Users" WHERE UserID = %s'
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(dictionary=True)
        query = "SELECT MFA_Secret FROM Users WHERE UserID = %s"

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
        cursor = cnx.cursor()
        query = 'UPDATE "Users" SET MFA_Secret = NULL WHERE UserID = %s'
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(dictionary=True)
        query = "UPDATE Users SET MFA_Secret = NULL WHERE UserID = %s"

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
        from psycopg.rows import dict_row
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
        query = (
            'SELECT * FROM "Episodes" INNER JOIN "Podcasts" ON "Episodes".PodcastID = "Podcasts".PodcastID WHERE "Podcasts".FeedURL = %s'
        )
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(dictionary=True)
        query = (
            "SELECT * FROM Episodes INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID WHERE Podcasts.FeedURL = %s"
        )

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
        from psycopg.rows import dict_row
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
        query = (
            'DELETE FROM "UserEpisodeHistory" '
            'WHERE UserID = %s AND EpisodeID IN ('
            'SELECT EpisodeID FROM "Episodes" '
            'WHERE EpisodeURL = %s AND EpisodeTitle = %s)'
        )
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(dictionary=True)
        query = (
            "DELETE FROM UserEpisodeHistory "
            "WHERE UserID = %s AND EpisodeID IN ("
            "SELECT EpisodeID FROM Episodes "
            "WHERE EpisodeURL = %s AND EpisodeTitle = %s)"
        )

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
        cursor = cnx.cursor()
        query = (
            'UPDATE "Users" SET Timezone = %s, TimeFormat = %s, DateFormat = %s, FirstLogin = %s WHERE UserID = %s'
        )
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(dictionary=True)
        query = (
            "UPDATE Users SET Timezone = %s, TimeFormat = %s, DateFormat = %s, FirstLogin = %s WHERE UserID = %s"
        )

    try:
        if database_type == "postgresql":
            cursor.execute(query, (timezone, hour_pref, date_format, True, user_id))
        else:
            cursor.execute(query, (timezone, hour_pref, date_format, 1, user_id))
        cnx.commit()
        cursor.close()

        return True
    except Exception as e:
        print("Error setting up time info:", e)
        return False



def get_time_info(database_type, cnx, user_id):
    if database_type == "postgresql":
        from psycopg.rows import dict_row
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
        query = 'SELECT Timezone, TimeFormat, DateFormat FROM "Users" WHERE UserID = %s'
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(dictionary=True)
        query = "SELECT Timezone, TimeFormat, DateFormat FROM Users WHERE UserID = %s"

    cursor.execute(query, (user_id,))
    result = cursor.fetchone()
    cursor.close()

    if result:
        if database_type == "postgresql":
            return result['timezone'], result['timeformat'], result['dateformat']
        else:
            return result['Timezone'], result['TimeFormat'], result['DateFormat']
    else:
        return None, None, None



def first_login_done(database_type, cnx, user_id):
    if database_type == "postgresql":
        from psycopg.rows import dict_row
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
        query = 'SELECT FirstLogin FROM "Users" WHERE UserID = %s'
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(dictionary=True)
        query = "SELECT FirstLogin FROM Users WHERE UserID = %s"

    try:
        cursor.execute(query, (user_id,))
        result = cursor.fetchone()
        cursor.close()

        first_login = result[0] if isinstance(result, tuple) else result['firstlogin']
        return first_login == 1
    except Exception as e:
        print("Error fetching first login status:", e)
        return False



def delete_selected_episodes(cnx, database_type, selected_episodes, user_id):
    cursor = cnx.cursor()
    for episode_id in selected_episodes:
        # Get the download ID and location from the DownloadedEpisodes table
        query = (
            'SELECT DownloadID, DownloadedLocation '
            'FROM "DownloadedEpisodes" '
            'WHERE EpisodeID = %s AND UserID = %s' if database_type == "postgresql" else
            "SELECT DownloadID, DownloadedLocation "
            "FROM DownloadedEpisodes "
            "WHERE EpisodeID = %s AND UserID = %s"
        )
        cursor.execute(query, (episode_id, user_id))
        result = cursor.fetchone()

        if not result:
            print(f"No matching download found for episode ID {episode_id}")
            continue

        download_id, downloaded_location = result

        # Delete the downloaded file
        os.remove(downloaded_location)

        # Remove the entry from the DownloadedEpisodes table
        query = (
            'DELETE FROM "DownloadedEpisodes" WHERE DownloadID = %s' if database_type == "postgresql" else
            "DELETE FROM DownloadedEpisodes WHERE DownloadID = %s"
        )
        cursor.execute(query, (download_id,))
        cnx.commit()
        print(f"Removed {cursor.rowcount} entry from the DownloadedEpisodes table.")

        # Update UserStats table to decrement EpisodesDownloaded count
        query = (
            'UPDATE "UserStats" SET EpisodesDownloaded = EpisodesDownloaded - 1 '
            'WHERE UserID = %s' if database_type == "postgresql" else
            "UPDATE UserStats SET EpisodesDownloaded = EpisodesDownloaded - 1 WHERE UserID = %s"
        )
        cursor.execute(query, (user_id,))

    cursor.close()

    return "success"



def delete_selected_podcasts(cnx, database_type, delete_list, user_id):
    cursor = cnx.cursor()
    for podcast_id in delete_list:
        # Get the download IDs and locations from the DownloadedEpisodes table
        query = (
            'SELECT "DownloadedEpisodes".DownloadID, "DownloadedEpisodes".DownloadedLocation '
            'FROM "DownloadedEpisodes" '
            'INNER JOIN "Episodes" ON "DownloadedEpisodes".EpisodeID = "Episodes".EpisodeID '
            'WHERE "Episodes".PodcastID = %s AND "DownloadedEpisodes".UserID = %s' if database_type == "postgresql" else
            "SELECT DownloadedEpisodes.DownloadID, DownloadedEpisodes.DownloadedLocation "
            "FROM DownloadedEpisodes "
            "INNER JOIN Episodes ON DownloadedEpisodes.EpisodeID = Episodes.EpisodeID "
            "WHERE Episodes.PodcastID = %s AND DownloadedEpisodes.UserID = %s"
        )
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
            query = (
                'DELETE FROM "DownloadedEpisodes" WHERE DownloadID = %s' if database_type == "postgresql" else
                "DELETE FROM DownloadedEpisodes WHERE DownloadID = %s"
            )
            cursor.execute(query, (download_id,))
            cnx.commit()
            print(f"Removed {cursor.rowcount} entry from the DownloadedEpisodes table.")

            # Update UserStats table to decrement EpisodesDownloaded count
            query = (
                'UPDATE "UserStats" SET EpisodesDownloaded = EpisodesDownloaded - 1 '
                'WHERE UserID = %s' if database_type == "postgresql" else
                "UPDATE UserStats SET EpisodesDownloaded = EpisodesDownloaded - 1 WHERE UserID = %s"
            )
            cursor.execute(query, (user_id,))

    cursor.close()
    return "success"



import time



def search_data(database_type, cnx, search_term, user_id):
    if database_type == "postgresql":
        from psycopg.rows import dict_row
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
        query = (
            'SELECT * FROM "Podcasts" '
            'INNER JOIN "Episodes" ON "Podcasts".PodcastID = "Episodes".PodcastID '
            'WHERE "Podcasts".UserID = %s AND '
            '"Episodes".EpisodeTitle ILIKE %s'
        )
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(dictionary=True)
        query = (
            "SELECT * FROM Podcasts "
            "INNER JOIN Episodes ON Podcasts.PodcastID = Episodes.PodcastID "
            "WHERE Podcasts.UserID = %s AND "
            "Episodes.EpisodeTitle LIKE %s"
        )

    # Add wildcards for the LIKE clause
    search_term = f"%{search_term}%"

    try:
        start = time.time()
        logging.info(f"Executing query: {query}")
        logging.info(f"Search term: {search_term}, User ID: {user_id}")
        cursor.execute(query, (user_id, search_term))
        result = cursor.fetchall()
        end = time.time()
        logging.info(f"Query executed in {end - start} seconds.")
        logging.info(f"Query result: {result}")
        cursor.close()

        if not result:
            return []

        # Convert column names to lowercase for MySQL
        result = lowercase_keys(result)

        # Post-process the results to cast boolean to integer for the 'explicit' field
        if database_type == "postgresql":
            for row in result:
                if 'explicit' in row:
                    row['explicit'] = 1 if row['explicit'] else 0

        return result
    except Exception as e:
        logging.error(f"Error retrieving Podcast Episodes: {e}")
        return None



def queue_pod(database_type, cnx, episode_id, user_id):
    if database_type == "postgresql":
        from psycopg.rows import dict_row
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
        query_get_max_pos = (
            'SELECT MAX(QueuePosition) AS max_pos FROM "EpisodeQueue" '
            'WHERE UserID = %s'
        )
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(dictionary=True)
        query_get_max_pos = (
            "SELECT MAX(QueuePosition) AS max_pos FROM EpisodeQueue "
            "WHERE UserID = %s"
        )

    cursor.execute(query_get_max_pos, (user_id,))
    result = cursor.fetchone()
    max_pos = result['max_pos'] if result['max_pos'] else 0

    # Insert the new episode into the queue
    query_queue_pod = (
        'INSERT INTO "EpisodeQueue"(UserID, EpisodeID, QueuePosition) '
        'VALUES (%s, %s, %s)' if database_type == "postgresql" else
        "INSERT INTO EpisodeQueue(UserID, EpisodeID, QueuePosition) "
        "VALUES (%s, %s, %s)"
    )
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

    return "Podcast Episode queued successfully."


def check_queued(database_type, cnx, episode_id, user_id):
    if database_type == "postgresql":
        from psycopg.rows import dict_row
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
        query = """
        SELECT * FROM "EpisodeQueue"
        WHERE EpisodeID = %s AND UserID = %s
        """
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(dictionary=True)
        query = """
        SELECT * FROM EpisodeQueue
        WHERE EpisodeID = %s AND UserID = %s
        """

    cursor.execute(query, (episode_id, user_id))
    result = cursor.fetchone()
    cursor.close()

    return True if result else False


def get_queue_value(result, key, default=None):
    """
    Helper function to extract value from result set.
    It handles both dictionaries and tuples.
    """
    key_lower = key.lower()
    if isinstance(result, dict):
        return result.get(key_lower, default)
    elif isinstance(result, tuple):
        # Define a mapping of field names to their tuple indices for your specific queries
        key_map = {
            "episodeid": 0,
            "queueposition": 1
        }
        index = key_map.get(key_lower)
        return result[index] if index is not None else default
    return default


def remove_queued_pod(database_type, cnx, episode_id, user_id):
    if database_type == "postgresql":
        from psycopg.rows import dict_row
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
        get_queue_data_query = """
        SELECT "EpisodeQueue".EpisodeID, "EpisodeQueue".QueuePosition
        FROM "EpisodeQueue"
        INNER JOIN "Episodes" ON "EpisodeQueue".EpisodeID = "Episodes".EpisodeID
        WHERE "Episodes".EpisodeID = %s AND "EpisodeQueue".UserID = %s
        """
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(dictionary=True)
        get_queue_data_query = """
        SELECT EpisodeQueue.EpisodeID, EpisodeQueue.QueuePosition
        FROM EpisodeQueue
        INNER JOIN Episodes ON EpisodeQueue.EpisodeID = Episodes.EpisodeID
        WHERE Episodes.EpisodeID = %s AND EpisodeQueue.UserID = %s
        """

    cursor.execute(get_queue_data_query, (episode_id, user_id))
    queue_data = cursor.fetchone()

    logging.debug(f"Queue data: {queue_data}")

    if queue_data is None:
        logging.warning(f"No queued episode found with ID {episode_id}")
        cursor.close()
        return None

    # Handle both dictionary and tuple results
    episode_id = get_queue_value(queue_data, "EpisodeID")
    removed_queue_position = get_queue_value(queue_data, "QueuePosition")

    delete_query = (
        'DELETE FROM "EpisodeQueue" WHERE UserID = %s AND EpisodeID = %s' if database_type == "postgresql" else
        "DELETE FROM EpisodeQueue WHERE UserID = %s AND EpisodeID = %s"
    )
    cursor.execute(delete_query, (user_id, episode_id))
    cnx.commit()

    update_queue_query = (
        'UPDATE "EpisodeQueue" SET QueuePosition = QueuePosition - 1 WHERE UserID = %s AND QueuePosition > %s' if database_type == "postgresql" else
        "UPDATE EpisodeQueue SET QueuePosition = QueuePosition - 1 WHERE UserID = %s AND QueuePosition > %s"
    )
    cursor.execute(update_queue_query, (user_id, removed_queue_position))
    cnx.commit()

    logging.info(f"Successfully removed episode from queue.")
    cursor.close()

    return {"status": "success"}



def get_queued_episodes(database_type, cnx, user_id):
    if database_type == "postgresql":
        from psycopg.rows import dict_row
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
        get_queued_episodes_query = """
        SELECT
            "Episodes".EpisodeTitle,
            "Podcasts".PodcastName,
            "Episodes".EpisodePubDate,
            "Episodes".EpisodeDescription,
            "Episodes".EpisodeArtwork,
            "Episodes".EpisodeURL,
            "EpisodeQueue".QueuePosition,
            "Episodes".EpisodeDuration,
            "EpisodeQueue".QueueDate,
            "UserEpisodeHistory".ListenDuration,
            "Episodes".EpisodeID,
            "Episodes".Completed
        FROM "EpisodeQueue"
        INNER JOIN "Episodes" ON "EpisodeQueue".EpisodeID = "Episodes".EpisodeID
        INNER JOIN "Podcasts" ON "Episodes".PodcastID = "Podcasts".PodcastID
        LEFT JOIN "UserEpisodeHistory" ON "EpisodeQueue".EpisodeID = "UserEpisodeHistory".EpisodeID AND "EpisodeQueue".UserID = "UserEpisodeHistory".UserID
        WHERE "EpisodeQueue".UserID = %s
        ORDER BY "EpisodeQueue".QueuePosition ASC
        """
    else:  # MySQL or MariaDB
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
            Episodes.EpisodeID,
            Episodes.Completed
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

    # Normalize keys to lowercase
    queued_episodes = lowercase_keys(queued_episodes)

    return queued_episodes


def check_episode_exists(cnx, database_type, user_id, episode_title, episode_url):
    cursor = cnx.cursor()
    query = """
        SELECT EXISTS(
            SELECT 1 FROM "Episodes"
            JOIN "Podcasts" ON "Episodes".PodcastID = "Podcasts".PodcastID
            WHERE "Podcasts".UserID = %s AND "Episodes".EpisodeTitle = %s AND "Episodes".EpisodeURL = %s
        )
    """ if database_type == "postgresql" else """
        SELECT EXISTS(
            SELECT 1 FROM Episodes
            JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID
            WHERE Podcasts.UserID = %s AND Episodes.EpisodeTitle = %s AND Episodes.EpisodeURL = %s
        )
    """
    cursor.execute(query, (user_id, episode_title, episode_url))
    result = cursor.fetchone()
    cursor.close()

    # Check if result is a dictionary or a tuple
    if isinstance(result, dict):
        return result['exists'] == 1
    elif isinstance(result, tuple):
        return result[0] == 1
    else:
        raise TypeError("Unexpected type for 'result'")



def add_gpodder_settings(database_type, cnx, user_id, gpodder_url, gpodder_token, login_name):
    print("Adding gPodder settings")
    print(f"User ID: {user_id}, gPodder URL: {gpodder_url}, gPodder Token: {gpodder_token}, Login Name: {login_name}")
    the_key = get_encryption_key(cnx, database_type)

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

    query = (
        'UPDATE "Users" SET GpodderUrl = %s, GpodderLoginName = %s, GpodderToken = %s WHERE UserID = %s' if database_type == "postgresql" else
        "UPDATE Users SET GpodderUrl = %s, GpodderLoginName = %s, GpodderToken = %s WHERE UserID = %s"
    )

    cursor.execute(query, (gpodder_url, login_name, decoded_token, user_id))

    # Check if the update was successful
    if cursor.rowcount == 0:
        return None

    cnx.commit()  # Commit changes to the database
    cursor.close()

    return True



def get_gpodder_settings(database_type, cnx, user_id):
    cursor = cnx.cursor()
    query = (
        'SELECT GpodderUrl, GpodderToken FROM "Users" WHERE UserID = %s' if database_type == "postgresql" else
        "SELECT GpodderUrl, GpodderToken FROM Users WHERE UserID = %s"
    )
    cursor.execute(query, (user_id,))
    result = cursor.fetchone()
    cursor.close()

    # Ensure result is consistent
    if result:
        if isinstance(result, tuple):
            # Convert tuple result to a dictionary
            if database_type == 'postgresql':
                result = {
                    "gpodderurl": result[0],
                    "gpoddertoken": result[1]
                }
            else:
                result = {
                    "GpodderUrl": result[0],
                    "GpodderToken": result[1]
                }
        elif isinstance(result, dict):
            # Normalize keys to lower case if necessary
            result = {k.lower(): v for k, v in result.items()}
    else:
        result = {}

    lower_result = lowercase_keys(result)

    return lower_result




def get_nextcloud_settings(database_type, cnx, user_id):
    cursor = cnx.cursor()
    query = (
        'SELECT GpodderUrl, GpodderToken, GpodderLoginName FROM "Users" WHERE UserID = %s' if database_type == "postgresql" else
        "SELECT GpodderUrl, GpodderToken, GpodderLoginName FROM Users WHERE UserID = %s"
    )
    cursor.execute(query, (user_id,))
    result = cursor.fetchone()
    cursor.close()
    if result and result[0] and result[1] and result[2]:
        return result[0], result[1], result[2]
    else:
        return None, None, None



def remove_gpodder_settings(database_type, cnx, user_id):
    cursor = cnx.cursor()
    query = (
        'UPDATE "Users" SET GpodderUrl = %s, GpodderToken = %s WHERE UserID = %s' if database_type == "postgresql" else
        "UPDATE Users SET GpodderUrl = %s, GpodderToken = %s WHERE UserID = %s"
    )
    cursor.execute(query, ('', '', user_id))
    cnx.commit()
    cursor.close()



def check_gpodder_settings(database_type, cnx, user_id):
    cursor = cnx.cursor()
    query = (
        'SELECT GpodderUrl, GpodderToken FROM "Users" WHERE UserID = %s' if database_type == "postgresql" else
        "SELECT GpodderUrl, GpodderToken FROM Users WHERE UserID = %s"
    )
    cursor.execute(query, (user_id,))
    result = cursor.fetchone()
    cursor.close()
    if result:
        # Check if result is a dictionary
        if isinstance(result, dict):
            gpodder_url = result.get('gpodderurl')
            gpodder_token = result.get('gpoddertoken')
        else:  # result is a tuple
            gpodder_url = result[0]
            gpodder_token = result[1]

        if gpodder_url and gpodder_token:
            return True

    return False


def get_nextcloud_users(database_type, cnx):
    cursor = cnx.cursor()

    # Query to select users with set Nextcloud gPodder URLs and Tokens
    if database_type == "postgresql":
        query = """
            SELECT UserID, GpodderUrl, GpodderToken, GpodderLoginName
            FROM "Users"
            WHERE GpodderUrl <> '' AND GpodderToken <> '' AND GpodderLoginName <> ''
        """
    else:  # MySQL or MariaDB
        query = """
            SELECT UserID, GpodderUrl, GpodderToken, GpodderLoginName
            FROM Users
            WHERE GpodderUrl <> '' AND GpodderToken <> '' AND GpodderLoginName <> ''
        """
    cursor.execute(query)

    # Fetch all matching records
    users = cursor.fetchall()
    cursor.close()

    return users


import datetime

def current_timestamp():
    # Return the current time in 'YYYY-MM-DDTHH:MM:SS' format, without fractional seconds or 'Z'
    return datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%S')

def add_podcast_to_nextcloud(cnx, database_type, gpodder_url, gpodder_login, encrypted_gpodder_token, podcast_url):
    from cryptography.fernet import Fernet
    from requests.auth import HTTPBasicAuth

    encryption_key = get_encryption_key(cnx, database_type)
    encryption_key_bytes = base64.b64decode(encryption_key)

    cipher_suite = Fernet(encryption_key_bytes)

    # Decrypt the token
    if encrypted_gpodder_token is not None:
        decrypted_token_bytes = cipher_suite.decrypt(encrypted_gpodder_token.encode())
        gpodder_token = decrypted_token_bytes.decode()
    else:
        gpodder_token = None

    url = f"{gpodder_url}/index.php/apps/gpoddersync/subscription_change/create"
    auth = HTTPBasicAuth(gpodder_login, gpodder_token)  # Using Basic Auth
    data = {
        "add": [podcast_url],
        "remove": []
    }
    headers = {
        "Content-Type": "application/json"
    }
    response = requests.post(url, json=data, headers=headers, auth=auth)
    try:
        response.raise_for_status()
        print(f"Podcast added to Nextcloud successfully: {response.text}")
    except requests.exceptions.HTTPError as e:
        print(f"Failed to add podcast to Nextcloud: {e}")
        print(f"Response body: {response.text}")


def remove_podcast_from_nextcloud(cnx, database_type, gpodder_url, gpodder_login, encrypted_gpodder_token, podcast_url):
    from cryptography.fernet import Fernet
    from requests.auth import HTTPBasicAuth

    encryption_key = get_encryption_key(cnx, database_type)
    encryption_key_bytes = base64.b64decode(encryption_key)

    cipher_suite = Fernet(encryption_key_bytes)

    # Decrypt the token
    if encrypted_gpodder_token is not None:
        decrypted_token_bytes = cipher_suite.decrypt(encrypted_gpodder_token.encode())
        gpodder_token = decrypted_token_bytes.decode()
    else:
        gpodder_token = None

    url = f"{gpodder_url}/index.php/apps/gpoddersync/subscription_change/create"
    auth = HTTPBasicAuth(gpodder_login, gpodder_token)  # Using Basic Auth
    headers = {
        "Content-Type": "application/json"
    }
    data = {
        "add": [],
        "remove": [podcast_url]
    }
    response = requests.post(url, json=data, headers=headers, auth=auth)
    try:
        response.raise_for_status()
        print(f"Podcast removed from Nextcloud successfully: {response.text}")
    except requests.exceptions.HTTPError as e:
        print(f"Failed to remove podcast from Nextcloud: {e}")
        print(f"Response body: {response.text}")


def refresh_nextcloud_subscription(database_type, cnx, user_id, gpodder_url, encrypted_gpodder_token, gpodder_login):
    from cryptography.fernet import Fernet
    from requests.auth import HTTPBasicAuth
    # Fetch encryption key
    encryption_key = get_encryption_key(cnx, database_type)
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
    if database_type == "postgresql":
        query = 'SELECT FeedURL FROM "Podcasts" WHERE UserID = %s'
    else:  # MySQL or MariaDB
        query = "SELECT FeedURL FROM Podcasts WHERE UserID = %s"

    cursor.execute(query, (user_id,))
    local_podcasts = [row[0] for row in cursor.fetchall()]
    print(f"Local podcasts: {local_podcasts}")

    podcasts_to_add = set(nextcloud_podcasts) - set(local_podcasts)
    podcasts_to_remove = set(local_podcasts) - set(nextcloud_podcasts)
    print(f"Podcasts to add: {podcasts_to_add}, Podcasts to remove: {podcasts_to_remove}")

    # Update local database
    # Add new podcasts
    print("Adding new podcasts...")
    for feed_url in podcasts_to_add:
        podcast_values = get_podcast_values(feed_url, user_id)
        return_value = add_podcast(cnx, database_type, podcast_values, user_id)
        if return_value:
            print(f"{feed_url} added!")
        else:
            print(f"error adding {feed_url}")

    # Remove podcasts no longer in the subscription
    print("Removing podcasts...")
    for feed_url in podcasts_to_remove:
        print(f"Removing {feed_url}...")
        if database_type == "postgresql":
            query = 'SELECT PodcastName FROM "Podcasts" WHERE FeedURL = %s'
        else:  # MySQL or MariaDB
            query = "SELECT PodcastName FROM Podcasts WHERE FeedURL = %s"

        cursor.execute(query, (feed_url,))
        result = cursor.fetchone()
        print(f"Result: {result}")
        print(f"Feed URL: {feed_url}")
        if result:
            podcast_name = result[0]  # Unpack the tuple to get the podcast name
            remove_podcast(cnx, database_type, podcast_name, feed_url, user_id)
        else:
            print(f"No podcast found with URL: {feed_url}")

    cnx.commit()
    cursor.close()


    # Notify Nextcloud of changes made locally (if any)
    print("Syncing subscription changes...")
    if podcasts_to_add or podcasts_to_remove:
        sync_subscription_change(gpodder_url, {"Authorization": f"Bearer {gpodder_token}"}, list(podcasts_to_add),
                                 list(podcasts_to_remove))

    # from requests.exceptions import RequestException

    # Fetch episode actions from Nextcloud
    print("Fetching episode actions from Nextcloud...")
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
    print("Processing episode actions...")
    for action in episode_actions.get('actions', []):  # Ensure default to empty list if 'actions' is not found
        try:
            # Ensure action is relevant, such as a 'play' or 'update_time' action with a valid position
            print(f"Processing action: {action}")
            if action["action"].lower() in ["play", "update_time"]:
                print(f"Action details - Podcast: {action['podcast']}, Episode: {action['episode']}, Position: {action.get('position')}")
                if "position" in action and action["position"] != -1:
                    episode_id = get_episode_id_by_url(cnx, database_type, action["episode"])
                    if episode_id:
                        print(f"Recording listen duration for episode ID {episode_id} with position {action['position']}")
                        record_listen_duration(cnx, database_type, episode_id, user_id, int(action["position"]))
                    else:
                        print(f"No episode ID found for URL {action['episode']}")
                else:
                    print(f"Skipping action due to invalid position: {action}")

        except Exception as e:
            print(f"Error processing episode action {action}: {e}")

    # Collect local episode listen times and push to Nextcloud if necessary
    print("Collecting local episode listen times...")
    try:
        local_episode_times = get_local_episode_times(cnx, database_type, user_id)
    except Exception as e:
        print(f"Error fetching local episode times: {e}")
        local_episode_times = []

    UPLOAD_BULK_SIZE = 30
    # Send local episode listen times to Nextcloud
    update_actions = []
    for episode_time in local_episode_times:
        update_actions.append({
            "podcast": episode_time["podcast_url"],
            "episode": episode_time["episode_url"],
            "action": "play",
            "timestamp": current_timestamp(),
            "position": episode_time["listen_duration"],
            "started": 0,
            "total": episode_time["episode_duration"],
            "guid": generate_guid(episode_time)
        })
    print(f"Update actions: {update_actions}")
    # Split the list of update actions into chunks
    update_actions_chunks = [update_actions[i:i + UPLOAD_BULK_SIZE] for i in range(0, len(update_actions), UPLOAD_BULK_SIZE)]

    from urllib.parse import urljoin
    for chunk in update_actions_chunks:
        try:
            url = urljoin(gpodder_url, "/index.php/apps/gpoddersync/episode_action/create")
            response = requests.post(
                url,
                json=chunk,
                auth=HTTPBasicAuth(gpodder_login, gpodder_token),
                headers={"Accept": "application/json"}
            )
            if response.status_code != 200:
                raise RequestException(f"Unexpected status code: {response.status_code}")
            print(f"Update episode times response: {response.status_code}")
        except RequestException as e:
            print(f"Error updating episode times in Nextcloud: {e}")

# database_functions.py

def queue_bump(database_type, cnx, ep_url, title, user_id):
    cursor = cnx.cursor()

    if database_type == "postgresql":
        query_check_episode = """
            SELECT QueueID, QueuePosition FROM "EpisodeQueue"
            INNER JOIN "Episodes" ON "EpisodeQueue".EpisodeID = "Episodes".EpisodeID
            WHERE "Episodes".EpisodeURL = %s AND "Episodes".EpisodeTitle = %s AND "EpisodeQueue".UserID = %s
        """
        query_delete_episode = 'DELETE FROM "EpisodeQueue" WHERE QueueID = %s'
        query_update_queue = 'UPDATE "EpisodeQueue" SET QueuePosition = QueuePosition - 1 WHERE UserID = %s'
    else:
        query_check_episode = """
            SELECT QueueID, QueuePosition FROM EpisodeQueue
            INNER JOIN Episodes ON EpisodeQueue.EpisodeID = Episodes.EpisodeID
            WHERE Episodes.EpisodeURL = %s AND Episodes.EpisodeTitle = %s AND EpisodeQueue.UserID = %s
        """
        query_delete_episode = "DELETE FROM EpisodeQueue WHERE QueueID = %s"
        query_update_queue = "UPDATE EpisodeQueue SET QueuePosition = QueuePosition - 1 WHERE UserID = %s"

    cursor.execute(query_check_episode, (ep_url, title, user_id))
    result = cursor.fetchone()
    print(result)

    if result is not None:
        try:
            cursor.execute(query_delete_episode, (result[0],))
        except Exception as e:
            print(f"Error while deleting episode from queue: {e}")

    cursor.execute(query_update_queue, (user_id,))

    queue_pod(database_type, cnx, title, ep_url, user_id)

    cnx.commit()
    cursor.close()

    return {"detail": f"{title} moved to the front of the queue."}




def backup_user(database_type, cnx, user_id):
    if database_type == "postgresql":
        cursor = cnx.cursor(row_factory=psycopg.rows.dict_row)
        query_fetch_podcasts = 'SELECT PodcastName, FeedURL FROM "Podcasts" WHERE UserID = %s'
    else:
        cursor = cnx.cursor(dictionary=True)
        query_fetch_podcasts = "SELECT PodcastName, FeedURL FROM Podcasts WHERE UserID = %s"

    cursor.execute(query_fetch_podcasts, (user_id,))
    podcasts = cursor.fetchall()
    cursor.close()

    opml_content = '<?xml version="1.0" encoding="UTF-8"?>\n<opml version="2.0">\n  <head>\n    <title>Podcast Subscriptions</title>\n  </head>\n  <body>\n'

    if database_type == "postgresql":
        for podcast in podcasts:
            opml_content += f'    <outline text="{podcast["podcastname"]}" title="{podcast["podcastname"]}" type="rss" xmlUrl="{podcast["feedurl"]}" />\n'
    else:
        for podcast in podcasts:
            opml_content += f'    <outline text="{podcast["PodcastName"]}" title="{podcast["PodcastName"]}" type="rss" xmlUrl="{podcast["FeedURL"]}" />\n'

    opml_content += '  </body>\n</opml>'

    return opml_content



def backup_server(database_type, cnx, database_pass):
    # Replace with your database and authentication details
    print(f'pass: {database_pass}')

    if database_type == "postgresql":
        os.environ['PGPASSWORD'] = database_pass
        cmd = [
            "pg_dump",
            "-h", 'db',
            "-p", '5432',
            "-U", "postgres",
            "-d", "pypods_database",
            "-w"
        ]
    else:  # Assuming MySQL or MariaDB
        cmd = [
            "mysqldump",
            "-h", 'db',
            "-P", '3306',
            "-u", "root",
            "-p" + database_pass,
            "pypods_database"
        ]

    try:
        process = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        stdout, stderr = process.communicate()
        print("STDOUT:", stdout.decode())
        print("STDERR:", stderr.decode())

        if process.returncode != 0:
            # Handle error
            raise Exception(f"Backup failed with error: {stderr.decode()}")

        return stdout.decode()
    finally:
        if database_type == "postgresql":
            del os.environ['PGPASSWORD']


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
