import random
import string
import mysql.connector
from mysql.connector import errorcode
import mysql.connector.pooling
import sys
import os
import requests
import feedgenerator
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
import feedparser
import dateutil.parser
import re
import requests
from requests.auth import HTTPBasicAuth
from urllib.parse import urlparse, urlunparse
from typing import List, Optional
import pytz
from yt_dlp import YoutubeDL
from database_functions import youtube
from database_functions import mp3_metadata
import logging
from cryptography.fernet import Fernet
from requests.exceptions import RequestException
import shutil

# # Get the application root directory from the environment variable
# app_root = os.environ.get('APP_ROOT')
sys.path.append('/pinepods/'),
# Import the functions directly from app_functions.py located in the database_functions directory
from database_functions.app_functions import sync_subscription_change, get_podcast_values, check_valid_feed, sync_subscription_change_gpodder


def pascal_case(snake_str):
    return ''.join(word.title() for word in snake_str.split('_'))

def lowercase_keys(data):
    if isinstance(data, dict):
        return {k.lower(): (bool(v) if k.lower() == 'completed' else v) for k, v in data.items()}
    elif isinstance(data, list):
        return [lowercase_keys(item) for item in data]
    return data

def convert_bools(data, database_type):
    def convert_value(k, v):
        if k.lower() == 'explicit':
            if database_type == 'postgresql':
                return v == True
            else:
                return bool(v)
        return v

    if isinstance(data, dict):
        return {k: convert_value(k, v) for k, v in data.items()}
    elif isinstance(data, list):
        return [convert_bools(item, database_type) for item in data]
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

def add_custom_podcast(database_type, cnx, feed_url, user_id, username=None, password=None):
    # Proceed to extract and use podcast details if the feed is valid
    podcast_values = get_podcast_values(feed_url, user_id, username, password)
    try:
        result = add_podcast(cnx, database_type, podcast_values, user_id, username, password)
        if not result:
            raise Exception("Failed to add the podcast.")

        # Handle the tuple return value
        if isinstance(result, tuple):
            podcast_id = result[0]  # Extract just the podcast_id
        else:
            podcast_id = result

        return podcast_id

    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))

def add_news_feed_if_not_added(database_type, cnx):
    cursor = cnx.cursor()
    try:
        # Get all admin users
        if database_type == "postgresql":
            cursor.execute('SELECT UserID FROM "Users" WHERE IsAdmin = TRUE')
        else:  # MySQL or MariaDB
            cursor.execute("SELECT UserID FROM Users WHERE IsAdmin = 1")

        admin_users = cursor.fetchall()
        feed_url = "https://news.pinepods.online/feed.xml"

        # Add feed for each admin user if they don't already have it
        for admin in admin_users:
            user_id = admin[0]

            # Check if this user already has the news feed
            if database_type == "postgresql":
                cursor.execute('SELECT PodcastID FROM "Podcasts" WHERE UserID = %s AND FeedURL = %s', (user_id, feed_url))
            else:  # MySQL or MariaDB
                cursor.execute("SELECT PodcastID FROM Podcasts WHERE UserID = %s AND FeedURL = %s", (user_id, feed_url))

            existing_feed = cursor.fetchone()

            if existing_feed is None:
                add_custom_podcast(database_type, cnx, feed_url, user_id)
                cnx.commit()

    except (psycopg.ProgrammingError, mysql.connector.ProgrammingError) as e:
        print(f"Error in add_news_feed_if_not_added: {e}")
        cnx.rollback()
    finally:
        cursor.close()


def add_podcast(cnx, database_type, podcast_values, user_id, username=None, password=None, podcast_index_id=0):
    cursor = cnx.cursor()

    # If podcast_index_id is 0, try to fetch it from the API
    if podcast_index_id == 0:
        api_url = os.environ.get("SEARCH_API_URL", "https://api.pinepods.online/api/search")
        search_url = f"{api_url}?query={podcast_values['pod_title']}"

        try:
            response = requests.get(search_url)
            response.raise_for_status()
            data = response.json()

            if data['status'] == 'true' and data['feeds']:
                for feed in data['feeds']:
                    if feed['title'] == podcast_values['pod_title']:
                        podcast_index_id = feed['id']
                        break

            if podcast_index_id == 0:
                print(f"Couldn't find PodcastIndexID for {podcast_values['pod_title']}")
        except Exception as e:
            print(f"Error fetching PodcastIndexID: {e}")


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

        # Extract category names and convert to comma-separated string
        categories = podcast_values['categories']
        print(f"Categories: {categories}")

        if isinstance(categories, dict):
            category_list = ', '.join(categories.values())
        elif isinstance(categories, list):
            category_list = ', '.join(categories)
        elif isinstance(categories, str):
            category_list = categories
        else:
            category_list = ''

        if database_type == "postgresql":
            add_podcast_query = """
                INSERT INTO "Podcasts"
                (PodcastName, ArtworkURL, Author, Categories, Description, EpisodeCount, FeedURL, WebsiteURL, Explicit, UserID, Username, Password, PodcastIndexID)
                VALUES (%s, %s, %s, %s, %s, 0, %s, %s, %s, %s, %s, %s, %s) RETURNING PodcastID
            """
            explicit = podcast_values['pod_explicit']
        else:  # MySQL or MariaDB
            add_podcast_query = """
                INSERT INTO Podcasts
                (PodcastName, ArtworkURL, Author, Categories, Description, EpisodeCount, FeedURL, WebsiteURL, Explicit, UserID, Username, Password, PodcastIndexID)
                VALUES (%s, %s, %s, %s, %s, 0, %s, %s, %s, %s, %s, %s, %s)
            """
            explicit = 1 if podcast_values['pod_explicit'] else 0


        print("Inserting into db")
        print(podcast_values['pod_title'])
        print(podcast_values['pod_artwork'])
        print(podcast_values['pod_author'])
        print(category_list)
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
                category_list,
                podcast_values['pod_description'],
                podcast_values['pod_feed_url'],
                podcast_values['pod_website'],
                explicit,
                user_id,
                username,
                password,
                podcast_index_id
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
            first_episode_id = add_episodes(cnx, database_type, podcast_id, podcast_values['pod_feed_url'],
                                          podcast_values['pod_artwork'], False, username, password, user_id)  # Add user_id here
            print("episodes added")
            return podcast_id, first_episode_id

        except Exception as e:
            logging.error(f"Failed to add podcast: {e}")
            print(f"Failed to add podcast: {e}")
            cnx.rollback()
            cursor.close()
            raise Exception(f"Failed to add podcast: {e}")

    except Exception as e:
        print(f"Error during podcast insertion or UserStats update: {e}")
        logging.error(f"Error during podcast insertion or UserStats update: {e}")
        cnx.rollback()
        raise

    finally:
        cursor.close()

    # Return True to indicate success
    return True


def add_person_podcast(cnx, database_type, podcast_values, user_id, username=None, password=None, podcast_index_id=0):
    cursor = cnx.cursor()

    # If podcast_index_id is 0, try to fetch it from the API
    if podcast_index_id == 0:
        api_url = os.environ.get("SEARCH_API_URL", "https://api.pinepods.online/api/search")
        search_url = f"{api_url}?query={podcast_values['pod_title']}"

        try:
            response = requests.get(search_url)
            response.raise_for_status()
            data = response.json()

            if data['status'] == 'true' and data['feeds']:
                for feed in data['feeds']:
                    if feed['title'] == podcast_values['pod_title']:
                        podcast_index_id = feed['id']
                        break

            if podcast_index_id == 0:
                print(f"Couldn't find PodcastIndexID for {podcast_values['pod_title']}")
        except Exception as e:
            print(f"Error fetching PodcastIndexID: {e}")


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

        # Extract category names and convert to comma-separated string
        categories = podcast_values['categories']
        print(f"Categories: {categories}")

        if isinstance(categories, dict):
            category_list = ', '.join(categories.values())
        elif isinstance(categories, list):
            category_list = ', '.join(categories)
        elif isinstance(categories, str):
            category_list = categories
        else:
            category_list = ''

        if database_type == "postgresql":
            add_podcast_query = """
                INSERT INTO "Podcasts"
                (PodcastName, ArtworkURL, Author, Categories, Description, EpisodeCount, FeedURL, WebsiteURL, Explicit, UserID, Username, Password, PodcastIndexID)
                VALUES (%s, %s, %s, %s, %s, 0, %s, %s, %s, %s, %s, %s, %s) RETURNING PodcastID
            """
            explicit = podcast_values['pod_explicit']
        else:  # MySQL or MariaDB
            add_podcast_query = """
                INSERT INTO Podcasts
                (PodcastName, ArtworkURL, Author, Categories, Description, EpisodeCount, FeedURL, WebsiteURL, Explicit, UserID, Username, Password, PodcastIndexID)
                VALUES (%s, %s, %s, %s, %s, 0, %s, %s, %s, %s, %s, %s, %s)
            """
            explicit = 1 if podcast_values['pod_explicit'] else 0


        print("Inserting into db")
        print(podcast_values['pod_title'])
        print(podcast_values['pod_artwork'])
        print(podcast_values['pod_author'])
        print(category_list)
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
                category_list,
                podcast_values['pod_description'],
                podcast_values['pod_feed_url'],
                podcast_values['pod_website'],
                explicit,
                user_id,
                username,
                password,
                podcast_index_id
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
    try:
        print(f"Adding user with values: {user_values}")
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

        # Handle the user ID retrieval
        if database_type == "postgresql":
            result = cursor.fetchone()
            if result is None:
                raise Exception("Failed to create user - no ID returned")

            # Print the result for debugging
            print(f"Raw PostgreSQL result: {result}")
            logging.debug(f"Raw PostgreSQL result: {result}")

            # Handle different return types
            if isinstance(result, dict):
                # Try different case variations
                user_id = result.get('userid') or result.get('UserID') or result.get('userId') or result.get('user_id')
            else:
                user_id = result[0]

            if not user_id:
                raise Exception("Failed to create user - invalid ID returned")
        # Add user settings
        settings_query = """
            INSERT INTO "UserSettings"
            (UserID, Theme)
            VALUES (%s, %s)
        """ if database_type == "postgresql" else """
            INSERT INTO UserSettings
            (UserID, Theme)
            VALUES (%s, %s)
        """
        cursor.execute(settings_query, (user_id, 'Nordic'))

        # Add user stats
        stats_query = """
            INSERT INTO "UserStats"
            (UserID)
            VALUES (%s)
        """ if database_type == "postgresql" else """
            INSERT INTO UserStats
            (UserID)
            VALUES (%s)
        """
        cursor.execute(stats_query, (user_id,))

        cnx.commit()
        return user_id

    except Exception as e:
        cnx.rollback()
        logging.error(f"Error in add_user: {str(e)}")
        raise

    finally:
        cursor.close()

def add_admin_user(cnx, database_type, user_values):
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            add_user_query = """
                WITH inserted_user AS (
                    INSERT INTO "Users"
                    (Fullname, Username, Email, Hashed_PW, IsAdmin)
                    VALUES (%s, %s, %s, %s, TRUE)
                    ON CONFLICT (Username) DO NOTHING
                    RETURNING UserID
                )
                SELECT UserID FROM inserted_user
                UNION ALL
                SELECT UserID FROM "Users" WHERE Username = %s
                LIMIT 1
            """
            # Note: we add the username as an extra parameter here
            cursor.execute(add_user_query, user_values + (user_values[1],))
            user_id = cursor.fetchone()[0]
        else:  # MySQL or MariaDB
            add_user_query = """
                INSERT INTO Users
                (Fullname, Username, Email, Hashed_PW, IsAdmin)
                VALUES (%s, %s, %s, %s, 1)
            """
            cursor.execute(add_user_query, user_values)
            user_id = cursor.lastrowid

        # Now add settings and stats
        if database_type == "postgresql":
            add_user_settings_query = """
                INSERT INTO "UserSettings"
                (UserID, Theme)
                VALUES (%s, %s)
            """
        else:
            add_user_settings_query = """
                INSERT INTO UserSettings
                (UserID, Theme)
                VALUES (%s, %s)
            """
        cursor.execute(add_user_settings_query, (user_id, 'Nordic'))

        if database_type == "postgresql":
            add_user_stats_query = """
                INSERT INTO "UserStats"
                (UserID)
                VALUES (%s)
            """
        else:
            add_user_stats_query = """
                INSERT INTO UserStats
                (UserID)
                VALUES (%s)
            """
        cursor.execute(add_user_stats_query, (user_id,))
        cnx.commit()
        return user_id
    finally:
        cursor.close()

def get_pinepods_version():
    try:
        with open('/pinepods/current_version', 'r') as file:
            version = file.read().strip()
            if not version:
                return 'dev_mode'
            return version
    except FileNotFoundError:
        return "Version file not found."
    except Exception as e:
        return f"An error occurred: {e}"

def get_first_episode_id(cnx, database_type, podcast_id, user_id, is_youtube=False):
    print('getting first ep id')
    cursor = cnx.cursor()
    try:
        if is_youtube:
            if database_type == "postgresql":
                query = 'SELECT VIDEOID FROM "YouTubeVideos" WHERE PODCASTID = %s ORDER BY PUBLISHEDAT ASC LIMIT 1'
            else:  # MySQL or MariaDB
                query = "SELECT VideoID FROM YouTubeVideos WHERE PodcastID = %s ORDER BY PublishedAt ASC LIMIT 1"
        else:
            if database_type == "postgresql":
                query = 'SELECT EPISODEID FROM "Episodes" WHERE PODCASTID = %s ORDER BY EPISODEPUBDATE ASC LIMIT 1'
            else:  # MySQL or MariaDB
                query = "SELECT EpisodeID FROM Episodes WHERE PodcastID = %s ORDER BY EpisodePubDate ASC LIMIT 1"
        print(f'request finish')
        cursor.execute(query, (podcast_id,))
        result = cursor.fetchone()
        print(f'request result {result}')
        if isinstance(result, dict):
            return result.get("videoid" if is_youtube else "episodeid") if result else None
        elif isinstance(result, tuple):
            return result[0] if result else None
        else:
            return None
    finally:
        cursor.close()

def try_fetch_feed(url, username=None, password=None):
    headers = {
        'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36'
    }
    auth = HTTPBasicAuth(username, password) if username and password else None
    try:
        response = requests.get(
            url,
            auth=auth,
            headers=headers,
            timeout=30,
            allow_redirects=True,
            # verify=False  # Be cautious with this in production!
        )
        response.raise_for_status()
        return response.content
    except RequestException as e:
        print(f"Error fetching {url}: {str(e)}")
        return None

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

# Function to update the episode count
def update_episode_count(cnx, database_type, cursor, podcast_id):
    if database_type == "postgresql":
        update_query = 'UPDATE "Podcasts" SET EpisodeCount = EpisodeCount + 1 WHERE PodcastID = %s'
    else:  # MySQL or MariaDB
        update_query = "UPDATE Podcasts SET EpisodeCount = EpisodeCount + 1 WHERE PodcastID = %s"

    cursor.execute(update_query, (podcast_id,))
    cnx.commit()

def add_episodes(cnx, database_type, podcast_id, feed_url, artwork_url, auto_download, username=None, password=None, websocket=False):
    import feedparser
    first_episode_id = None

    # Try to fetch the feed
    content = try_fetch_feed(feed_url, username, password)

    if content is None:
        # If the original URL fails, try switching between www and non-www
        parsed_url = urlparse(feed_url)
        if parsed_url.netloc.startswith('www.'):
            alternate_netloc = parsed_url.netloc[4:]
        else:
            alternate_netloc = 'www.' + parsed_url.netloc

        alternate_url = urlunparse(parsed_url._replace(netloc=alternate_netloc))
        content = try_fetch_feed(alternate_url, username, password)

    if content is None:
        raise ValueError(f"Failed to fetch feed from both {feed_url} and its www/non-www alternative")

    episode_dump = feedparser.parse(content)

    cursor = cnx.cursor()

    new_episodes = []

    for entry in episode_dump.entries:
        # Check necessary fields are present
        if not all(hasattr(entry, attr) for attr in ["title", "summary", "enclosures"]):
            continue

        # Title is required - if missing, skip this episode
        if not hasattr(entry, 'title') or not entry.title:
            continue

        parsed_title = entry.title

        # Description - use placeholder if missing
        parsed_description = entry.get('content', [{}])[0].get('value') or entry.get('summary') or "No description available"

        # Audio URL can be empty (non-audio posts are allowed)
        parsed_audio_url = entry.enclosures[0].href if entry.enclosures else ""

        # Release date - use current time as fallback if parsing fails
        try:
            parsed_release_datetime = dateutil.parser.parse(entry.published).strftime("%Y-%m-%d %H:%M:%S")
        except (AttributeError, ValueError):
            parsed_release_datetime = datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S")

        # Artwork - use placeholders based on feed name/episode number
        parsed_artwork_url = (entry.get('itunes_image', {}).get('href') or
                            getattr(entry, 'image', {}).get('href') or
                            artwork_url or  # This is the podcast's default artwork
                            '/static/assets/default-episode.png')  # Final fallback artwork

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
        update_episode_count(cnx, database_type, cursor, podcast_id)
        # Get the EpisodeID for the newly added episode
        if cursor.rowcount > 0:
            print(f"Added episode '{parsed_title}'")
            check_and_send_notification(cnx, database_type, podcast_id, parsed_title)
            if websocket:
                # Get the episode ID using a SELECT query right after insert
                if database_type == "postgresql":
                    cursor.execute("""
                        SELECT EpisodeID FROM "Episodes"
                        WHERE PodcastID = %s AND EpisodeTitle = %s AND EpisodeURL = %s
                    """, (podcast_id, parsed_title, parsed_audio_url))
                else:
                    cursor.execute("""
                        SELECT EpisodeID FROM Episodes
                        WHERE PodcastID = %s AND EpisodeTitle = %s AND EpisodeURL = %s
                    """, (podcast_id, parsed_title, parsed_audio_url))

                episode_id = cursor.fetchone()
                if isinstance(episode_id, dict):
                    episode_id = episode_id.get('episodeid')
                elif isinstance(episode_id, tuple):
                    episode_id = episode_id[0]

                episode_data = {
                    "episode_id": episode_id,
                    "podcast_id": podcast_id,
                    "title": parsed_title,
                    "description": parsed_description,
                    "audio_url": parsed_audio_url,
                    "artwork_url": parsed_artwork_url,
                    "release_datetime": parsed_release_datetime,
                    "duration": parsed_duration,
                    "completed": False  # Assuming this is the default for new episodes
                }
                new_episodes.append(episode_data)
            if auto_download:  # Check if auto-download is enabled
                episode_id = get_episode_id(cnx, database_type, podcast_id, parsed_title, parsed_audio_url)

                user_id = get_user_id_from_pod_id(cnx, database_type, podcast_id)
                # Call your download function here
                download_podcast(cnx, database_type, episode_id, user_id)

    cnx.commit()

    # Now, retrieve the first episode ID
    if not websocket and first_episode_id is None:
        print(f'getting first id pre')
        first_episode_id = get_first_episode_id(cnx, database_type, podcast_id)
        print(f'first result {first_episode_id}')
    if websocket:
        return new_episodes
    return first_episode_id

def check_existing_channel_subscription(cnx, database_type: str, channel_id: str, user_id: int) -> Optional[int]:
    """Check if user is already subscribed to this channel"""
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            query = """
                SELECT PodcastID FROM "Podcasts"
                WHERE WebsiteURL = %s AND UserID = %s
            """
        else:  # MariaDB
            query = """
                SELECT PodcastID FROM Podcasts
                WHERE WebsiteURL = %s AND UserID = %s
            """

        cursor.execute(query, (f"https://www.youtube.com/channel/{channel_id}", user_id))
        result = cursor.fetchone()
        return result[0] if result else None
    except Exception as e:
        raise e

def add_youtube_channel(cnx, database_type: str, channel_info: dict, user_id: int) -> int:
    """Add YouTube channel to Podcasts table"""
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            query = """
                INSERT INTO "Podcasts" (
                    PodcastName, FeedURL, ArtworkURL, Author, Description,
                    WebsiteURL, UserID, IsYouTubeChannel, Categories
                ) VALUES (%s, %s, %s, %s, %s, %s, %s, TRUE, %s)
                RETURNING PodcastID
            """
        else:  # MariaDB
            query = """
                INSERT INTO Podcasts (
                    PodcastName, FeedURL, ArtworkURL, Author, Description,
                    WebsiteURL, UserID, IsYouTubeChannel, Categories
                ) VALUES (%s, %s, %s, %s, %s, %s, %s, 1, %s)
            """

        values = (
            channel_info['name'],
            f"https://www.youtube.com/channel/{channel_info['channel_id']}",
            channel_info['thumbnail_url'],
            channel_info['name'],
            channel_info['description'],
            f"https://www.youtube.com/channel/{channel_info['channel_id']}",
            user_id,
            ""
        )

        cursor.execute(query, values)
        if database_type == "postgresql":
            result = cursor.fetchone()
            if result is None:
                raise ValueError("No result returned from insert")
            # Handle both tuple and dict return types
            if isinstance(result, dict):
                podcast_id = result.get('podcastid')
                if podcast_id is None:
                    raise ValueError("No podcast ID in result dict")
            else:  # it's a tuple
                podcast_id = result[0]
            cnx.commit()  # Add this line for PostgreSQL
        else:  # MariaDB
            podcast_id = cursor.lastrowid
            cnx.commit()
        return podcast_id
    except Exception as e:
        print(f"Error in add_youtube_channel: {str(e)}")
        cnx.rollback()
        raise e

def add_youtube_videos(cnx, database_type: str, podcast_id: int, videos: list):
    """Add YouTube videos to YouTubeVideos table"""
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            query = """
                INSERT INTO "YouTubeVideos" (
                    PodcastID, VideoTitle, VideoDescription,
                    VideoURL, ThumbnailURL, PublishedAt,
                    Duration, YouTubeVideoID
                ) VALUES (%s, %s, %s, %s, %s, %s, %s, %s)
            """
        else:  # MariaDB
            query = """
                INSERT INTO YouTubeVideos (
                    PodcastID, VideoTitle, VideoDescription,
                    VideoURL, ThumbnailURL, PublishedAt,
                    Duration, YouTubeVideoID
                ) VALUES (%s, %s, %s, %s, %s, %s, %s, %s)
            """

        for video in videos:
            cursor.execute(query, (
                podcast_id,
                video['title'],
                video['description'],
                video['url'],
                video['thumbnail'],
                video['publish_date'],
                video['duration'],
                video['id']
            ))
        cnx.commit()
    except Exception as e:
        cnx.rollback()
        raise e

def add_people_episodes(cnx, database_type, person_id: int, podcast_id: int, feed_url: str):
    import feedparser
    import dateutil.parser
    try:
        headers = {
            'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.3',
            'Accept-Language': 'en-US,en;q=0.9'
        }
        content = feedparser.parse(feed_url, request_headers=headers)
        cursor = cnx.cursor()

        # Start a transaction
        if database_type == "postgresql":
            cursor.execute("BEGIN")

        # Get existing episode IDs before processing
        if database_type == "postgresql":
            existing_query = """
                SELECT EpisodeID FROM "PeopleEpisodes"
                WHERE PersonID = %s::integer
                AND PodcastID = %s::integer
            """
        else:
            existing_query = """
                SELECT EpisodeID FROM PeopleEpisodes
                WHERE PersonID = %s
                AND PodcastID = %s
            """

        cursor.execute(existing_query, (person_id, podcast_id))
        existing_episodes = {row[0] for row in cursor.fetchall()}
        processed_episodes = set()

        for entry in content.entries:
            if not all(hasattr(entry, attr) for attr in ["title", "summary"]):
                continue

            # Extract episode information using more robust parsing
            parsed_title = entry.title
            parsed_description = entry.get('content', [{}])[0].get('value', entry.summary)

            # Get audio URL from enclosures
            parsed_audio_url = ""
            for enclosure in entry.get('enclosures', []):
                if enclosure.get('type', '').startswith('audio/'):
                    parsed_audio_url = enclosure.get('href', '')
                    break

            if not parsed_audio_url:
                continue

            # Parse publish date
            try:
                parsed_release_datetime = dateutil.parser.parse(entry.published).strftime("%Y-%m-%d %H:%M:%S")
            except (AttributeError, ValueError):
                parsed_release_datetime = datetime.now().strftime("%Y-%m-%d %H:%M:%S")

            # Get artwork URL with fallbacks
            parsed_artwork_url = (entry.get('itunes_image', {}).get('href') or
                                getattr(entry, 'image', {}).get('href'))

            # Duration parsing with multiple fallbacks
            parsed_duration = 0
            duration_str = getattr(entry, 'itunes_duration', '')
            if ':' in duration_str:
                time_parts = list(map(int, duration_str.split(':')))
                while len(time_parts) < 3:
                    time_parts.insert(0, 0)
                h, m, s = time_parts
                parsed_duration = h * 3600 + m * 60 + s
            elif duration_str.isdigit():
                parsed_duration = int(duration_str)
            elif hasattr(entry, 'itunes_duration_seconds'):
                parsed_duration = int(entry.itunes_duration_seconds)
            elif hasattr(entry, 'duration'):
                parsed_duration = parse_duration(entry.duration)
            elif hasattr(entry, 'length'):
                parsed_duration = int(entry.length)

            try:
                # Check for existing episode
                if database_type == "postgresql":
                    episode_check_query = """
                        SELECT EpisodeID FROM "PeopleEpisodes"
                        WHERE PersonID = %s::integer
                        AND PodcastID = %s::integer
                        AND EpisodeURL = %s
                    """
                else:
                    episode_check_query = """
                        SELECT EpisodeID FROM PeopleEpisodes
                        WHERE PersonID = %s
                        AND PodcastID = %s
                        AND EpisodeURL = %s
                    """

                cursor.execute(episode_check_query, (person_id, podcast_id, parsed_audio_url))
                episode_result = cursor.fetchone()

                if episode_result:
                    episode_id = episode_result[0]
                    processed_episodes.add(episode_id)
                    continue

                # Insert new episode
                if database_type == "postgresql":
                    insert_query = """
                        INSERT INTO "PeopleEpisodes"
                        (PersonID, PodcastID, EpisodeTitle, EpisodeDescription,
                        EpisodeURL, EpisodeArtwork, EpisodePubDate, EpisodeDuration)
                        VALUES (%s::integer, %s::integer, %s, %s, %s, %s, %s, %s)
                        RETURNING EpisodeID
                    """
                else:
                    insert_query = """
                        INSERT INTO PeopleEpisodes
                        (PersonID, PodcastID, EpisodeTitle, EpisodeDescription,
                        EpisodeURL, EpisodeArtwork, EpisodePubDate, EpisodeDuration)
                        VALUES (%s, %s, %s, %s, %s, %s, %s, %s)
                    """

                cursor.execute(insert_query, (
                    person_id,
                    podcast_id,
                    parsed_title,
                    parsed_description,
                    parsed_audio_url,
                    parsed_artwork_url,
                    parsed_release_datetime,
                    parsed_duration
                ))

                # Get the ID of the newly inserted episode
                if database_type == "postgresql":
                    new_episode_id = cursor.fetchone()[0]
                else:
                    cursor.execute('SELECT LAST_INSERT_ID()')
                    new_episode_id = cursor.fetchone()[0]

                processed_episodes.add(new_episode_id)

            except Exception as e:
                logging.debug(f"Skipping episode '{parsed_title}' during person podcast import - {str(e)}")
                continue

        # Clean up old episodes
        episodes_to_delete = existing_episodes - processed_episodes
        if episodes_to_delete:
            if database_type == "postgresql":
                delete_query = """
                    DELETE FROM "PeopleEpisodes"
                    WHERE PersonID = %s::integer
                    AND PodcastID = %s::integer
                    AND EpisodeID = ANY(%s)
                    AND EpisodePubDate < NOW() - INTERVAL '30 days'
                """
                cursor.execute(delete_query, (person_id, podcast_id, list(episodes_to_delete)))
            else:
                if episodes_to_delete:  # Only proceed if there are episodes to delete
                    placeholders = ','.join(['%s'] * len(episodes_to_delete))
                    delete_query = f"""
                        DELETE FROM PeopleEpisodes
                        WHERE PersonID = %s
                        AND PodcastID = %s
                        AND EpisodeID IN ({placeholders})
                        AND EpisodePubDate < DATE_SUB(NOW(), INTERVAL 30 DAY)
                    """
                    cursor.execute(delete_query, (person_id, podcast_id) + tuple(episodes_to_delete))

        cnx.commit()

    except Exception as e:
        if database_type == "postgresql":
            cursor.execute("ROLLBACK")
        else:
            cnx.rollback()
        logging.error(f"Error processing feed {feed_url}: {str(e)}")
        raise

    finally:
        cursor.close()

def remove_youtube_channel_by_url(cnx, database_type, channel_name, channel_url, user_id):
    cursor = cnx.cursor()
    print('got to remove youtube channel')
    try:
        # Get the PodcastID first
        if database_type == "postgresql":
            select_podcast_id = '''
                SELECT PodcastID
                FROM "Podcasts"
                WHERE PodcastName = %s
                AND FeedURL = %s
                AND UserID = %s
                AND IsYouTubeChannel = TRUE
            '''
        else:  # MySQL or MariaDB
            select_podcast_id = '''
                SELECT PodcastID
                FROM Podcasts
                WHERE PodcastName = %s
                AND FeedURL = %s
                AND UserID = %s
                AND IsYouTubeChannel = TRUE
            '''

        cursor.execute(select_podcast_id, (channel_name, channel_url, user_id))
        result = cursor.fetchone()

        if result:
            podcast_id = result[0] if not isinstance(result, dict) else result.get('podcastid')
        else:
            raise ValueError(f"No YouTube channel found with name {channel_name}")

        # Delete related data
        if database_type == "postgresql":
            delete_queries = [
                ('DELETE FROM "UserEpisodeHistory" WHERE EpisodeID IN (SELECT VideoID FROM "YouTubeVideos" WHERE PodcastID = %s)', (podcast_id,)),
                ('DELETE FROM "DownloadedEpisodes" WHERE EpisodeID IN (SELECT VideoID FROM "YouTubeVideos" WHERE PodcastID = %s)', (podcast_id,)),
                ('DELETE FROM "SavedEpisodes" WHERE EpisodeID IN (SELECT VideoID FROM "YouTubeVideos" WHERE PodcastID = %s)', (podcast_id,)),
                ('DELETE FROM "EpisodeQueue" WHERE EpisodeID IN (SELECT VideoID FROM "YouTubeVideos" WHERE PodcastID = %s)', (podcast_id,)),
                ('DELETE FROM "YouTubeVideos" WHERE PodcastID = %s', (podcast_id,)),
                ('DELETE FROM "Podcasts" WHERE PodcastID = %s AND IsYouTubeChannel = TRUE', (podcast_id,))
            ]
        else:  # MySQL or MariaDB
            delete_queries = [
                ("DELETE FROM UserEpisodeHistory WHERE EpisodeID IN (SELECT VideoID FROM YouTubeVideos WHERE PodcastID = %s)", (podcast_id,)),
                ("DELETE FROM DownloadedEpisodes WHERE EpisodeID IN (SELECT VideoID FROM YouTubeVideos WHERE PodcastID = %s)", (podcast_id,)),
                ("DELETE FROM SavedEpisodes WHERE EpisodeID IN (SELECT VideoID FROM YouTubeVideos WHERE PodcastID = %s)", (podcast_id,)),
                ("DELETE FROM EpisodeQueue WHERE EpisodeID IN (SELECT VideoID FROM YouTubeVideos WHERE PodcastID = %s)", (podcast_id,)),
                ("DELETE FROM YouTubeVideos WHERE PodcastID = %s", (podcast_id,)),
                ("DELETE FROM Podcasts WHERE PodcastID = %s AND IsYouTubeChannel = TRUE", (podcast_id,))
            ]

        for query, params in delete_queries:
            cursor.execute(query, params)

        # Update UserStats table
        if database_type == "postgresql":
            query = 'UPDATE "UserStats" SET PodcastsAdded = GREATEST(PodcastsAdded - 1, 0) WHERE UserID = %s'
        else:  # MySQL or MariaDB
            query = "UPDATE UserStats SET PodcastsAdded = GREATEST(PodcastsAdded - 1, 0) WHERE UserID = %s"

        cursor.execute(query, (user_id,))
        cnx.commit()

    except (psycopg.Error, mysql.connector.Error) as err:
        print(f"Database Error: {err}")
        cnx.rollback()
        raise
    except Exception as e:
        print(f"General Error in remove_youtube_channel_by_url: {e}")
        cnx.rollback()
        raise
    finally:
        cursor.close()

def remove_podcast(cnx, database_type, podcast_name, podcast_url, user_id):
    cursor = cnx.cursor()
    print('got to remove')
    try:
        # Get the PodcastID first
        if database_type == "postgresql":
            select_podcast_id = 'SELECT PodcastID FROM "Podcasts" WHERE PodcastName = %s AND FeedURL = %s AND UserID = %s'
        else:  # MySQL or MariaDB
            select_podcast_id = "SELECT PodcastID FROM Podcasts WHERE PodcastName = %s AND FeedURL = %s AND UserID = %s"

        cursor.execute(select_podcast_id, (podcast_name, podcast_url, user_id))
        result = cursor.fetchone()

        if result:
            podcast_id = result[0] if not isinstance(result, dict) else result.get('podcastid')
        else:
            raise ValueError(f"No podcast found with name {podcast_name}")

        # Special handling for initialization-added feeds
        if podcast_url == "https://news.pinepods.online/feed.xml":
            # First, delete all related entries manually to avoid foreign key issues
            if database_type == "postgresql":
                queries = [
                    'DELETE FROM "UserEpisodeHistory" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s)',
                    'DELETE FROM "DownloadedEpisodes" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s)',
                    'DELETE FROM "SavedEpisodes" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s)',
                    'DELETE FROM "EpisodeQueue" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s)',
                    'DELETE FROM "Episodes" WHERE PodcastID = %s',
                    'DELETE FROM "Podcasts" WHERE PodcastID = %s',
                    'UPDATE "AppSettings" SET NewsFeedSubscribed = FALSE'
                ]
            else:  # MySQL or MariaDB
                queries = [
                    "DELETE FROM UserEpisodeHistory WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = %s)",
                    "DELETE FROM DownloadedEpisodes WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = %s)",
                    "DELETE FROM SavedEpisodes WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = %s)",
                    "DELETE FROM EpisodeQueue WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = %s)",
                    "SET FOREIGN_KEY_CHECKS = 0",
                    "DELETE FROM Episodes WHERE PodcastID = %s",
                    "DELETE FROM Podcasts WHERE PodcastID = %s",
                    "SET FOREIGN_KEY_CHECKS = 1",
                    "UPDATE AppSettings SET NewsFeedSubscribed = 0"
                ]

            for query in queries:
                if query.startswith('SET'):
                    cursor.execute(query)
                elif query.startswith('UPDATE'):
                    cursor.execute(query)
                else:
                    cursor.execute(query, (podcast_id,))

        else:
            # Normal podcast deletion process
            if database_type == "postgresql":
                delete_queries = [
                    ('DELETE FROM "UserEpisodeHistory" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s)', (podcast_id,)),
                    ('DELETE FROM "DownloadedEpisodes" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s)', (podcast_id,)),
                    ('DELETE FROM "SavedEpisodes" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s)', (podcast_id,)),
                    ('DELETE FROM "EpisodeQueue" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s)', (podcast_id,)),
                    ('DELETE FROM "Episodes" WHERE PodcastID = %s', (podcast_id,)),
                    ('DELETE FROM "Podcasts" WHERE PodcastID = %s', (podcast_id,))
                ]
            else:  # MySQL or MariaDB
                delete_queries = [
                    ("DELETE FROM UserEpisodeHistory WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = %s)", (podcast_id,)),
                    ("DELETE FROM DownloadedEpisodes WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = %s)", (podcast_id,)),
                    ("DELETE FROM SavedEpisodes WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = %s)", (podcast_id,)),
                    ("DELETE FROM EpisodeQueue WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = %s)", (podcast_id,)),
                    ("DELETE FROM Episodes WHERE PodcastID = %s", (podcast_id,)),
                    ("DELETE FROM Podcasts WHERE PodcastID = %s", (podcast_id,))
                ]

            for query, params in delete_queries:
                cursor.execute(query, params)

        # Update UserStats table to decrement PodcastsAdded count
        if database_type == "postgresql":
            query = 'UPDATE "UserStats" SET PodcastsAdded = GREATEST(PodcastsAdded - 1, 0) WHERE UserID = %s'
        else:  # MySQL or MariaDB
            query = "UPDATE UserStats SET PodcastsAdded = GREATEST(PodcastsAdded - 1, 0) WHERE UserID = %s"

        cursor.execute(query, (user_id,))
        cnx.commit()

    except (psycopg.Error, mysql.connector.Error) as err:
        print(f"Database Error: {err}")
        cnx.rollback()
        raise
    except Exception as e:
        print(f"General Error in remove_podcast: {e}")
        cnx.rollback()
        raise
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

def remove_youtube_channel(cnx, database_type, podcast_id, user_id):
    cursor = cnx.cursor()
    try:
        # Delete from the related tables
        if database_type == "postgresql":
            delete_history = 'DELETE FROM "UserEpisodeHistory" WHERE EpisodeID IN (SELECT VideoID FROM "YouTubeVideos" WHERE PodcastID = %s)'
            delete_downloaded = 'DELETE FROM "DownloadedEpisodes" WHERE EpisodeID IN (SELECT VideoID FROM "YouTubeVideos" WHERE PodcastID = %s)'
            delete_saved = 'DELETE FROM "SavedEpisodes" WHERE EpisodeID IN (SELECT VideoID FROM "YouTubeVideos" WHERE PodcastID = %s)'
            delete_queue = 'DELETE FROM "EpisodeQueue" WHERE EpisodeID IN (SELECT VideoID FROM "YouTubeVideos" WHERE PodcastID = %s)'
            delete_videos = 'DELETE FROM "YouTubeVideos" WHERE PodcastID = %s'
            delete_podcast = 'DELETE FROM "Podcasts" WHERE PodcastID = %s AND IsYouTubeChannel = TRUE'
            update_user_stats = 'UPDATE "UserStats" SET PodcastsAdded = PodcastsAdded - 1 WHERE UserID = %s'
        else:  # MySQL or MariaDB
            delete_history = "DELETE FROM UserEpisodeHistory WHERE EpisodeID IN (SELECT VideoID FROM YouTubeVideos WHERE PodcastID = %s)"
            delete_downloaded = "DELETE FROM DownloadedEpisodes WHERE EpisodeID IN (SELECT VideoID FROM YouTubeVideos WHERE PodcastID = %s)"
            delete_saved = "DELETE FROM SavedEpisodes WHERE EpisodeID IN (SELECT VideoID FROM YouTubeVideos WHERE PodcastID = %s)"
            delete_queue = "DELETE FROM EpisodeQueue WHERE EpisodeID IN (SELECT VideoID FROM YouTubeVideos WHERE PodcastID = %s)"
            delete_videos = "DELETE FROM YouTubeVideos WHERE PodcastID = %s"
            delete_podcast = "DELETE FROM Podcasts WHERE PodcastID = %s AND IsYouTubeChannel = TRUE"
            update_user_stats = "UPDATE UserStats SET PodcastsAdded = PodcastsAdded - 1 WHERE UserID = %s"

        cursor.execute(delete_history, (podcast_id,))
        cursor.execute(delete_downloaded, (podcast_id,))
        cursor.execute(delete_saved, (podcast_id,))
        cursor.execute(delete_queue, (podcast_id,))
        cursor.execute(delete_videos, (podcast_id,))
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
        query = """
            SELECT * FROM (
                SELECT
                    "Podcasts".PodcastName as podcastname,
                    "Episodes".EpisodeTitle as episodetitle,
                    "Episodes".EpisodePubDate as episodepubdate,
                    "Episodes".EpisodeDescription as episodedescription,
                    "Episodes".EpisodeArtwork as episodeartwork,
                    "Episodes".EpisodeURL as episodeurl,
                    "Episodes".EpisodeDuration as episodeduration,
                    "UserEpisodeHistory".ListenDuration as listenduration,
                    "Episodes".EpisodeID as episodeid,
                    "Episodes".Completed as completed,
                    CASE WHEN "SavedEpisodes".EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                    CASE WHEN "EpisodeQueue".EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS queued,
                    CASE WHEN "DownloadedEpisodes".EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                    FALSE as is_youtube
                FROM "Episodes"
                INNER JOIN "Podcasts" ON "Episodes".PodcastID = "Podcasts".PodcastID
                LEFT JOIN "UserEpisodeHistory" ON
                    "Episodes".EpisodeID = "UserEpisodeHistory".EpisodeID
                    AND "UserEpisodeHistory".UserID = %s
                LEFT JOIN "SavedEpisodes" ON
                    "Episodes".EpisodeID = "SavedEpisodes".EpisodeID
                    AND "SavedEpisodes".UserID = %s
                LEFT JOIN "EpisodeQueue" ON
                    "Episodes".EpisodeID = "EpisodeQueue".EpisodeID
                    AND "EpisodeQueue".UserID = %s
                LEFT JOIN "DownloadedEpisodes" ON
                    "Episodes".EpisodeID = "DownloadedEpisodes".EpisodeID
                    AND "DownloadedEpisodes".UserID = %s
                WHERE "Episodes".EpisodePubDate >= NOW() - INTERVAL '30 days'
                AND "Podcasts".UserID = %s

                UNION ALL

                SELECT
                    "Podcasts".PodcastName as podcastname,
                    "YouTubeVideos".VideoTitle as episodetitle,
                    "YouTubeVideos".PublishedAt as episodepubdate,
                    "YouTubeVideos".VideoDescription as episodedescription,
                    "YouTubeVideos".ThumbnailURL as episodeartwork,
                    "YouTubeVideos".VideoURL as episodeurl,
                    "YouTubeVideos".Duration as episodeduration,
                    "YouTubeVideos".ListenPosition as listenduration,
                    "YouTubeVideos".VideoID as episodeid,
                    "YouTubeVideos".Completed as completed,
                    CASE WHEN "SavedVideos".VideoID IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                    CASE WHEN "EpisodeQueue".EpisodeID IS NOT NULL AND "EpisodeQueue".is_youtube = TRUE THEN TRUE ELSE FALSE END AS queued,
                    CASE WHEN "DownloadedVideos".VideoID IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                    TRUE as is_youtube
                FROM "YouTubeVideos"
                INNER JOIN "Podcasts" ON "YouTubeVideos".PodcastID = "Podcasts".PodcastID
                LEFT JOIN "SavedVideos" ON
                    "YouTubeVideos".VideoID = "SavedVideos".VideoID
                    AND "SavedVideos".UserID = %s
                LEFT JOIN "EpisodeQueue" ON
                    "YouTubeVideos".VideoID = "EpisodeQueue".EpisodeID
                    AND "EpisodeQueue".UserID = %s
                    AND "EpisodeQueue".is_youtube = TRUE
                LEFT JOIN "DownloadedVideos" ON
                    "YouTubeVideos".VideoID = "DownloadedVideos".VideoID
                    AND "DownloadedVideos".UserID = %s
                WHERE "YouTubeVideos".PublishedAt >= NOW() - INTERVAL '30 days'
                AND "Podcasts".UserID = %s
            ) combined
            ORDER BY episodepubdate DESC
        """
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(dictionary=True)
        query = """
            SELECT * FROM (
                SELECT
                    Podcasts.PodcastName as podcastname,
                    Episodes.EpisodeTitle as episodetitle,
                    Episodes.EpisodePubDate as episodepubdate,
                    Episodes.EpisodeDescription as episodedescription,
                    Episodes.EpisodeArtwork as episodeartwork,
                    Episodes.EpisodeURL as episodeurl,
                    Episodes.EpisodeDuration as episodeduration,
                    UserEpisodeHistory.ListenDuration as listenduration,
                    Episodes.EpisodeID as episodeid,
                    Episodes.Completed as completed,
                    CASE WHEN SavedEpisodes.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                    CASE WHEN EpisodeQueue.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS queued,
                    CASE WHEN DownloadedEpisodes.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                    FALSE as is_youtube
                FROM Episodes
                INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID
                LEFT JOIN UserEpisodeHistory ON
                    Episodes.EpisodeID = UserEpisodeHistory.EpisodeID
                    AND UserEpisodeHistory.UserID = %s
                LEFT JOIN SavedEpisodes ON
                    Episodes.EpisodeID = SavedEpisodes.EpisodeID
                    AND SavedEpisodes.UserID = %s
                LEFT JOIN EpisodeQueue ON
                    Episodes.EpisodeID = EpisodeQueue.EpisodeID
                    AND EpisodeQueue.UserID = %s
                LEFT JOIN DownloadedEpisodes ON
                    Episodes.EpisodeID = DownloadedEpisodes.EpisodeID
                    AND DownloadedEpisodes.UserID = %s
                WHERE Episodes.EpisodePubDate >= DATE_SUB(NOW(), INTERVAL 30 DAY)
                AND Podcasts.UserID = %s

                UNION ALL

                SELECT
                    Podcasts.PodcastName as podcastname,
                    YouTubeVideos.VideoTitle as episodetitle,
                    YouTubeVideos.PublishedAt as episodepubdate,
                    YouTubeVideos.VideoDescription as episodedescription,
                    YouTubeVideos.ThumbnailURL as episodeartwork,
                    YouTubeVideos.VideoURL as episodeurl,
                    YouTubeVideos.Duration as episodeduration,
                    YouTubeVideos.ListenPosition as listenduration,
                    YouTubeVideos.VideoID as episodeid,
                    YouTubeVideos.Completed as completed,
                    CASE WHEN SavedVideos.VideoID IS NOT NULL THEN 1 ELSE 0 END AS saved,
                    CASE WHEN EpisodeQueue.EpisodeID IS NOT NULL AND EpisodeQueue.is_youtube = 1 THEN 1 ELSE 0 END AS queued,
                    CASE WHEN DownloadedVideos.VideoID IS NOT NULL THEN 1 ELSE 0 END AS downloaded,
                    1 as is_youtube
                FROM YouTubeVideos
                INNER JOIN Podcasts ON YouTubeVideos.PodcastID = Podcasts.PodcastID
                LEFT JOIN SavedVideos ON
                    YouTubeVideos.VideoID = SavedVideos.VideoID
                    AND SavedVideos.UserID = %s
                LEFT JOIN EpisodeQueue ON
                    YouTubeVideos.VideoID = EpisodeQueue.EpisodeID
                    AND EpisodeQueue.UserID = %s
                    AND EpisodeQueue.is_youtube = 1
                LEFT JOIN DownloadedVideos ON
                    YouTubeVideos.VideoID = DownloadedVideos.VideoID
                    AND DownloadedVideos.UserID = %s
                WHERE YouTubeVideos.PublishedAt >= DATE_SUB(NOW(), INTERVAL 30 DAY)
                AND Podcasts.UserID = %s
            ) combined
            ORDER BY episodepubdate DESC
        """

    # Execute with all params for both unions
    params = (user_id,) * 9  # user_id repeated 9 times for all the places needed
    cursor.execute(query, params)
    rows = cursor.fetchall()
    cursor.close()

    if not rows:
        return []

    if database_type != "postgresql":
        # Convert column names to lowercase for MySQL and ensure boolean fields are actual booleans
        bool_fields = ['completed', 'saved', 'queued', 'downloaded', 'is_youtube']
        rows = [{k.lower(): (bool(v) if k.lower() in bool_fields else v)
                for k, v in row.items()} for row in rows]

    return rows

def return_person_episodes(database_type, cnx, user_id: int, person_id: int):
    if database_type == "postgresql":
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
    else:
        cursor = cnx.cursor(dictionary=True)

    try:
        if database_type == "postgresql":
            query = """
            SELECT
                e.EpisodeID,  -- Will be NULL if no match in Episodes table
                pe.EpisodeTitle,
                pe.EpisodeDescription,
                pe.EpisodeURL,
                CASE
                    WHEN pe.EpisodeArtwork IS NULL THEN
                        (SELECT ArtworkURL FROM "Podcasts" WHERE PodcastID = pe.PodcastID)
                    ELSE pe.EpisodeArtwork
                END as EpisodeArtwork,
                pe.EpisodePubDate,
                pe.EpisodeDuration,
                p.PodcastName,
                CASE
                    WHEN (
                        SELECT 1 FROM "Podcasts"
                        WHERE PodcastID = pe.PodcastID
                        AND UserID = %s
                    ) IS NOT NULL THEN
                    CASE
                        WHEN s.EpisodeID IS NOT NULL THEN TRUE
                        ELSE FALSE
                    END
                    ELSE FALSE
                END AS Saved,
                CASE
                    WHEN (
                        SELECT 1 FROM "Podcasts"
                        WHERE PodcastID = pe.PodcastID
                        AND UserID = %s
                    ) IS NOT NULL THEN
                    CASE
                        WHEN d.EpisodeID IS NOT NULL THEN TRUE
                        ELSE FALSE
                    END
                    ELSE FALSE
                END AS Downloaded,
                CASE
                    WHEN (
                        SELECT 1 FROM "Podcasts"
                        WHERE PodcastID = pe.PodcastID
                        AND UserID = %s
                    ) IS NOT NULL THEN
                    COALESCE(h.ListenDuration, 0)
                    ELSE 0
                END AS ListenDuration,
                FALSE as is_youtube
            FROM "PeopleEpisodes" pe
            INNER JOIN "People" pp ON pe.PersonID = pp.PersonID
            INNER JOIN "Podcasts" p ON pe.PodcastID = p.PodcastID
            LEFT JOIN "Episodes" e ON e.EpisodeURL = pe.EpisodeURL AND e.PodcastID = pe.PodcastID
            LEFT JOIN (
                SELECT * FROM "SavedEpisodes" WHERE UserID = %s
            ) s ON s.EpisodeID = e.EpisodeID
            LEFT JOIN (
                SELECT * FROM "DownloadedEpisodes" WHERE UserID = %s
            ) d ON d.EpisodeID = e.EpisodeID
            LEFT JOIN (
                SELECT * FROM "UserEpisodeHistory" WHERE UserID = %s
            ) h ON h.EpisodeID = e.EpisodeID
            WHERE pe.PersonID = %s
            AND pe.EpisodePubDate >= NOW() - INTERVAL '30 days'
            ORDER BY pe.EpisodePubDate DESC;
            """
        else:
            query = """
            SELECT
                e.EpisodeID,  -- Will be NULL if no match in Episodes table
                pe.EpisodeTitle,
                pe.EpisodeDescription,
                pe.EpisodeURL,
                COALESCE(pe.EpisodeArtwork, p.ArtworkURL) as EpisodeArtwork,
                pe.EpisodePubDate,
                pe.EpisodeDuration,
                p.PodcastName,
                IF(
                    EXISTS(
                        SELECT 1 FROM Podcasts
                        WHERE PodcastID = pe.PodcastID
                        AND UserID = %s
                    ),
                    IF(s.EpisodeID IS NOT NULL, TRUE, FALSE),
                    FALSE
                ) AS Saved,
                IF(
                    EXISTS(
                        SELECT 1 FROM Podcasts
                        WHERE PodcastID = pe.PodcastID
                        AND UserID = %s
                    ),
                    IF(d.EpisodeID IS NOT NULL, TRUE, FALSE),
                    FALSE
                ) AS Downloaded,
                IF(
                    EXISTS(
                        SELECT 1 FROM Podcasts
                        WHERE PodcastID = pe.PodcastID
                        AND UserID = %s
                    ),
                    COALESCE(h.ListenDuration, 0),
                    0
                ) AS ListenDuration,
                FALSE as is_youtube
            FROM PeopleEpisodes pe
            INNER JOIN People pp ON pe.PersonID = pp.PersonID
            INNER JOIN Podcasts p ON pe.PodcastID = p.PodcastID
            LEFT JOIN Episodes e ON e.EpisodeURL = pe.EpisodeURL AND e.PodcastID = pe.PodcastID
            LEFT JOIN (
                SELECT * FROM SavedEpisodes WHERE UserID = %s
            ) s ON s.EpisodeID = e.EpisodeID
            LEFT JOIN (
                SELECT * FROM DownloadedEpisodes WHERE UserID = %s
            ) d ON d.EpisodeID = e.EpisodeID
            LEFT JOIN (
                SELECT * FROM UserEpisodeHistory WHERE UserID = %s
            ) h ON h.EpisodeID = e.EpisodeID
            WHERE pe.PersonID = %s
            AND pe.EpisodePubDate >= DATE_SUB(NOW(), INTERVAL 30 DAY)
            ORDER BY pe.EpisodePubDate DESC;
            """

        cursor.execute(query, (user_id,) * 6 + (person_id,))
        rows = cursor.fetchall()

        if not rows:
            return []

        if database_type != "postgresql":
            rows = [{k.lower(): (bool(v) if k.lower() in ['saved', 'downloaded'] else v)
                    for k, v in row.items()} for row in rows]

        return rows

    except Exception as e:
        print(f"Error fetching person episodes: {e}")
        return None
    finally:
        cursor.close()

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
            '"Episodes".Completed, '
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
            "Episodes.Completed, "
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

    # Normalize keys
    rows = capitalize_keys(rows)

    if database_type != "postgresql":
        for row in rows:
            row['Completed'] = bool(row['Completed'])

    return rows or None

def return_youtube_episodes(database_type, cnx, user_id, podcast_id):
    if database_type == "postgresql":
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
        cursor = cnx.cursor(dictionary=True)

    if database_type == "postgresql":
        query = (
            'SELECT "Podcasts".PodcastID, "Podcasts".PodcastName, "YouTubeVideos".VideoID AS EpisodeID, '
            '"YouTubeVideos".VideoTitle AS EpisodeTitle, "YouTubeVideos".PublishedAt AS EpisodePubDate, '
            '"YouTubeVideos".VideoDescription AS EpisodeDescription, '
            '"YouTubeVideos".ThumbnailURL AS EpisodeArtwork, "YouTubeVideos".VideoURL AS EpisodeURL, '
            '"YouTubeVideos".Duration AS EpisodeDuration, '
            '"YouTubeVideos".ListenPosition AS ListenDuration, '
            '"YouTubeVideos".YouTubeVideoID AS guid '
            'FROM "YouTubeVideos" '
            'INNER JOIN "Podcasts" ON "YouTubeVideos".PodcastID = "Podcasts".PodcastID '
            'WHERE "Podcasts".PodcastID = %s AND "Podcasts".UserID = %s '
            'ORDER BY "YouTubeVideos".PublishedAt DESC'
        )
    else:  # MySQL or MariaDB
        query = (
            "SELECT Podcasts.PodcastID, Podcasts.PodcastName, YouTubeVideos.VideoID AS EpisodeID, "
            "YouTubeVideos.VideoTitle AS EpisodeTitle, YouTubeVideos.PublishedAt AS EpisodePubDate, "
            "YouTubeVideos.VideoDescription AS EpisodeDescription, "
            "YouTubeVideos.ThumbnailURL AS EpisodeArtwork, YouTubeVideos.VideoURL AS EpisodeURL, "
            "YouTubeVideos.Duration AS EpisodeDuration, "
            "YouTubeVideos.ListenPosition AS ListenDuration, "
            "YouTubeVideos.YouTubeVideoID AS guid "
            "FROM YouTubeVideos "
            "INNER JOIN Podcasts ON YouTubeVideos.PodcastID = Podcasts.PodcastID "
            "WHERE Podcasts.PodcastID = %s AND Podcasts.UserID = %s "
            "ORDER BY YouTubeVideos.PublishedAt DESC"
        )

    cursor.execute(query, (podcast_id, user_id))
    rows = cursor.fetchall()
    cursor.close()

    # Normalize keys
    rows = capitalize_keys(rows)
    return rows or None

def get_podcast_details(database_type, cnx, user_id, podcast_id):
    if isinstance(podcast_id, tuple):
        pod_id, episode_id = podcast_episode
    else:
        pod_id = podcast_id
        episode_id = None

    if database_type == "postgresql":
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
    else:
        cursor = cnx.cursor(dictionary=True)

    print(f"pulling pod deets for podcast ID: {pod_id}, episode ID: {episode_id}")

    if database_type == "postgresql":
        query = """
            SELECT *
            FROM "Podcasts"
            WHERE PodcastID = %s AND UserID = %s
        """
    else:
        query = """
            SELECT *
            FROM Podcasts
            WHERE PodcastID = %s AND UserID = %s
        """

    cursor.execute(query, (pod_id, user_id))
    details = cursor.fetchone()

    if not details:
        cursor.execute(query, (pod_id, 1))
        details = cursor.fetchone()

    if details:
        lower_row = lowercase_keys(details)

        # Only get count from YouTubeVideos if this is a YouTube channel
        if lower_row.get('isyoutubechannel', False):
            if database_type == "postgresql":
                count_query = """
                    SELECT COUNT(*) as count
                    FROM "YouTubeVideos"
                    WHERE PodcastID = %s
                """
            else:
                count_query = """
                    SELECT COUNT(*) as count
                    FROM YouTubeVideos
                    WHERE PodcastID = %s
                """

            cursor.execute(count_query, (pod_id,))
            count_result = cursor.fetchone()
            episode_count = count_result['count'] if isinstance(count_result, dict) else count_result[0]
            lower_row['episodecount'] = episode_count

        if database_type != "postgresql":
            lower_row['explicit'] = bool(lower_row.get('explicit', 0))
            lower_row['isyoutubechannel'] = bool(lower_row.get('isyoutubechannel', 0))
            # You might also want to handle autodownload if it's used in the frontend
            lower_row['autodownload'] = bool(lower_row.get('autodownload', 0))

        bool_fix = convert_bools(lower_row, database_type)
        cursor.close()
        return bool_fix

    cursor.close()
    return None


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

def delete_episode(database_type, cnx, episode_id, user_id, is_youtube=False):
    cursor = cnx.cursor()
    try:
        if is_youtube:
            # Get the download ID from the DownloadedVideos table
            if database_type == "postgresql":
                query = (
                    'SELECT DownloadID, DownloadedLocation '
                    'FROM "DownloadedVideos" '
                    'INNER JOIN "YouTubeVideos" ON "DownloadedVideos".VideoID = "YouTubeVideos".VideoID '
                    'INNER JOIN "Podcasts" ON "YouTubeVideos".PodcastID = "Podcasts".PodcastID '
                    'WHERE "YouTubeVideos".VideoID = %s AND "Podcasts".UserID = %s'
                )
            else:
                query = (
                    "SELECT DownloadID, DownloadedLocation "
                    "FROM DownloadedVideos "
                    "INNER JOIN YouTubeVideos ON DownloadedVideos.VideoID = YouTubeVideos.VideoID "
                    "INNER JOIN Podcasts ON YouTubeVideos.PodcastID = Podcasts.PodcastID "
                    "WHERE YouTubeVideos.VideoID = %s AND Podcasts.UserID = %s"
                )
        else:
            # Original podcast episode query
            if database_type == "postgresql":
                query = (
                    'SELECT DownloadID, DownloadedLocation '
                    'FROM "DownloadedEpisodes" '
                    'INNER JOIN "Episodes" ON "DownloadedEpisodes".EpisodeID = "Episodes".EpisodeID '
                    'INNER JOIN "Podcasts" ON "Episodes".PodcastID = "Podcasts".PodcastID '
                    'WHERE "Episodes".EpisodeID = %s AND "Podcasts".UserID = %s'
                )
            else:
                query = (
                    "SELECT DownloadID, DownloadedLocation "
                    "FROM DownloadedEpisodes "
                    "INNER JOIN Episodes ON DownloadedEpisodes.EpisodeID = Episodes.EpisodeID "
                    "INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID "
                    "WHERE Episodes.EpisodeID = %s AND Podcasts.UserID = %s"
                )

        logging.debug(f"Executing query: {query} with ID: {episode_id} and UserID: {user_id}")
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

        # Delete the downloaded file (but not source YouTube file)
        if downloaded_location and os.path.exists(downloaded_location):
            if is_youtube:
                # Only delete if it's not in the YouTube source directory
                if not downloaded_location.startswith("/opt/pinepods/downloads/youtube/"):
                    os.remove(downloaded_location)
            else:
                os.remove(downloaded_location)
        else:
            logging.warning(f"Downloaded file not found: {downloaded_location}")

        # Remove the entry from the appropriate downloads table
        if is_youtube:
            if database_type == "postgresql":
                query = 'DELETE FROM "DownloadedVideos" WHERE DownloadID = %s'
            else:
                query = "DELETE FROM DownloadedVideos WHERE DownloadID = %s"
        else:
            if database_type == "postgresql":
                query = 'DELETE FROM "DownloadedEpisodes" WHERE DownloadID = %s'
            else:
                query = "DELETE FROM DownloadedEpisodes WHERE DownloadID = %s"

        cursor.execute(query, (download_id,))
        cnx.commit()
        logging.info(f"Removed {cursor.rowcount} entry from the downloads table.")

        # Update UserStats table
        if database_type == "postgresql":
            query = 'UPDATE "UserStats" SET EpisodesDownloaded = EpisodesDownloaded - 1 WHERE UserID = %s'
        else:
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
    else:
        cursor = cnx.cursor(dictionary=True)

    # Base query remains the same but handles nulls and empty strings with NULLIF
    if database_type == "postgresql":
        query = """
            SELECT
                p.PodcastID,
                COALESCE(NULLIF(p.PodcastName, ''), 'Unknown Podcast') as PodcastName,
                COALESCE(NULLIF(p.ArtworkURL, ''), '/static/assets/default-podcast.png') as ArtworkURL,
                COALESCE(NULLIF(p.Description, ''), 'No description available') as Description,
                COALESCE(p.EpisodeCount, 0) as EpisodeCount,
                COALESCE(NULLIF(p.WebsiteURL, ''), '') as WebsiteURL,
                COALESCE(NULLIF(p.FeedURL, ''), '') as FeedURL,
                COALESCE(NULLIF(p.Author, ''), 'Unknown Author') as Author,
                COALESCE(NULLIF(p.Categories, ''), '') as Categories,
                COALESCE(p.Explicit, FALSE) as Explicit,
                COALESCE(p.PodcastIndexID, 0) as PodcastIndexID,
                COUNT(DISTINCT h.UserEpisodeHistoryID) as play_count,
                MIN(e.EpisodePubDate) as oldest_episode_date,
                COALESCE(
                    (SELECT COUNT(DISTINCT ueh.EpisodeID)
                     FROM "UserEpisodeHistory" ueh
                     JOIN "Episodes" ep ON ueh.EpisodeID = ep.EpisodeID
                     WHERE ep.PodcastID = p.PodcastID
                     AND ueh.UserID = %s),
                    0
                ) as episodes_played
            FROM "Podcasts" p
            LEFT JOIN "Episodes" e ON p.PodcastID = e.PodcastID
            LEFT JOIN "UserEpisodeHistory" h ON e.EpisodeID = h.EpisodeID AND h.UserID = %s
            WHERE p.UserID = %s
            GROUP BY p.PodcastID
        """
    else:  # MySQL/MariaDB version
        query = """
            SELECT
                p.PodcastID,
                COALESCE(NULLIF(p.PodcastName, ''), 'Unknown Podcast') as PodcastName,
                COALESCE(NULLIF(p.ArtworkURL, ''), '/static/assets/default-podcast.png') as ArtworkURL,
                COALESCE(NULLIF(p.Description, ''), 'No description available') as Description,
                COALESCE(p.EpisodeCount, 0) as EpisodeCount,
                COALESCE(NULLIF(p.WebsiteURL, ''), '') as WebsiteURL,
                COALESCE(NULLIF(p.FeedURL, ''), '') as FeedURL,
                COALESCE(NULLIF(p.Author, ''), 'Unknown Author') as Author,
                COALESCE(NULLIF(p.Categories, ''), '') as Categories,
                COALESCE(p.Explicit, FALSE) as Explicit,
                COALESCE(p.PodcastIndexID, 0) as PodcastIndexID,
                COUNT(DISTINCT h.UserEpisodeHistoryID) as play_count,
                MIN(e.EpisodePubDate) as oldest_episode_date,
                COALESCE(
                    (SELECT COUNT(DISTINCT ueh.EpisodeID)
                     FROM UserEpisodeHistory ueh
                     JOIN Episodes ep ON ueh.EpisodeID = ep.EpisodeID
                     WHERE ep.PodcastID = p.PodcastID
                     AND ueh.UserID = %s),
                    0
                ) as episodes_played
            FROM Podcasts p
            LEFT JOIN Episodes e ON p.PodcastID = e.PodcastID
            LEFT JOIN UserEpisodeHistory h ON e.EpisodeID = h.EpisodeID AND h.UserID = %s
            WHERE p.UserID = %s
            GROUP BY p.PodcastID
        """

    try:
        cursor.execute(query, (user_id, user_id, user_id))
        rows = cursor.fetchall()
    except Exception as e:
        logging.error(f"Database error in return_pods: {str(e)}")
        return []
    finally:
        cursor.close()

    if not rows:
        return []

    # Process all rows, regardless of database type
    processed_rows = []
    for row in rows:
        # Convert to lowercase keys for consistency
        processed_row = {k.lower(): v for k, v in row.items()}

        # Define default values
        defaults = {
            'podcastname': 'Unknown Podcast',
            'artworkurl': '/static/assets/logo_random/11.jpeg',
            'description': 'No description available',
            'episodecount': 0,
            'websiteurl': '',
            'feedurl': '',
            'author': 'Unknown Author',
            'categories': '',
            'explicit': False,
            'podcastindexid': 0,
            'play_count': 0,
            'episodes_played': 0
        }

        # Apply defaults for any missing or null values
        for key, default_value in defaults.items():
            if key not in processed_row or processed_row[key] is None or processed_row[key] == "":
                processed_row[key] = default_value

        processed_rows.append(processed_row)

    return processed_rows

def check_self_service(cnx, database_type):
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = 'SELECT SelfServiceUser FROM "AppSettings"'
    else:  # MySQL or MariaDB
        query = "SELECT SelfServiceUser FROM AppSettings"
    cursor.execute(query)
    result = cursor.fetchone()
    cursor.close()

    if database_type == "postgresql":
        print(f'debug result: {result}')
        logging.debug(f'debug result: {result}')
        self_service = result['selfserviceuser'] if isinstance(result, dict) else result[0]
    else:  # MySQL or MariaDB
        self_service = result[0]

    if self_service == 1:
        return True
    elif self_service == 0:
        return False
    else:
        return None

def refresh_pods_for_user(cnx, database_type, podcast_id):
    print(f'Refresh begin for podcast {podcast_id}')
    cursor = cnx.cursor()
    if database_type == "postgresql":
        select_podcast = '''
            SELECT "podcastid", "feedurl", "artworkurl", "autodownload", "username", "password",
                   "isyoutubechannel", COALESCE("feedurl", '') as channel_id
            FROM "Podcasts"
            WHERE "podcastid" = %s
        '''
    else:  # MySQL or MariaDB
        select_podcast = '''
            SELECT PodcastID, FeedURL, ArtworkURL, AutoDownload, Username, Password,
                   IsYouTubeChannel, COALESCE(FeedURL, '') as channel_id
            FROM Podcasts
            WHERE PodcastID = %s
        '''
    cursor.execute(select_podcast, (podcast_id,))
    result = cursor.fetchone()
    new_episodes = []

    if result:
        if isinstance(result, dict):
            if database_type == "postgresql":
                # PostgreSQL - lowercase keys
                podcast_id = result['podcastid']
                feed_url = result['feedurl']
                artwork_url = result['artworkurl']
                auto_download = result['autodownload']
                username = result['username']
                password = result['password']
                is_youtube = result['isyoutubechannel']
                channel_id = result['channel_id']
            else:
                # MariaDB - uppercase keys
                podcast_id = result['PodcastID']
                feed_url = result['FeedURL']
                artwork_url = result['ArtworkURL']
                auto_download = result['AutoDownload']
                username = result['Username']
                password = result['Password']
                is_youtube = result['IsYouTubeChannel']
                channel_id = result['channel_id']
        else:
            podcast_id, feed_url, artwork_url, auto_download, username, password, is_youtube, channel_id = result

        print(f'Processing podcast: {podcast_id}')
        if is_youtube:
            channel_id = feed_url.split('channel/')[-1] if 'channel/' in feed_url else feed_url
            channel_id = channel_id.split('/')[0].split('?')[0]
            youtube.process_youtube_videos(database_type, podcast_id, channel_id, cnx)
        else:
            episodes = add_episodes(cnx, database_type, podcast_id, feed_url,
                                  artwork_url, auto_download, username, password,
                                  websocket=True)
            new_episodes.extend(episodes)

    cursor.close()
    return new_episodes


def refresh_pods(cnx, database_type):
    print('refresh begin')
    cursor = cnx.cursor()
    if database_type == "postgresql":
        select_podcasts = '''
            SELECT PodcastID, FeedURL, ArtworkURL, AutoDownload, Username, Password,
                   IsYouTubeChannel, UserID, COALESCE(FeedURL, '') as channel_id
            FROM "Podcasts"
        '''
    else:
        select_podcasts = '''
            SELECT PodcastID, FeedURL, ArtworkURL, AutoDownload, Username, Password,
                   IsYouTubeChannel, UserID, COALESCE(FeedURL, '') as channel_id
            FROM Podcasts
        '''
    cursor.execute(select_podcasts)
    result_set = cursor.fetchall()
    for result in result_set:
        podcast_id = None
        try:
            if isinstance(result, tuple):
                podcast_id, feed_url, artwork_url, auto_download, username, password, is_youtube, user_id, channel_id = result
            elif isinstance(result, dict):
                if database_type == "postgresql":
                    podcast_id = result["podcastid"]
                    feed_url = result["feedurl"]
                    artwork_url = result["artworkurl"]
                    auto_download = result["autodownload"]
                    username = result["username"]
                    password = result["password"]
                    is_youtube = result["isyoutubechannel"]
                    user_id = result["userid"]
                    channel_id = result["channel_id"]
                else:
                    podcast_id = result["PodcastID"]
                    feed_url = result["FeedURL"]
                    artwork_url = result["ArtworkURL"]
                    auto_download = result["AutoDownload"]
                    username = result["Username"]
                    password = result["Password"]
                    is_youtube = result["IsYouTubeChannel"]
                    user_id = result["UserID"]
                    channel_id = result["channel_id"]
            else:
                raise ValueError(f"Unexpected result type: {type(result)}")
            print(f'Running for: {podcast_id}')
            if is_youtube:
                # Extract channel ID from feed URL
                channel_id = feed_url.split('channel/')[-1] if 'channel/' in feed_url else feed_url
                # Clean up any trailing slashes or query parameters
                channel_id = channel_id.split('/')[0].split('?')[0]
                youtube.process_youtube_videos(database_type, podcast_id, channel_id, cnx)
            else:
                add_episodes(cnx, database_type, podcast_id, feed_url, artwork_url,
                           auto_download, username, password, user_id)  # Added user_id here
        except Exception as e:
            print(f"Error refreshing podcast {podcast_id}: {str(e)}")
            continue
    cursor.close()



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
    #
def get_podcast_feed_by_id(cnx, database_type, podcast_id):
    cursor = cnx.cursor()

    # get the podcast ID for the specified title
    # get the podcast ID for the specified title
    if database_type == "postgresql":
        cursor.execute('SELECT FeedURL FROM "Podcasts" WHERE PodcastID = %s', (podcast_id,))
    else:  # MySQL or MariaDB
        cursor.execute("SELECT FeedURL FROM Podcasts WHERE PodcastID = %s", (podcast_id,))

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

def record_podcast_history(cnx, database_type, episode_id, user_id, episode_pos, is_youtube=False):
    from datetime import datetime
    cursor = cnx.cursor()
    now = datetime.now()
    new_listen_duration = round(episode_pos)

    if is_youtube:
        # Handle YouTube video history
        if database_type == "postgresql":
            check_history = 'SELECT UserVideoHistoryID FROM "UserVideoHistory" WHERE VideoID = %s AND UserID = %s'
        else:
            check_history = "SELECT UserVideoHistoryID FROM UserVideoHistory WHERE VideoID = %s AND UserID = %s"

        cursor.execute(check_history, (episode_id, user_id))
        result = cursor.fetchone()

        if result is not None:
            # Update existing video history
            history_id = get_hist_value(result, "UserVideoHistoryID")
            if history_id is not None:
                if database_type == "postgresql":
                    update_history = 'UPDATE "UserVideoHistory" SET ListenDuration = %s, ListenDate = %s WHERE UserVideoHistoryID = %s'
                else:
                    update_history = "UPDATE UserVideoHistory SET ListenDuration = %s, ListenDate = %s WHERE UserVideoHistoryID = %s"
                cursor.execute(update_history, (new_listen_duration, now, history_id))
        else:
            # Add new video history record
            if database_type == "postgresql":
                add_history = 'INSERT INTO "UserVideoHistory" (VideoID, UserID, ListenDuration, ListenDate) VALUES (%s, %s, %s, %s)'
            else:
                add_history = "INSERT INTO UserVideoHistory (VideoID, UserID, ListenDuration, ListenDate) VALUES (%s, %s, %s, %s)"
            cursor.execute(add_history, (episode_id, user_id, new_listen_duration, now))
    else:
        # Handle regular podcast episode history (existing logic)
        if database_type == "postgresql":
            check_history = 'SELECT UserEpisodeHistoryID FROM "UserEpisodeHistory" WHERE EpisodeID = %s AND UserID = %s'
        else:
            check_history = "SELECT UserEpisodeHistoryID FROM UserEpisodeHistory WHERE EpisodeID = %s AND UserID = %s"

        cursor.execute(check_history, (episode_id, user_id))
        result = cursor.fetchone()

        if result is not None:
            history_id = get_hist_value(result, "UserEpisodeHistoryID")
            if history_id is not None:
                if database_type == "postgresql":
                    update_history = 'UPDATE "UserEpisodeHistory" SET ListenDuration = %s, ListenDate = %s WHERE UserEpisodeHistoryID = %s'
                else:
                    update_history = "UPDATE UserEpisodeHistory SET ListenDuration = %s, ListenDate = %s WHERE UserEpisodeHistoryID = %s"
                cursor.execute(update_history, (new_listen_duration, now, history_id))
        else:
            if database_type == "postgresql":
                add_history = 'INSERT INTO "UserEpisodeHistory" (EpisodeID, UserID, ListenDuration, ListenDate) VALUES (%s, %s, %s, %s)'
            else:
                add_history = "INSERT INTO UserEpisodeHistory (EpisodeID, UserID, ListenDuration, ListenDate) VALUES (%s, %s, %s, %s)"
            cursor.execute(add_history, (episode_id, user_id, new_listen_duration, now))

    cnx.commit()
    cursor.close()


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

def get_existing_youtube_videos(cnx, database_type, podcast_id):
    """Get list of existing YouTube video URLs for a podcast"""
    cursor = cnx.cursor()
    if database_type == "postgresql":
        query = '''
            SELECT VideoURL FROM "YouTubeVideos"
            WHERE PodcastID = %s
        '''
    else:
        query = '''
            SELECT VideoURL FROM YouTubeVideos
            WHERE PodcastID = %s
        '''

    cursor.execute(query, (podcast_id,))
    results = cursor.fetchall()
    cursor.close()

    existing_urls = set()
    if results:
        for result in results:
            if isinstance(result, dict):
                url = result.get("videourl")
            elif isinstance(result, tuple):
                url = result[0]
            if url:
                existing_urls.add(url)

    return existing_urls

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
            query = """
                SELECT * FROM (
                    SELECT
                        "Episodes".EpisodeID as episodeid,
                        "UserEpisodeHistory".ListenDate as listendate,
                        "UserEpisodeHistory".ListenDuration as listenduration,
                        "Episodes".EpisodeTitle as episodetitle,
                        "Episodes".EpisodeDescription as episodedescription,
                        "Episodes".EpisodeArtwork as episodeartwork,
                        "Episodes".EpisodeURL as episodeurl,
                        "Episodes".EpisodeDuration as episodeduration,
                        "Podcasts".PodcastName as podcastname,
                        "Episodes".EpisodePubDate as episodepubdate,
                        "Episodes".Completed as completed,
                        FALSE as is_youtube
                    FROM "UserEpisodeHistory"
                    JOIN "Episodes" ON "UserEpisodeHistory".EpisodeID = "Episodes".EpisodeID
                    JOIN "Podcasts" ON "Episodes".PodcastID = "Podcasts".PodcastID
                    WHERE "UserEpisodeHistory".UserID = %s

                    UNION ALL

                    SELECT
                        "YouTubeVideos".VideoID as episodeid,
                        NULL as listendate,  -- YouTube doesn't track listen date currently
                        "YouTubeVideos".ListenPosition as listenduration,
                        "YouTubeVideos".VideoTitle as episodetitle,
                        "YouTubeVideos".VideoDescription as episodedescription,
                        "YouTubeVideos".ThumbnailURL as episodeartwork,
                        "YouTubeVideos".VideoURL as episodeurl,
                        "YouTubeVideos".Duration as episodeduration,
                        "Podcasts".PodcastName as podcastname,
                        "YouTubeVideos".PublishedAt as episodepubdate,
                        "YouTubeVideos".Completed as completed,
                        TRUE as is_youtube
                    FROM "YouTubeVideos"
                    JOIN "Podcasts" ON "YouTubeVideos".PodcastID = "Podcasts".PodcastID
                    WHERE "YouTubeVideos".ListenPosition > 0
                    AND "Podcasts".UserID = %s
                ) combined
                ORDER BY listendate DESC NULLS LAST
            """
        else:  # MySQL/MariaDB
            cursor = cnx.cursor(dictionary=True)
            query = """
                SELECT * FROM (
                    SELECT
                        Episodes.EpisodeID as episodeid,
                        UserEpisodeHistory.ListenDate as listendate,
                        UserEpisodeHistory.ListenDuration as listenduration,
                        Episodes.EpisodeTitle as episodetitle,
                        Episodes.EpisodeDescription as episodedescription,
                        Episodes.EpisodeArtwork as episodeartwork,
                        Episodes.EpisodeURL as episodeurl,
                        Episodes.EpisodeDuration as episodeduration,
                        Podcasts.PodcastName as podcastname,
                        Episodes.EpisodePubDate as episodepubdate,
                        Episodes.Completed as completed,
                        FALSE as is_youtube
                    FROM UserEpisodeHistory
                    JOIN Episodes ON UserEpisodeHistory.EpisodeID = Episodes.EpisodeID
                    JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID
                    WHERE UserEpisodeHistory.UserID = %s

                    UNION ALL

                    SELECT
                        YouTubeVideos.VideoID as episodeid,
                        NULL as listendate,
                        YouTubeVideos.ListenPosition as listenduration,
                        YouTubeVideos.VideoTitle as episodetitle,
                        YouTubeVideos.VideoDescription as episodedescription,
                        YouTubeVideos.ThumbnailURL as episodeartwork,
                        YouTubeVideos.VideoURL as episodeurl,
                        YouTubeVideos.Duration as episodeduration,
                        Podcasts.PodcastName as podcastname,
                        YouTubeVideos.PublishedAt as episodepubdate,
                        YouTubeVideos.Completed as completed,
                        TRUE as is_youtube
                    FROM YouTubeVideos
                    JOIN Podcasts ON YouTubeVideos.PodcastID = Podcasts.PodcastID
                    WHERE YouTubeVideos.ListenPosition > 0
                    AND Podcasts.UserID = %s
                ) combined
                ORDER BY listendate DESC
            """

        cursor.execute(query, (user_id, user_id))
        results = cursor.fetchall()
        if not results:
            logging.info("No results found for user history.")
            return []

        # Get column descriptions
        columns = [col[0].lower() for col in cursor.description]

        # Convert results to list of dictionaries
        history_episodes = []
        for row in results:
            episode = {}
            if isinstance(row, tuple):
                for idx, column_name in enumerate(columns):
                    value = row[idx]
                    if column_name in ['completed', 'is_youtube']:
                        value = bool(value)
                    episode[column_name] = value
            elif isinstance(row, dict):
                for k, v in row.items():
                    column_name = k.lower()
                    value = v
                    if column_name in ['completed', 'is_youtube']:
                        value = bool(value)
                    episode[column_name] = value
            else:
                logging.error(f"Unexpected row type: {type(row)}")
            history_episodes.append(episode)

        return lowercase_keys(history_episodes)

    except Exception as e:
        logging.error(f"Error executing user_history query: {e}")
        raise
    finally:
        cursor.close()

def download_podcast(cnx, database_type, episode_id, user_id):  # Fixed parameter name
    cursor = cnx.cursor()
    # First, get all the episode details we need
    if database_type == "postgresql":
        query = '''
            SELECT
                e.EpisodeID,
                e.PodcastID,
                e.EpisodeTitle,
                e.EpisodePubDate,
                e.EpisodeURL,
                e.EpisodeDescription,
                e.EpisodeArtwork,
                p.PodcastName,
                p.Author,
                p.ArtworkURL
            FROM "Episodes" e
            JOIN "Podcasts" p ON e.PodcastID = p.PodcastID
            WHERE e.EpisodeID = %s
        '''
    else:
        query = '''
            SELECT
                e.EpisodeID,
                e.PodcastID,
                e.EpisodeTitle,
                e.EpisodePubDate,
                e.EpisodeURL,
                e.EpisodeDescription,
                e.EpisodeArtwork,
                p.PodcastName,
                p.Author,
                p.ArtworkURL
            FROM Episodes e
            JOIN Podcasts p ON e.PodcastID = p.PodcastID
            WHERE e.EpisodeID = %s
        '''

    cursor.execute(query, (episode_id,))
    result = cursor.fetchone()

    if result is None:
        cursor.close()
        return False

    # Get values based on result type
    if isinstance(result, dict):
        episode_url = result.get('episodeurl') or result.get('EpisodeURL')
        podcast_name = result.get('podcastname') or result.get('PodcastName')
        episode_title = result.get('episodetitle') or result.get('EpisodeTitle')
        pub_date = result.get('episodepubdate') or result.get('EpisodePubDate')
        author = result.get('author') or result.get('Author')
        episode_artwork = result.get('episodeartwork') or result.get('EpisodeArtwork')
        artwork_url = result.get('artworkurl') or result.get('ArtworkURL')
    else:
        # Match positions from SELECT query
        episode_url = result[4]      # EpisodeURL
        podcast_name = result[7]     # PodcastName
        episode_title = result[2]    # EpisodeTitle
        pub_date = result[3]         # EpisodePubDate
        author = result[8]           # Author
        episode_artwork = result[6]   # EpisodeArtwork
        artwork_url = result[9]      # ArtworkURL

    # Get user's time and date preferences
    timezone, time_format, date_format = get_time_info(database_type, cnx, user_id)

    # Use default format if user preferences aren't set
    if not date_format:
        date_format = "ISO"

    # Format the publication date based on user preference
    date_format_map = {
        "ISO": "%Y-%m-%d",
        "USA": "%m/%d/%Y",
        "EUR": "%d.%m.%Y",
        "JIS": "%Y-%m-%d",
        "MDY": "%m-%d-%Y",
        "DMY": "%d-%m-%Y",
        "YMD": "%Y-%m-%d",
    }

    date_format_str = date_format_map.get(date_format, "%Y-%m-%d")  # Default to ISO if format not found
    pub_date_str = pub_date.strftime(date_format_str)

    # Clean filenames of invalid characters
    podcast_name = "".join(c for c in podcast_name if c.isalnum() or c in (' ', '-', '_')).strip()
    episode_title = "".join(c for c in episode_title if c.isalnum() or c in (' ', '-', '_')).strip()

    # Create the download directory
    download_dir = os.path.join("/opt/pinepods/downloads", podcast_name)
    os.makedirs(download_dir, exist_ok=True)
    uid = int(os.environ.get('PUID', 1000))
    gid = int(os.environ.get('PGID', 1000))
    os.chown(download_dir, uid, gid)

    # Generate filename with enhanced details
    filename = f"{pub_date_str}_{episode_title}_{user_id}-{episode_id}.mp3"
    file_path = os.path.join(download_dir, filename)

    # Check if already downloaded
    if database_type == "postgresql":
        query = 'SELECT 1 FROM "DownloadedEpisodes" WHERE EpisodeID = %s AND UserID = %s'
    else:
        query = "SELECT 1 FROM DownloadedEpisodes WHERE EpisodeID = %s AND UserID = %s"

    cursor.execute(query, (episode_id, user_id))
    if cursor.fetchone():
        cursor.close()
        return True

    if os.path.exists(file_path):
        # File exists but not in database, let's add the database entry
        downloaded_date = datetime.datetime.fromtimestamp(os.path.getctime(file_path))
        file_size = os.path.getsize(file_path)

        if database_type == "postgresql":
            query = '''
                INSERT INTO "DownloadedEpisodes"
                (UserID, EpisodeID, DownloadedDate, DownloadedSize, DownloadedLocation)
                VALUES (%s, %s, %s, %s, %s)
            '''
        else:
            query = '''
                INSERT INTO DownloadedEpisodes
                (UserID, EpisodeID, DownloadedDate, DownloadedSize, DownloadedLocation)
                VALUES (%s, %s, %s, %s, %s)
            '''

        cursor.execute(query, (user_id, episode_id, downloaded_date, file_size, file_path))
        cnx.commit()
        cursor.close()
        return True

    # Download the file
    response = requests.get(episode_url, stream=True)
    response.raise_for_status()
    downloaded_date = datetime.datetime.now()
    file_size = int(response.headers.get("Content-Length", 0))

    with open(file_path, "wb") as f:
        for chunk in response.iter_content(chunk_size=1024):
            f.write(chunk)

    uid = int(os.environ.get('PUID', 1000))
    gid = int(os.environ.get('PGID', 1000))
    os.chown(file_path, uid, gid)
    os.chown(download_dir, uid, gid)

    # After successful download, add metadata
    metadata = {
        'title': episode_title,
        'artist': get_value(result, "Author"),
        'album': podcast_name,
        'date': pub_date_str,
        'artwork_url': (get_value(result, "EpisodeArtwork") or
                        get_value(result, "ArtworkURL"))
    }

    mp3_metadata.add_podcast_metadata(file_path, metadata)

    # Update database
    if database_type == "postgresql":
        query = '''
            INSERT INTO "DownloadedEpisodes"
            (UserID, EpisodeID, DownloadedDate, DownloadedSize, DownloadedLocation)
            VALUES (%s, %s, %s, %s, %s)
        '''
    else:
        query = '''
            INSERT INTO DownloadedEpisodes
            (UserID, EpisodeID, DownloadedDate, DownloadedSize, DownloadedLocation)
            VALUES (%s, %s, %s, %s, %s)
        '''

    cursor.execute(query, (user_id, episode_id, downloaded_date, file_size, file_path))

    # Update download count
    if database_type == "postgresql":
        query = 'UPDATE "UserStats" SET EpisodesDownloaded = EpisodesDownloaded + 1 WHERE UserID = %s'
    else:
        query = "UPDATE UserStats SET EpisodesDownloaded = EpisodesDownloaded + 1 WHERE UserID = %s"

    cursor.execute(query, (user_id,))
    cnx.commit()
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

def get_video_ids_for_podcast(cnx, database_type, podcast_id):
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            query = """
                SELECT VideoID
                FROM "YouTubeVideos"
                WHERE PodcastID = %s
                ORDER BY PublishedAt DESC
            """
        else:
            query = """
                SELECT VideoID
                FROM YouTubeVideos
                WHERE PodcastID = %s
                ORDER BY PublishedAt DESC
            """

        cursor.execute(query, (podcast_id,))
        results = cursor.fetchall()

        # Extract the video IDs, handling both tuple and dict results
        video_ids = [row[0] if isinstance(row, tuple) else row['videoid'] for row in results]
        return video_ids

    finally:
        cursor.close()

def get_podcast_index_id(cnx, database_type, podcast_id):
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            query = 'SELECT PodcastIndexID FROM "Podcasts" WHERE PodcastID = %s'
        else:  # MySQL or MariaDB
            query = "SELECT PodcastIndexID FROM Podcasts WHERE PodcastID = %s"

        cursor.execute(query, (podcast_id,))
        result = cursor.fetchone()
        if result:
            return result[0] if isinstance(result, tuple) else result.get("podcastindexid")
        return None
    finally:
        cursor.close()


def download_youtube_video(cnx, database_type, video_id, user_id):
    cursor = cnx.cursor()

    # Get video details
    if database_type == "postgresql":
        query = '''
            SELECT
                v.VideoID,
                v.PodcastID,
                v.VideoTitle,
                v.PublishedAt,
                v.VideoURL,
                v.VideoDescription,
                v.ThumbnailURL,
                v.YouTubeVideoID,
                p.PodcastName,
                p.Author
            FROM "YouTubeVideos" v
            JOIN "Podcasts" p ON v.PodcastID = p.PodcastID
            WHERE v.VideoID = %s
        '''
    else:
        query = '''
            SELECT
                v.VideoID,
                v.PodcastID,
                v.VideoTitle,
                v.PublishedAt,
                v.VideoURL,
                v.VideoDescription,
                v.ThumbnailURL,
                v.YouTubeVideoID,
                p.PodcastName,
                p.Author
            FROM YouTubeVideos v
            JOIN Podcasts p ON v.PodcastID = p.PodcastID
            WHERE v.VideoID = %s
        '''

    cursor.execute(query, (video_id,))
    result = cursor.fetchone()

    if result is None:
        cursor.close()
        return False

    # Extract values
    if isinstance(result, dict):
        youtube_video_id = result.get('youtubevideoid') or result.get('YouTubeVideoID')
        video_title = result.get('videotitle') or result.get('VideoTitle')
        pub_date = result.get('publishedat') or result.get('PublishedAt')
        channel_name = result.get('podcastname') or result.get('PodcastName')
        author = result.get('author') or result.get('Author')
    else:
        youtube_video_id = result[7]  # YouTubeVideoID
        video_title = result[2]      # VideoTitle
        pub_date = result[3]         # PublishedAt
        channel_name = result[8]     # PodcastName
        author = result[9]           # Author

    # Get user's time/date preferences and format date
    timezone, time_format, date_format = get_time_info(database_type, cnx, user_id)
    date_format = date_format or "ISO"
    date_format_map = {
        "ISO": "%Y-%m-%d",
        "USA": "%m/%d/%Y",
        "EUR": "%d.%m.%Y",
        "JIS": "%Y-%m-%d",
        "MDY": "%m-%d-%Y",
        "DMY": "%d-%m-%Y",
        "YMD": "%Y-%m-%d",
    }
    date_format_str = date_format_map.get(date_format, "%Y-%m-%d")
    pub_date_str = pub_date.strftime(date_format_str)

    # Clean filenames
    channel_name = "".join(c for c in channel_name if c.isalnum() or c in (' ', '-', '_')).strip()
    video_title = "".join(c for c in video_title if c.isalnum() or c in (' ', '-', '_')).strip()

    # Source and destination paths
    source_path = f"/opt/pinepods/downloads/youtube/{youtube_video_id}.mp3"
    if not os.path.exists(source_path):
        source_path = f"{source_path}.mp3"  # Try with double extension
        if not os.path.exists(source_path):
            cursor.close()
            return False

    # Create destination directory
    download_dir = os.path.join("/opt/pinepods/downloads", channel_name)
    os.makedirs(download_dir, exist_ok=True)

    # Set proper file permissions
    uid = int(os.environ.get('PUID', 1000))
    gid = int(os.environ.get('PGID', 1000))
    os.chown(download_dir, uid, gid)

    # Generate destination filename
    filename = f"{pub_date_str}_{video_title}_{user_id}-{video_id}.mp3"
    dest_path = os.path.join(download_dir, filename)

    # Copy file and update metadata
    shutil.copy2(source_path, dest_path)
    os.chown(dest_path, uid, gid)

    # Update metadata
    metadata = {
        'title': video_title,
        'artist': author,
        'album': channel_name,
        'date': pub_date_str
    }
    mp3_metadata.add_podcast_metadata(dest_path, metadata)

    # Record in database
    file_size = os.path.getsize(dest_path)
    downloaded_date = datetime.datetime.now()

    if database_type == "postgresql":
        query = '''
            INSERT INTO "DownloadedVideos"
            (UserID, VideoID, DownloadedDate, DownloadedSize, DownloadedLocation)
            VALUES (%s, %s, %s, %s, %s)
        '''
    else:
        query = '''
            INSERT INTO DownloadedVideos
            (UserID, VideoID, DownloadedDate, DownloadedSize, DownloadedLocation)
            VALUES (%s, %s, %s, %s, %s)
        '''

    cursor.execute(query, (user_id, video_id, downloaded_date, file_size, dest_path))

    # Update download count
    if database_type == "postgresql":
        query = 'UPDATE "UserStats" SET EpisodesDownloaded = EpisodesDownloaded + 1 WHERE UserID = %s'
    else:
        query = "UPDATE UserStats SET EpisodesDownloaded = EpisodesDownloaded + 1 WHERE UserID = %s"

    cursor.execute(query, (user_id,))
    cnx.commit()
    cursor.close()

    return True




def get_podcast_id_from_episode(cnx, database_type, episode_id, user_id, is_youtube=False):
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            if is_youtube:
                query = """
                    SELECT "YouTubeVideos".PodcastID
                    FROM "YouTubeVideos"
                    INNER JOIN "Podcasts" ON "YouTubeVideos".PodcastID = "Podcasts".PodcastID
                    WHERE "YouTubeVideos".VideoID = %s AND "Podcasts".UserID = %s
                """
            else:
                query = """
                    SELECT "Episodes".PodcastID
                    FROM "Episodes"
                    INNER JOIN "Podcasts" ON "Episodes".PodcastID = "Podcasts".PodcastID
                    WHERE "Episodes".EpisodeID = %s AND "Podcasts".UserID = %s
                """
        else:  # MySQL or MariaDB
            if is_youtube:
                query = """
                    SELECT YouTubeVideos.PodcastID
                    FROM YouTubeVideos
                    INNER JOIN Podcasts ON YouTubeVideos.PodcastID = Podcasts.PodcastID
                    WHERE YouTubeVideos.VideoID = %s AND Podcasts.UserID = %s
                """
            else:
                query = """
                    SELECT Episodes.PodcastID
                    FROM Episodes
                    INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID
                    WHERE Episodes.EpisodeID = %s AND Podcasts.UserID = %s
                """

        # First try with provided user_id
        cursor.execute(query, (episode_id, user_id))
        result = cursor.fetchone()

        # If not found, try with system user (1)
        if not result:
            cursor.execute(query, (episode_id, 1))
            result = cursor.fetchone()

        if result:
            return result[0] if isinstance(result, tuple) else result.get("podcastid")
        return None
    except Exception as e:
        logging.error(f"Error in get_podcast_id_from_episode: {str(e)}")
        return None
    finally:
        cursor.close()

def get_podcast_id_from_episode_name(cnx, database_type, episode_name, episode_url, user_id):
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            query = """
                SELECT podcast_id FROM (
                    SELECT "Episodes".PodcastID as podcast_id
                    FROM "Episodes"
                    INNER JOIN "Podcasts" ON "Episodes".PodcastID = "Podcasts".PodcastID
                    WHERE "Episodes".EpisodeTitle = %s
                    AND "Episodes".EpisodeURL = %s
                    AND "Podcasts".UserID = %s

                    UNION

                    SELECT "YouTubeVideos".PodcastID as podcast_id
                    FROM "YouTubeVideos"
                    INNER JOIN "Podcasts" ON "YouTubeVideos".PodcastID = "Podcasts".PodcastID
                    WHERE "YouTubeVideos".VideoTitle = %s
                    AND "YouTubeVideos".VideoURL = %s
                    AND "Podcasts".UserID = %s
                ) combined_results
                LIMIT 1
            """
            # Pass the parameters twice because we're using them in both parts of the UNION
            cursor.execute(query, (episode_name, episode_url, user_id, episode_name, episode_url, user_id))
        else:  # MySQL or MariaDB
            query = """
                SELECT podcast_id FROM (
                    SELECT Episodes.PodcastID as podcast_id
                    FROM Episodes
                    INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID
                    WHERE Episodes.EpisodeTitle = %s
                    AND Episodes.EpisodeURL = %s
                    AND Podcasts.UserID = %s

                    UNION

                    SELECT YouTubeVideos.PodcastID as podcast_id
                    FROM YouTubeVideos
                    INNER JOIN Podcasts ON YouTubeVideos.PodcastID = Podcasts.PodcastID
                    WHERE YouTubeVideos.VideoTitle = %s
                    AND YouTubeVideos.VideoURL = %s
                    AND Podcasts.UserID = %s
                ) combined_results
                LIMIT 1
            """
            cursor.execute(query, (episode_name, episode_url, user_id, episode_name, episode_url, user_id))

        result = cursor.fetchone()
        if result:
            return result[0] if isinstance(result, tuple) else result.get("podcast_id")
        return None
    except Exception as e:
        logging.error(f"Error in get_podcast_id_from_episode_name: {str(e)}")
        return None
    finally:
        cursor.close()


def mark_episode_completed(cnx, database_type, episode_id, user_id, is_youtube=False):
    cursor = cnx.cursor()
    try:
        if is_youtube:
            # Handle YouTube video
            if database_type == "postgresql":
                duration_query = 'SELECT Duration FROM "YouTubeVideos" WHERE VideoID = %s'
                update_query = 'UPDATE "YouTubeVideos" SET Completed = TRUE WHERE VideoID = %s'
                history_query = '''
                    INSERT INTO "UserVideoHistory" (UserID, VideoID, ListenDate, ListenDuration)
                    VALUES (%s, %s, NOW(), %s)
                    ON CONFLICT (UserID, VideoID)
                    DO UPDATE SET ListenDuration = %s, ListenDate = NOW()
                '''
            else:
                duration_query = "SELECT Duration FROM YouTubeVideos WHERE VideoID = %s"
                update_query = "UPDATE YouTubeVideos SET Completed = 1 WHERE VideoID = %s"
                history_query = '''
                    INSERT INTO UserVideoHistory (UserID, VideoID, ListenDate, ListenDuration)
                    VALUES (%s, %s, NOW(), %s)
                    ON DUPLICATE KEY UPDATE
                        ListenDuration = %s,
                        ListenDate = NOW()
                '''
        else:
            # Original episode logic
            if database_type == "postgresql":
                duration_query = 'SELECT EpisodeDuration FROM "Episodes" WHERE EpisodeID = %s'
                update_query = 'UPDATE "Episodes" SET Completed = TRUE WHERE EpisodeID = %s'
                history_query = '''
                    INSERT INTO "UserEpisodeHistory" (UserID, EpisodeID, ListenDate, ListenDuration)
                    VALUES (%s, %s, NOW(), %s)
                    ON CONFLICT (UserID, EpisodeID)
                    DO UPDATE SET ListenDuration = %s, ListenDate = NOW()
                '''
            else:
                duration_query = "SELECT EpisodeDuration FROM Episodes WHERE EpisodeID = %s"
                update_query = "UPDATE Episodes SET Completed = 1 WHERE EpisodeID = %s"
                history_query = '''
                    INSERT INTO UserEpisodeHistory (UserID, EpisodeID, ListenDate, ListenDuration)
                    VALUES (%s, %s, NOW(), %s)
                    ON DUPLICATE KEY UPDATE
                        ListenDuration = %s,
                        ListenDate = NOW()
                '''

        # Get duration
        cursor.execute(duration_query, (episode_id,))
        duration_result = cursor.fetchone()
        if duration_result:
            if isinstance(duration_result, dict):
                duration = duration_result['episodeduration' if not is_youtube else 'duration']
            else:  # tuple
                duration = duration_result[0]
        else:
            duration = None

        if duration:
            # Update completion status
            cursor.execute(update_query, (episode_id,))

            # Update history
            history_params = (user_id, episode_id, duration, duration)
            cursor.execute(history_query, history_params)

        cnx.commit()
    except Exception as e:
        cnx.rollback()
        print(f"Error in mark_episode_completed: {str(e)}")
        raise e
    finally:
        cursor.close()

def mark_episode_uncompleted(cnx, database_type, episode_id, user_id, is_youtube=False):
    cursor = cnx.cursor()
    try:
        if is_youtube:
            # Handle YouTube video
            if database_type == "postgresql":
                update_query = 'UPDATE "YouTubeVideos" SET Completed = FALSE WHERE VideoID = %s'
                history_query = '''
                    UPDATE "UserVideoHistory"
                    SET ListenDuration = 0, ListenDate = NOW()
                    WHERE UserID = %s AND VideoID = %s
                '''
            else:
                update_query = "UPDATE YouTubeVideos SET Completed = 0 WHERE VideoID = %s"
                history_query = '''
                    UPDATE UserVideoHistory
                    SET ListenDuration = 0, ListenDate = NOW()
                    WHERE UserID = %s AND VideoID = %s
                '''
        else:
            # Original episode logic
            if database_type == "postgresql":
                update_query = 'UPDATE "Episodes" SET Completed = FALSE WHERE EpisodeID = %s'
                history_query = '''
                    UPDATE "UserEpisodeHistory"
                    SET ListenDuration = 0, ListenDate = NOW()
                    WHERE UserID = %s AND EpisodeID = %s
                '''
            else:
                update_query = "UPDATE Episodes SET Completed = 0 WHERE EpisodeID = %s"
                history_query = '''
                    UPDATE UserEpisodeHistory
                    SET ListenDuration = 0, ListenDate = NOW()
                    WHERE UserID = %s AND EpisodeID = %s
                '''

        cursor.execute(update_query, (episode_id,))
        cursor.execute(history_query, (user_id, episode_id))
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
    try:
        if database_type == "postgresql":
            query = """
                SELECT StartSkip, EndSkip
                FROM "Podcasts"
                WHERE PodcastID = %s AND UserID = %s
            """
        else:
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

        # If no result found (user isn't subscribed), return default values
        return 0, 0
    finally:
        cursor.close()


def check_downloaded(cnx, database_type, user_id, content_id, is_youtube=False):
    cursor = cnx.cursor()

    if is_youtube:
        if database_type == "postgresql":
            query = 'SELECT 1 FROM "DownloadedVideos" WHERE VideoID = %s AND UserID = %s'
        else:
            query = "SELECT 1 FROM DownloadedVideos WHERE VideoID = %s AND UserID = %s"
    else:
        if database_type == "postgresql":
            query = 'SELECT 1 FROM "DownloadedEpisodes" WHERE EpisodeID = %s AND UserID = %s'
        else:
            query = "SELECT 1 FROM DownloadedEpisodes WHERE EpisodeID = %s AND UserID = %s"

    cursor.execute(query, (content_id, user_id))
    result = cursor.fetchone() is not None
    cursor.close()
    return result


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

def get_youtube_video_location(cnx, database_type, episode_id, user_id):
    cursor = cnx.cursor()
    try:
        logging.info(f"Looking up YouTube video location for episode_id: {episode_id}, user_id: {user_id}")

        if database_type == "postgresql":
            query = '''
                SELECT "YouTubeVideos"."youtubevideoid"
                FROM "YouTubeVideos"
                INNER JOIN "Podcasts" ON "YouTubeVideos"."podcastid" = "Podcasts"."podcastid"
                WHERE "YouTubeVideos"."videoid" = %s AND "Podcasts"."userid" = %s
            '''
        else:
            query = '''
                SELECT YouTubeVideos.YouTubeVideoID
                FROM YouTubeVideos
                INNER JOIN Podcasts ON YouTubeVideos.PodcastID = Podcasts.PodcastID
                WHERE YouTubeVideos.VideoID = %s AND Podcasts.UserID = %s
            '''

        logging.info(f"Executing query: {query}")
        logging.info(f"With parameters: episode_id={episode_id}, user_id={user_id}")

        cursor.execute(query, (episode_id, user_id))
        result = cursor.fetchone()

        logging.info(f"Query result: {result}")

        if result:
            # Handle both dict and tuple results
            youtube_id = result['youtubevideoid'] if isinstance(result, dict) else result[0]
            logging.info(f"Found YouTube ID: {youtube_id}")

            file_path = os.path.join('/opt/pinepods/downloads/youtube', f'{youtube_id}.mp3')
            file_path_double = os.path.join('/opt/pinepods/downloads/youtube', f'{youtube_id}.mp3.mp3')

            logging.info(f"Checking paths: {file_path} and {file_path_double}")

            if os.path.exists(file_path):
                logging.info(f"Found file at {file_path}")
                return file_path
            elif os.path.exists(file_path_double):
                logging.info(f"Found file at {file_path_double}")
                return file_path_double
            else:
                logging.info("No file found at either path")

        else:
            logging.info("No YouTube video found in database")

        return None
    except Exception as e:
        logging.error(f"Error retrieving YouTube video location: {e}")
        import traceback
        logging.error(f"Traceback: {traceback.format_exc()}")
        return None
    finally:
        cursor.close()

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
    else:
        cursor = cnx.cursor(dictionary=True)

    if database_type == "postgresql":
        query = """
            SELECT * FROM (
                SELECT
                    "Podcasts".PodcastID,
                    "Podcasts".PodcastName,
                    "Podcasts".ArtworkURL,
                    "Episodes".EpisodeID,
                    "Episodes".EpisodeTitle as episodetitle,
                    "Episodes".EpisodePubDate as episodepubdate,
                    "Episodes".EpisodeDescription as episodedescription,
                    "Episodes".EpisodeArtwork as episodeartwork,
                    "Episodes".EpisodeURL as episodeurl,
                    "Episodes".EpisodeDuration as episodeduration,
                    "Podcasts".PodcastIndexID,
                    "Podcasts".WebsiteURL,
                    "DownloadedEpisodes".DownloadedLocation,
                    "UserEpisodeHistory".ListenDuration as listenduration,
                    "Episodes".Completed,
                    FALSE as is_youtube
                FROM "DownloadedEpisodes"
                INNER JOIN "Episodes" ON "DownloadedEpisodes".EpisodeID = "Episodes".EpisodeID
                INNER JOIN "Podcasts" ON "Episodes".PodcastID = "Podcasts".PodcastID
                LEFT JOIN "UserEpisodeHistory" ON
                    "DownloadedEpisodes".EpisodeID = "UserEpisodeHistory".EpisodeID
                    AND "DownloadedEpisodes".UserID = "UserEpisodeHistory".UserID
                WHERE "DownloadedEpisodes".UserID = %s

                UNION ALL

                SELECT
                    "Podcasts".PodcastID,
                    "Podcasts".PodcastName,
                    "Podcasts".ArtworkURL,
                    "YouTubeVideos".VideoID as EpisodeID,
                    "YouTubeVideos".VideoTitle as episodetitle,
                    "YouTubeVideos".PublishedAt as episodepubdate,
                    "YouTubeVideos".VideoDescription as episodedescription,
                    "YouTubeVideos".ThumbnailURL as episodeartwork,
                    "YouTubeVideos".VideoURL as episodeurl,
                    "YouTubeVideos".Duration as episodeduration,
                    "Podcasts".PodcastIndexID,
                    "Podcasts".WebsiteURL,
                    "DownloadedVideos".DownloadedLocation,
                    "YouTubeVideos".ListenPosition as listenduration,
                    "YouTubeVideos".Completed,
                    TRUE as is_youtube
                FROM "DownloadedVideos"
                INNER JOIN "YouTubeVideos" ON "DownloadedVideos".VideoID = "YouTubeVideos".VideoID
                INNER JOIN "Podcasts" ON "YouTubeVideos".PodcastID = "Podcasts".PodcastID
                WHERE "DownloadedVideos".UserID = %s
            ) combined
            ORDER BY episodepubdate DESC
        """
    else:  # MySQL or MariaDB
        query = """
            SELECT * FROM (
                SELECT
                    Podcasts.PodcastID,
                    Podcasts.PodcastName,
                    Podcasts.ArtworkURL,
                    Episodes.EpisodeID,
                    Episodes.EpisodeTitle as episodetitle,
                    Episodes.EpisodePubDate as episodepubdate,
                    Episodes.EpisodeDescription as episodedescription,
                    Episodes.EpisodeArtwork as episodeartwork,
                    Episodes.EpisodeURL as episodeurl,
                    Episodes.EpisodeDuration as episodeduration,
                    Podcasts.PodcastIndexID,
                    Podcasts.WebsiteURL,
                    DownloadedEpisodes.DownloadedLocation,
                    UserEpisodeHistory.ListenDuration as listenduration,
                    Episodes.Completed,
                    0 as is_youtube
                FROM DownloadedEpisodes
                INNER JOIN Episodes ON DownloadedEpisodes.EpisodeID = Episodes.EpisodeID
                INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID
                LEFT JOIN UserEpisodeHistory ON
                    DownloadedEpisodes.EpisodeID = UserEpisodeHistory.EpisodeID
                    AND DownloadedEpisodes.UserID = UserEpisodeHistory.UserID
                WHERE DownloadedEpisodes.UserID = %s

                UNION ALL

                SELECT
                    Podcasts.PodcastID,
                    Podcasts.PodcastName,
                    Podcasts.ArtworkURL,
                    YouTubeVideos.VideoID as EpisodeID,
                    YouTubeVideos.VideoTitle as episodetitle,
                    YouTubeVideos.PublishedAt as episodepubdate,
                    YouTubeVideos.VideoDescription as episodedescription,
                    YouTubeVideos.ThumbnailURL as episodeartwork,
                    YouTubeVideos.VideoURL as episodeurl,
                    YouTubeVideos.Duration as episodeduration,
                    Podcasts.PodcastIndexID,
                    Podcasts.WebsiteURL,
                    DownloadedVideos.DownloadedLocation,
                    YouTubeVideos.ListenPosition as listenduration,
                    YouTubeVideos.Completed,
                    1 as is_youtube
                FROM DownloadedVideos
                INNER JOIN YouTubeVideos ON DownloadedVideos.VideoID = YouTubeVideos.VideoID
                INNER JOIN Podcasts ON YouTubeVideos.PodcastID = Podcasts.PodcastID
                WHERE DownloadedVideos.UserID = %s
            ) combined
            ORDER BY episodepubdate DESC
        """

    cursor.execute(query, (user_id, user_id))  # Pass user_id twice for both parts of UNION
    rows = cursor.fetchall()
    cursor.close()

    if not rows:
        return None

    downloaded_episodes = lowercase_keys(rows)

    if database_type != "postgresql":
        for episode in downloaded_episodes:
            episode['completed'] = bool(episode['completed'])
            episode['is_youtube'] = bool(episode['is_youtube'])
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

def get_episode_id_ep_name(cnx, database_type, podcast_title, episode_url):
    if database_type == "postgresql":
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
        query = '''
            SELECT e.EpisodeID
            FROM "Episodes" e
            JOIN "Podcasts" p ON e.PodcastID = p.PodcastID
            WHERE p.PodcastName = %s AND e.EpisodeURL = %s
        '''
    else:  # MySQL or MariaDB
        cursor = cnx.cursor()
        query = '''
            SELECT e.EpisodeID
            FROM Episodes e
            JOIN Podcasts p ON e.PodcastID = p.PodcastID
            WHERE p.PodcastName = %s AND e.EpisodeURL = %s
        '''

    params = (podcast_title, episode_url)
    print(f"Executing query: {query} with params: {params}")

    # Extra debugging: Check the values before executing the query
    print(f"Podcast Title: {podcast_title}")
    print(f"Episode URL: {episode_url}")

    cursor.execute(query, params)
    result = cursor.fetchone()

    if result:
        episode_id = result['episodeid'] if database_type == "postgresql" else result[0]
    else:
        episode_id = None
        print(f"No match found for Podcast Name: '{podcast_title}' and Episode URL: '{episode_url}'")

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


def record_youtube_listen_duration(cnx, database_type, video_id, user_id, listen_duration):
    if listen_duration < 0:
        logging.info(f"Skipped updating listen duration for user {user_id} and video {video_id} due to invalid duration: {listen_duration}")
        return

    listen_date = datetime.datetime.now()
    cursor = cnx.cursor()
    try:
        # Check if UserVideoHistory exists (we'll need to create this table)
        if database_type == "postgresql":
            cursor.execute('SELECT ListenDuration FROM "UserVideoHistory" WHERE UserID=%s AND VideoID=%s', (user_id, video_id))
        else:
            cursor.execute("SELECT ListenDuration FROM UserVideoHistory WHERE UserID=%s AND VideoID=%s", (user_id, video_id))

        result = cursor.fetchone()

        if result is not None:
            existing_duration = result[0] if isinstance(result, tuple) else result.get("ListenDuration")
            existing_duration = existing_duration if existing_duration is not None else 0

            if listen_duration > existing_duration:
                if database_type == "postgresql":
                    update_listen_duration = 'UPDATE "UserVideoHistory" SET ListenDuration=%s, ListenDate=%s WHERE UserID=%s AND VideoID=%s'
                else:
                    update_listen_duration = "UPDATE UserVideoHistory SET ListenDuration=%s, ListenDate=%s WHERE UserID=%s AND VideoID=%s"
                cursor.execute(update_listen_duration, (listen_duration, listen_date, user_id, video_id))

                # Also update the ListenPosition in YouTubeVideos table
                if database_type == "postgresql":
                    cursor.execute('UPDATE "YouTubeVideos" SET ListenPosition=%s WHERE VideoID=%s',
                                 (listen_duration, video_id))
                else:
                    cursor.execute("UPDATE YouTubeVideos SET ListenPosition=%s WHERE VideoID=%s",
                                 (listen_duration, video_id))

                print(f"Updated listen duration for user {user_id} and video {video_id} to {listen_duration}")
        else:
            # Insert new row
            if database_type == "postgresql":
                add_listen_duration = 'INSERT INTO "UserVideoHistory" (UserID, VideoID, ListenDate, ListenDuration) VALUES (%s, %s, %s, %s)'
            else:
                add_listen_duration = "INSERT INTO UserVideoHistory (UserID, VideoID, ListenDate, ListenDuration) VALUES (%s, %s, %s, %s)"
            cursor.execute(add_listen_duration, (user_id, video_id, listen_date, listen_duration))

            # Update ListenPosition in YouTubeVideos
            if database_type == "postgresql":
                cursor.execute('UPDATE "YouTubeVideos" SET ListenPosition=%s WHERE VideoID=%s',
                             (listen_duration, video_id))
            else:
                cursor.execute("UPDATE YouTubeVideos SET ListenPosition=%s WHERE VideoID=%s",
                             (listen_duration, video_id))

            print(f"Inserted new listen duration for user {user_id} and video {video_id}: {listen_duration}")

        cnx.commit()
    except Exception as e:
        logging.error(f"Failed to record YouTube listen duration due to: {e}")
        cnx.rollback()
    finally:
        cursor.close()


def get_local_episode_times(cnx, database_type, user_id):
    if database_type == "postgresql":
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(dictionary=True)

    if database_type == "postgresql":
        cursor.execute("""
        SELECT
            e.EpisodeURL,
            p.FeedURL,
            ueh.ListenDuration,
            e.EpisodeDuration,
            e.Completed
        FROM "UserEpisodeHistory" ueh
        JOIN "Episodes" e ON ueh.EpisodeID = e.EpisodeID
        JOIN "Podcasts" p ON e.PodcastID = p.PodcastID
        WHERE ueh.UserID = %s
        """, (user_id,))
    else:  # MySQL or MariaDB
        cursor.execute("""
        SELECT
            e.EpisodeURL,
            p.FeedURL,
            ueh.ListenDuration,
            e.EpisodeDuration,
            e.Completed
        FROM UserEpisodeHistory ueh
        JOIN Episodes e ON ueh.EpisodeID = e.EpisodeID
        JOIN Podcasts p ON e.PodcastID = p.PodcastID
        WHERE ueh.UserID = %s
        """, (user_id,))

    # Handle psycopg3's inconsistent return types
    episode_times = []
    for row in cursor.fetchall():
        if isinstance(row, dict):
            episode_times.append({
                "episode_url": row["episodeurl"],
                "podcast_url": row["feedurl"],
                "listen_duration": row["listenduration"],
                "episode_duration": row["episodeduration"],
                "completed": row["completed"]
            })
        else:
            episode_times.append({
                "episode_url": row[0],
                "podcast_url": row[1],
                "listen_duration": row[2],
                "episode_duration": row[3],
                "completed": row[4]
            })
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


def get_my_user_info(database_type, cnx, user_id):
    try:
        if database_type == "postgresql":
            cnx.row_factory = dict_row
            cursor = cnx.cursor()
            query = '''
                SELECT UserID, Fullname, Username, Email,
                       CASE WHEN IsAdmin THEN 1 ELSE 0 END AS IsAdmin
                FROM "Users"
                WHERE UserID = %s
            '''
        else:  # MySQL or MariaDB
            cursor = cnx.cursor(dictionary=True)
            query = """
                SELECT UserID, Fullname, Username, Email, IsAdmin
                FROM Users
                WHERE UserID = %s
            """
        cursor.execute(query, (user_id,))
        row = cursor.fetchone()

        if not row:
            return None

        # Handle both dict and tuple cases
        if isinstance(row, dict):
            # For MySQL, convert keys to lowercase
            if database_type != "postgresql":
                return {k.lower(): v if v is not None else "" for k, v in row.items()}
            return {k: v if v is not None else "" for k, v in row.items()}
        else:
            # Handle tuple case by creating dict with known column order
            columns = ['userid', 'fullname', 'username', 'email', 'isadmin']
            return {columns[i]: v if v is not None else "" for i, v in enumerate(row)}

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
        cursor = cnx.cursor(dictionary=True)
        query = "SELECT APIKey FROM APIKeys WHERE APIKeyID = %s"

    cursor.execute(query, (api_id,))
    result = cursor.fetchone()
    cursor.close()

    if result:
        if isinstance(result, tuple):
            # Convert tuple to dictionary
            result = dict(zip([desc[0] for desc in cursor.description], result))
        if database_type == 'postgresql':
            if result.get('apikey') == api_key:
                return True
        else:
            if result.get('APIKey') == api_key:
                return True
    return False


def belongs_to_guest_user(cnx, database_type, api_id):
    if database_type == "postgresql":
        cursor = cnx.cursor()
        query = 'SELECT UserID FROM "APIKeys" WHERE APIKeyID = %s'
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(dictionary=True)
        query = "SELECT UserID FROM APIKeys WHERE APIKeyID = %s"

    cursor.execute(query, (api_id,))
    result = cursor.fetchone()
    cursor.close()

    if result:
        if isinstance(result, tuple):
            # Convert tuple to dictionary
            result = dict(zip([desc[0] for desc in cursor.description], result))
        if database_type == 'postgresql':
            return result.get('userid') == 1
        else:
            return result.get('UserID') == 1
    return False


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

    if database_type == "postgresql":
        query = 'UPDATE "Users" SET IsAdmin = %s WHERE UserID = %s'
        # For PostgreSQL, use boolean directly instead of converting to int
        cursor.execute(query, (isadmin, user_id))
    else:  # MySQL or MariaDB
        query = "UPDATE Users SET IsAdmin = %s WHERE UserID = %s"
        isadmin_int = int(isadmin)
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
        query = 'SELECT COUNT(*) FROM "Users" WHERE IsAdmin = TRUE'
    else:  # MySQL or MariaDB
        query = "SELECT COUNT(*) FROM Users WHERE IsAdmin = 1"

    cursor.execute(query)
    result = cursor.fetchone()
    # Handle both tuple and dict results
    admin_count = result[0] if isinstance(result, tuple) else result['count']

    if admin_count == 1:
        if database_type == "postgresql":
            query = 'SELECT IsAdmin FROM "Users" WHERE UserID = %s'
        else:  # MySQL or MariaDB
            query = "SELECT IsAdmin FROM Users WHERE UserID = %s"

        cursor.execute(query, (user_id,))
        result = cursor.fetchone()
        # Handle both tuple and dict results
        is_admin = result[0] if isinstance(result, tuple) else result['isadmin']

        # For PostgreSQL boolean or MySQL/MariaDB int
        if is_admin:
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




def check_admin_exists(cnx, database_type):
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            query = """
                SELECT COUNT(*) as count FROM "Users"
                WHERE IsAdmin = TRUE
                AND Username != 'background_tasks'
            """
        else:  # MySQL or MariaDB
            query = """
                SELECT COUNT(*) FROM Users
                WHERE IsAdmin = 1
                AND Username != 'background_tasks'
            """
        cursor.execute(query)
        result = cursor.fetchone()

        if result:
            if isinstance(result, dict):
                return result['count']
            else:
                return result[0]
        return 0
    finally:
        cursor.close()

def self_service_status(cnx, database_type):
    cursor = cnx.cursor()
    try:
        # Get self-service status
        if database_type == "postgresql":
            query = 'SELECT SelfServiceUser FROM "AppSettings" WHERE SelfServiceUser = TRUE'
        else:  # MySQL or MariaDB
            query = "SELECT SelfServiceUser FROM AppSettings WHERE SelfServiceUser = 1"
        cursor.execute(query)
        self_service_result = cursor.fetchone()

        # Get admin status
        admin_exists = check_admin_exists(cnx, database_type)

        return {
            "status": bool(self_service_result),
            "first_admin_created": bool(admin_exists > 0)  # Convert to boolean
        }
    finally:
        cursor.close()

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

    try:
        cursor.execute(query, (passed_key,))
        result = cursor.fetchone()
        print(f'Query executed. result: {result}')
    except Exception as e:
        print(f'Error during query execution: {str(e)}')
        result = None
    finally:
        cursor.close()

    return True if result else False


def get_rss_feed_status(cnx, database_type: str, user_id: int) -> bool:
    cursor = cnx.cursor()
    logging.info(f"Checking RSS feed status for user {user_id}")
    try:
        if database_type == "postgresql":
            cursor.execute('SELECT enablerssfeeds FROM "Users" WHERE userid = %s', (user_id,))
        else:
            cursor.execute("SELECT EnableRSSFeeds FROM Users WHERE UserID = %s", (user_id,))

        result = cursor.fetchone()
        logging.info(f"RSS feed status raw result: {result}")

        value = get_value_from_result(result, 'enablerssfeeds', False)
        logging.info(f"RSS feed status processed value: {value}")

        return bool(value)
    except Exception as e:
        logging.error(f"Error checking RSS feed status: {e}")
        return False
    finally:
        cursor.close()


def toggle_rss_feeds(cnx, database_type: str, user_id: int) -> bool:
    cursor = cnx.cursor()
    try:
        # Get current status
        if database_type == "postgresql":
            cursor.execute('SELECT EnableRSSFeeds FROM "Users" WHERE UserID = %s', (user_id,))
        else:
            cursor.execute("SELECT EnableRSSFeeds FROM Users WHERE UserID = %s", (user_id,))

        current_status = cursor.fetchone()

        # Handle different return types
        if isinstance(current_status, dict):
            new_status = not current_status.get('EnableRSSFeeds', False)
        else:
            new_status = not bool(current_status[0]) if current_status and current_status[0] is not None else True

        # Update status
        if database_type == "postgresql":
            cursor.execute(
                'UPDATE "Users" SET EnableRSSFeeds = %s WHERE UserID = %s',
                (new_status, user_id)
            )
        else:
            cursor.execute(
                "UPDATE Users SET EnableRSSFeeds = %s WHERE UserID = %s",
                (new_status, user_id)
            )
        cnx.commit()
        return new_status
    finally:
        cursor.close()


def parse_date_safely(date_str):
    """Safely parse a date string into a datetime object"""
    if isinstance(date_str, dt):
        return date_str if date_str.tzinfo else date_str.replace(tzinfo=timezone.utc)

    try:
        # PostgreSQL timestamp format
        dt_obj = dt.strptime(date_str, '%Y-%m-%d %H:%M:%S')
        return dt_obj.replace(tzinfo=timezone.utc)
    except (ValueError, TypeError):
        try:
            # Try with microseconds
            dt_obj = dt.strptime(date_str, '%Y-%m-%d %H:%M:%S.%f')
            return dt_obj.replace(tzinfo=timezone.utc)
        except (ValueError, TypeError):
            try:
                # ISO format
                dt_obj = dt.fromisoformat(date_str.replace('Z', '+00:00'))
                return dt_obj if dt_obj.tzinfo else dt_obj.replace(tzinfo=timezone.utc)
            except (ValueError, TypeError):
                # Default to current time if all parsing fails
                return dt.now(timezone.utc)


def get_value_from_rss_result(result, key_name: str, default=None):
    """Helper function to safely extract values from psycopg results"""
    if result is None:
        return default

    # Handle dictionary result
    if isinstance(result, dict):
        # Try different case variations for PostgreSQL
        return result.get(key_name.lower()) or result.get(key_name.upper()) or default

    # Handle tuple result
    if isinstance(result, (tuple, list)) and len(result) > 0:
        return result[0] if result[0] is not None else default

    return default

# Define the custom feed class at module level
class PodcastFeed(feedgenerator.Rss201rev2Feed):
    def root_attributes(self):
        attrs = super().root_attributes()
        attrs['xmlns:itunes'] = 'http://www.itunes.com/dtds/podcast-1.0.dtd'
        return attrs

    def add_root_elements(self, handler):
        super().add_root_elements(handler)
        # Access podcast_image and podcast_name through instance variables
        if hasattr(self, 'podcast_image') and self.podcast_image:
            handler.addQuickElement('itunes:image',
                attrs={'href': self.podcast_image})
            handler.startElement('image', {})
            handler.addQuickElement('url', self.podcast_image)
            handler.addQuickElement('title', self.podcast_name)
            handler.addQuickElement('link', 'https://github.com/madeofpendletonwool/pinepods')
            handler.endElement('image')

    def add_item_elements(self, handler, item):
        super().add_item_elements(handler, item)
        if 'artwork_url' in item:
            handler.addQuickElement('itunes:image',
                attrs={'href': item['artwork_url']})


def generate_podcast_rss(database_type: str, cnx, user_id: int, api_key: str, podcast_id: Optional[int] = None) -> str:
    from datetime import datetime as dt, timezone
    cursor = cnx.cursor()
    logging.basicConfig(level=logging.INFO)
    logger = logging.getLogger(__name__)

    try:
        # Check if RSS feeds are enabled for user
        if not get_rss_feed_status(cnx, database_type, user_id):
            raise HTTPException(status_code=403, detail="RSS feeds not enabled for this user")

        # Get user info for feed metadata
        if database_type == "postgresql":
            cursor.execute('SELECT username FROM "Users" WHERE userid = %s', (user_id,))
        else:
            cursor.execute("SELECT Username FROM Users WHERE UserID = %s", (user_id,))

        user = cursor.fetchone()
        if not user:
            raise HTTPException(status_code=404, detail="User not found")

        username = get_value_from_rss_result(user, 'username', 'Unknown User')

        # Build the query with correct case for each database type
        if database_type == "postgresql":
            base_query = '''
                SELECT
                    e.episodeid,
                    e.podcastid,
                    e.episodetitle,
                    e.episodedescription,
                    e.episodeurl,
                    e.episodeartwork,
                    e.episodepubdate,
                    e.episodeduration,
                    p.podcastname,
                    p.author,
                    p.artworkurl,
                    p.description as podcastdescription
                FROM "Episodes" e
                JOIN "Podcasts" p ON e.podcastid = p.podcastid
                WHERE p.userid = %s
            '''
        else:
            base_query = '''
                SELECT
                    e.EpisodeID,
                    e.PodcastID,
                    e.EpisodeTitle,
                    e.EpisodeDescription,
                    e.EpisodeURL,
                    e.EpisodeArtwork,
                    e.EpisodePubDate,
                    e.EpisodeDuration,
                    p.PodcastName,
                    p.Author,
                    p.ArtworkURL,
                    p.Description as PodcastDescription
                FROM Episodes e
                JOIN Podcasts p ON e.PodcastID = p.PodcastID
                WHERE p.UserID = %s
            '''

        params = [user_id]
        if podcast_id is not None:
            base_query += f' AND {"p.podcastid" if database_type == "postgresql" else "p.PodcastID"} = %s'
            params.append(podcast_id)

        base_query += f' ORDER BY {"e.episodepubdate" if database_type == "postgresql" else "e.EpisodePubDate"} DESC LIMIT 100'

        cursor.execute(base_query, tuple(params))
        print('q1')
        # Get column names and create result mapping
        columns = [desc[0].lower() for desc in cursor.description]
        column_map = {name: idx for idx, name in enumerate(columns)}
        # Inside generate_podcast_rss, replace the dictionary creation section with:

        episodes = []
        all_rows = cursor.fetchall()

        for row_idx, row in enumerate(all_rows):
            try:
                episode_dict = {}

                # If row is already a dictionary, use it directly
                if isinstance(row, dict):
                    source_dict = row
                else:
                    # Convert tuple to dictionary using column names
                    source_dict = dict(zip(columns, row))

                # Process each column
                for col in columns:
                    try:

                        # Get value either from dictionary or by index
                        if isinstance(row, dict):
                            raw_value = row.get(col)
                        else:
                            col_idx = column_map[col]
                            raw_value = row[col_idx] if col_idx < len(row) else None

                        # Special handling for dates
                        if col == 'episodepubdate' and raw_value is not None:
                            try:
                                if isinstance(raw_value, dt):
                                    value = raw_value if raw_value.tzinfo else raw_value.replace(tzinfo=timezone.utc)
                                else:
                                    value = dt.strptime(str(raw_value), '%Y-%m-%d %H:%M:%S')
                                    value = value.replace(tzinfo=timezone.utc)
                            except Exception as e:
                                logger.error(f"Date parsing failed: {str(e)}")
                                value = dt.now(timezone.utc)
                        else:
                            value = raw_value if raw_value is not None else ''

                        episode_dict[col] = value

                    except Exception as e:
                        logger.error(f"Error processing column {col}: {str(e)}", exc_info=True)
                        # Use safe defaults
                        if col == 'episodepubdate':
                            episode_dict[col] = dt.now(timezone.utc)
                        else:
                            episode_dict[col] = ''

                episodes.append(episode_dict)

            except Exception as e:
                logger.error(f"Error processing row {row_idx}: {str(e)}", exc_info=True)
                continue

        logger.info(f"Successfully processed {len(episodes)} episodes")

        # Get podcast name if podcast_id is provided
        podcast_name = "All Podcasts"
        feed_image = "/var/www/html/static/assets/favicon.png"  # Default to Pinepods logo

        if podcast_id is not None:
            try:
                if database_type == "postgresql":
                    cursor.execute(
                        'SELECT podcastname, artworkurl FROM "Podcasts" WHERE podcastid = %s',  # Added artworkurl
                        (podcast_id,)
                    )
                else:
                    cursor.execute(
                        "SELECT PodcastName, ArtworkURL FROM Podcasts WHERE PodcastID = %s",  # Added ArtworkURL
                        (podcast_id,)
                    )
                result = cursor.fetchone()
                if result:
                    podcast_name = result[0] if isinstance(result, tuple) else result.get('podcastname', 'Unknown Podcast')
                    feed_image = result[1] if isinstance(result, tuple) else result.get('artworkurl', feed_image)
                else:
                    podcast_name = "Unknown Podcast"
            except Exception as e:
                logger.error(f"Error fetching podcast name: {str(e)}")
                podcast_name = "Unknown Podcast"

        # Initialize feed with custom class
        feed = PodcastFeed(
            title=f"Pinepods - {podcast_name}",
            link="https://github.com/madeofpendletonwool/pinepods",
            description=f"RSS feed for {'all' if podcast_id is None else 'selected'} podcasts from Pinepods",
            language="en",
            author_name=username,
            feed_url="",
            ttl="60"
        )

        # Set feed image - use podcast artwork for specific podcast, Pinepods logo for all podcasts
        feed.podcast_image = feed_image
        feed.podcast_name = podcast_name

        # Set podcast image if available
        if episodes:
            feed.podcast_image = episodes[0].get('artworkurl')
            feed.podcast_name = podcast_name

        # Debug logging for image URLs
        logger.info(f"Podcast artwork URL: {episodes[0].get('artworkurl') if episodes else 'None'}")

        # Add items to feed
        for episode in episodes:
            try:
                episode_image = episode.get('episodeartwork') or episode.get('artworkurl', '')
                logger.info(f"Episode {episode.get('episodetitle')} artwork: {episode_image}")

                feed.add_item(
                    title=str(episode.get('episodetitle', 'Untitled Episode')),
                    link=str(episode.get('episodeurl', '')),
                    description=str(episode.get('episodedescription', '')),
                    unique_id=str(episode.get('episodeid', '')),
                    enclosure=feedgenerator.Enclosure(
                        url=str(episode.get('episodeurl', '')),
                        length=str(episode.get('episodeduration', '0')),
                        mime_type='audio/mpeg'
                    ),
                    pubdate=episode.get('episodepubdate', dt.now(timezone.utc)),
                    author=str(episode.get('author', '')),
                    artwork_url=episode_image
                )
            except Exception as e:
                logger.error(f"Error adding episode to feed: {str(e)}")
                continue

        return feed.writeString('utf-8')

    except Exception as e:
        logger.error(f"Error generating RSS feed: {str(e)}", exc_info=True)
        raise HTTPException(status_code=500, detail=f"Error generating RSS feed: {str(e)}")
    finally:
        cursor.close()


def set_rss_feed_status(cnx, database_type: str, user_id: int, enable: bool) -> bool:
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            cursor.execute(
                'UPDATE "Users" SET EnableRSSFeeds = %s WHERE UserID = %s',
                (enable, user_id)
            )
        else:
            cursor.execute(
                "UPDATE Users SET EnableRSSFeeds = %s WHERE UserID = %s",
                (enable, user_id)
            )
        cnx.commit()
        return enable
    finally:
        cursor.close()


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

def get_value_from_result(result, key_name: str, default=None):
    """Helper function to safely extract values from psycopg results"""
    if result is None:
        return default

    # Handle dictionary result
    if isinstance(result, dict):
        # Try different case variations for PostgreSQL
        return result.get(key_name.lower()) or result.get(key_name.upper()) or default

    # Handle tuple result
    if isinstance(result, (tuple, list)):
        # For tuples, we assume the first element is what we want
        return result[0] if result[0] is not None else default

    return default


def id_from_api_key(cnx, database_type, passed_key):
    logging.info(f"Fetching user ID for API key: {passed_key}")
    cursor = cnx.cursor()

    try:
        if database_type == "postgresql":
            query = 'SELECT userid FROM "APIKeys" WHERE apikey = %s'
        else:
            query = "SELECT UserID FROM APIKeys WHERE APIKey = %s"

        cursor.execute(query, (passed_key,))
        result = cursor.fetchone()
        logging.info(f"API key lookup raw result: {result}")

        user_id = get_value_from_result(result, 'userid')
        logging.info(f"API key lookup processed value: {user_id}")

        return user_id
    except Exception as e:
        logging.error(f"Error fetching user ID for API key: {e}")
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
    # Check if result is a dictionary or a tuple and create stats accordingly
    if isinstance(result, dict):
        if database_type == 'postgresql':
            stats = {
                "UserCreated": result['usercreated'],
                "PodcastsPlayed": result['podcastsplayed'],
                "TimeListened": result['timelistened'],
                "PodcastsAdded": result['podcastsadded'],
                "EpisodesSaved": result['episodessaved'],
                "EpisodesDownloaded": result['episodesdownloaded']
            }
        else:
            stats = {
                "UserCreated": result['UserCreated'],
                "PodcastsPlayed": result['PodcastsPlayed'],
                "TimeListened": result['TimeListened'],
                "PodcastsAdded": result['PodcastsAdded'],
                "EpisodesSaved": result['EpisodesSaved'],
                "EpisodesDownloaded": result['EpisodesDownloaded']
            }
    else:  # Assume it's a tuple
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
        query = """
            SELECT * FROM (
                SELECT
                    "Podcasts".PodcastName as podcastname,
                    "Episodes".EpisodeTitle as episodetitle,
                    "Episodes".EpisodePubDate as episodepubdate,
                    "Episodes".EpisodeDescription as episodedescription,
                    "Episodes".EpisodeID as episodeid,
                    "Episodes".EpisodeArtwork as episodeartwork,
                    "Episodes".EpisodeURL as episodeurl,
                    "Episodes".EpisodeDuration as episodeduration,
                    "Podcasts".WebsiteURL as websiteurl,
                    "UserEpisodeHistory".ListenDuration as listenduration,
                    "Episodes".Completed as completed,
                    FALSE as is_youtube
                FROM "SavedEpisodes"
                INNER JOIN "Episodes" ON "SavedEpisodes".EpisodeID = "Episodes".EpisodeID
                INNER JOIN "Podcasts" ON "Episodes".PodcastID = "Podcasts".PodcastID
                LEFT JOIN "UserEpisodeHistory" ON
                    "SavedEpisodes".EpisodeID = "UserEpisodeHistory".EpisodeID
                    AND "UserEpisodeHistory".UserID = %s
                WHERE "SavedEpisodes".UserID = %s

                UNION ALL

                SELECT
                    "Podcasts".PodcastName as podcastname,
                    "YouTubeVideos".VideoTitle as episodetitle,
                    "YouTubeVideos".PublishedAt as episodepubdate,
                    "YouTubeVideos".VideoDescription as episodedescription,
                    "YouTubeVideos".VideoID as episodeid,
                    "YouTubeVideos".ThumbnailURL as episodeartwork,
                    "YouTubeVideos".VideoURL as episodeurl,
                    "YouTubeVideos".Duration as episodeduration,
                    "Podcasts".WebsiteURL as websiteurl,
                    "UserVideoHistory".ListenDuration as listenduration,
                    "YouTubeVideos".Completed as completed,
                    0 = 1 as is_youtube
                FROM "SavedVideos"
                INNER JOIN "YouTubeVideos" ON "SavedVideos".VideoID = "YouTubeVideos".VideoID
                INNER JOIN "Podcasts" ON "YouTubeVideos".PodcastID = "Podcasts".PodcastID
                LEFT JOIN "UserVideoHistory" ON
                    "SavedVideos".VideoID = "UserVideoHistory".VideoID
                    AND "UserVideoHistory".UserID = %s
                WHERE "SavedVideos".UserID = %s
            ) combined
            ORDER BY episodepubdate DESC
        """
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(dictionary=True)
        query = """
            SELECT * FROM (
                SELECT
                    Podcasts.PodcastName as podcastname,
                    Episodes.EpisodeTitle as episodetitle,
                    Episodes.EpisodePubDate as episodepubdate,
                    Episodes.EpisodeDescription as episodedescription,
                    Episodes.EpisodeID as episodeid,
                    Episodes.EpisodeArtwork as episodeartwork,
                    Episodes.EpisodeURL as episodeurl,
                    Episodes.EpisodeDuration as episodeduration,
                    Podcasts.WebsiteURL as websiteurl,
                    UserEpisodeHistory.ListenDuration as listenduration,
                    Episodes.Completed as completed,
                    0 as is_youtube
                FROM SavedEpisodes
                INNER JOIN Episodes ON SavedEpisodes.EpisodeID = Episodes.EpisodeID
                INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID
                LEFT JOIN UserEpisodeHistory ON
                    SavedEpisodes.EpisodeID = UserEpisodeHistory.EpisodeID
                    AND UserEpisodeHistory.UserID = %s
                WHERE SavedEpisodes.UserID = %s

                UNION ALL

                SELECT
                    Podcasts.PodcastName as podcastname,
                    YouTubeVideos.VideoTitle as episodetitle,
                    YouTubeVideos.PublishedAt as episodepubdate,
                    YouTubeVideos.VideoDescription as episodedescription,
                    YouTubeVideos.VideoID as episodeid,
                    YouTubeVideos.ThumbnailURL as episodeartwork,
                    YouTubeVideos.VideoURL as episodeurl,
                    YouTubeVideos.Duration as episodeduration,
                    Podcasts.WebsiteURL as websiteurl,
                    UserVideoHistory.ListenDuration as listenduration,
                    YouTubeVideos.Completed as completed,
                    1 as is_youtube
                FROM SavedVideos
                INNER JOIN YouTubeVideos ON SavedVideos.VideoID = YouTubeVideos.VideoID
                INNER JOIN Podcasts ON YouTubeVideos.PodcastID = Podcasts.PodcastID
                LEFT JOIN UserVideoHistory ON
                    SavedVideos.VideoID = UserVideoHistory.VideoID
                    AND UserVideoHistory.UserID = %s
                WHERE SavedVideos.UserID = %s
            ) combined
            ORDER BY episodepubdate DESC
        """

    cursor.execute(query, (user_id, user_id, user_id, user_id))  # Four user_id parameters needed now
    rows = cursor.fetchall()
    cursor.close()

    if not rows:
        return None

    saved_episodes = lowercase_keys(rows)

    if database_type != "postgresql":
        for episode in saved_episodes:
            episode['completed'] = bool(episode['completed'])
            episode['is_youtube'] = bool(episode['is_youtube'])

    return saved_episodes

def save_episode(cnx, database_type, episode_id, user_id, is_youtube=False):
    cursor = cnx.cursor()
    try:
        if is_youtube:
            if database_type == "postgresql":
                query = 'INSERT INTO "SavedVideos" (UserID, VideoID) VALUES (%s, %s)'
            else:
                query = "INSERT INTO SavedVideos (UserID, VideoID) VALUES (%s, %s)"
        else:
            if database_type == "postgresql":
                query = 'INSERT INTO "SavedEpisodes" (UserID, EpisodeID) VALUES (%s, %s)'
            else:
                query = "INSERT INTO SavedEpisodes (UserID, EpisodeID) VALUES (%s, %s)"

        cursor.execute(query, (user_id, episode_id))

        # Update UserStats table
        if database_type == "postgresql":
            query = 'UPDATE "UserStats" SET EpisodesSaved = EpisodesSaved + 1 WHERE UserID = %s'
        else:
            query = "UPDATE UserStats SET EpisodesSaved = EpisodesSaved + 1 WHERE UserID = %s"
        cursor.execute(query, (user_id,))

        cnx.commit()
        return True
    except Exception as e:
        print(f"Error saving {'video' if is_youtube else 'episode'}: {e}")
        return False
    finally:
        cursor.close()

def check_saved(cnx, database_type, user_id, episode_id, is_youtube=False):
    cursor = cnx.cursor()
    try:
        if is_youtube:
            if database_type == "postgresql":
                query = 'SELECT * FROM "SavedVideos" WHERE UserID = %s AND VideoID = %s'
            else:
                query = "SELECT * FROM SavedVideos WHERE UserID = %s AND VideoID = %s"
        else:
            if database_type == "postgresql":
                query = 'SELECT * FROM "SavedEpisodes" WHERE UserID = %s AND EpisodeID = %s'
            else:
                query = "SELECT * FROM SavedEpisodes WHERE UserID = %s AND EpisodeID = %s"

        cursor.execute(query, (user_id, episode_id))
        result = cursor.fetchone()
        return bool(result)
    except Exception as err:
        print(f"Error checking saved {'video' if is_youtube else 'episode'}: {err}")
        return False
    finally:
        cursor.close()

def remove_saved_episode(cnx, database_type, episode_id, user_id, is_youtube=False):
    cursor = cnx.cursor()
    try:
        logging.info(f"Removing {'video' if is_youtube else 'episode'} {episode_id} for user {user_id}")
        if is_youtube:
            if database_type == "postgresql":
                query = """
                    SELECT SaveID FROM "SavedVideos"
                    WHERE VideoID = %s AND UserID = %s
                """
            else:
                query = """
                    SELECT SaveID FROM SavedVideos
                    WHERE VideoID = %s AND UserID = %s
                """
        else:
            if database_type == "postgresql":
                query = """
                    SELECT SaveID FROM "SavedEpisodes"
                    WHERE EpisodeID = %s AND UserID = %s
                """
            else:
                query = """
                    SELECT SaveID FROM SavedEpisodes
                    WHERE EpisodeID = %s AND UserID = %s
                """
        cursor.execute(query, (episode_id, user_id))
        result = cursor.fetchone()
        if not result:
            logging.warning(f"No saved {'video' if is_youtube else 'episode'} found for ID {episode_id} and user {user_id}")
            return

        # Handle both dictionary and tuple result types
        save_id = result['saveid'] if isinstance(result, dict) else result[0]
        logging.info(f"Found SaveID: {save_id}")

        # Remove the saved entry
        if is_youtube:
            if database_type == "postgresql":
                query = 'DELETE FROM "SavedVideos" WHERE SaveID = %s'
            else:
                query = "DELETE FROM SavedVideos WHERE SaveID = %s"
        else:
            if database_type == "postgresql":
                query = 'DELETE FROM "SavedEpisodes" WHERE SaveID = %s'
            else:
                query = "DELETE FROM SavedEpisodes WHERE SaveID = %s"

        cursor.execute(query, (save_id,))
        rows_affected = cursor.rowcount
        logging.info(f"Deleted {rows_affected} rows")

        # Update UserStats
        if database_type == "postgresql":
            query = 'UPDATE "UserStats" SET EpisodesSaved = EpisodesSaved - 1 WHERE UserID = %s'
        else:
            query = "UPDATE UserStats SET EpisodesSaved = EpisodesSaved - 1 WHERE UserID = %s"

        cursor.execute(query, (user_id,))
        stats_rows_affected = cursor.rowcount
        logging.info(f"Updated {stats_rows_affected} user stats rows")

        cnx.commit()
    except Exception as e:
        logging.error(f"Error during {'video' if is_youtube else 'episode'} removal: {e}")
        cnx.rollback()
    finally:
        cursor.close()

def get_categories(cnx, database_type, podcast_id, user_id):
    cursor = cnx.cursor()

    try:
        if database_type == "postgresql":
            query = (
                'SELECT "categories" '
                'FROM "Podcasts" '
                'WHERE "podcastid" = %s AND "userid" = %s'
            )
        else:  # For MySQL or MariaDB
            query = (
                "SELECT Categories "
                "FROM Podcasts "
                "WHERE PodcastID = %s AND UserID = %s"
            )
        logging.debug(f"Executing query: {query} with PodcastID: {podcast_id} and UserID: {user_id}")
        cursor.execute(query, (podcast_id, user_id))
        result = cursor.fetchone()

        if not result:
            logging.warning("No matching podcast found.")
            cursor.close()
            return []

        # Check if the result is a dictionary or a tuple
        if isinstance(result, dict):
            # For dictionary, access the field by key
            categories_field = result.get('categories')  # Adjust key based on your schema
        elif isinstance(result, tuple):
            # For tuple, access the field by index
            categories_field = result[0]
        else:
            logging.error(f"Unexpected result type: {type(result)}")
            return []

        # Split the categories if they exist
        categories = categories_field.split(', ') if categories_field else []

        return categories

    except Exception as e:
        logging.error(f"Error retrieving categories: {e}")
        raise
    finally:
        cursor.close()



def add_category(cnx, database_type, podcast_id, user_id, category):
    cursor = cnx.cursor()

    try:
        if database_type == "postgresql":
            query = (
                'SELECT categories '
                'FROM "Podcasts" '
                'WHERE "podcastid" = %s AND "userid" = %s'
            )
        else:  # For MySQL or MariaDB
            query = (
                "SELECT Categories "
                "FROM Podcasts "
                "WHERE PodcastID = %s AND UserID = %s"
            )
        logging.debug(f"Executing query: {query} with PodcastID: {podcast_id} and UserID: {user_id}")
        cursor.execute(query, (podcast_id, user_id))
        result = cursor.fetchone()

        if not result:
            logging.warning("No matching podcast found.")
            cursor.close()
            return False

        # Extract the categories and split them into a list
        # Check if the result is a dictionary or a tuple
        if isinstance(result, dict):
            # For dictionary, access the field by key
            categories_field = result.get('categories')  # Adjust key based on your schema
        elif isinstance(result, tuple):
            # For tuple, access the field by index
            categories_field = result[0]
        else:
            logging.error(f"Unexpected result type: {type(result)}")
            return []

        # Split the categories if they exist
        categories = categories_field.split(', ') if categories_field else []


        # Add the new category if it doesn't exist
        if category not in categories:
            categories.append(category)

        # Join the updated categories back into a comma-separated string
        updated_categories = ', '.join(categories)

        # Update the database with the new categories list
        if database_type == "postgresql":
            update_query = (
                'UPDATE "Podcasts" '
                'SET "categories" = %s '
                'WHERE "podcastid" = %s AND "userid" = %s'
            )
        else:
            update_query = (
                "UPDATE Podcasts "
                "SET Categories = %s "
                "WHERE PodcastID = %s AND UserID = %s"
            )
        cursor.execute(update_query, (updated_categories, podcast_id, user_id))
        cnx.commit()

        return True

    except Exception as e:
        logging.error(f"Error adding category: {e}")
        raise
    finally:
        cursor.close()

def remove_category(cnx, database_type, podcast_id, user_id, category):
    cursor = cnx.cursor()

    try:
        if database_type == "postgresql":
            query = (
                'SELECT categories '
                'FROM "Podcasts" '
                'WHERE "podcastid" = %s AND "userid" = %s'
            )
        else:  # For MySQL or MariaDB
            query = (
                "SELECT Categories "
                "FROM Podcasts "
                "WHERE PodcastID = %s AND UserID = %s"
            )
        logging.debug(f"Executing query: {query} with PodcastID: {podcast_id} and UserID: {user_id}")
        cursor.execute(query, (podcast_id, user_id))
        result = cursor.fetchone()

        print(f'heres cats: {result}')

        if not result:
            logging.warning("No matching podcast found.")
            cursor.close()
            return

        # Extract the categories and split them into a list
        # Check if the result is a dictionary or a tuple
        if isinstance(result, dict):
            # For dictionary, access the field by key
            categories_field = result.get('categories')  # Adjust key based on your schema
        elif isinstance(result, tuple):
            # For tuple, access the field by index
            categories_field = result[0]
        else:
            logging.error(f"Unexpected result type: {type(result)}")
            return []

        # Split the categories if they exist
        categories = categories_field.split(', ') if categories_field else []

        # Remove the category if it exists
        if category in categories:
            categories.remove(category)

        # Join the updated categories back into a comma-separated string
        updated_categories = ', '.join(categories)

        # Update the database with the new categories list
        if database_type == "postgresql":
            update_query = (
                'UPDATE "Podcasts" '
                'SET "categories" = %s '
                'WHERE "podcastid" = %s AND "userid" = %s'
            )
        else:
            update_query = (
                "UPDATE Podcasts "
                "SET Categories = %s "
                "WHERE PodcastID = %s AND UserID = %s"
            )
        cursor.execute(update_query, (updated_categories, podcast_id, user_id))
        cnx.commit()

    except Exception as e:
        logging.error(f"Error removing category: {e}")
        raise
    finally:
        cursor.close()


# In database_functions/functions.py
#
def send_ntfy_notification(topic: str, server_url: str, title: str, message: str):
    try:
        import requests

        # Default to ntfy.sh if no server URL provided
        base_url = server_url.rstrip('/') if server_url else "https://ntfy.sh"
        url = f"{base_url}/{topic}"

        headers = {
            "Title": title,
            "Content-Type": "text/plain"
        }

        response = requests.post(url, headers=headers, data=message)
        response.raise_for_status()
        return True
    except Exception as e:
        logging.error(f"Error sending NTFY notification: {e}")
        return False

def send_gotify_notification(server_url: str, token: str, title: str, message: str):
    try:
        import requests

        url = f"{server_url.rstrip('/')}/message"

        headers = {
            "X-Gotify-Key": token
        }

        data = {
            "title": title,
            "message": message,
            "priority": 5
        }

        response = requests.post(url, headers=headers, json=data)
        response.raise_for_status()
        return True
    except Exception as e:
        logging.error(f"Error sending Gotify notification: {e}")
        return False

# Base notification functions for actual episode notifications
def send_ntfy_notification(topic: str, server_url: str, title: str, message: str):
    try:
        base_url = server_url.rstrip('/') if server_url else "https://ntfy.sh"
        url = f"{base_url}/{topic}"
        headers = {
            "Title": title,
            "Content-Type": "text/plain"
        }
        # Add short timeout - if it takes more than 2 seconds, abort
        response = requests.post(url, headers=headers, data=message, timeout=2)
        response.raise_for_status()
        return True
    except requests.Timeout:
        logging.error(f"Timeout sending notification to {url}")
        return False
    except Exception as e:
        logging.error(f"Error sending NTFY notification: {e}")
        return False

def send_gotify_notification(server_url: str, token: str, title: str, message: str):
    try:
        url = f"{server_url.rstrip('/')}/message"
        data = {
            "title": title,
            "message": message,
            "priority": 5
        }
        headers = {
            "X-Gotify-Key": token
        }
        response = requests.post(url, headers=headers, json=data, timeout=2)
        response.raise_for_status()
        return True
    except requests.Timeout:
        logging.error(f"Timeout sending notification to {url}")
        return False
    except Exception as e:
        logging.error(f"Error sending Gotify notification: {e}")
        return False

# Test versions that specifically mention they're test notifications
def send_test_ntfy_notification(topic: str, server_url: str):
    return send_ntfy_notification(
        topic=topic,
        server_url=server_url,
        title="Pinepods Test Notification",
        message="This is a test notification from your Pinepods server!"
    )

def send_test_gotify_notification(server_url: str, token: str):
    return send_gotify_notification(
        server_url=server_url,
        token=token,
        title="Pinepods Test Notification",
        message="This is a test notification from your Pinepods server!"
    )

def send_test_notification(cnx, database_type, user_id, platform):
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            query = """
                SELECT Platform, Enabled, NtfyTopic, NtfyServerUrl, GotifyUrl, GotifyToken
                FROM "UserNotificationSettings"
                WHERE UserID = %s AND Platform = %s AND Enabled = TRUE
            """
        else:
            query = """
                SELECT Platform, Enabled, NtfyTopic, NtfyServerUrl, GotifyUrl, GotifyToken
                FROM UserNotificationSettings
                WHERE UserID = %s AND Platform = %s AND Enabled = TRUE
            """
        cursor.execute(query, (user_id, platform))
        settings = cursor.fetchone()
        if not settings:
            logging.error("No notification settings found")
            return False

        if isinstance(settings, dict):  # PostgreSQL dict case
            if platform == 'ntfy':
                return send_test_ntfy_notification(
                    topic=settings['ntfytopic'],  # Note: lowercase from your logs
                    server_url=settings['ntfyserverurl']  # Note: lowercase from your logs
                )
            else:  # gotify
                return send_test_gotify_notification(
                    server_url=settings['gotifyurl'],  # Note: lowercase from your logs
                    token=settings['gotifytoken']  # Note: lowercase from your logs
                )
        else:  # MySQL or PostgreSQL tuple case
            if platform == 'ntfy':
                return send_test_ntfy_notification(
                    settings[2],  # NtfyTopic
                    settings[3]  # NtfyServerUrl
                )
            else:  # gotify
                return send_test_gotify_notification(
                    settings[4],  # GotifyUrl
                    settings[5]  # GotifyToken
                )
    except Exception as e:
        logging.error(f"Error sending test notification: {e}")
        logging.error(f"Settings object type: {type(settings)}")
        logging.error(f"Settings content: {settings}")
        return False
    finally:
        cursor.close()

def get_notification_settings(cnx, database_type, user_id):
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            query = """
                SELECT Platform, Enabled, NtfyTopic, NtfyServerUrl, GotifyUrl, GotifyToken
                FROM "UserNotificationSettings"
                WHERE UserID = %s
            """
        else:  # MySQL
            query = """
                SELECT Platform, Enabled, NtfyTopic, NtfyServerUrl, GotifyUrl, GotifyToken
                FROM UserNotificationSettings
                WHERE UserID = %s
            """

        cursor.execute(query, (user_id,))
        result = cursor.fetchall()

        settings = []
        for row in result:
            if isinstance(row, dict):  # PostgreSQL with RealDictCursor
                setting = {
                    "platform": row["platform"],
                    "enabled": bool(row["enabled"]),
                    "ntfy_topic": row["ntfytopic"],
                    "ntfy_server_url": row["ntfyserverurl"],
                    "gotify_url": row["gotifyurl"],
                    "gotify_token": row["gotifytoken"]
                }
            else:  # MySQL or PostgreSQL with regular cursor
                setting = {
                    "platform": row[0],
                    "enabled": bool(row[1]),
                    "ntfy_topic": row[2],
                    "ntfy_server_url": row[3],
                    "gotify_url": row[4],
                    "gotify_token": row[5]
                }
            settings.append(setting)

        return settings

    except Exception as e:
        logging.error(f"Error fetching notification settings: {e}")
        raise
    finally:
        cursor.close()

def update_notification_settings(cnx, database_type, user_id, platform, enabled, ntfy_topic=None,
                               ntfy_server_url=None, gotify_url=None, gotify_token=None):
    cursor = cnx.cursor()
    try:
        # First check if settings exist for this user and platform
        if database_type == "postgresql":
            check_query = """
                SELECT 1 FROM "UserNotificationSettings"
                WHERE UserID = %s AND Platform = %s
            """
        else:
            check_query = """
                SELECT 1 FROM UserNotificationSettings
                WHERE UserID = %s AND Platform = %s
            """

        cursor.execute(check_query, (user_id, platform))
        exists = cursor.fetchone() is not None

        if exists:
            if database_type == "postgresql":
                query = """
                    UPDATE "UserNotificationSettings"
                    SET Enabled = %s,
                        NtfyTopic = %s,
                        NtfyServerUrl = %s,
                        GotifyUrl = %s,
                        GotifyToken = %s
                    WHERE UserID = %s AND Platform = %s
                """
            else:
                query = """
                    UPDATE UserNotificationSettings
                    SET Enabled = %s,
                        NtfyTopic = %s,
                        NtfyServerUrl = %s,
                        GotifyUrl = %s,
                        GotifyToken = %s
                    WHERE UserID = %s AND Platform = %s
                """
        else:
            if database_type == "postgresql":
                query = """
                    INSERT INTO "UserNotificationSettings"
                    (UserID, Platform, Enabled, NtfyTopic, NtfyServerUrl, GotifyUrl, GotifyToken)
                    VALUES (%s, %s, %s, %s, %s, %s, %s)
                """
            else:
                query = """
                    INSERT INTO UserNotificationSettings
                    (UserID, Platform, Enabled, NtfyTopic, NtfyServer_url, GotifyUrl, GotifyToken)
                    VALUES (%s, %s, %s, %s, %s, %s, %s)
                """

        params = (
            enabled if exists else user_id,
            ntfy_topic if exists else platform,
            ntfy_server_url if exists else enabled,
            gotify_url if exists else ntfy_topic,
            gotify_token if exists else ntfy_server_url,
            user_id if exists else gotify_url,
            platform if exists else gotify_token
        )

        cursor.execute(query, params)
        cnx.commit()
        return True

    except Exception as e:
        logging.error(f"Error updating notification settings: {e}")
        cnx.rollback()
        raise
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

def check_youtube_channel(cnx, database_type, user_id, channel_name, channel_url):
    cursor = None
    try:
        cursor = cnx.cursor()
        if database_type == "postgresql":
            query = '''
                SELECT PodcastID
                FROM "Podcasts"
                WHERE UserID = %s
                AND PodcastName = %s
                AND FeedURL = %s
                AND IsYouTubeChannel = TRUE
            '''
        else:  # MySQL or MariaDB
            query = '''
                SELECT PodcastID
                FROM Podcasts
                WHERE UserID = %s
                AND PodcastName = %s
                AND FeedURL = %s
                AND IsYouTubeChannel = TRUE
            '''
        cursor.execute(query, (user_id, channel_name, channel_url))
        return cursor.fetchone() is not None
    except Exception:
        return False
    finally:
        if cursor:
            cursor.close()


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

def get_episode_metadata(database_type, cnx, episode_id, user_id, person_episode=False, is_youtube=False):
    if database_type == "postgresql":
        from psycopg.rows import dict_row
        cnx.row_factory = dict_row
        cursor = cnx.cursor()

        if is_youtube:
            # Query for YouTube videos
            query_youtube = """
                SELECT "Podcasts".PodcastID, "Podcasts".PodcastIndexID, "Podcasts".FeedURL,
                        "Podcasts".PodcastName, "Podcasts".ArtworkURL,
                        "YouTubeVideos".VideoTitle as EpisodeTitle,
                        "YouTubeVideos".PublishedAt as EpisodePubDate,
                        "YouTubeVideos".VideoDescription as EpisodeDescription,
                        "YouTubeVideos".ThumbnailURL as EpisodeArtwork,
                        "YouTubeVideos".VideoURL as EpisodeURL,
                        "YouTubeVideos".Duration as EpisodeDuration,
                        "YouTubeVideos".VideoID as EpisodeID,
                        "YouTubeVideos".ListenPosition as ListenDuration,
                        "YouTubeVideos".Completed,
                        CASE WHEN q.EpisodeID IS NOT NULL THEN true ELSE false END as is_queued,
                        CASE WHEN s.EpisodeID IS NOT NULL THEN true ELSE false END as is_saved,
                        CASE WHEN d.EpisodeID IS NOT NULL THEN true ELSE false END as is_downloaded,
                        TRUE::boolean as is_youtube
                FROM "YouTubeVideos"
                INNER JOIN "Podcasts" ON "YouTubeVideos".PodcastID = "Podcasts".PodcastID
                LEFT JOIN "EpisodeQueue" q ON "YouTubeVideos".VideoID = q.EpisodeID AND q.UserID = %s
                LEFT JOIN "SavedEpisodes" s ON "YouTubeVideos".VideoID = s.EpisodeID AND s.UserID = %s
                LEFT JOIN "DownloadedEpisodes" d ON "YouTubeVideos".VideoID = d.EpisodeID AND d.UserID = %s
                WHERE "YouTubeVideos".VideoID = %s AND "Podcasts".UserID = %s
            """
            cursor.execute(query_youtube, (user_id, user_id, user_id, episode_id, user_id))
            result = cursor.fetchone()

            # If not found, try with system user (1)
            if not result:
                cursor.execute(query_youtube, (user_id, user_id, user_id, episode_id, 1))
                result = cursor.fetchone()

        elif person_episode:
            # First get the episode from PeopleEpisodes and match with Episodes using title and URL
            query_people = """
                SELECT pe.*,
                        p.PodcastID, p.PodcastName, p.ArtworkURL as podcast_artwork,
                        p.FeedURL, p.WebsiteURL, p.PodcastIndexID,
                        e.EpisodeID as real_episode_id,
                        COALESCE(pe.EpisodeArtwork, p.ArtworkURL) as final_artwork,
                        CASE WHEN q.EpisodeID IS NOT NULL THEN true ELSE false END as is_queued,
                        CASE WHEN s.EpisodeID IS NOT NULL THEN true ELSE false END as is_saved,
                        CASE WHEN d.EpisodeID IS NOT NULL THEN true ELSE false END as is_downloaded,
                        FALSE::boolean as is_youtube
                FROM "PeopleEpisodes" pe
                JOIN "Podcasts" p ON pe.PodcastID = p.PodcastID
                JOIN "Episodes" e ON (
                    e.EpisodeTitle = pe.EpisodeTitle
                    AND e.EpisodeURL = pe.EpisodeURL
                )
                LEFT JOIN "EpisodeQueue" q ON e.EpisodeID = q.EpisodeID AND q.UserID = %s
                LEFT JOIN "SavedEpisodes" s ON e.EpisodeID = s.EpisodeID AND s.UserID = %s
                LEFT JOIN "DownloadedEpisodes" d ON e.EpisodeID = d.EpisodeID AND d.UserID = %s
                WHERE pe.EpisodeID = %s
            """
            cursor.execute(query_people, (user_id, user_id, user_id, episode_id))
            people_episode = cursor.fetchone()

            if not people_episode:
                raise ValueError(f"No people episode found with ID {episode_id}")

            # Now get additional data using the real episode ID
            query_history = """
                SELECT "UserEpisodeHistory".ListenDuration, "Episodes".Completed
                FROM "Episodes"
                LEFT JOIN "UserEpisodeHistory" ON
                    "Episodes".EpisodeID = "UserEpisodeHistory".EpisodeID
                    AND "UserEpisodeHistory".UserID = %s
                WHERE "Episodes".EpisodeID = %s
            """
            cursor.execute(query_history, (user_id, people_episode['real_episode_id']))
            history_data = cursor.fetchone() or {}

            # Combine the data
            result = {
                'episodetitle': people_episode['episodetitle'],
                'podcastname': people_episode['podcastname'],
                'podcastid': people_episode['podcastid'],
                'podcastindexid': people_episode['podcastindexid'],
                'feedurl': people_episode['feedurl'],
                'episodepubdate': people_episode['episodepubdate'].isoformat() if people_episode['episodepubdate'] else None,
                'episodedescription': people_episode['episodedescription'],
                'episodeartwork': people_episode['final_artwork'],
                'episodeurl': people_episode['episodeurl'],
                'episodeduration': people_episode['episodeduration'],
                'listenduration': history_data.get('listenduration'),
                'episodeid': people_episode['real_episode_id'],
                'completed': history_data.get('completed', False),
                'is_queued': people_episode['is_queued'],
                'is_saved': people_episode['is_saved'],
                'is_downloaded': people_episode['is_downloaded']
            }
        else:
            # Original query for regular episodes
            query = """
                SELECT "Podcasts".PodcastID, "Podcasts".PodcastIndexID, "Podcasts".FeedURL,
                        "Podcasts".PodcastName, "Podcasts".ArtworkURL, "Episodes".EpisodeTitle,
                        "Episodes".EpisodePubDate, "Episodes".EpisodeDescription,
                        "Episodes".EpisodeArtwork, "Episodes".EpisodeURL, "Episodes".EpisodeDuration,
                        "Episodes".EpisodeID, "Podcasts".WebsiteURL,
                        "UserEpisodeHistory".ListenDuration, "Episodes".Completed,
                        CASE WHEN q.EpisodeID IS NOT NULL THEN true ELSE false END as is_queued,
                        CASE WHEN s.EpisodeID IS NOT NULL THEN true ELSE false END as is_saved,
                        CASE WHEN d.EpisodeID IS NOT NULL THEN true ELSE false END as is_downloaded,
                        FALSE::boolean as is_youtube
                FROM "Episodes"
                INNER JOIN "Podcasts" ON "Episodes".PodcastID = "Podcasts".PodcastID
                LEFT JOIN "UserEpisodeHistory" ON
                    "Episodes".EpisodeID = "UserEpisodeHistory".EpisodeID
                    AND "Podcasts".UserID = "UserEpisodeHistory".UserID
                LEFT JOIN "EpisodeQueue" q ON "Episodes".EpisodeID = q.EpisodeID AND q.UserID = %s
                LEFT JOIN "SavedEpisodes" s ON "Episodes".EpisodeID = s.EpisodeID AND s.UserID = %s
                LEFT JOIN "DownloadedEpisodes" d ON "Episodes".EpisodeID = d.EpisodeID AND d.UserID = %s
                WHERE "Episodes".EpisodeID = %s AND "Podcasts".UserID = %s
            """
            cursor.execute(query, (user_id, user_id, user_id, episode_id, user_id))
            result = cursor.fetchone()

            # If not found, try with system user (1)
            if not result:
                cursor.execute(query, (user_id, user_id, user_id, episode_id, 1))
                result = cursor.fetchone()

        cursor.close()

        if not result:
            raise ValueError(f"No episode found with ID {episode_id}" +
                            (" for person episode" if person_episode else f" for user {user_id}"))

        lower_row = lowercase_keys(result)
        bool_fix = convert_bools(lower_row, database_type)
        return bool_fix


    else:
        cursor = cnx.cursor(dictionary=True)
        if is_youtube:
            # MariaDB version of YouTube videos query
            query = """
                SELECT Podcasts.PodcastID, Podcasts.PodcastIndexID, Podcasts.FeedURL,
                    Podcasts.PodcastName, Podcasts.ArtworkURL,
                    YouTubeVideos.VideoTitle as EpisodeTitle,
                    YouTubeVideos.PublishedAt as EpisodePubDate,
                    YouTubeVideos.VideoDescription as EpisodeDescription,
                    YouTubeVideos.ThumbnailURL as EpisodeArtwork,
                    YouTubeVideos.VideoURL as EpisodeURL,
                    YouTubeVideos.Duration as EpisodeDuration,
                    YouTubeVideos.VideoID as EpisodeID,
                    YouTubeVideos.ListenPosition as ListenDuration,
                    YouTubeVideos.Completed,
                    CASE WHEN q.EpisodeID IS NOT NULL THEN 1 ELSE 0 END as is_queued,
                    CASE WHEN s.EpisodeID IS NOT NULL THEN 1 ELSE 0 END as is_saved,
                    CASE WHEN d.EpisodeID IS NOT NULL THEN 1 ELSE 0 END as is_downloaded,
                    1 as is_youtube
                FROM YouTubeVideos
                INNER JOIN Podcasts ON YouTubeVideos.PodcastID = Podcasts.PodcastID
                LEFT JOIN EpisodeQueue q ON YouTubeVideos.VideoID = q.EpisodeID AND q.UserID = %s
                LEFT JOIN SavedEpisodes s ON YouTubeVideos.VideoID = s.EpisodeID AND s.UserID = %s
                LEFT JOIN DownloadedEpisodes d ON YouTubeVideos.VideoID = d.EpisodeID AND d.UserID = %s
                WHERE YouTubeVideos.VideoID = %s AND Podcasts.UserID = %s
            """
            cursor.execute(query, (user_id, user_id, user_id, episode_id, user_id))
            result = cursor.fetchone()
        elif person_episode:
                # MariaDB version of people episodes query
                query_people = """
                    SELECT pe.*,
                        p.PodcastID, p.PodcastName, p.ArtworkURL as podcast_artwork,
                        p.FeedURL, p.WebsiteURL, p.PodcastIndexID,
                        e.EpisodeID as real_episode_id,
                        COALESCE(pe.EpisodeArtwork, p.ArtworkURL) as final_artwork,
                        CASE WHEN q.EpisodeID IS NOT NULL THEN 1 ELSE 0 END as is_queued,
                        CASE WHEN s.EpisodeID IS NOT NULL THEN 1 ELSE 0 END as is_saved,
                        CASE WHEN d.EpisodeID IS NOT NULL THEN 1 ELSE 0 END as is_downloaded,
                        FALSE as is_youtube
                    FROM PeopleEpisodes pe
                    JOIN Podcasts p ON pe.PodcastID = p.PodcastID
                    JOIN Episodes e ON (
                        e.EpisodeTitle = pe.EpisodeTitle
                        AND e.EpisodeURL = pe.EpisodeURL
                    )
                    LEFT JOIN EpisodeQueue q ON e.EpisodeID = q.EpisodeID AND q.UserID = %s
                    LEFT JOIN SavedEpisodes s ON e.EpisodeID = s.EpisodeID AND s.UserID = %s
                    LEFT JOIN DownloadedEpisodes d ON e.EpisodeID = d.EpisodeID AND d.UserID = %s
                    WHERE pe.EpisodeID = %s
                """
                cursor.execute(query_people, (user_id, user_id, user_id, episode_id))
                people_episode = cursor.fetchone()

                if not people_episode:
                    raise ValueError(f"No people episode found with ID {episode_id}")

                # Get additional data using the real episode ID
                query_history = """
                    SELECT UserEpisodeHistory.ListenDuration, Episodes.Completed
                    FROM Episodes
                    LEFT JOIN UserEpisodeHistory ON
                        Episodes.EpisodeID = UserEpisodeHistory.EpisodeID
                        AND UserEpisodeHistory.UserID = %s
                    WHERE Episodes.EpisodeID = %s
                """
                cursor.execute(query_history, (user_id, people_episode['real_episode_id']))
                history_data = cursor.fetchone() or {}

                # Combine the data
                result = {
                    'episodetitle': people_episode['episodetitle'],
                    'podcastname': people_episode['podcastname'],
                    'podcastid': people_episode['podcastid'],
                    'podcastindexid': people_episode['podcastindexid'],
                    'feedurl': people_episode['feedurl'],
                    'episodepubdate': people_episode['episodepubdate'].isoformat() if people_episode['episodepubdate'] else None,
                    'episodedescription': people_episode['episodedescription'],
                    'episodeartwork': people_episode['final_artwork'],
                    'episodeurl': people_episode['episodeurl'],
                    'episodeduration': people_episode['episodeduration'],
                    'listenduration': history_data.get('listenduration'),
                    'episodeid': people_episode['real_episode_id'],
                    'completed': bool(history_data.get('completed', 0)),
                    'is_queued': bool(people_episode['is_queued']),
                    'is_saved': bool(people_episode['is_saved']),
                    'is_downloaded': bool(people_episode['is_downloaded'])
                }
        else:
            # MariaDB version of regular episodes query
            query = """
                SELECT Podcasts.PodcastID, Podcasts.PodcastIndexID, Podcasts.FeedURL,
                    Podcasts.PodcastName, Podcasts.ArtworkURL, Episodes.EpisodeTitle,
                    Episodes.EpisodePubDate, Episodes.EpisodeDescription,
                    Episodes.EpisodeArtwork, Episodes.EpisodeURL, Episodes.EpisodeDuration,
                    Episodes.EpisodeID, Podcasts.WebsiteURL,
                    UserEpisodeHistory.ListenDuration, Episodes.Completed,
                    CASE WHEN q.EpisodeID IS NOT NULL THEN 1 ELSE 0 END as is_queued,
                    CASE WHEN s.EpisodeID IS NOT NULL THEN 1 ELSE 0 END as is_saved,
                    CASE WHEN d.EpisodeID IS NOT NULL THEN 1 ELSE 0 END as is_downloaded,
                    FALSE as is_youtube
                FROM Episodes
                INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID
                LEFT JOIN UserEpisodeHistory ON
                    Episodes.EpisodeID = UserEpisodeHistory.EpisodeID
                    AND Podcasts.UserID = UserEpisodeHistory.UserID
                LEFT JOIN EpisodeQueue q ON Episodes.EpisodeID = q.EpisodeID AND q.UserID = %s
                LEFT JOIN SavedEpisodes s ON Episodes.EpisodeID = s.EpisodeID AND s.UserID = %s
                LEFT JOIN DownloadedEpisodes d ON Episodes.EpisodeID = d.EpisodeID AND d.UserID = %s
                WHERE Episodes.EpisodeID = %s AND Podcasts.UserID = %s
            """
            cursor.execute(query, (user_id, user_id, user_id, episode_id, user_id))
            result = cursor.fetchone()

            # If not found, try with system user (1)
            if not result:
                cursor.execute(query, (user_id, user_id, user_id, episode_id, 1))
                result = cursor.fetchone()

        cursor.close()

        if not result:
            raise ValueError(f"No episode found with ID {episode_id}" +
                        (" for person episode" if person_episode else f" for user {user_id}"))

        # Convert boolean fields for MariaDB
        if result:
            result['completed'] = bool(result.get('completed', 0))
            result['is_queued'] = bool(result.get('is_queued', 0))
            result['is_saved'] = bool(result.get('is_saved', 0))
            result['is_downloaded'] = bool(result.get('is_downloaded', 0))
            result['is_youtube'] = bool(result.get('is_youtube', 0))

            # Format date if present
            if result.get('episodepubdate'):
                result['episodepubdate'] = result['episodepubdate'].isoformat()

        lower_row = lowercase_keys(result)
        bool_fix = convert_bools(lower_row, database_type)
        return bool_fix


def get_episode_metadata_id(database_type, cnx, episode_id):
    if database_type == "postgresql":
        from psycopg.rows import dict_row
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
        query = """
            SELECT * FROM (
                SELECT
                    "Podcasts".PodcastID,
                    "Podcasts".FeedURL,
                    "Podcasts".PodcastName,
                    "Podcasts".ArtworkURL,
                    "Episodes".EpisodeTitle,
                    "Episodes".EpisodePubDate,
                    "Episodes".EpisodeDescription,
                    "Episodes".EpisodeArtwork,
                    "Episodes".EpisodeURL,
                    "Episodes".EpisodeDuration,
                    "Episodes".EpisodeID,
                    "Podcasts".WebsiteURL,
                    "UserEpisodeHistory".ListenDuration,
                    "Episodes".Completed,
                    FALSE::boolean as is_youtube
                FROM "Episodes"
                INNER JOIN "Podcasts" ON "Episodes".PodcastID = "Podcasts".PodcastID
                LEFT JOIN "UserEpisodeHistory" ON
                    "Episodes".EpisodeID = "UserEpisodeHistory".EpisodeID
                    AND "Podcasts".UserID = "UserEpisodeHistory".UserID
                WHERE "Episodes".EpisodeID = %s

                UNION ALL

                SELECT
                    "Podcasts".PodcastID,
                    "Podcasts".FeedURL,
                    "Podcasts".PodcastName,
                    "Podcasts".ArtworkURL,
                    "YouTubeVideos".VideoTitle as EpisodeTitle,
                    "YouTubeVideos".PublishedAt as EpisodePubDate,
                    "YouTubeVideos".VideoDescription as EpisodeDescription,
                    "YouTubeVideos".ThumbnailURL as EpisodeArtwork,
                    "YouTubeVideos".VideoURL as EpisodeURL,
                    "YouTubeVideos".Duration as EpisodeDuration,
                    "YouTubeVideos".VideoID as EpisodeID,
                    "Podcasts".WebsiteURL,
                    "YouTubeVideos".ListenPosition as ListenDuration,
                    "YouTubeVideos".Completed,
                    TRUE::boolean as is_youtube
                FROM "YouTubeVideos"
                INNER JOIN "Podcasts" ON "YouTubeVideos".PodcastID = "Podcasts".PodcastID
                WHERE "YouTubeVideos".VideoID = %s
            ) combined
            LIMIT 1
        """
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(dictionary=True)
        query = """
            SELECT * FROM (
                SELECT
                    Podcasts.PodcastID,
                    Podcasts.FeedURL,
                    Podcasts.PodcastName,
                    Podcasts.ArtworkURL,
                    Episodes.EpisodeTitle,
                    Episodes.EpisodePubDate,
                    Episodes.EpisodeDescription,
                    Episodes.EpisodeArtwork,
                    Episodes.EpisodeURL,
                    Episodes.EpisodeDuration,
                    Episodes.EpisodeID,
                    Podcasts.WebsiteURL,
                    UserEpisodeHistory.ListenDuration,
                    Episodes.Completed,
                    FALSE as is_youtube
                FROM Episodes
                INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID
                LEFT JOIN UserEpisodeHistory ON
                    Episodes.EpisodeID = UserEpisodeHistory.EpisodeID
                    AND Podcasts.UserID = UserEpisodeHistory.UserID
                WHERE Episodes.EpisodeID = %s

                UNION ALL

                SELECT
                    Podcasts.PodcastID,
                    Podcasts.FeedURL,
                    Podcasts.PodcastName,
                    Podcasts.ArtworkURL,
                    YouTubeVideos.VideoTitle as EpisodeTitle,
                    YouTubeVideos.PublishedAt as EpisodePubDate,
                    YouTubeVideos.VideoDescription as EpisodeDescription,
                    YouTubeVideos.ThumbnailURL as EpisodeArtwork,
                    YouTubeVideos.VideoURL as EpisodeURL,
                    YouTubeVideos.Duration as EpisodeDuration,
                    YouTubeVideos.VideoID as EpisodeID,
                    Podcasts.WebsiteURL,
                    YouTubeVideos.ListenPosition as ListenDuration,
                    YouTubeVideos.Completed,
                    TRUE as is_youtube
                FROM YouTubeVideos
                INNER JOIN Podcasts ON YouTubeVideos.PodcastID = Podcasts.PodcastID
                WHERE YouTubeVideos.VideoID = %s
            ) combined
            LIMIT 1
        """

    cursor.execute(query, (episode_id, episode_id))
    row = cursor.fetchone()
    cursor.close()

    if not row:
        raise ValueError(f"No episode found with ID {episode_id}")

    lower_row = lowercase_keys(row)
    bool_fix = convert_bools(lower_row, database_type)
    return bool_fix



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
        logging.info(f"Successfully saved MFA secret for user")
        return True
    except Exception as e:
        logging.error(f"Error saving MFA secret for user")
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

        if isinstance(result, tuple):
            # Convert result to dictionary format for consistency
            result = dict(zip([desc[0] for desc in cursor.description], result))

        if isinstance(result, dict):
            if database_type == 'postgresql':
                return result.get('mfa_secret')
            else:
                return result.get('MFA_Secret')
        else:
            print("Unexpected result format:", result)
            return None
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
        # Check if result is a dict or tuple
        if isinstance(result, dict):
            # Handle both postgres (lowercase) and mysql (uppercase) dict keys
            timezone = result.get('timezone') or result.get('Timezone')
            timeformat = result.get('timeformat') or result.get('TimeFormat')
            dateformat = result.get('dateformat') or result.get('DateFormat')
        else:
            # Handle tuple result (order should match SELECT query)
            timezone, timeformat, dateformat = result

        return timezone, timeformat, dateformat
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

        if database_type == "postgresql":

            first_login = result[0] if isinstance(result, tuple) else result['firstlogin']
        else:
            first_login = result[0] if isinstance(result, tuple) else result['FirstLogin']
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



def search_data(database_type, cnx, search_term, user_id):
    if database_type == "postgresql":
        from psycopg.rows import dict_row
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
        query = """
            SELECT
                p.PodcastID as podcastid,
                p.PodcastName as podcastname,
                p.ArtworkURL as artworkurl,
                p.Author as author,
                p.Categories as categories,
                p.Description as description,
                p.EpisodeCount as episodecount,
                p.FeedURL as feedurl,
                p.WebsiteURL as websiteurl,
                p.Explicit as explicit,
                p.UserID as userid,
                p.IsYouTubeChannel as is_youtube,
                COALESCE(e.EpisodeID, y.VideoID) as episodeid,
                COALESCE(e.EpisodeTitle, y.VideoTitle) as episodetitle,
                COALESCE(e.EpisodeDescription, y.VideoDescription) as episodedescription,
                COALESCE(e.EpisodeURL, y.VideoURL) as episodeurl,
                COALESCE(e.EpisodeArtwork, y.ThumbnailURL) as episodeartwork,
                COALESCE(e.EpisodePubDate, y.PublishedAt) as episodepubdate,
                COALESCE(e.EpisodeDuration, y.Duration) as episodeduration,
                CASE
                    WHEN y.VideoID IS NOT NULL THEN y.ListenPosition
                    ELSE h.ListenDuration
                END as listenduration,
                COALESCE(e.Completed, y.Completed) as completed
            FROM "Podcasts" p
            LEFT JOIN (
                SELECT * FROM "Episodes" WHERE EpisodeTitle ILIKE %s
            ) e ON p.PodcastID = e.PodcastID
            LEFT JOIN (
                SELECT * FROM "YouTubeVideos" WHERE VideoTitle ILIKE %s
            ) y ON p.PodcastID = y.PodcastID
            LEFT JOIN "UserEpisodeHistory" h ON
                (e.EpisodeID = h.EpisodeID AND h.UserID = %s)
            WHERE p.UserID = %s
                AND (e.EpisodeID IS NOT NULL OR y.VideoID IS NOT NULL)
        """
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(dictionary=True)
        query = """
            SELECT
                p.PodcastID as podcastid,
                p.PodcastName as podcastname,
                p.ArtworkURL as artworkurl,
                p.Author as author,
                p.Categories as categories,
                p.Description as description,
                p.EpisodeCount as episodecount,
                p.FeedURL as feedurl,
                p.WebsiteURL as websiteurl,
                p.Explicit as explicit,
                p.UserID as userid,
                p.IsYouTubeChannel as is_youtube,
                COALESCE(e.EpisodeID, y.VideoID) as episodeid,
                COALESCE(e.EpisodeTitle, y.VideoTitle) as episodetitle,
                COALESCE(e.EpisodeDescription, y.VideoDescription) as episodedescription,
                COALESCE(e.EpisodeURL, y.VideoURL) as episodeurl,
                COALESCE(e.EpisodeArtwork, y.ThumbnailURL) as episodeartwork,
                COALESCE(e.EpisodePubDate, y.PublishedAt) as episodepubdate,
                COALESCE(e.EpisodeDuration, y.Duration) as episodeduration,
                CASE
                    WHEN y.VideoID IS NOT NULL THEN y.ListenPosition
                    ELSE h.ListenDuration
                END as listenduration,
                COALESCE(e.Completed, y.Completed) as completed
            FROM Podcasts p
            LEFT JOIN (
                SELECT * FROM Episodes WHERE EpisodeTitle LIKE %s
            ) e ON p.PodcastID = e.PodcastID
            LEFT JOIN (
                SELECT * FROM YouTubeVideos WHERE VideoTitle LIKE %s
            ) y ON p.PodcastID = y.PodcastID
            LEFT JOIN UserEpisodeHistory h ON
                (e.EpisodeID = h.EpisodeID AND h.UserID = %s)
            WHERE p.UserID = %s
                AND (e.EpisodeID IS NOT NULL OR y.VideoID IS NOT NULL)
        """

    # Add wildcards for the LIKE/ILIKE clause
    search_term = f"%{search_term}%"

    try:
        start = time.time()
        logging.info(f"Executing query: {query}")
        logging.info(f"Search term: {search_term}, User ID: {user_id}")
        cursor.execute(query, (search_term, search_term, user_id, user_id))
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

        if database_type != "postgresql":
            for row in result:
                row['is_youtube'] = bool(row.get('is_youtube', 0))
                row['completed'] = bool(row.get('completed', 0))

        return result

    except Exception as e:
        logging.error(f"Error retrieving Podcast Episodes: {e}")
        return None


def queue_pod(database_type, cnx, episode_id, user_id, is_youtube=False):
    if database_type == "postgresql":
        from psycopg.rows import dict_row
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
        query_get_max_pos = (
            'SELECT MAX(QueuePosition) AS max_pos FROM "EpisodeQueue" '
            'WHERE UserID = %s'
        )
    else:
        cursor = cnx.cursor(dictionary=True)
        query_get_max_pos = (
            "SELECT MAX(QueuePosition) AS max_pos FROM EpisodeQueue "
            "WHERE UserID = %s"
        )

    cursor.execute(query_get_max_pos, (user_id,))
    result = cursor.fetchone()
    max_pos = result['max_pos'] if result['max_pos'] else 0

    # Insert the new item into the queue
    query_queue_pod = (
        'INSERT INTO "EpisodeQueue"(UserID, EpisodeID, QueuePosition, is_youtube) '
        'VALUES (%s, %s, %s, %s)' if database_type == "postgresql" else
        "INSERT INTO EpisodeQueue(UserID, EpisodeID, QueuePosition, is_youtube) "
        "VALUES (%s, %s, %s, %s)"
    )

    new_pos = max_pos + 1
    try:
        start = time.time()
        cursor.execute(query_queue_pod, (user_id, episode_id, new_pos, is_youtube))
        cnx.commit()
        end = time.time()
        print(f"Query executed in {end - start} seconds.")
    except Exception as e:
        print(f"Error queueing {'video' if is_youtube else 'episode'}:", e)
        return None
    return f"{'Video' if is_youtube else 'Episode'} queued successfully."

def reorder_queued_episodes(database_type, cnx, user_id, episode_ids):
    if database_type == "postgresql":
        from psycopg.rows import dict_row
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
        query_update_position = (
            'UPDATE "EpisodeQueue" SET QueuePosition = %s '
            'WHERE UserID = %s AND EpisodeID = %s'
        )
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(dictionary=True)
        query_update_position = (
            "UPDATE EpisodeQueue SET QueuePosition = %s "
            "WHERE UserID = %s AND EpisodeID = %s"
        )

    try:
        start = time.time()

        # Update the position of each episode in the order they appear in the list
        for position, episode_id in enumerate(episode_ids, start=1):
            cursor.execute(query_update_position, (position, user_id, episode_id))

        cnx.commit()  # Commit the changes
        end = time.time()
        print(f"Query executed in {end - start} seconds.")
        return True
    except Exception as e:
        print("Error reordering Podcast Episodes:", e)
        return False



def check_queued(database_type, cnx, episode_id, user_id, is_youtube=False):
    if database_type == "postgresql":
        from psycopg.rows import dict_row
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
        query = """
        SELECT * FROM "EpisodeQueue"
        WHERE EpisodeID = %s AND UserID = %s AND is_youtube = %s
        """
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(dictionary=True)
        query = """
        SELECT * FROM EpisodeQueue
        WHERE EpisodeID = %s AND UserID = %s AND is_youtube = %s
        """
    cursor.execute(query, (episode_id, user_id, is_youtube))
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


def remove_queued_pod(database_type, cnx, episode_id, user_id, is_youtube=False):
    print(f'ep id: {episode_id}')
    if database_type == "postgresql":
        from psycopg.rows import dict_row
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
        if is_youtube:
            get_queue_data_query = """
            SELECT "EpisodeQueue".EpisodeID, "EpisodeQueue".QueuePosition
            FROM "EpisodeQueue"
            INNER JOIN "YouTubeVideos" ON "EpisodeQueue".EpisodeID = "YouTubeVideos".VideoID
            WHERE "YouTubeVideos".VideoID = %s AND "EpisodeQueue".UserID = %s AND "EpisodeQueue".is_youtube = TRUE
            """
        else:
            get_queue_data_query = """
            SELECT "EpisodeQueue".EpisodeID, "EpisodeQueue".QueuePosition
            FROM "EpisodeQueue"
            INNER JOIN "Episodes" ON "EpisodeQueue".EpisodeID = "Episodes".EpisodeID
            WHERE "Episodes".EpisodeID = %s AND "EpisodeQueue".UserID = %s AND "EpisodeQueue".is_youtube = FALSE
            """
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(dictionary=True)
        if is_youtube:
            get_queue_data_query = """
            SELECT EpisodeQueue.EpisodeID, EpisodeQueue.QueuePosition
            FROM EpisodeQueue
            INNER JOIN YouTubeVideos ON EpisodeQueue.EpisodeID = YouTubeVideos.VideoID
            WHERE YouTubeVideos.VideoID = %s AND EpisodeQueue.UserID = %s AND EpisodeQueue.is_youtube = TRUE
            """
        else:
            get_queue_data_query = """
            SELECT EpisodeQueue.EpisodeID, EpisodeQueue.QueuePosition
            FROM EpisodeQueue
            INNER JOIN Episodes ON EpisodeQueue.EpisodeID = Episodes.EpisodeID
            WHERE Episodes.EpisodeID = %s AND EpisodeQueue.UserID = %s AND EpisodeQueue.is_youtube = FALSE
            """

    cursor.execute(get_queue_data_query, (episode_id, user_id))
    queue_data = cursor.fetchone()
    print(f"Queue data: {queue_data}")

    if queue_data is None:
        print(f"No queued {'video' if is_youtube else 'episode'} found with ID {episode_id}")
        cursor.close()
        return None

    removed_queue_position = queue_data['queueposition'] if database_type == "postgresql" else queue_data['QueuePosition']
    print(f'delete on the way')

    delete_query = (
        'DELETE FROM "EpisodeQueue" WHERE UserID = %s AND EpisodeID = %s AND is_youtube = %s' if database_type == "postgresql" else
        "DELETE FROM EpisodeQueue WHERE UserID = %s AND EpisodeID = %s AND is_youtube = %s"
    )
    cursor.execute(delete_query, (user_id, episode_id, is_youtube))
    affected_rows = cursor.rowcount
    print(f'Rows affected by delete: {affected_rows}')

    if affected_rows == 0:
        print(f"No rows were deleted. UserID: {user_id}, {'VideoID' if is_youtube else 'EpisodeID'}: {episode_id}")
        return {"status": "error", "message": "No matching row found for deletion"}

    print(f'{"video" if is_youtube else "episode"} deleted')
    cnx.commit()

    update_queue_query = (
        'UPDATE "EpisodeQueue" SET QueuePosition = QueuePosition - 1 WHERE UserID = %s AND QueuePosition > %s AND is_youtube = %s' if database_type == "postgresql" else
        "UPDATE EpisodeQueue SET QueuePosition = QueuePosition - 1 WHERE UserID = %s AND QueuePosition > %s AND is_youtube = %s"
    )
    cursor.execute(update_queue_query, (user_id, removed_queue_position, is_youtube))
    cnx.commit()
    print(f"Successfully removed {'video' if is_youtube else 'episode'} from queue.")
    cursor.close()
    return {"status": "success"}



def get_queued_episodes(database_type, cnx, user_id):
    if database_type == "postgresql":
        from psycopg.rows import dict_row
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
        get_queued_episodes_query = """
        SELECT * FROM (
            SELECT
                "Episodes".EpisodeTitle as episodetitle,
                "Podcasts".PodcastName as podcastname,
                "Episodes".EpisodePubDate as episodepubdate,
                "Episodes".EpisodeDescription as episodedescription,
                "Episodes".EpisodeArtwork as episodeartwork,
                "Episodes".EpisodeURL as episodeurl,
                "EpisodeQueue".QueuePosition as queueposition,
                "Episodes".EpisodeDuration as episodeduration,
                "EpisodeQueue".QueueDate as queuedate,
                "UserEpisodeHistory".ListenDuration as listenduration,
                "Episodes".EpisodeID as episodeid,
                "Episodes".Completed as completed,
                FALSE as is_youtube
            FROM "EpisodeQueue"
            INNER JOIN "Episodes" ON "EpisodeQueue".EpisodeID = "Episodes".EpisodeID
            INNER JOIN "Podcasts" ON "Episodes".PodcastID = "Podcasts".PodcastID
            LEFT JOIN "UserEpisodeHistory" ON
                "EpisodeQueue".EpisodeID = "UserEpisodeHistory".EpisodeID
                AND "EpisodeQueue".UserID = "UserEpisodeHistory".UserID
            WHERE "EpisodeQueue".UserID = %s
            AND "EpisodeQueue".is_youtube = FALSE

            UNION ALL

            SELECT
                "YouTubeVideos".VideoTitle as episodetitle,
                "Podcasts".PodcastName as podcastname,
                "YouTubeVideos".PublishedAt as episodepubdate,
                "YouTubeVideos".VideoDescription as episodedescription,
                "YouTubeVideos".ThumbnailURL as episodeartwork,
                "YouTubeVideos".VideoURL as episodeurl,
                "EpisodeQueue".QueuePosition as queueposition,
                "YouTubeVideos".Duration as episodeduration,
                "EpisodeQueue".QueueDate as queuedate,
                "YouTubeVideos".ListenPosition as listenduration,
                "YouTubeVideos".VideoID as episodeid,
                "YouTubeVideos".Completed as completed,
                TRUE as is_youtube
            FROM "EpisodeQueue"
            INNER JOIN "YouTubeVideos" ON "EpisodeQueue".EpisodeID = "YouTubeVideos".VideoID
            INNER JOIN "Podcasts" ON "YouTubeVideos".PodcastID = "Podcasts".PodcastID
            WHERE "EpisodeQueue".UserID = %s
            AND "EpisodeQueue".is_youtube = TRUE
        ) combined
        ORDER BY queueposition ASC
        """
    else:  # MySQL or MariaDB
        cursor = cnx.cursor(dictionary=True)
        get_queued_episodes_query = """
        SELECT * FROM (
            SELECT
                Episodes.EpisodeTitle as episodetitle,
                Podcasts.PodcastName as podcastname,
                Episodes.EpisodePubDate as episodepubdate,
                Episodes.EpisodeDescription as episodedescription,
                Episodes.EpisodeArtwork as episodeartwork,
                Episodes.EpisodeURL as episodeurl,
                EpisodeQueue.QueuePosition as queueposition,
                Episodes.EpisodeDuration as episodeduration,
                EpisodeQueue.QueueDate as queuedate,
                UserEpisodeHistory.ListenDuration as listenduration,
                Episodes.EpisodeID as episodeid,
                Episodes.Completed as completed,
                0 as is_youtube
            FROM EpisodeQueue
            INNER JOIN Episodes ON EpisodeQueue.EpisodeID = Episodes.EpisodeID
            INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID
            LEFT JOIN UserEpisodeHistory ON
                EpisodeQueue.EpisodeID = UserEpisodeHistory.EpisodeID
                AND EpisodeQueue.UserID = UserEpisodeHistory.UserID
            WHERE EpisodeQueue.UserID = %s
            AND EpisodeQueue.is_youtube = FALSE

            UNION ALL

            SELECT
                YouTubeVideos.VideoTitle as episodetitle,
                Podcasts.PodcastName as podcastname,
                YouTubeVideos.PublishedAt as episodepubdate,
                YouTubeVideos.VideoDescription as episodedescription,
                YouTubeVideos.ThumbnailURL as episodeartwork,
                YouTubeVideos.VideoURL as episodeurl,
                EpisodeQueue.QueuePosition as queueposition,
                YouTubeVideos.Duration as episodeduration,
                EpisodeQueue.QueueDate as queuedate,
                YouTubeVideos.ListenPosition as listenduration,
                YouTubeVideos.VideoID as episodeid,
                YouTubeVideos.Completed as completed,
                1 as is_youtube
            FROM EpisodeQueue
            INNER JOIN YouTubeVideos ON EpisodeQueue.EpisodeID = YouTubeVideos.VideoID
            INNER JOIN Podcasts ON YouTubeVideos.PodcastID = Podcasts.PodcastID
            WHERE EpisodeQueue.UserID = %s
            AND EpisodeQueue.is_youtube = TRUE
        ) combined
        ORDER BY queueposition ASC
        """

    cursor.execute(get_queued_episodes_query, (user_id, user_id))
    queued_episodes = cursor.fetchall()
    cursor.close()
    queued_episodes = lowercase_keys(queued_episodes)
    if database_type != "postgresql":
        for episode in queued_episodes:
            episode['completed'] = bool(episode['completed'])
            episode['is_youtube'] = bool(episode['is_youtube'])
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


def add_shared_episode(database_type, cnx, episode_id, url_key, expiration_date):
    cursor = cnx.cursor()

    if database_type == "postgresql":
        query = '''
            INSERT INTO "SharedEpisodes" (EpisodeID, UrlKey, ExpirationDate)
            VALUES (%s, %s, %s)
        '''
    else:  # MySQL/MariaDB version
        query = '''
            INSERT INTO SharedEpisodes (EpisodeID, UrlKey, ExpirationDate)
            VALUES (%s, %s, %s)
        '''

    try:
        cursor.execute(query, (episode_id, url_key, expiration_date))
        cnx.commit()  # Commit the changes
        cursor.close()
        return True
    except Exception as e:
        print(f"Error sharing episode: {e}")
        cursor.close()
        return False

def cleanup_old_episodes(cnx, database_type):
    """
    Master cleanup function that handles both PeopleEpisodes and SharedEpisodes tables
    """
    cleanup_old_people_episodes(cnx, database_type)
    cleanup_expired_shared_episodes(cnx, database_type)

def cleanup_old_people_episodes(cnx, database_type, days=30):
    """
    Remove episodes from PeopleEpisodes that are older than the specified number of days
    """
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            delete_query = """
                DELETE FROM "PeopleEpisodes"
                WHERE AddedDate < CURRENT_TIMESTAMP - INTERVAL '%s days'
            """
        else:  # MySQL or MariaDB
            delete_query = """
                DELETE FROM PeopleEpisodes
                WHERE AddedDate < DATE_SUB(NOW(), INTERVAL %s DAY)
            """

        cursor.execute(delete_query, (days,))
        deleted_count = cursor.rowcount
        print(f"Cleaned up {deleted_count} episodes older than {days} days from PeopleEpisodes")
        cnx.commit()

    except Exception as e:
        print(f"Error during PeopleEpisodes cleanup: {str(e)}")
        cnx.rollback()
    finally:
        cursor.close()

def cleanup_expired_shared_episodes(cnx, database_type):
    """
    Remove expired episodes from SharedEpisodes based on ExpirationDate
    """
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            delete_query = """
                DELETE FROM "SharedEpisodes"
                WHERE ExpirationDate < CURRENT_TIMESTAMP
            """
        else:  # MySQL or MariaDB
            delete_query = """
                DELETE FROM SharedEpisodes
                WHERE ExpirationDate < NOW()
            """

        cursor.execute(delete_query)
        deleted_count = cursor.rowcount
        print(f"Cleaned up {deleted_count} expired episodes from SharedEpisodes")
        cnx.commit()

    except Exception as e:
        print(f"Error during SharedEpisodes cleanup: {str(e)}")
        cnx.rollback()
    finally:
        cursor.close()

def get_episode_id_by_url_key(database_type, cnx, url_key):
    cursor = cnx.cursor()

    query = '''
        SELECT EpisodeID FROM "SharedEpisodes" WHERE UrlKey = %s AND ExpirationDate > NOW()
    ''' if database_type == "postgresql" else '''
        SELECT EpisodeID FROM SharedEpisodes WHERE UrlKey = %s AND ExpirationDate > NOW()
    '''

    try:
        cursor.execute(query, (url_key,))
        result = cursor.fetchone()

        # Debug: print the result type and value
        print(f"Result: {result}, Type: {type(result)}")

        if result:
            # Safely handle result as either tuple or dict
            if isinstance(result, tuple):
                print('tuple')
                episode_id = result[0]  # Access tuple
            elif isinstance(result, dict):
                print('dict')
                if database_type == 'postgresql':
                    episode_id = result['episodeid']  # Access dict
                else:
                    episode_id = result['EpisodeID']  # Access dict
            else:
                episode_id = None  # If somehow it's neither, default to None
        else:
            episode_id = None
        print(episode_id)
        cursor.close()
        return episode_id
    except Exception as e:
        print(f"Error retrieving episode by URL key: {e}")
        cursor.close()
        return None



def add_gpodder_settings(database_type, cnx, user_id, gpodder_url, gpodder_token, login_name, pod_sync_type):
    print("Adding gPodder settings")
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
        'UPDATE "Users" SET GpodderUrl = %s, GpodderLoginName = %s, GpodderToken = %s, Pod_Sync_Type = %s WHERE UserID = %s' if database_type == "postgresql" else
        "UPDATE Users SET GpodderUrl = %s, GpodderLoginName = %s, GpodderToken = %s, Pod_Sync_Type = %s WHERE UserID = %s"
    )

    cursor.execute(query, (gpodder_url, login_name, decoded_token, pod_sync_type, user_id))

    # Check if the update was successful
    if cursor.rowcount == 0:
        return None

    cnx.commit()  # Commit changes to the database
    cursor.close()

    return True

def add_gpodder_server(database_type, cnx, user_id, gpodder_url, gpodder_username, gpodder_password):
    print("Adding gPodder settings")
    the_key = get_encryption_key(cnx, database_type)

    cursor = cnx.cursor()
    from cryptography.fernet import Fernet

    encryption_key_bytes = base64.b64decode(the_key)

    cipher_suite = Fernet(encryption_key_bytes)

    # Only encrypt password if it's not None
    if gpodder_password is not None:
        encrypted_password = cipher_suite.encrypt(gpodder_password.encode())
        # Decode encrypted password back to string
        decoded_token = encrypted_password.decode()
    else:
        decoded_token = None

    query = (
        'UPDATE "Users" SET GpodderUrl = %s, GpodderLoginName = %s, GpodderToken = %s, Pod_Sync_Type = %s WHERE UserID = %s' if database_type == "postgresql" else
        "UPDATE Users SET GpodderUrl = %s, GpodderLoginName = %s, GpodderToken = %s, Pod_Sync_Type = %s WHERE UserID = %s"
    )
    pod_sync_type = "gpodder"
    cursor.execute(query, (gpodder_url, gpodder_username, decoded_token, pod_sync_type, user_id))

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
    try:
        query = (
            'SELECT GpodderUrl, GpodderToken, GpodderLoginName FROM "Users" WHERE UserID = %s' if database_type == "postgresql" else
            "SELECT GpodderUrl, GpodderToken, GpodderLoginName FROM Users WHERE UserID = %s"
        )
        cursor.execute(query, (user_id,))
        result = cursor.fetchone()

        if result:
            if isinstance(result, dict):
                # Handle PostgreSQL dictionary result
                url = result.get('gpodderurl')
                token = result.get('gpoddertoken')
                login = result.get('gpodderloginname')
            else:
                # Handle tuple result
                url, token, login = result[0], result[1], result[2]

            if url and token and login:
                return url, token, login

        return None, None, None
    finally:
        cursor.close()

def get_gpodder_type(cnx, database_type, user_id):
    cursor = cnx.cursor()
    query = (
        'SELECT Pod_Sync_Type FROM "Users" WHERE UserID = %s' if database_type == "postgresql" else
        "SELECT Pod_Sync_Type FROM Users WHERE UserID = %s"
    )
    cursor.execute(query, (user_id,))
    result = cursor.fetchone()
    cursor.close()

    if result:
        if isinstance(result, dict):
            return result.get('pod_sync_type' if database_type == 'postgresql' else 'Pod_Sync_Type')
        elif isinstance(result, (list, tuple)):
            return result[0]
    return None




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
        if isinstance(result, dict):
            gpodder_url = result.get('gpodderurl' if database_type == 'postgresql' else 'GpodderUrl')
            gpodder_token = result.get('gpoddertoken' if database_type == 'postgresql' else 'GpodderToken')
        elif isinstance(result, (list, tuple)):
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

def add_podcast_to_opodsync(cnx, database_type, gpodder_url, gpodder_login, encrypted_gpodder_token, podcast_url, device_id="default"):
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

    # Adjust the URL for oPodSync
    url = f"{gpodder_url}/api/2/subscriptions/{gpodder_login}/{device_id}.json"
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
        print(f"Podcast added to oPodSync successfully: {response.text}")
    except requests.exceptions.HTTPError as e:
        print(f"Failed to add podcast to oPodSync: {e}")
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


def remove_podcast_from_opodsync(cnx, database_type, gpodder_url, gpodder_login, encrypted_gpodder_token, podcast_url, device_id="default"):
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

    # Adjust the URL for oPodSync
    url = f"{gpodder_url}/api/2/subscriptions/{gpodder_login}/{device_id}.json"
    auth = HTTPBasicAuth(gpodder_login, gpodder_token)  # Using Basic Auth
    data = {
        "add": [],
        "remove": [podcast_url]
    }
    headers = {
        "Content-Type": "application/json"
    }
    response = requests.post(url, json=data, headers=headers, auth=auth)
    try:
        response.raise_for_status()
        print(f"Podcast removed from oPodSync successfully: {response.text}")
    except requests.exceptions.HTTPError as e:
        print(f"Failed to remove podcast from oPodSync: {e}")
        print(f"Response body: {response.text}")

def refresh_nextcloud_subscription(database_type, cnx, user_id, gpodder_url, encrypted_gpodder_token, gpodder_login, pod_sync_type):
    # Set up logging
    logging.basicConfig(level=logging.INFO)
    logger = logging.getLogger(__name__)

    try:
        # Fetch and decrypt token
        encryption_key = get_encryption_key(cnx, database_type)
        encryption_key_bytes = base64.b64decode(encryption_key)
        cipher_suite = Fernet(encryption_key_bytes)

        if encrypted_gpodder_token is not None:
            decrypted_token_bytes = cipher_suite.decrypt(encrypted_gpodder_token.encode())
            gpodder_token = decrypted_token_bytes.decode()
        else:
            gpodder_token = None

        auth = HTTPBasicAuth(gpodder_login, gpodder_token)
        logger.info("Starting Nextcloud subscription refresh")

        # Get Nextcloud subscriptions
        response = requests.get(
            f"{gpodder_url}/index.php/apps/gpoddersync/subscriptions",
            auth=auth
        )
        response.raise_for_status()

        nextcloud_podcasts = response.json().get("add", [])
        logger.info(f"Fetched Nextcloud podcasts: {nextcloud_podcasts}")

        # Get local podcasts
        cursor = cnx.cursor()
        if database_type == "postgresql":
            query = 'SELECT FeedURL FROM "Podcasts" WHERE UserID = %s'
        else:
            query = "SELECT FeedURL FROM Podcasts WHERE UserID = %s"

        cursor.execute(query, (user_id,))
        local_podcasts = [row[0] for row in cursor.fetchall()]

        podcasts_to_add = set(nextcloud_podcasts) - set(local_podcasts)
        podcasts_to_remove = set(local_podcasts) - set(nextcloud_podcasts)

        # Track successful operations
        successful_additions = set()
        successful_removals = set()

        # Add new podcasts with individual error handling
        logger.info("Adding new podcasts...")
        for feed_url in podcasts_to_add:
            try:
                podcast_values = get_podcast_values(feed_url, user_id)
                return_value = add_podcast(cnx, database_type, podcast_values, user_id)
                if return_value:
                    logger.info(f"Successfully added {feed_url}")
                    successful_additions.add(feed_url)
                else:
                    logger.error(f"Failed to add {feed_url}")
            except Exception as e:
                logger.error(f"Error processing {feed_url}: {str(e)}")
                continue  # Continue with next podcast even if this one fails

        # Remove podcasts with individual error handling
        logger.info("Removing podcasts...")
        for feed_url in podcasts_to_remove:
            try:
                if database_type == "postgresql":
                    query = 'SELECT PodcastName FROM "Podcasts" WHERE FeedURL = %s'
                else:
                    query = "SELECT PodcastName FROM Podcasts WHERE FeedURL = %s"

                cursor.execute(query, (feed_url,))
                result = cursor.fetchone()

                if result:
                    podcast_name = result[0]
                    if remove_podcast(cnx, database_type, podcast_name, feed_url, user_id):
                        successful_removals.add(feed_url)
                        logger.info(f"Successfully removed {feed_url}")
                    else:
                        logger.error(f"Failed to remove {feed_url}")
                else:
                    logger.warning(f"No podcast found with URL: {feed_url}")
            except Exception as e:
                logger.error(f"Error removing {feed_url}: {str(e)}")
                continue

        cnx.commit()
        cursor.close()

        # Sync changes with Nextcloud
        if successful_additions or successful_removals:
            try:
                sync_subscription_change(
                    gpodder_url,
                    {"Authorization": f"Bearer {gpodder_token}"},
                    list(successful_additions),
                    list(successful_removals)
                )
            except Exception as e:
                logger.error(f"Error syncing changes with Nextcloud: {str(e)}")

        # Process episode actions
        try:
            process_nextcloud_episode_actions(gpodder_url, gpodder_token, cnx, database_type, user_id)
        except Exception as e:
            logger.error(f"Error processing episode actions: {str(e)}")

        # Sync local episode times
        try:
            sync_nextcloud_episode_times(gpodder_url, gpodder_login, gpodder_token, cnx, database_type, user_id)
        except Exception as e:
            logger.error(f"Error syncing local episode times: {str(e)}")

    except Exception as e:
        logger.error(f"Major error in refresh_nextcloud_subscription: {str(e)}")
        raise

def process_nextcloud_episode_actions(gpodder_url, gpodder_token, cnx, database_type, user_id):
    logger = logging.getLogger(__name__)

    try:
        # Use the correct Nextcloud endpoint
        response = requests.get(
            f"{gpodder_url}/index.php/apps/gpoddersync/episode_action",
            headers={"Authorization": f"Bearer {gpodder_token}"}
        )
        response.raise_for_status()
        episode_actions = response.json()

        cursor = cnx.cursor()

        for action in episode_actions.get('actions', []):
            try:
                if action["action"].lower() in ["play", "update_time"]:
                    if "position" in action and action["position"] != -1:
                        episode_id = get_episode_id_by_url(cnx, database_type, action["episode"])
                        if episode_id:
                            # Update listen duration
                            record_listen_duration(cnx, database_type, episode_id, user_id, int(action["position"]))

                            # Check for completion, mirroring gPodder logic
                            if ("total" in action and action["total"] > 0 and
                                action["position"] >= action["total"]):
                                if database_type == "postgresql":
                                    update_query = '''
                                        UPDATE "Episodes"
                                        SET Completed = TRUE
                                        WHERE EpisodeID = %s
                                    '''
                                else:
                                    update_query = '''
                                        UPDATE Episodes
                                        SET Completed = TRUE
                                        WHERE EpisodeID = %s
                                    '''
                                cursor.execute(update_query, (episode_id,))
                                cnx.commit()
                                logger.info(f"Marked episode {episode_id} as completed")

                            logger.info(f"Recorded listen duration for episode {episode_id}")
                        else:
                            logger.warning(f"No episode ID found for URL {action['episode']}")
            except Exception as e:
                logger.error(f"Error processing episode action {action}: {str(e)}")
                continue

        cursor.close()
    except Exception as e:
        logger.error(f"Error fetching episode actions: {str(e)}")
        raise

def sync_nextcloud_episode_times(gpodder_url, gpodder_login, gpodder_token, cnx, database_type, user_id, UPLOAD_BULK_SIZE=30):
    logger = logging.getLogger(__name__)

    try:
        local_episode_times = get_local_episode_times(cnx, database_type, user_id)
        update_actions = []

        for episode_time in local_episode_times:
            # Only include episodes with valid duration data
            if episode_time["episode_duration"] and episode_time["listen_duration"]:
                # If episode is completed, set position equal to total duration
                position = (episode_time["episode_duration"]
                          if episode_time["completed"]
                          else episode_time["listen_duration"])

                action = {
                    "podcast": episode_time["podcast_url"],
                    "episode": episode_time["episode_url"],
                    "action": "play",
                    "timestamp": current_timestamp(),
                    "position": position,
                    "started": 0,
                    "total": episode_time["episode_duration"],
                    "guid": generate_guid(episode_time)
                }
                update_actions.append(action)

        # Split into chunks and process
        update_actions_chunks = [
            update_actions[i:i + UPLOAD_BULK_SIZE]
            for i in range(0, len(update_actions), UPLOAD_BULK_SIZE)
        ]

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
                response.raise_for_status()
                logger.info(f"Successfully uploaded chunk of {len(chunk)} episode times")
            except Exception as e:
                logger.error(f"Error uploading chunk: {str(e)}")
                continue

    except Exception as e:
        logger.error(f"Error syncing local episode times: {str(e)}")
        raise

def refresh_gpodder_subscription(database_type, cnx, user_id, gpodder_url, encrypted_gpodder_token, gpodder_login, pod_sync_type):
    from cryptography.fernet import Fernet
    from requests.auth import HTTPBasicAuth
    import logging

    # Set up logging
    logging.basicConfig(level=logging.INFO)
    logger = logging.getLogger(__name__)

    try:
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

        # Get gPodder subscriptions
        response = requests.get(f"{gpodder_url}/api/2/subscriptions/{gpodder_login}/default.json", auth=auth)
        response.raise_for_status()

        gpodder_data = response.json()
        gpodder_podcasts_add = set(gpodder_data.get("add", []))
        gpodder_podcasts_remove = set(gpodder_data.get("remove", []))

        logger.info(f"gPodder podcasts to add: {gpodder_podcasts_add}")
        logger.info(f"gPodder podcasts to remove: {gpodder_podcasts_remove}")

        # Get local podcasts
        cursor = cnx.cursor()
        if database_type == "postgresql":
            query = 'SELECT FeedURL FROM "Podcasts" WHERE UserID = %s'
        else:
            query = "SELECT FeedURL FROM Podcasts WHERE UserID = %s"

        cursor.execute(query, (user_id,))
        local_podcasts = set()
        for row in cursor.fetchall():
            if isinstance(row, dict):
                local_podcasts.add(row["feedurl"])  # PostgreSQL dict case
            else:
                local_podcasts.add(row[0])  # Tuple case

        podcasts_to_add = gpodder_podcasts_add - local_podcasts
        podcasts_to_remove = gpodder_podcasts_remove & local_podcasts

        # Track successful additions and removals for sync
        successful_additions = set()
        successful_removals = set()

        # Add new podcasts with individual error handling
        logger.info("Adding new podcasts...")
        for feed_url in podcasts_to_add:
            try:
                podcast_values = get_podcast_values(feed_url, user_id)
                print(f"Debug - Explicit value: {podcast_values['pod_explicit']}")
                print(f"Debug - Episode count: {podcast_values['pod_episode_count']}")
                print(f"Debug - Full podcast values: {podcast_values}")
                return_value = add_podcast(cnx, database_type, podcast_values, user_id)
                if return_value:
                    logger.info(f"Successfully added {feed_url}")
                    successful_additions.add(feed_url)
                else:
                    logger.error(f"Failed to add {feed_url}")
            except Exception as e:
                logger.error(f"Error processing {feed_url}: {str(e)}")
                continue  # Continue with next podcast even if this one fails

        # Remove podcasts with individual error handling
        logger.info("Removing podcasts...")
        for feed_url in podcasts_to_remove:
            try:
                if database_type == "postgresql":
                    query = 'SELECT PodcastName FROM "Podcasts" WHERE FeedURL = %s'
                else:
                    query = "SELECT PodcastName FROM Podcasts WHERE FeedURL = %s"

                cursor.execute(query, (feed_url,))
                result = cursor.fetchone()

                if result:
                    podcast_name = result[0]
                    if remove_podcast(cnx, database_type, podcast_name, feed_url, user_id):
                        successful_removals.add(feed_url)
                        logger.info(f"Successfully removed {feed_url}")
                    else:
                        logger.error(f"Failed to remove {feed_url}")
                else:
                    logger.warning(f"No podcast found with URL: {feed_url}")
            except Exception as e:
                logger.error(f"Error removing {feed_url}: {str(e)}")
                continue

        cnx.commit()
        cursor.close()

        # Only sync successfully processed changes
        if successful_additions or successful_removals:
            try:
                sync_subscription_change_gpodder(
                    gpodder_url,
                    gpodder_login,
                    auth,
                    list(successful_additions),
                    list(successful_removals)
                )
            except Exception as e:
                logger.error(f"Error syncing changes with gPodder: {str(e)}")

        # Process episode actions
        try:
            process_episode_actions(gpodder_url, gpodder_login, auth, cnx, database_type, user_id)
        except Exception as e:
            logger.error(f"Error processing episode actions: {str(e)}")

        # Sync local episode times
        try:
            sync_local_episode_times(gpodder_url, gpodder_login, auth, cnx, database_type, user_id)
        except Exception as e:
            logger.error(f"Error syncing local episode times: {str(e)}")

    except Exception as e:
        logger.error(f"Major error in refresh_gpodder_subscription: {str(e)}")
        raise

def sync_local_episode_times(gpodder_url, gpodder_login, auth, cnx, database_type, user_id, UPLOAD_BULK_SIZE=30):
    logger = logging.getLogger(__name__)

    try:
        local_episode_times = get_local_episode_times(cnx, database_type, user_id)
        update_actions = []

        for episode_time in local_episode_times:
            # Only include episodes with valid duration data
            if episode_time["episode_duration"] and episode_time["listen_duration"]:
                # If episode is completed, set position to total duration
                position = (episode_time["episode_duration"]
                          if episode_time["completed"]
                          else episode_time["listen_duration"])

                action = {
                    "podcast": episode_time["podcast_url"],
                    "episode": episode_time["episode_url"],
                    "action": "play",
                    "timestamp": current_timestamp(),
                    "position": position,
                    "started": 0,
                    "total": episode_time["episode_duration"]
                }

                # Add guid if available
                if episode_time.get("guid"):
                    action["guid"] = episode_time["guid"]

                update_actions.append(action)

        # Split into chunks and process
        update_actions_chunks = [
            update_actions[i:i + UPLOAD_BULK_SIZE]
            for i in range(0, len(update_actions), UPLOAD_BULK_SIZE)
        ]

        for chunk in update_actions_chunks:
            try:
                response = requests.post(
                    f"{gpodder_url}/api/2/episodes/{gpodder_login}.json",
                    json=chunk,
                    auth=auth,
                    headers={"Accept": "application/json"}
                )
                response.raise_for_status()
                logger.info(f"Successfully synced {len(chunk)} episode actions")
            except Exception as e:
                logger.error(f"Error uploading chunk: {str(e)}")
                continue

    except Exception as e:
        logger.error(f"Error syncing local episode times: {str(e)}")
        raise

def process_episode_actions(gpodder_url, gpodder_login, auth, cnx, database_type, user_id):
    """Process incoming episode actions from gPodder"""
    logger = logging.getLogger(__name__)

    try:
        episode_actions_response = requests.get(
            f"{gpodder_url}/api/2/episodes/{gpodder_login}.json",
            auth=auth
        )
        episode_actions_response.raise_for_status()
        episode_actions = episode_actions_response.json()

        cursor = cnx.cursor()

        for action in episode_actions.get('actions', []):
            try:
                if action["action"].lower() in ["play", "update_time"]:
                    if "position" in action and action["position"] != -1:
                        episode_id = get_episode_id_by_url(cnx, database_type, action["episode"])
                        if episode_id:
                            # Update listen duration
                            record_listen_duration(cnx, database_type, episode_id, user_id, int(action["position"]))

                            # Check for completion
                            if ("total" in action and action["total"] > 0 and
                                action["position"] >= action["total"]):
                                if database_type == "postgresql":
                                    update_query = '''
                                        UPDATE "Episodes"
                                        SET Completed = TRUE
                                        WHERE EpisodeID = %s
                                    '''
                                else:
                                    update_query = '''
                                        UPDATE Episodes
                                        SET Completed = TRUE
                                        WHERE EpisodeID = %s
                                    '''
                                cursor.execute(update_query, (episode_id,))
                                cnx.commit()
                                logger.info(f"Marked episode {episode_id} as completed")

            except Exception as e:
                logger.error(f"Error processing episode action {action}: {str(e)}")
                continue

        cursor.close()

    except Exception as e:
        logger.error(f"Error fetching episode actions: {str(e)}")
        raise


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

def subscribe_to_person(cnx, database_type, user_id: int, person_id: int, person_name: str, person_img: str, podcast_id: int) -> tuple[bool, int]:
    cursor = cnx.cursor()
    try:
        print(f"Starting subscribe_to_person with: user_id={user_id}, person_id={person_id}, person_name={person_name}, podcast_id={podcast_id}")

        if database_type == "postgresql":
            # Check if a person with the same PeopleDBID (if not 0) or Name (if PeopleDBID is 0) exists
            if person_id != 0:
                query = """
                    SELECT PersonID, AssociatedPodcasts FROM "People"
                    WHERE UserID = %s AND PeopleDBID = %s
                """
                print(f"Executing query for non-zero person_id: {query} with params: ({user_id}, {person_id})")
                cursor.execute(query, (user_id, person_id))
            else:
                query = """
                    SELECT PersonID, AssociatedPodcasts FROM "People"
                    WHERE UserID = %s AND Name = %s AND PeopleDBID = 0
                """
                print(f"Executing query for zero person_id: {query} with params: ({user_id}, {person_name})")
                cursor.execute(query, (user_id, person_name))

            existing_person = cursor.fetchone()
            print(f"Query result: {existing_person}")

            if existing_person:
                print("Found existing person, updating...")
                # Person exists, update AssociatedPodcasts and possibly update image/description
                person_id, associated_podcasts = existing_person
                podcast_list = associated_podcasts.split(',') if associated_podcasts else []
                if str(podcast_id) not in podcast_list:
                    podcast_list.append(str(podcast_id))
                    new_associated_podcasts = ','.join(podcast_list)
                    update_query = """
                        UPDATE "People"
                        SET AssociatedPodcasts = %s,
                            PersonImg = COALESCE(%s, PersonImg)
                        WHERE PersonID = %s
                    """
                    print(f"Executing update query: {update_query} with params: ({new_associated_podcasts}, {person_img}, {person_id})")
                    cursor.execute(update_query, (new_associated_podcasts, person_img, person_id))
                return True, person_id
            else:
                print("No existing person found, inserting new record...")
                # Person doesn't exist, insert new record with image and description
                insert_query = """
                    INSERT INTO "People"
                    (UserID, PeopleDBID, Name, PersonImg, AssociatedPodcasts)
                    VALUES (%s, %s, %s, %s, %s)
                    RETURNING PersonID;
                """
                print(f"Executing insert query: {insert_query} with params: ({user_id}, {person_id}, {person_name}, {person_img}, {str(podcast_id)})")
                cursor.execute(insert_query, (user_id, person_id, person_name, person_img, str(podcast_id)))
                result = cursor.fetchone()
                print(f"Insert result: {result}")
                if result is not None:
                    # Handle both tuple and dict return types
                    if isinstance(result, dict):
                        new_person_id = result['personid']
                    else:
                        new_person_id = result[0]
                    print(f"Insert successful, new PersonID: {new_person_id}")
                    cnx.commit()
                    return True, new_person_id
                else:
                    print("Insert did not return a PersonID")
                    cnx.rollback()
                    return False, 0

        else:  # MariaDB
            # Check if person exists
            if person_id != 0:
                query = """
                    SELECT PersonID, AssociatedPodcasts FROM People
                    WHERE UserID = %s AND PeopleDBID = %s
                """
                print(f"Executing query for non-zero person_id: {query} with params: ({user_id}, {person_id})")
                cursor.execute(query, (user_id, person_id))
            else:
                query = """
                    SELECT PersonID, AssociatedPodcasts FROM People
                    WHERE UserID = %s AND Name = %s AND PeopleDBID = 0
                """
                print(f"Executing query for zero person_id: {query} with params: ({user_id}, {person_name})")
                cursor.execute(query, (user_id, person_name))

            existing_person = cursor.fetchone()
            print(f"Query result: {existing_person}")

            if existing_person:
                print("Found existing person, updating...")
                # Person exists, update AssociatedPodcasts
                person_id = existing_person[0]  # MariaDB returns tuple
                associated_podcasts = existing_person[1]
                podcast_list = associated_podcasts.split(',') if associated_podcasts else []

                if str(podcast_id) not in podcast_list:
                    podcast_list.append(str(podcast_id))
                    new_associated_podcasts = ','.join(podcast_list)

                    update_query = """
                        UPDATE People
                        SET AssociatedPodcasts = %s,
                            PersonImg = COALESCE(%s, PersonImg)
                        WHERE PersonID = %s
                    """
                    print(f"Executing update query: {update_query} with params: ({new_associated_podcasts}, {person_img}, {person_id})")
                    cursor.execute(update_query, (new_associated_podcasts, person_img, person_id))
                    cnx.commit()
                return True, person_id
            else:
                print("No existing person found, inserting new record...")
                # Person doesn't exist, insert new record
                insert_query = """
                    INSERT INTO People
                    (UserID, PeopleDBID, Name, PersonImg, AssociatedPodcasts)
                    VALUES (%s, %s, %s, %s, %s)
                """
                print(f"Executing insert query: {insert_query} with params: ({user_id}, {person_id}, {person_name}, {person_img}, {str(podcast_id)})")
                cursor.execute(insert_query, (user_id, person_id, person_name, person_img, str(podcast_id)))
                cnx.commit()

                # Get the inserted ID
                new_person_id = cursor.lastrowid
                print(f"Insert successful, new PersonID: {new_person_id}")

                if new_person_id:
                    return True, new_person_id
                else:
                    print("Insert did not return a PersonID")
                    cnx.rollback()
                    return False, 0

    except Exception as e:
        print(f"Detailed error in subscribe_to_person: {str(e)}\nType: {type(e)}")
        import traceback
        print(f"Traceback: {traceback.format_exc()}")
        cnx.rollback()
        return False, 0
    finally:
        cursor.close()

    return False, 0  # In case we somehow get here

def unsubscribe_from_person(cnx, database_type, user_id: int, person_id: int, person_name: str) -> bool:
    cursor = cnx.cursor()
    try:
        print(f"Attempting to unsubscribe user {user_id} from person {person_name} (ID: {person_id})")
        if database_type == "postgresql":
            # Use PersonID instead of PeopleDBID for looking up the record to delete
            person_query = 'SELECT PersonID FROM "People" WHERE UserID = %s AND PersonID = %s'
            print(f"Searching for person with query: {person_query} and params: {user_id}, {person_id}")
            cursor.execute(person_query, (user_id, person_id))

        else:
            person_query = "SELECT PersonID FROM People WHERE UserID = %s AND PersonID = %s"
            cursor.execute(person_query, (user_id, person_id))

        result = cursor.fetchone()
        print(f"Query result: {result}")
        if not result:
            print(f"No person found for user {user_id} with ID {person_id}")
            return False

        # Handle both tuple and dict return types
        # Handle both tuple and dict return types
        if isinstance(result, dict):
            person_db_id = result['personid']
        else:
            person_db_id = result[0]
        print(f"Found PersonID: {person_db_id}")

        if database_type == "postgresql":
            check_query = 'SELECT COUNT(*) FROM "People" WHERE PersonID = %s'
            delete_query = 'DELETE FROM "People" WHERE PersonID = %s'
        else:
            check_query = "SELECT COUNT(*) FROM People WHERE PersonID = %s"
            delete_query = "DELETE FROM People WHERE PersonID = %s"

        # Check subscriber count for both database types
        cursor.execute(check_query, (person_id,))
        subscriber_count = cursor.fetchone()[0]

        # Only delete episodes if this is the last subscriber
        if subscriber_count <= 1:
            if database_type == "postgresql":
                episodes_query = 'DELETE FROM "PeopleEpisodes" WHERE PersonID = %s'
            else:
                episodes_query = "DELETE FROM PeopleEpisodes WHERE PersonID = %s"

            print(f"Deleting episodes for PersonID {person_db_id}")
            cursor.execute(episodes_query, (person_db_id,))
            episode_count = cursor.rowcount
            print(f"Deleted {episode_count} episodes")

        # Always delete the person record for this user
        print(f"Deleting person record for PersonID {person_db_id}")
        cursor.execute(delete_query, (person_db_id,))
        person_count = cursor.rowcount
        print(f"Deleted {person_count} person records")

        cnx.commit()
        return True

    except Exception as e:
        print(f"Error unsubscribing from person: {str(e)}")
        print(f"Error type: {type(e)}")
        if hasattr(e, '__cause__'):
            print(f"Cause: {e.__cause__}")
        cnx.rollback()
        return False
    finally:
        cursor.close()

def get_person_subscriptions(cnx, database_type, user_id: int) -> List[dict]:
    try:
        if database_type == "postgresql":
            cursor = cnx.cursor(row_factory=dict_row)
            query = 'SELECT * FROM "People" WHERE UserID = %s'
        else:  # MySQL or MariaDB
            cursor = cnx.cursor(dictionary=True)
            query = "SELECT * FROM People WHERE UserID = %s"

        cursor.execute(query, (user_id,))
        result = cursor.fetchall()
        print(f"Raw database result: {result}")

        formatted_result = []
        for row in result:
            if database_type == "postgresql":
                # PostgreSQL returns lowercase keys
                formatted_row = {
                    'personid': int(row['personid']),
                    'userid': int(row['userid']),
                    'name': row['name'],
                    'image': row['personimg'],
                    'peopledbid': int(row['peopledbid']) if row['peopledbid'] is not None else None,
                    'associatedpodcasts': row['associatedpodcasts'],
                }
            else:
                # MariaDB returns uppercase keys
                formatted_row = {
                    'personid': int(row['PersonID']),
                    'userid': int(row['UserID']),
                    'name': row['Name'],
                    'image': row['PersonImg'],
                    'peopledbid': int(row['PeopleDBID']) if row['PeopleDBID'] is not None else None,
                    'associatedpodcasts': row['AssociatedPodcasts'],
                }
            formatted_result.append(formatted_row)

        return formatted_result

    except Exception as e:
        print(f"Error getting person subscriptions: {e}")
        import traceback
        print(f"Traceback: {traceback.format_exc()}")
        return []
    finally:
        cursor.close()


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
    # Get database name from environment variable
    db_name = os.environ.get("DB_NAME", "pinepods_database")  # Default to pinepods_database if not set
    db_host = os.environ.get("DB_HOST", "db")
    db_port = os.environ.get("DB_PORT", "5432" if database_type == "postgresql" else "3306")
    db_user = os.environ.get("DB_USER", "postgres" if database_type == "postgresql" else "root")

    print(f'pass: {database_pass}')
    if database_type == "postgresql":
        os.environ['PGPASSWORD'] = database_pass
        cmd = [
            "pg_dump",
            "-h", db_host,
            "-p", db_port,
            "-U", db_user,
            "-d", db_name,
            "-w"
        ]
    else:  # Assuming MySQL or MariaDB
        cmd = [
            "mysqldump",
            "-h", db_host,
            "-P", db_port,
            "-u", db_user,
            "--password=" + database_pass,
            db_name
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


def restore_server(cnx, database_pass, file_content):
    import tempfile

    with tempfile.NamedTemporaryFile(mode='wb', delete=True) as tempf:
        tempf.write(file_content)
        tempf.flush()

        cmd = [
            "mysql",
            "-h", os.environ.get("DB_HOST", "db"),
            "-P", os.environ.get("DB_PORT", "3306"),
            "-u", os.environ.get("DB_USER", "root"),
            f"-p{database_pass}",
            os.environ.get("DB_NAME", "pinepods_database")
        ]

        process = subprocess.Popen(
            cmd,
            stdin=open(tempf.name, 'rb'),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE
        )

        stdout, stderr = process.communicate()
        if process.returncode != 0:
            raise Exception(f"Restoration failed with error: {stderr.decode()}")

    return "Restoration completed successfully!"


def get_video_date(video_id):
    logging.basicConfig(level=logging.INFO)
    logger = logging.getLogger(__name__)
    """Get upload date for a single video"""
    url = f"https://www.youtube.com/watch?v={video_id}"
    headers = {
        "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
    }

    try:
        # Add a small random delay to avoid rate limiting
        time.sleep(random.uniform(0.5, 1.5))

        response = requests.get(url, headers=headers)
        response.raise_for_status()

        # Look for uploadDate in page content
        date_pattern = r'"uploadDate":"([^"]+)"'
        date_match = re.search(date_pattern, response.text)

        if date_match:
            date_str = date_match.group(1)
            # Convert ISO format to datetime
            upload_date = datetime.datetime.fromisoformat(date_str.replace('Z', '+00:00'))
            return upload_date
        return None

    except Exception as e:
        logger.error(f"Error fetching date for video {video_id}: {e}")
        return None

def check_and_send_notification(cnx, database_type, podcast_id, episode_title):
    cursor = cnx.cursor()
    try:
        # First check if notifications are enabled for this podcast
        if database_type == "postgresql":
            query = """
                SELECT p.NotificationsEnabled, p.UserID, p.PodcastName,
                       uns.Platform, uns.Enabled, uns.NtfyTopic, uns.NtfyServerUrl,
                       uns.GotifyUrl, uns.GotifyToken
                FROM "Podcasts" p
                JOIN "UserNotificationSettings" uns ON p.UserID = uns.UserID
                WHERE p.PodcastID = %s AND p.NotificationsEnabled = true AND uns.Enabled = true
            """
        else:
            query = """
                SELECT p.NotificationsEnabled, p.UserID, p.PodcastName,
                       uns.Platform, uns.Enabled, uns.NtfyTopic, uns.NtfyServerUrl,
                       uns.GotifyUrl, uns.GotifyToken
                FROM Podcasts p
                JOIN UserNotificationSettings uns ON p.UserID = uns.UserID
                WHERE p.PodcastID = %s AND p.NotificationsEnabled = 1 AND uns.Enabled = 1
            """
        cursor.execute(query, (podcast_id,))
        results = cursor.fetchall()  # Get all enabled notification settings
        if not results:
            return False

        success = False  # Track if at least one notification was sent

        for result in results:
            try:
                if isinstance(result, dict):
                    platform = result['platform'] if 'platform' in result else result['Platform']
                    podcast_name = result['podcastname'] if 'podcastname' in result else result['PodcastName']

                    if platform == 'ntfy':
                        # Try both casings for each field
                        ntfy_topic = result.get('ntfytopic') or result.get('NtfyTopic')
                        ntfy_server = result.get('ntfyserverurl') or result.get('NtfyServerUrl')

                        if ntfy_topic and ntfy_server:
                            if send_ntfy_notification(
                                topic=ntfy_topic,
                                server_url=ntfy_server,
                                title=f"New Episode: {podcast_name}",
                                message=f"New episode published: {episode_title}"
                            ):
                                success = True

                    elif platform == 'gotify':
                        gotify_url = result.get('gotifyurl') or result.get('GotifyUrl')
                        gotify_token = result.get('gotifytoken') or result.get('GotifyToken')

                        if gotify_url and gotify_token:
                            if send_gotify_notification(
                                server_url=gotify_url,
                                token=gotify_token,
                                title=f"New Episode: {podcast_name}",
                                message=f"New episode published: {episode_title}"
                            ):
                                success = True
                else:
                    platform = result[3]
                    podcast_name = result[2]
                    if platform == 'ntfy':
                        if send_ntfy_notification(
                            topic=result[5],
                            server_url=result[6],
                            title=f"New Episode: {podcast_name}",
                            message=f"New episode published: {episode_title}"
                        ):
                            success = True
                    elif platform == 'gotify':
                        if send_gotify_notification(
                            server_url=result[7],
                            token=result[8],
                            title=f"New Episode: {podcast_name}",
                            message=f"New episode published: {episode_title}"
                        ):
                            success = True
            except Exception as e:
                logging.error(f"Error sending {platform} notification: {e}")
                # Continue trying other platforms even if one fails
                continue

        return success

    except Exception as e:
        logging.error(f"Error checking/sending notifications: {e}")
        return False
    finally:
        cursor.close()

def toggle_podcast_notifications(cnx, database_type, podcast_id, user_id, enabled):
    cursor = cnx.cursor()
    try:
        # First verify the user owns this podcast
        if database_type == "postgresql":
            check_query = """
                SELECT 1 FROM "Podcasts"
                WHERE PodcastID = %s AND UserID = %s
            """
        else:
            check_query = """
                SELECT 1 FROM Podcasts
                WHERE PodcastID = %s AND UserID = %s
            """

        cursor.execute(check_query, (podcast_id, user_id))
        if not cursor.fetchone():
            logging.warning(f"User {user_id} attempted to modify notifications for podcast {podcast_id} they don't own")
            return False

        # Update the notification setting
        if database_type == "postgresql":
            update_query = """
                UPDATE "Podcasts"
                SET NotificationsEnabled = %s
                WHERE PodcastID = %s AND UserID = %s
            """
        else:
            update_query = """
                UPDATE Podcasts
                SET NotificationsEnabled = %s
                WHERE PodcastID = %s AND UserID = %s
            """

        cursor.execute(update_query, (enabled, podcast_id, user_id))
        cnx.commit()
        return True

    except Exception as e:
        logging.error(f"Error toggling podcast notifications: {e}")
        cnx.rollback()
        return False
    finally:
        cursor.close()

def get_podcast_notification_status(cnx, database_type, podcast_id, user_id):
    cursor = cnx.cursor()
    try:
        # Query the notification status
        if database_type == "postgresql":
            query = """
                SELECT NotificationsEnabled
                FROM "Podcasts"
                WHERE PodcastID = %s AND UserID = %s
            """
        else:
            query = """
                SELECT NotificationsEnabled
                FROM Podcasts
                WHERE PodcastID = %s AND UserID = %s
            """
        cursor.execute(query, (podcast_id, user_id))
        result = cursor.fetchone()
        if result:
            if isinstance(result, dict):  # PostgreSQL with RealDictCursor
                # Try all possible case variations
                for key in ['NotificationsEnabled', 'notificationsenabled']:
                    if key in result:
                        return bool(result[key])
            else:  # MySQL or regular PostgreSQL cursor
                return bool(result[0])
        return False  # Default to False if no result found
    except Exception as e:
        logging.error(f"Error getting podcast notification status: {e}")
        logging.error(f"Result content: {result}")  # Add this for debugging
        return False
    finally:
        cursor.close()
