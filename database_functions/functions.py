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
import tempfile
import secrets

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
        # Handle both tuple and dictionary return types
        if isinstance(result, dict):
            return result['apikey']
        else:
            return result[0]
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
        else:  # MySQL or MariaDB
            # Get the last inserted ID for MySQL
            user_id = cursor.lastrowid
            if not user_id:
                raise Exception("Failed to create user - no ID returned from MySQL")
            print(f"MySQL generated user_id: {user_id}")

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

def add_oidc_provider(cnx, database_type, provider_values):
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            add_provider_query = """
                INSERT INTO "OIDCProviders"
                (ProviderName, ClientID, ClientSecret, AuthorizationURL,
                TokenURL, UserInfoURL, ButtonText, Scope,
                ButtonColor, ButtonTextColor, IconSVG)
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
                RETURNING ProviderID
            """
        else:  # MySQL
            add_provider_query = """
                INSERT INTO OIDCProviders
                (ProviderName, ClientID, ClientSecret, AuthorizationURL,
                TokenURL, UserInfoURL, ButtonText, Scope,
                ButtonColor, ButtonTextColor, IconSVG)
                VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
            """
        cursor.execute(add_provider_query, provider_values)

        if database_type == "postgresql":
            result = cursor.fetchone()
            if isinstance(result, dict):
                provider_id = result.get('providerid') or result.get('ProviderID') or result.get('provider_id')
            else:
                provider_id = result[0]
        else:
            provider_id = cursor.lastrowid

        cnx.commit()
        return provider_id
    except Exception as e:
        cnx.rollback()
        logging.error(f"Error in add_oidc_provider: {str(e)}")
        raise
    finally:
        cursor.close()

def remove_oidc_provider(cnx, database_type, provider_id):
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            delete_query = """
                DELETE FROM "OIDCProviders"
                WHERE ProviderID = %s
            """
        else:
            delete_query = """
                DELETE FROM OIDCProviders
                WHERE ProviderID = %s
            """
        cursor.execute(delete_query, (provider_id,))
        rows_affected = cursor.rowcount
        cnx.commit()
        return rows_affected > 0
    except Exception as e:
        cnx.rollback()
        logging.error(f"Error in remove_oidc_provider: {str(e)}")
        raise
    finally:
        cursor.close()

def list_oidc_providers(cnx, database_type):
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            list_query = """
                SELECT ProviderID, ProviderName, ClientID, AuthorizationURL,
                TokenURL, UserInfoURL, ButtonText,
                Scope, ButtonColor, ButtonTextColor, IconSVG, Enabled, Created, Modified
                FROM "OIDCProviders"
                ORDER BY ProviderName
            """
        else:
            list_query = """
                SELECT ProviderID, ProviderName, ClientID, AuthorizationURL,
                TokenURL, UserInfoURL, ButtonText,
                Scope, ButtonColor, ButtonTextColor, IconSVG, Enabled, Created, Modified
                FROM OIDCProviders
                ORDER BY ProviderName
            """
        cursor.execute(list_query)
        if database_type == "postgresql":
            results = cursor.fetchall()
            providers = []
            for row in results:
                if isinstance(row, dict):
                    # For dict results, normalize the keys
                    normalized = {}
                    for key, value in row.items():
                        normalized_key = key.lower()
                        if normalized_key == "providerid":
                            normalized["provider_id"] = value
                        elif normalized_key == "providername":
                            normalized["provider_name"] = value
                        elif normalized_key == "clientid":
                            normalized["client_id"] = value
                        elif normalized_key == "authorizationurl":
                            normalized["authorization_url"] = value
                        elif normalized_key == "tokenurl":
                            normalized["token_url"] = value
                        elif normalized_key == "userinfourl":
                            normalized["user_info_url"] = value
                        elif normalized_key == "buttontext":
                            normalized["button_text"] = value
                        elif normalized_key == "buttoncolor":
                            normalized["button_color"] = value
                        elif normalized_key == "buttontextcolor":
                            normalized["button_text_color"] = value
                        elif normalized_key == "iconsvg":
                            normalized["icon_svg"] = value
                        else:
                            normalized[normalized_key] = value
                    providers.append(normalized)
                else:
                    # For tuple results, use the existing mapping
                    providers.append({
                        'provider_id': row[0],
                        'provider_name': row[1],
                        'client_id': row[2],
                        'authorization_url': row[3],
                        'token_url': row[4],
                        'user_info_url': row[5],
                        'button_text': row[6],
                        'scope': row[7],
                        'button_color': row[8],
                        'button_text_color': row[9],
                        'icon_svg': row[10],
                        'enabled': row[11],
                        'created': row[12],
                        'modified': row[13]
                    })
        else:
            columns = [col[0] for col in cursor.description]
            results = [dict(zip(columns, row)) for row in cursor.fetchall()]
            # Normalize MySQL results the same way
            providers = []
            for row in results:
                normalized = {}
                for key, value in row.items():
                    normalized_key = key.lower()
                    if normalized_key == "providerid":
                        normalized["provider_id"] = value
                    elif normalized_key == "providername":
                        normalized["provider_name"] = value
                    elif normalized_key == "clientid":
                        normalized["client_id"] = value
                    elif normalized_key == "authorizationurl":
                        normalized["authorization_url"] = value
                    elif normalized_key == "tokenurl":
                        normalized["token_url"] = value
                    elif normalized_key == "userinfourl":
                        normalized["user_info_url"] = value
                    elif normalized_key == "buttontext":
                        normalized["button_text"] = value
                    elif normalized_key == "buttoncolor":
                        normalized["button_color"] = value
                    elif normalized_key == "buttontextcolor":
                        normalized["button_text_color"] = value
                    elif normalized_key == "iconsvg":
                        normalized["icon_svg"] = value
                    elif normalized_key == "enabled":
                        # Convert MySQL TINYINT to boolean
                        normalized["enabled"] = bool(value)
                    else:
                        normalized[normalized_key] = value
                providers.append(normalized)
        return providers
    except Exception as e:
        logging.error(f"Error in list_oidc_providers: {str(e)}")
        raise
    finally:
        cursor.close()

def get_public_oidc_providers(cnx, database_type):
    """Get minimal provider info needed for login buttons."""
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            query = '''
                SELECT
                    ProviderID,
                    ProviderName,
                    ClientID,
                    AuthorizationURL,
                    Scope,
                    ButtonColor,
                    ButtonText,
                    ButtonTextColor,
                    IconSVG
                FROM "OIDCProviders"
                WHERE Enabled = TRUE
            '''
        else:
            query = '''
                SELECT
                    ProviderID,
                    ProviderName,
                    ClientID,
                    AuthorizationURL,
                    Scope,
                    ButtonColor,
                    ButtonText,
                    ButtonTextColor,
                    IconSVG
                FROM OIDCProviders
                WHERE Enabled = TRUE
            '''
        cursor.execute(query)
        results = cursor.fetchall()
        providers = []

        for row in results:
            if isinstance(row, dict):
                # For dict results, normalize the keys
                normalized = {}
                for key, value in row.items():
                    normalized_key = key.lower()
                    if normalized_key == "providerid":
                        normalized["provider_id"] = value
                    elif normalized_key == "providername":
                        normalized["provider_name"] = value
                    elif normalized_key == "clientid":
                        normalized["client_id"] = value
                    elif normalized_key == "authorizationurl":
                        normalized["authorization_url"] = value
                    elif normalized_key == "buttoncolor":
                        normalized["button_color"] = value
                    elif normalized_key == "buttontext":
                        normalized["button_text"] = value
                    elif normalized_key == "buttontextcolor":
                        normalized["button_text_color"] = value
                    elif normalized_key == "iconsvg":
                        normalized["icon_svg"] = value
                    else:
                        normalized[normalized_key] = value
                providers.append(normalized)
            else:
                # For tuple results, use index-based mapping
                providers.append({
                    "provider_id": row[0],
                    "provider_name": row[1],
                    "client_id": row[2],
                    "authorization_url": row[3],
                    "scope": row[4],
                    "button_color": row[5],
                    "button_text": row[6],
                    "button_text_color": row[7],
                    "icon_svg": row[8]
                })

        return providers
    except Exception as e:
        logging.error(f"Error in get_public_oidc_providers: {str(e)}")
        raise
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
        def estimate_duration_from_file_size(file_size_bytes, bitrate_kbps=128):
            """
            Estimate duration in seconds based on file size and bitrate.

            Args:
                file_size_bytes (int): Size of the media file in bytes
                bitrate_kbps (int): Bitrate in kilobits per second (default: 128)

            Returns:
                int: Estimated duration in seconds
            """
            bytes_per_second = (bitrate_kbps * 1000) / 8  # Convert kbps to bytes per second
            return int(file_size_bytes / bytes_per_second)

        # Duration parsing section for the add_episodes function
        parsed_duration = 0
        duration_str = getattr(entry, 'itunes_duration', '')
        if ':' in duration_str:
            # If duration contains ":", then process as HH:MM:SS or MM:SS
            time_parts = list(map(int, duration_str.split(':')))
            while len(time_parts) < 3:
                time_parts.insert(0, 0)  # Pad missing values with zeros

            # Fix for handling more than 3 time parts
            if len(time_parts) > 3:
                print(f"Warning: Duration string '{duration_str}' has more than 3 parts, using first 3")
                h, m, s = time_parts[0], time_parts[1], time_parts[2]
            else:
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
        # Check for enclosure length attribute as a last resort
        elif entry.enclosures and len(entry.enclosures) > 0:
            enclosure = entry.enclosures[0]
            if hasattr(enclosure, 'length') and enclosure.length:
                try:
                    file_size = int(enclosure.length)
                    # Only estimate if the size seems reasonable (to avoid errors)
                    if file_size > 1000000:  # Only consider files larger than 1MB
                        parsed_duration = estimate_duration_from_file_size(file_size)
                        # print(f"Estimated duration from file size {file_size} bytes: {parsed_duration} seconds")
                except (ValueError, TypeError) as e:
                    print(f"Error parsing enclosure length: {e}")


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
                    # DELETE FROM PLAYLIST CONTENTS - Add this first!
                    'DELETE FROM "PlaylistContents" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s)',
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
                    # DELETE FROM PLAYLIST CONTENTS - Add this first!
                    "DELETE FROM PlaylistContents WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = %s)",
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
                    # DELETE FROM PLAYLIST CONTENTS - Add this first!
                    ('DELETE FROM "PlaylistContents" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s)', (podcast_id,)),
                    ('DELETE FROM "UserEpisodeHistory" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s)', (podcast_id,)),
                    ('DELETE FROM "DownloadedEpisodes" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s)', (podcast_id,)),
                    ('DELETE FROM "SavedEpisodes" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s)', (podcast_id,)),
                    ('DELETE FROM "EpisodeQueue" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s)', (podcast_id,)),
                    ('DELETE FROM "Episodes" WHERE PodcastID = %s', (podcast_id,)),
                    ('DELETE FROM "Podcasts" WHERE PodcastID = %s', (podcast_id,))
                ]
            else:  # MySQL or MariaDB
                delete_queries = [
                    # DELETE FROM PLAYLIST CONTENTS - Add this first!
                    ("DELETE FROM PlaylistContents WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = %s)", (podcast_id,)),
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
            # DELETE FROM PLAYLIST CONTENTS - Add this first!
            delete_playlist_contents = 'DELETE FROM "PlaylistContents" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s)'
            delete_history = 'DELETE FROM "UserEpisodeHistory" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s)'
            delete_downloaded = 'DELETE FROM "DownloadedEpisodes" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s)'
            delete_saved = 'DELETE FROM "SavedEpisodes" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s)'
            delete_queue = 'DELETE FROM "EpisodeQueue" WHERE EpisodeID IN (SELECT EpisodeID FROM "Episodes" WHERE PodcastID = %s)'
            delete_episodes = 'DELETE FROM "Episodes" WHERE PodcastID = %s'
            delete_podcast = 'DELETE FROM "Podcasts" WHERE PodcastID = %s'
            update_user_stats = 'UPDATE "UserStats" SET PodcastsAdded = PodcastsAdded - 1 WHERE UserID = %s'
        else:  # MySQL or MariaDB
            # DELETE FROM PLAYLIST CONTENTS - Add this first!
            delete_playlist_contents = "DELETE FROM PlaylistContents WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = %s)"
            delete_history = "DELETE FROM UserEpisodeHistory WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = %s)"
            delete_downloaded = "DELETE FROM DownloadedEpisodes WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = %s)"
            delete_saved = "DELETE FROM SavedEpisodes WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = %s)"
            delete_queue = "DELETE FROM EpisodeQueue WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = %s)"
            delete_episodes = "DELETE FROM Episodes WHERE PodcastID = %s"
            delete_podcast = "DELETE FROM Podcasts WHERE PodcastID = %s"
            update_user_stats = "UPDATE UserStats SET PodcastsAdded = PodcastsAdded - 1 WHERE UserID = %s"

        # Execute the deletion statements in order
        cursor.execute(delete_playlist_contents, (podcast_id,))
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
                if podcast_id == 69:
                    print(f"DEBUG - Podcast 69 NICE data: {result}")
                    # Log the variables right before the line that's causing the error
                if podcast_id == 70:
                    print(f"DEBUG - Podcast 70 data: {result}")
                    # Log the variables right before the line that's causing the error
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

def download_podcast(cnx, database_type, episode_id, user_id, task_id=None, progress_callback=None):
    logging.basicConfig(level=logging.INFO)
    logger = logging.getLogger(__name__)
    print('download podcast is running')
    """
    Download a podcast episode with progress tracking.

    Args:
        cnx: Database connection
        database_type: Type of database (postgresql or mysql)
        episode_id: ID of the episode to download
        user_id: ID of the user requesting the download
        task_id: Optional Celery task ID for progress tracking
        progress_callback: Optional callback function to report progress (fn(progress, status))

    Returns:
        bool: True if successful, False otherwise
    """
    cursor = None
    temp_file = None

    try:
        # Import task-specific modules inside function to avoid circular imports
        if task_id:
            from database_functions.tasks import download_manager

        cursor = cnx.cursor()

        # First, check if already downloaded to avoid duplicate work
        if database_type == "postgresql":
            query = 'SELECT 1 FROM "DownloadedEpisodes" WHERE EpisodeID = %s AND UserID = %s'
        else:
            query = "SELECT 1 FROM DownloadedEpisodes WHERE EpisodeID = %s AND UserID = %s"

        cursor.execute(query, (episode_id, user_id))
        if cursor.fetchone():
            logger.info(f"Episode {episode_id} already downloaded for user {user_id}")
            # Update task progress to 100% if task_id is provided
            if task_id:
                download_manager.update_task(task_id, 100.0, "SUCCESS")
            if progress_callback:
                progress_callback(100.0, "SUCCESS")
            return True

        # Get episode details
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
            logger.error(f"Episode {episode_id} not found")
            if task_id:
                download_manager.update_task(task_id, 0.0, "FAILED")
            if progress_callback:
                progress_callback(0.0, "FAILED")
            return False

        # Extract episode details
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
            episode_artwork = result[6]  # EpisodeArtwork
            artwork_url = result[9]      # ArtworkURL

        # Update task progress if task_id is provided
        if task_id:
            download_manager.update_task(task_id, 5.0, "STARTED")
        if progress_callback:
            progress_callback(5.0, "STARTED")

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

        date_format_str = date_format_map.get(date_format, "%Y-%m-%d")
        filename_date_format_str = date_format_str.replace('/', '-').replace('\\', '-')
        pub_date_str = pub_date.strftime(filename_date_format_str)


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

        # Check if file already exists
        if os.path.exists(file_path):
            # File exists but not in database, add the database entry
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

            if task_id:
                download_manager.update_task(task_id, 100.0, "SUCCESS")
            if progress_callback:
                progress_callback(100.0, "SUCCESS")

            logger.info(f"File already exists, added to database: {file_path}")
            return True

        # Create a temporary file for download
        temp_file = tempfile.NamedTemporaryFile(delete=False, suffix='.mp3')
        temp_path = temp_file.name
        temp_file.close()

        if task_id:
            download_manager.update_task(task_id, 10.0, "DOWNLOADING")
        if progress_callback:
            progress_callback(10.0, "DOWNLOADING")

        # Download the file with progress tracking
        logger.info(f"Starting download of episode {episode_id} from {episode_url}")

        try:
            with requests.get(episode_url, stream=True) as response:
                response.raise_for_status()
                downloaded_date = datetime.datetime.now()
                file_size = int(response.headers.get("Content-Length", 0))

                # Stream the download to temporary file with progress tracking
                downloaded_bytes = 0
                with open(temp_path, "wb") as f:
                    for chunk in response.iter_content(chunk_size=8192):
                        if chunk:
                            f.write(chunk)
                            downloaded_bytes += len(chunk)

                            # Update progress every ~5% if file size is known
                            if file_size > 0:
                                progress = (downloaded_bytes / file_size) * 100
                                # Only update at certain intervals to reduce overhead
                                if downloaded_bytes % (file_size // 20 + 1) < 8192:  # ~5% intervals
                                    download_progress = 10.0 + (progress * 0.8)  # Scale to 10-90%
                                    if task_id:
                                        download_manager.update_task(task_id, download_progress, "DOWNLOADING")
                                    if progress_callback:
                                        progress_callback(download_progress, "DOWNLOADING")
        except Exception as e:
            logger.error(f"Failed to download episode {episode_id}: {str(e)}")
            if task_id:
                download_manager.update_task(task_id, 0.0, "FAILED")
            if progress_callback:
                progress_callback(0.0, "FAILED")

            # Clean up temp file
            if os.path.exists(temp_path):
                os.unlink(temp_path)

            raise

        if task_id:
            download_manager.update_task(task_id, 90.0, "FINALIZING")
        if progress_callback:
            progress_callback(90.0, "FINALIZING")

        print(f"DEBUG - Moving temp file from: {temp_path}")
        print(f"DEBUG - Moving to destination: {file_path}")
        print(f"DEBUG - Directory exists check: {os.path.exists(os.path.dirname(file_path))}")
        print(f"DEBUG - Date format being used: {date_format} -> {date_format_str}")
        print(f"DEBUG - Formatted date: {pub_date_str}")

        # Move the temporary file to the final location
        shutil.move(temp_path, file_path)

        # Set permissions
        os.chown(file_path, uid, gid)

        # Add metadata to the file
        metadata = {
            'title': episode_title,
            'artist': author,
            'album': podcast_name,
            'date': pub_date_str,
            'artwork_url': episode_artwork or artwork_url
        }

        try:
            from database_functions import mp3_metadata
            mp3_metadata.add_podcast_metadata(file_path, metadata)
        except Exception as e:
            logger.warning(f"Failed to add metadata to {file_path}: {e}")

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

        if task_id:
            download_manager.update_task(task_id, 100.0, "SUCCESS")
        if progress_callback:
            progress_callback(100.0, "SUCCESS")

        logger.info(f"Successfully downloaded episode {episode_id} to {file_path}")
        return True

    except requests.RequestException as e:
        logger.error(f"Network error downloading episode {episode_id}: {e}")
        if cursor:
            cnx.rollback()
        if task_id:
            download_manager.update_task(task_id, 0.0, "FAILED")
        if progress_callback:
            progress_callback(0.0, "FAILED")
        return False
    except Exception as e:
        logger.error(f"Error downloading episode {episode_id}: {e}", exc_info=True)
        if cursor:
            cnx.rollback()
        if task_id:
            download_manager.update_task(task_id, 0.0, "FAILED")
        if progress_callback:
            progress_callback(0.0, "FAILED")
        return False
    finally:
        if cursor:
            cursor.close()
        # Clean up temporary file if it exists and wasn't moved
        if temp_file and os.path.exists(temp_file.name):
            try:
                os.unlink(temp_file.name)
            except:
                pass

def get_episode_ids_for_podcast(cnx, database_type, podcast_id):
    """
    Get episode IDs and titles for a podcast.
    Handles both PostgreSQL and MariaDB/MySQL return types.
    PostgreSQL uses lowercase column names, MariaDB uses uppercase.
    """
    cursor = cnx.cursor()
    print(f"Database type: {database_type}")

    if database_type == "postgresql":
        # In PostgreSQL, table names are capitalized but column names are lowercase
        query = 'SELECT "episodeid", "episodetitle" FROM "Episodes" WHERE "podcastid" = %s'
    else:  # MySQL or MariaDB
        query = "SELECT EpisodeID, EpisodeTitle FROM Episodes WHERE PodcastID = %s"

    print(f"Executing query: {query} with podcast_id: {podcast_id}")
    cursor.execute(query, (podcast_id,))
    results = cursor.fetchall()
    print(f"Raw query results (first 3): {results[:3]}")

    episodes = []
    for row in results:
        # Handle different return types from different database drivers
        if isinstance(row, dict):
            # Dictionary return (sometimes from MariaDB)
            if "episodeid" in row:  # PostgreSQL lowercase keys
                episode_id = row["episodeid"]
                episode_title = row.get("episodetitle", "")
            else:  # MariaDB uppercase keys
                episode_id = row["EpisodeID"]
                episode_title = row.get("EpisodeTitle", "")
        else:
            # Tuple return (most common from PostgreSQL)
            episode_id = row[0]
            episode_title = row[1] if len(row) > 1 else ""

        # Check for None, empty string, or 'None' string
        if not episode_title or episode_title == 'None':
            # Get a real episode title from the database if possible
            title_query = (
                'SELECT "episodetitle" FROM "Episodes" WHERE "episodeid" = %s'
                if database_type == "postgresql"
                else "SELECT EpisodeTitle FROM Episodes WHERE EpisodeID = %s"
            )
            cursor.execute(title_query, (episode_id,))
            title_result = cursor.fetchone()

            if title_result and title_result[0]:
                episode_title = title_result[0]
            else:
                # Look up the title by podcast name + episode number if we can
                ordinal_query = (
                    'SELECT p."podcastname", COUNT(*) as episode_num FROM "Episodes" e '
                    'JOIN "Podcasts" p ON e."podcastid" = p."podcastid" '
                    'WHERE p."podcastid" = %s AND e."episodeid" <= %s '
                    'GROUP BY p."podcastname"'
                    if database_type == "postgresql"
                    else "SELECT p.PodcastName, COUNT(*) as episode_num FROM Episodes e "
                         "JOIN Podcasts p ON e.PodcastID = p.PodcastID "
                         "WHERE p.PodcastID = %s AND e.EpisodeID <= %s "
                         "GROUP BY p.PodcastName"
                )
                cursor.execute(ordinal_query, (podcast_id, episode_id))
                ordinal_result = cursor.fetchone()

                if ordinal_result and len(ordinal_result) >= 2:
                    podcast_name = ordinal_result[0]
                    episode_num = ordinal_result[1]
                    episode_title = f"{podcast_name} - Episode {episode_num}"
                else:
                    # Last resort fallback
                    episode_title = f"Episode #{episode_id}"

        episodes.append({"id": episode_id, "title": episode_title})

    print(f"Processed episodes (first 3): {episodes[:3]}")
    cursor.close()
    return episodes

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


def download_youtube_video(cnx, database_type, video_id, user_id, task_id=None, progress_callback=None):
    """
    Download a YouTube video with progress tracking.

    Args:
        cnx: Database connection
        database_type: Type of database (postgresql or mysql)
        video_id: ID of the video to download
        user_id: ID of the user requesting the download
        task_id: Optional Celery task ID for progress tracking
        progress_callback: Optional callback function to report progress (fn(progress, status))

    Returns:
        bool: True if successful, False otherwise
    """
    cursor = None

    try:
        # Import task-specific modules inside function to avoid circular imports
        if task_id:
            from database_functions.tasks import download_manager

        cursor = cnx.cursor()

        # Check if already downloaded
        if database_type == "postgresql":
            query = 'SELECT 1 FROM "DownloadedVideos" WHERE VideoID = %s AND UserID = %s'
        else:
            query = "SELECT 1 FROM DownloadedVideos WHERE VideoID = %s AND UserID = %s"

        cursor.execute(query, (video_id, user_id))
        if cursor.fetchone():
            # Update task progress to 100% if task_id is provided
            if task_id:
                download_manager.update_task(task_id, 100.0, "SUCCESS")
            if progress_callback:
                progress_callback(100.0, "SUCCESS")
            return True

        # Update progress if task_id is provided
        if task_id:
            download_manager.update_task(task_id, 5.0, "STARTED")
        if progress_callback:
            progress_callback(5.0, "STARTED")

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
            if task_id:
                download_manager.update_task(task_id, 0.0, "FAILED")
            if progress_callback:
                progress_callback(0.0, "FAILED")
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

        if task_id:
            download_manager.update_task(task_id, 10.0, "PROCESSING")
        if progress_callback:
            progress_callback(10.0, "PROCESSING")

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
        filename_date_format_str = date_format_str.replace('/', '-').replace('\\', '-')
        pub_date_str = pub_date.strftime(filename_date_format_str)

        # Clean filenames
        channel_name = "".join(c for c in channel_name if c.isalnum() or c in (' ', '-', '_')).strip()
        video_title = "".join(c for c in video_title if c.isalnum() or c in (' ', '-', '_')).strip()

        # Source and destination paths
        source_path = f"/opt/pinepods/downloads/youtube/{youtube_video_id}.mp3"
        if not os.path.exists(source_path):
            source_path = f"{source_path}.mp3"  # Try with double extension
            if not os.path.exists(source_path):
                if task_id:
                    download_manager.update_task(task_id, 0.0, "FAILED")
                if progress_callback:
                    progress_callback(0.0, "FAILED")
                return False

        if task_id:
            download_manager.update_task(task_id, 30.0, "PREPARING_DESTINATION")
        if progress_callback:
            progress_callback(30.0, "PREPARING_DESTINATION")

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

        if task_id:
            download_manager.update_task(task_id, 50.0, "DOWNLOADING")
        if progress_callback:
            progress_callback(50.0, "DOWNLOADING")

        # Copy file with progress tracking
        try:
            # Get source file size for progress tracking
            source_size = os.path.getsize(source_path)

            # Use buffer-based copying to enable progress tracking
            with open(source_path, 'rb') as src_file, open(dest_path, 'wb') as dest_file:
                copied = 0
                buffer_size = 8192  # 8KB buffer

                while True:
                    buffer = src_file.read(buffer_size)
                    if not buffer:
                        break

                    dest_file.write(buffer)
                    copied += len(buffer)

                    if source_size > 0:
                        # Calculate progress (50-80% range for copying)
                        copy_progress = 50.0 + ((copied / source_size) * 30.0)

                        # Update progress every ~5% to reduce overhead
                        if copied % (source_size // 20 + 1) < buffer_size:
                            if task_id:
                                download_manager.update_task(task_id, copy_progress, "DOWNLOADING")
                            if progress_callback:
                                progress_callback(copy_progress, "DOWNLOADING")

        except Exception as e:
            if os.path.exists(dest_path):
                os.unlink(dest_path)  # Clean up incomplete file
            if task_id:
                download_manager.update_task(task_id, 0.0, "FAILED")
            if progress_callback:
                progress_callback(0.0, "FAILED")
            raise

        # Set proper permissions on destination file
        os.chown(dest_path, uid, gid)

        if task_id:
            download_manager.update_task(task_id, 80.0, "FINALIZING")
        if progress_callback:
            progress_callback(80.0, "FINALIZING")

        # Update metadata
        try:
            metadata = {
                'title': video_title,
                'artist': author,
                'album': channel_name,
                'date': pub_date_str
            }
            from database_functions import mp3_metadata
            mp3_metadata.add_podcast_metadata(dest_path, metadata)
        except Exception as e:
            print(f"Failed to add metadata to {dest_path}: {e}")
            # Continue despite metadata failure

        if task_id:
            download_manager.update_task(task_id, 90.0, "UPDATING_DATABASE")
        if progress_callback:
            progress_callback(90.0, "UPDATING_DATABASE")

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

        if task_id:
            download_manager.update_task(task_id, 100.0, "SUCCESS")
        if progress_callback:
            progress_callback(100.0, "SUCCESS")

        print(f"Successfully downloaded YouTube video {video_id} to {dest_path}")
        return True

    except Exception as e:
        print(f"Error downloading YouTube video {video_id}: {str(e)}", exc_info=True)
        if cursor:
            cnx.rollback()
        if task_id:
            download_manager.update_task(task_id, 0.0, "FAILED")
        if progress_callback:
            progress_callback(0.0, "FAILED")
        return False
    finally:
        if cursor:
            cursor.close()




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
    try:
        if database_type == "postgresql":
            query = 'SELECT EpisodeID FROM "Episodes" WHERE EpisodeURL = %s'
        else:
            query = "SELECT EpisodeID FROM Episodes WHERE EpisodeURL = %s"

        params = (episode_url,)  # Ensure this is a tuple
        cursor.execute(query, params)
        result = cursor.fetchone()

        if result:
            # Handle both tuple and dictionary-like results
            if isinstance(result, dict):
                # Try with both camelCase and lowercase keys
                episode_id = result.get("episodeid") or result.get("EpisodeID")
            else:  # Assume it's a tuple or tuple-like
                episode_id = result[0]

            return episode_id
        return None  # No matching episode found
    except Exception as e:
        print(f"Error in get_episode_id_by_url: {e}")
        return None
    finally:
        cursor.close()



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

def get_user_api_key(cnx, database_type, user_id):
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            query = """
                SELECT APIKey
                FROM "APIKeys"
                WHERE UserID = %s
                ORDER BY Created DESC
                LIMIT 1
            """
        else:
            query = """
                SELECT APIKey
                FROM APIKeys
                WHERE UserID = %s
                ORDER BY Created DESC
                LIMIT 1
            """
        cursor.execute(query, (user_id,))
        result = cursor.fetchone()
        if result:
            return result[0] if isinstance(result, tuple) else result['apikey']
        return None
    finally:
        cursor.close()


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
        logging.info(f'verify_api_key result type: {type(result)}, value: {result}')
        return True if result else False
    except Exception as e:
        logging.error(f'verify_api_key error: {str(e)}')
        return False
    finally:
        cursor.close()

def get_user_gpodder_status(cnx, database_type, user_id):
    cursor = cnx.cursor()
    try:
        print(f"Getting status for user_id: {user_id}")
        cursor.execute(
            'SELECT Pod_Sync_Type, GpodderUrl, GpodderLoginName FROM "Users" WHERE UserID = %s',
            (user_id,)
        )
        user_data = cursor.fetchone()
        print(f"Raw user_data: {user_data}, type: {type(user_data)}")

        if not user_data:
            print("No user data found")
            return None

        # Handle both dict and tuple return types
        if isinstance(user_data, dict):
            print("Handling dict type return")
            sync_type = user_data.get('Pod_Sync_Type')
            print(f"Dict sync_type before default: {sync_type}")
            sync_type = sync_type if sync_type else "None"
            gpodder_url = user_data.get('GpodderUrl')
            gpodder_login = user_data.get('GpodderLoginName')
        else:
            # It's a tuple/list
            print("Handling tuple/list type return")
            sync_type = user_data[0]
            print(f"Tuple sync_type before default: {sync_type}")
            sync_type = sync_type if sync_type else "None"
            gpodder_url = user_data[1] if len(user_data) > 1 else None
            gpodder_login = user_data[2] if len(user_data) > 2 else None

        print(f"Final sync_type: {sync_type}")

        # Create a proper structure for the result
        result = {
            "sync_type": sync_type,
            "gpodder_url": gpodder_url,
            "gpodder_login": gpodder_login
        }
        print(f"Returning user status: {result}")
        return result
    except Exception as e:
        print(f"Database error in get_user_gpodder_status: {str(e)}")
        return None
    finally:
        cursor.close()

def update_user_gpodder_sync(cnx, database_type, user_id, new_sync_type):
    cursor = cnx.cursor()
    try:
        print(f"Updating sync type for user_id {user_id} to {new_sync_type}")
        cursor.execute(
            'UPDATE "Users" SET Pod_Sync_Type = %s WHERE UserID = %s',
            (new_sync_type, user_id)
        )
        rows_affected = cursor.rowcount
        print(f"Rows affected by update: {rows_affected}")
        cnx.commit()
        print("Transaction committed")

        # Verify the update was successful
        verify_cursor = cnx.cursor()
        verify_cursor.execute(
            'SELECT Pod_Sync_Type FROM "Users" WHERE UserID = %s',
            (user_id,)
        )
        updated_value = verify_cursor.fetchone()
        verify_cursor.close()
        print(f"Verification after update: {updated_value}")

        return rows_affected > 0
    except Exception as e:
        print(f"Database error in update_user_gpodder_sync: {e}")
        return False
    finally:
        cursor.close()


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
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            query = 'SELECT userid FROM "APIKeys" WHERE apikey = %s'
        else:
            query = "SELECT UserID FROM APIKeys WHERE APIKey = %s"
        cursor.execute(query, (passed_key,))
        result = cursor.fetchone()
        logging.info(f"id_from_api_key result type: {type(result)}, value: {result}")

        if result is None:
            logging.error("No result found for API key")
            return None

        try:
            user_id = get_value_from_result(result, 'userid')
            logging.info(f"Successfully extracted user_id: {user_id}")
            return user_id
        except Exception as e:
            logging.error(f"Error extracting user_id from result: {e}")
            # If we failed to get from dict, try tuple
            if isinstance(result, tuple) and len(result) > 0:
                return result[0]
            raise

    except Exception as e:
        logging.error(f"Error in id_from_api_key: {e}")
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
                    (UserID, Platform, Enabled, NtfyTopic, NtfyServerUrl, GotifyUrl, GotifyToken)
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


def build_playlist_query(playlist, database_type):
    # Debug the incoming playlist data
    print(f"DEBUG - Playlist time filter value: {playlist.get('timefilterhours')}")
    print(f"DEBUG - Playlist keys: {list(playlist.keys())}")

    # Check and print the progress threshold values
    progress_min = playlist.get('playprogressmin')
    progress_max = playlist.get('playprogressmax')
    print(f"DEBUG - Progress min value: {progress_min}")
    print(f"DEBUG - Progress max value: {progress_max}")

    conditions = []
    params = []

    # Check if this is a system playlist (owned by user 1)
    is_system_playlist = playlist['userid'] == 1 and playlist['issystemplaylist']
    playlist_name = playlist.get('name', '')

    # Special case handling for playlists that need to filter by user listening history
    needs_user_history = playlist_name in ['Currently Listening', 'Almost Done'] or not is_system_playlist

    # Ensure Fresh Releases has time filter set
    if playlist_name == 'Fresh Releases' and playlist.get('timefilterhours') is None:
        playlist['timefilterhours'] = 24
        print(f"Setting default 24 hour time filter for Fresh Releases playlist")

    if database_type == "postgresql":
        # Special case for playlists that filter by user listening progress
        if playlist['includepartiallyplayed'] and not playlist['includeunplayed'] and not playlist['includeplayed']:
            # Base query for partially played episodes - IMPORTANT: Include all ORDER BY columns in SELECT
            query = """
                    SELECT DISTINCT e.episodeid, e.episodepubdate
                    FROM "Episodes" e
                    JOIN "Podcasts" p ON e.podcastid = p.podcastid
                    JOIN "UserEpisodeHistory" h ON e.episodeid = h.episodeid
                    WHERE h.listenduration > 0
                    AND h.listenduration < e.episodeduration
                    AND e.Completed = FALSE
                    AND e.episodeduration > 0
            """
            params = []

            # Add progress min filter if specified - this drives the Almost Done functionality
            if progress_min is not None:
                min_decimal = float(progress_min) / 100.0
                # Use %s parameter placeholder for safety
                query += ' AND (h.listenduration::float / e.episodeduration::float) >= %s'
                params.append(min_decimal)
                print(f"Adding progress min filter: {min_decimal} ({progress_min}% complete)")

            # Add progress max filter if specified
            if progress_max is not None:
                max_decimal = float(progress_max) / 100.0
                query += ' AND (h.listenduration::float / e.episodeduration::float) <= %s'
                params.append(max_decimal)
                print(f"Adding progress max filter: {max_decimal}")

            print(f"Special query for in-progress playlist with filters")

            # Add sort order
            if playlist['sortorder']:
                sort_mapping = {
                    'date_asc': 'e.episodepubdate ASC',
                    'date_desc': 'e.episodepubdate DESC',
                    'duration_asc': 'e.episodeduration ASC',
                    'duration_desc': 'e.episodeduration DESC',
                    'listen_progress': '(h.listenduration::float / e.episodeduration::float) DESC',
                    'completion': '(h.listenduration::float / e.episodeduration::float) DESC'
                }
                order_by = sort_mapping.get(playlist['sortorder'], 'e.episodepubdate DESC')
                query += f" ORDER BY {order_by}"

        else:
            # Basic query structure depends on playlist type
            if is_system_playlist:
                if needs_user_history:
                    # System playlist that needs user listening history (e.g., Currently Listening)
                    query = """
                            SELECT e.episodeid
                            FROM "Episodes" e
                            JOIN "Podcasts" p ON e.podcastid = p.podcastid
                            LEFT JOIN "UserEpisodeHistory" h ON e.episodeid = h.episodeid AND h.userid = %s
                            JOIN "Users" u ON u.UserID = %s
                            WHERE 1=1
                        """
                    params.extend([playlist['userid'], playlist['userid']])
                else:
                    # System playlist that doesn't need user history filtering (e.g., Fresh Releases)
                    query = """
                            SELECT e.episodeid
                            FROM "Episodes" e
                            JOIN "Podcasts" p ON e.podcastid = p.podcastid
                            LEFT JOIN "UserEpisodeHistory" h ON e.episodeid = h.episodeid
                            JOIN "Users" u ON u.UserID = %s
                            WHERE 1=1
                        """
                    params.extend([playlist['userid']])  # Only needed for timezone

                print(f"System playlist detected - showing all podcasts")
            else:
                # User-specific playlist - only show user's podcasts
                query = """
                        SELECT e.episodeid
                        FROM "Episodes" e
                        JOIN "Podcasts" p ON e.podcastid = p.podcastid
                        LEFT JOIN "UserEpisodeHistory" h ON e.episodeid = h.episodeid AND h.userid = %s
                        JOIN "Users" u ON u.UserID = %s
                        WHERE p.UserID = %s
                    """
                params.extend([playlist['userid'], playlist['userid'], playlist['userid']])
                print(f"User playlist detected - only showing podcasts for user {playlist['userid']}")

            # Podcast filter for PostgreSQL
            if playlist['podcastids']:
                conditions.append('e.podcastid = ANY(%s)')
                params.append(playlist['podcastids'])

            # Duration filters
            if playlist['minduration'] is not None:
                conditions.append('e.episodeduration >= %s')
                params.append(playlist['minduration'])
            if playlist['maxduration'] is not None:
                conditions.append('e.episodeduration <= %s')
                params.append(playlist['maxduration'])

            # Play state filters with progress
            play_state_conditions = []

            if playlist['includeunplayed']:
                play_state_conditions.append('h.listenduration IS NULL')

            if playlist['includepartiallyplayed']:
                # Base condition: episodes with some progress but not fully listened
                partial_condition = '(h.listenduration > 0 AND h.listenduration < e.episodeduration AND e.Completed = FALSE)'

                # Add progress range conditions if specified
                if playlist.get('playprogressmin') is not None:
                    min_decimal = float(playlist["playprogressmin"]) / 100.0
                    partial_condition += f' AND (h.listenduration::float / NULLIF(e.episodeduration, 0)) >= {min_decimal}'

                if playlist.get('playprogressmax') is not None:
                    max_decimal = float(playlist["playprogressmax"]) / 100.0
                    partial_condition += f' AND (h.listenduration::float / NULLIF(e.episodeduration, 0)) <= {max_decimal}'

                play_state_conditions.append(partial_condition)

            if playlist['includeplayed']:
                play_state_conditions.append('h.listenduration >= e.episodeduration')

            if play_state_conditions:
                conditions.append(f"({' OR '.join(play_state_conditions)})")

            # Time filter for PostgreSQL with timezone support
            if playlist.get('timefilterhours') is not None:
                print(f"Applying time filter of {playlist['timefilterhours']} hours with timezone support")
                conditions.append('''
                    e.episodepubdate AT TIME ZONE 'UTC'
                    AT TIME ZONE COALESCE(u.TimeZone, 'UTC') >
                    (CURRENT_TIMESTAMP AT TIME ZONE 'UTC'
                    AT TIME ZONE COALESCE(u.TimeZone, 'UTC') - INTERVAL '%s hours')
                ''')
                params.append(playlist['timefilterhours'])

            # Add all conditions
            if conditions:
                query += " AND " + " AND ".join(conditions)

            # Sorting for PostgreSQL
            sort_mapping = {
                'date_asc': 'e.episodepubdate ASC',
                'date_desc': 'e.episodepubdate DESC',
                'duration_asc': 'e.episodeduration ASC',
                'duration_desc': 'e.episodeduration DESC',
                'listen_progress': '(COALESCE(h.listenduration, 0)::float / NULLIF(e.episodeduration, 0)) DESC',
                'completion': 'COALESCE(h.listenduration::float / NULLIF(e.episodeduration, 0), 0) DESC'
            }

            order_by = sort_mapping.get(playlist['sortorder'], 'e.episodepubdate DESC')
            if playlist['groupbypodcast']:
                order_by = f'e.podcastid, {order_by}'

            query += f" ORDER BY {order_by}"

    else:  # MySQL version
        # Check for partially played episodes with progress threshold (Almost Done-like functionality)
        if playlist['includepartiallyplayed'] and not playlist['includeunplayed'] and not playlist['includeplayed'] and playlist.get('playprogressmin') is not None and float(playlist.get('playprogressmin')) >= 75.0:
            # This is the "Almost Done" pattern - episodes that are 75%+ complete but not finished
            query = """
                    SELECT DISTINCT e.episodeid
                    FROM Episodes e
                    JOIN Podcasts p ON e.podcastid = p.podcastid
                    JOIN UserEpisodeHistory h ON e.episodeid = h.episodeid
                    WHERE h.listenduration > 0
                    AND h.listenduration < e.episodeduration
                    AND e.Completed = 0
                    AND (h.listenduration / NULLIF(e.episodeduration, 0)) >= %s
            """
            min_decimal = float(playlist["playprogressmin"]) / 100.0
            params = [min_decimal]

            # Add progress max constraint if specified
            if playlist.get('playprogressmax') is not None:
                max_decimal = float(playlist["playprogressmax"]) / 100.0
                query += f' AND (h.listenduration / NULLIF(e.episodeduration, 0)) <= %s'
                params.append(max_decimal)

            print(f"Special query for playlist with high progress threshold ({playlist.get('playprogressmin')}%+)")

        # Check for partially played episodes without progress threshold (Currently Listening-like functionality)
        elif playlist['includepartiallyplayed'] and not playlist['includeunplayed'] and not playlist['includeplayed'] and (playlist.get('playprogressmin') is None or float(playlist.get('playprogressmin')) < 75.0):
            # This is the "Currently Listening" pattern - any episode that's started but not finished
            query = """
                    SELECT DISTINCT e.episodeid
                    FROM Episodes e
                    JOIN Podcasts p ON e.podcastid = p.podcastid
                    JOIN UserEpisodeHistory h ON e.episodeid = h.episodeid
                    WHERE h.listenduration > 0
                    AND h.listenduration < e.episodeduration
                    AND e.Completed = 0
            """
            params = []

            # Add progress min constraint if specified
            if playlist.get('playprogressmin') is not None:
                min_decimal = float(playlist["playprogressmin"]) / 100.0
                query += f' AND (h.listenduration / NULLIF(e.episodeduration, 0)) >= %s'
                params.append(min_decimal)

            # Add progress max constraint if specified
            if playlist.get('playprogressmax') is not None:
                max_decimal = float(playlist["playprogressmax"]) / 100.0
                query += f' AND (h.listenduration / NULLIF(e.episodeduration, 0)) <= %s'
                params.append(max_decimal)

            print(f"Special query for playlist with in-progress episodes")

        else:
            # Basic query structure depends on playlist type
            if is_system_playlist:
                if needs_user_history:
                    # System playlist that needs user listening history (e.g., Currently Listening)
                    query = """
                            SELECT e.episodeid
                            FROM Episodes e
                            JOIN Podcasts p ON e.podcastid = p.podcastid
                            LEFT JOIN UserEpisodeHistory h ON e.episodeid = h.episodeid AND h.userid = %s
                            JOIN Users u ON u.UserID = %s
                            WHERE 1=1
                        """
                    params.extend([playlist['userid'], playlist['userid']])
                else:
                    # System playlist that doesn't need user history filtering (e.g., Fresh Releases)
                    query = """
                            SELECT e.episodeid
                            FROM Episodes e
                            JOIN Podcasts p ON e.podcastid = p.podcastid
                            LEFT JOIN UserEpisodeHistory h ON e.episodeid = h.episodeid
                            JOIN Users u ON u.UserID = %s
                            WHERE 1=1
                        """
                    params.extend([playlist['userid']])  # Only needed for timezone

                print(f"System playlist detected - showing all podcasts")
            else:
                # User-specific playlist - only show user's podcasts
                query = """
                        SELECT e.episodeid
                        FROM Episodes e
                        JOIN Podcasts p ON e.podcastid = p.podcastid
                        LEFT JOIN UserEpisodeHistory h ON e.episodeid = h.episodeid AND h.userid = %s
                        JOIN Users u ON u.UserID = %s
                        WHERE p.UserID = %s
                    """
                params.extend([playlist['userid'], playlist['userid'], playlist['userid']])
                print(f"User playlist detected - only showing podcasts for user {playlist['userid']}")

            # Podcast filter for MySQL
            if playlist['podcastids']:
                # Convert the PostgreSQL array to a list of integers for MySQL
                if isinstance(playlist['podcastids'], list):
                    podcast_ids = playlist['podcastids']
                else:
                    # If it's a string representation of a list
                    import json
                    try:
                        podcast_ids = json.loads(playlist['podcastids'])
                    except:
                        # Fallback for PostgreSQL array string format like '{1,2,3}'
                        podcast_ids = [int(id.strip()) for id in playlist['podcastids'].strip('{}').split(',') if id.strip()]

                if len(podcast_ids) == 1:
                    # Simple equality for a single podcast
                    conditions.append('e.podcastid = %s')
                    params.append(podcast_ids[0])
                else:
                    # IN clause for multiple podcasts
                    placeholders = ', '.join(['%s'] * len(podcast_ids))
                    conditions.append(f'e.podcastid IN ({placeholders})')
                    params.extend(podcast_ids)

            # Duration filters
            if playlist['minduration'] is not None:
                conditions.append('e.episodeduration >= %s')
                params.append(playlist['minduration'])
            if playlist['maxduration'] is not None:
                conditions.append('e.episodeduration <= %s')
                params.append(playlist['maxduration'])

            # Play state filters with progress
            play_state_conditions = []

            if playlist['includeunplayed']:
                play_state_conditions.append('h.listenduration IS NULL')

            if playlist['includepartiallyplayed']:
                # Base condition: episodes with some progress but not fully listened
                partial_condition = '(h.listenduration > 0 AND h.listenduration < e.episodeduration AND e.Completed = 0)'

                # Add progress range conditions if specified
                if playlist.get('playprogressmin') is not None:
                    min_decimal = float(playlist["playprogressmin"]) / 100.0
                    partial_condition += f' AND (h.listenduration / NULLIF(e.episodeduration, 0)) >= {min_decimal}'

                if playlist.get('playprogressmax') is not None:
                    max_decimal = float(playlist["playprogressmax"]) / 100.0
                    partial_condition += f' AND (h.listenduration / NULLIF(e.episodeduration, 0)) <= {max_decimal}'

                play_state_conditions.append(partial_condition)

            if playlist['includeplayed']:
                play_state_conditions.append('h.listenduration >= e.episodeduration')

            if play_state_conditions:
                conditions.append(f"({' OR '.join(play_state_conditions)})")

            # Time filter for MySQL with timezone support
            if playlist.get('timefilterhours') is not None:
                print(f"Applying time filter of {playlist['timefilterhours']} hours with timezone support")
                conditions.append('''
                    CONVERT_TZ(e.episodepubdate, 'UTC', COALESCE(u.TimeZone, 'UTC')) >
                    DATE_SUB(CONVERT_TZ(NOW(), 'UTC', COALESCE(u.TimeZone, 'UTC')), INTERVAL %s HOUR)
                ''')
                params.append(playlist['timefilterhours'])

            # Add all conditions
            if conditions:
                query += " AND " + " AND ".join(conditions)

            # Sorting for MySQL
            sort_mapping = {
                'date_asc': 'e.episodepubdate ASC',
                'date_desc': 'e.episodepubdate DESC',
                'duration_asc': 'e.episodeduration ASC',
                'duration_desc': 'e.episodeduration DESC',
                'listen_progress': '(COALESCE(h.listenduration, 0) / NULLIF(e.episodeduration, 0)) DESC',
                'completion': 'COALESCE(h.listenduration / NULLIF(e.episodeduration, 0), 0) DESC'
            }

            order_by = sort_mapping.get(playlist['sortorder'], 'e.episodepubdate DESC')
            if playlist['groupbypodcast']:
                order_by = f'e.podcastid, {order_by}'

            query += f" ORDER BY {order_by}"

    # Add limit (same for both databases)
    if playlist['maxepisodes']:
        query += " LIMIT %s"
        params.append(playlist['maxepisodes'])

    return query, params

def update_fresh_releases_playlist(cnx, database_type):
    """
    Special function to update the Fresh Releases playlist for all users
    considering their individual timezones.
    """
    cursor = cnx.cursor()
    try:
        # First, identify the Fresh Releases playlist ID
        if database_type == "postgresql":
            cursor.execute("""
                SELECT PlaylistID
                FROM "Playlists"
                WHERE Name = 'Fresh Releases' AND IsSystemPlaylist = TRUE
            """)
        else:  # MySQL
            cursor.execute("""
                SELECT PlaylistID
                FROM Playlists
                WHERE Name = 'Fresh Releases' AND IsSystemPlaylist = 1
            """)

        playlist_id = cursor.fetchone()[0]
        if not playlist_id:
            raise Exception("Fresh Releases playlist not found in system")

        print(f"Updating Fresh Releases playlist (ID: {playlist_id})")

        # Clear existing contents from the playlist
        if database_type == "postgresql":
            cursor.execute('DELETE FROM "PlaylistContents" WHERE playlistid = %s', (playlist_id,))
        else:  # MySQL
            cursor.execute('DELETE FROM PlaylistContents WHERE playlistid = %s', (playlist_id,))

        # Get all users and their timezones
        if database_type == "postgresql":
            cursor.execute('SELECT UserID, TimeZone FROM "Users"')
        else:  # MySQL
            cursor.execute('SELECT UserID, TimeZone FROM Users')

        users = cursor.fetchall()
        added_episodes = set()  # Track episodes we've already added to avoid duplicates
        position = 0  # For ordering episodes in the playlist

        # Process each user
        for user in users:
            user_id = user[0]
            timezone = user[1] or 'UTC'  # Default to UTC if timezone is not set

            print(f"Processing user {user_id} with timezone {timezone}")

            # Get episodes from last 24 hours based on user's timezone
            if database_type == "postgresql":
                query = """
                    SELECT e.episodeid
                    FROM "Episodes" e
                    JOIN "Podcasts" p ON e.podcastid = p.podcastid
                    WHERE e.episodepubdate AT TIME ZONE 'UTC'
                          AT TIME ZONE %s >
                          (CURRENT_TIMESTAMP AT TIME ZONE 'UTC'
                          AT TIME ZONE %s - INTERVAL '24 hours')
                    ORDER BY e.episodepubdate DESC
                """
                cursor.execute(query, (timezone, timezone))
            else:  # MySQL
                query = """
                    SELECT e.episodeid
                    FROM Episodes e
                    JOIN Podcasts p ON e.podcastid = p.podcastid
                    WHERE CONVERT_TZ(e.episodepubdate, 'UTC', %s) >
                          DATE_SUB(CONVERT_TZ(NOW(), 'UTC', %s), INTERVAL 24 HOUR)
                    ORDER BY e.episodepubdate DESC
                """
                cursor.execute(query, (timezone, timezone))

            recent_episodes = cursor.fetchall()
            print(f"Found {len(recent_episodes)} recent episodes for user {user_id}")

            # Add episodes to playlist if not already added
            for episode in recent_episodes:
                episode_id = episode[0]
                if episode_id not in added_episodes:
                    if database_type == "postgresql":
                        cursor.execute("""
                            INSERT INTO "PlaylistContents" (playlistid, episodeid, position)
                            VALUES (%s, %s, %s)
                        """, (playlist_id, episode_id, position))
                    else:  # MySQL
                        cursor.execute("""
                            INSERT INTO PlaylistContents (playlistid, episodeid, position)
                            VALUES (%s, %s, %s)
                        """, (playlist_id, episode_id, position))

                    added_episodes.add(episode_id)
                    position += 1

        # Update LastUpdated timestamp
        if database_type == "postgresql":
            cursor.execute("""
                UPDATE "Playlists"
                SET lastupdated = CURRENT_TIMESTAMP
                WHERE playlistid = %s
            """, (playlist_id,))
        else:  # MySQL
            cursor.execute("""
                UPDATE Playlists
                SET lastupdated = CURRENT_TIMESTAMP
                WHERE playlistid = %s
            """, (playlist_id,))

        cnx.commit()
        print(f"Successfully updated Fresh Releases playlist with {len(added_episodes)} unique episodes")

    except Exception as e:
        print(f"ERROR updating Fresh Releases playlist: {str(e)}")
        cnx.rollback()
        raise
    finally:
        cursor.close()


def update_playlist_contents(cnx, database_type, playlist):
    cursor = cnx.cursor()
    try:
        print(f"\n======= UPDATE PLAYLIST: {playlist['name']} (ID: {playlist['playlistid']}) =======")

        # Clear existing contents - database specific
        if database_type == "postgresql":
            cursor.execute('DELETE FROM "PlaylistContents" WHERE playlistid = %s',
                          (playlist['playlistid'],))
        else:  # MySQL
            # For MySQL, add retry logic to handle deadlocks
            max_retries = 3
            retry_count = 0

            while retry_count < max_retries:
                try:
                    # Start a fresh transaction for each attempt
                    cnx.rollback()  # Clear any previous transaction state

                    cursor.execute('DELETE FROM PlaylistContents WHERE playlistid = %s',
                                  (playlist['playlistid'],))
                    break  # Exit the retry loop if successful
                except mysql.connector.errors.InternalError as e:
                    if "Deadlock" in str(e) and retry_count < max_retries - 1:
                        # If it's a deadlock and we have retries left
                        retry_count += 1
                        print(f"Deadlock detected, retrying operation (attempt {retry_count}/{max_retries})")
                        # Add a small delay before retrying to reduce contention
                        import time
                        time.sleep(0.5 * retry_count)  # Increasing backoff
                    else:
                        # Either not a deadlock or we've exhausted retries
                        raise

        print(f"Cleared existing contents for playlist {playlist['playlistid']}")

        # Build and execute query
        query, params = build_playlist_query(playlist, database_type)

        # Try to create a debug query with params substituted
        debug_query = query
        debug_params = list(params)  # Make a copy

        try:
            for i, param in enumerate(debug_params):
                placeholder = "%s"
                if param is None:
                    replacement = "NULL"
                elif isinstance(param, list):
                    if database_type == "postgresql":
                        replacement = f"ARRAY[{','.join(map(str, param))}]"
                    else:  # MySQL
                        replacement = f"({','.join(map(str, param))})"
                elif isinstance(param, str):
                    replacement = f"'{param}'"
                else:
                    replacement = str(param)

                debug_query = debug_query.replace(placeholder, replacement, 1)

            print(f"DEBUG QUERY: {debug_query}")
        except Exception as e:
            print(f"Error creating debug query: {e}")

        # First, let's check if there are any episodes at all for this user
        if database_type == "postgresql":
            basic_check_query = f"""
                SELECT COUNT(*) FROM "Episodes" e
                JOIN "Podcasts" p ON e.podcastid = p.podcastid
                WHERE p.UserID = {playlist['userid']}
            """
        else:  # MySQL
            basic_check_query = f"""
                SELECT COUNT(*) FROM Episodes e
                JOIN Podcasts p ON e.podcastid = p.podcastid
                WHERE p.UserID = {playlist['userid']}
            """
        cursor.execute(basic_check_query)
        # Handle both dictionary and tuple result formats
        result = cursor.fetchone()
        if isinstance(result, dict):
            # Dictionary format - use first key in the dict
            total_episodes = result[list(result.keys())[0]]
        else:
            # Tuple format - use first element
            total_episodes = result[0]

        print(f"Total episodes available for user {playlist['userid']}: {total_episodes}")

        # Now execute the actual filtered query
        cursor.execute(query, params)
        episodes = cursor.fetchall()
        episode_count = len(episodes)
        print(f"Found {episode_count} episodes matching criteria for playlist {playlist['playlistid']}")

        # If we found episodes, show some details
        if episode_count > 0:
            # Handle both tuple and dict format episodes
            episode_ids = []
            for ep in episodes[:5]:
                if isinstance(ep, dict):
                    episode_ids.append(ep.get('episodeid'))
                else:
                    episode_ids.append(ep[0])

            print(f"First few episode IDs: {episode_ids}")

            # Get details for the first episode
            if episode_count > 0:
                if isinstance(episodes[0], dict):
                    first_ep_id = episodes[0].get('episodeid')
                else:
                    first_ep_id = episodes[0][0]

                if database_type == "postgresql":
                    cursor.execute("""
                        SELECT e.episodeid, e.episodetitle, e.episodeduration,
                               h.listenduration, p.podcastid, p.podcastname, p.userid
                        FROM "Episodes" e
                        JOIN "Podcasts" p ON e.podcastid = p.podcastid
                        LEFT JOIN "UserEpisodeHistory" h ON e.episodeid = h.episodeid AND h.userid = %s
                        WHERE e.episodeid = %s
                    """, (playlist['userid'], first_ep_id))
                else:  # MySQL
                    cursor.execute("""
                        SELECT e.episodeid, e.episodetitle, e.episodeduration,
                               h.listenduration, p.podcastid, p.podcastname, p.userid
                        FROM Episodes e
                        JOIN Podcasts p ON e.podcastid = p.podcastid
                        LEFT JOIN UserEpisodeHistory h ON e.episodeid = h.episodeid AND h.userid = %s
                        WHERE e.episodeid = %s
                    """, (playlist['userid'], first_ep_id))

                ep_details = cursor.fetchone()
                print(f"First episode details: {ep_details}")

        # Insert episodes into playlist
        for position, episode in enumerate(episodes):
            if isinstance(episode, dict):
                episode_id = episode.get('episodeid')
            else:
                episode_id = episode[0]

            if database_type == "postgresql":
                cursor.execute("""
                    INSERT INTO "PlaylistContents" (playlistid, episodeid, position)
                    VALUES (%s, %s, %s)
                """, (playlist['playlistid'], episode_id, position))
            else:  # MySQL
                cursor.execute("""
                    INSERT INTO PlaylistContents (playlistid, episodeid, position)
                    VALUES (%s, %s, %s)
                """, (playlist['playlistid'], episode_id, position))

        # Update LastUpdated timestamp
        if database_type == "postgresql":
            cursor.execute("""
                UPDATE "Playlists"
                SET lastupdated = CURRENT_TIMESTAMP
                WHERE playlistid = %s
            """, (playlist['playlistid'],))
        else:  # MySQL
            cursor.execute("""
                UPDATE Playlists
                SET lastupdated = CURRENT_TIMESTAMP
                WHERE playlistid = %s
            """, (playlist['playlistid'],))

        cnx.commit()
        print(f"Successfully updated playlist {playlist['playlistid']} with {episode_count} episodes")

    except Exception as e:
        print(f"ERROR updating playlist {playlist['name']}: {str(e)}")
        import traceback
        print(traceback.format_exc())
        cnx.rollback()
        raise
    finally:
        cursor.close()


def update_all_playlists(cnx, database_type):
    """
    Update all playlists based on their rules
    """
    cursor = cnx.cursor()
    try:
        print("\n=================== PLAYLIST UPDATE STARTING ===================")
        print("Starting to fetch all playlists")

        if database_type == "postgresql":
            cursor.execute('''
                SELECT
                    playlistid, userid, name, description, issystemplaylist,
                    podcastids, includeunplayed, includepartiallyplayed,
                    includeplayed, minduration, maxduration, sortorder,
                    groupbypodcast, maxepisodes, playprogressmin,
                    playprogressmax, timefilterhours
                FROM "Playlists"
            ''')
        else:  # MySQL
            cursor.execute('''
                SELECT
                    PlaylistID, UserID, Name, Description, IsSystemPlaylist,
                    PodcastIDs, IncludeUnplayed, IncludePartiallyPlayed,
                    IncludePlayed, MinDuration, MaxDuration, SortOrder,
                    GroupByPodcast, MaxEpisodes, PlayProgressMin,
                    PlayProgressMax, TimeFilterHours
                FROM Playlists
            ''')

        columns = [desc[0].lower() for desc in cursor.description]
        print(f"Playlist columns: {columns}")
        playlists = cursor.fetchall()
        total_playlists = len(playlists)
        print(f"Found {total_playlists} playlists to update")

        # Let's print info about users and their podcasts
        if database_type == "postgresql":
            cursor.execute("""
                SELECT userid, COUNT(DISTINCT podcastid) as podcast_count
                FROM "Podcasts"
                GROUP BY userid
            """)
        else:  # MySQL
            cursor.execute("""
                SELECT UserID, COUNT(DISTINCT PodcastID) as podcast_count
                FROM Podcasts
                GROUP BY UserID
            """)

        user_podcast_counts = cursor.fetchall()
        print(f"User podcast counts: {user_podcast_counts}")

        if database_type == "postgresql":
            cursor.execute("""
                SELECT p.userid, p.podcastid, COUNT(e.episodeid) as episode_count
                FROM "Podcasts" p
                JOIN "Episodes" e ON p.podcastid = e.podcastid
                GROUP BY p.userid, p.podcastid
                ORDER BY p.userid, p.podcastid
            """)
        else:  # MySQL
            cursor.execute("""
                SELECT p.UserID, p.PodcastID, COUNT(e.EpisodeID) as episode_count
                FROM Podcasts p
                JOIN Episodes e ON p.PodcastID = e.PodcastID
                GROUP BY p.UserID, p.PodcastID
                ORDER BY p.UserID, p.PodcastID
            """)

        podcast_episode_counts = cursor.fetchall()
        print(f"First few podcast episode counts: {podcast_episode_counts[:5]}")

        # Handle Fresh Releases separately
        update_fresh_releases_playlist(cnx, database_type)

        for idx, playlist in enumerate(playlists, 1):
            if isinstance(playlist, tuple):
                playlist_dict = dict(zip(columns, playlist))
                print(f"DEBUG - Playlist dict keys: {list(playlist_dict.keys())}")
                print(f"DEBUG - Time filter value: {playlist_dict.get('timefilterhours')}")
            else:
                # If it's already a dict, we need to ensure keys are lowercase
                playlist_dict = {k.lower(): v for k, v in playlist.items()}
                print(f"DEBUG - Playlist dict keys: {list(playlist_dict.keys())}")
                print(f"DEBUG - Time filter value: {playlist_dict.get('timefilterhours')}")

            # Ensure timefilterhours is properly set
            if 'timefilterhours' not in playlist_dict and 'TimeFilterHours' in playlist_dict:
                playlist_dict['timefilterhours'] = playlist_dict['TimeFilterHours']

            # Skip Fresh Releases as it's handled separately
            if playlist_dict.get('name') == 'Fresh Releases' and playlist_dict.get('issystemplaylist', playlist_dict.get('issystemplaylist', False)):
                print(f"Skipping Fresh Releases playlist (ID: {playlist_dict.get('playlistid')}) as it's handled separately")
                continue

            print(f"\nProcessing playlist {idx}/{total_playlists}: {playlist_dict.get('name')} (ID: {playlist_dict.get('playlistid')})")
            print(f"UserID: {playlist_dict.get('userid')}")

            try:
                update_playlist_contents(cnx, database_type, playlist_dict)
                print(f"Successfully completed playlist {idx}/{total_playlists}")
            except Exception as e:
                print(f"Error updating playlist {idx}/{total_playlists} ID {playlist_dict.get('playlistid')}: {str(e)}")
                continue

        print(f"Finished processing all {total_playlists} playlists")
        print("=============== PLAYLIST UPDATE COMPLETE ===============\n")
        cnx.commit()

    except Exception as e:
        print(f"Error in update_all_playlists: {str(e)}")
        if hasattr(e, '__traceback__'):
            import traceback
            print(traceback.format_exc())
        cnx.rollback()
    finally:
        cursor.close()

def create_playlist(cnx, database_type, playlist_data):
    """
    Create a new playlist and return its ID
    """
    cursor = cnx.cursor()
    try:
        logging.info(f"Attempting to create playlist with data: {playlist_data}")
        min_duration = playlist_data.min_duration * 60 if playlist_data.min_duration is not None else None
        max_duration = playlist_data.max_duration * 60 if playlist_data.max_duration is not None else None

        # Convert podcast_ids list to appropriate format based on database type
        if database_type == "postgresql":
            podcast_ids = playlist_data.podcast_ids  # PostgreSQL can handle list directly
        else:  # MySQL
            # Always ensure podcast_ids is a list before processing
            if playlist_data.podcast_ids is None:
                podcast_ids = ""
            elif isinstance(playlist_data.podcast_ids, (list, tuple)):
                if len(playlist_data.podcast_ids) == 0:
                    podcast_ids = ""
                else:
                    # Convert list to comma-separated string
                    podcast_ids = ','.join(str(id) for id in playlist_data.podcast_ids)
            else:
                # Handle single value case
                podcast_ids = str(playlist_data.podcast_ids)

        # Create tuple of values for insert and log them
        insert_values = (
            playlist_data.user_id,
            playlist_data.name,
            playlist_data.description,
            podcast_ids,
            playlist_data.include_unplayed,
            playlist_data.include_partially_played,
            playlist_data.include_played,
            min_duration,
            max_duration,
            playlist_data.sort_order,
            playlist_data.group_by_podcast,
            playlist_data.max_episodes,
            playlist_data.icon_name,
            playlist_data.play_progress_min,
            playlist_data.play_progress_max,
            playlist_data.time_filter_hours
        )
        logging.info(f"Insert values: {insert_values}")

        try:
            if database_type == "postgresql":
                cursor.execute("""
                    INSERT INTO "Playlists" (
                        UserID,
                        Name,
                        Description,
                        IsSystemPlaylist,
                        PodcastIDs,
                        IncludeUnplayed,
                        IncludePartiallyPlayed,
                        IncludePlayed,
                        MinDuration,
                        MaxDuration,
                        SortOrder,
                        GroupByPodcast,
                        MaxEpisodes,
                        IconName,
                        PlayProgressMin,
                        PlayProgressMax,
                        TimeFilterHours
                    ) VALUES (
                        %s, %s, %s, FALSE, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s
                    ) RETURNING PlaylistID;
                """, insert_values)

                try:
                    result = cursor.fetchone()
                    logging.info(f"Insert result: {result}")
                    if result is None:
                        raise Exception("No playlist ID returned from insert")
                    # Handle both dict and tuple results
                    if isinstance(result, dict):
                        playlist_id = result['playlistid']
                    else:
                        playlist_id = result[0]
                    cnx.commit()

                    # Get the newly created playlist details to update it
                    # Make sure podcast_ids is always a list for update_playlist_contents
                    update_podcast_ids = playlist_data.podcast_ids
                    if update_podcast_ids is None:
                        update_podcast_ids = []
                    elif not isinstance(update_podcast_ids, (list, tuple)):
                        update_podcast_ids = [update_podcast_ids]

                    playlist_dict = {
                        'playlistid': playlist_id,
                        'userid': playlist_data.user_id,
                        'name': playlist_data.name,
                        'description': playlist_data.description,
                        'issystemplaylist': False,
                        'podcastids': update_podcast_ids,
                        'includeunplayed': playlist_data.include_unplayed,
                        'includepartiallyplayed': playlist_data.include_partially_played,
                        'includeplayed': playlist_data.include_played,
                        'minduration': min_duration,
                        'maxduration': max_duration,
                        'sortorder': playlist_data.sort_order,
                        'groupbypodcast': playlist_data.group_by_podcast,
                        'maxepisodes': playlist_data.max_episodes,
                        'playprogressmin': playlist_data.play_progress_min,
                        'playprogressmax': playlist_data.play_progress_max,
                        'timefilterhours': playlist_data.time_filter_hours
                    }

                    # Update the playlist contents immediately
                    update_playlist_contents(cnx, database_type, playlist_dict)

                    return playlist_id
                except Exception as fetch_e:
                    logging.error(f"Error fetching result: {fetch_e}")
                    raise

            else:  # MySQL
                cursor.execute("""
                    INSERT INTO Playlists (
                        UserID,
                        Name,
                        Description,
                        IsSystemPlaylist,
                        PodcastIDs,
                        IncludeUnplayed,
                        IncludePartiallyPlayed,
                        IncludePlayed,
                        MinDuration,
                        MaxDuration,
                        SortOrder,
                        GroupByPodcast,
                        MaxEpisodes,
                        IconName,
                        PlayProgressMin,
                        PlayProgressMax,
                        TimeFilterHours
                    ) VALUES (
                        %s, %s, %s, FALSE, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s
                    );
                """, insert_values)

                # For MySQL, we need to get the last inserted ID
                playlist_id = cursor.lastrowid
                if playlist_id is None:
                    raise Exception("No playlist ID returned from insert")
                cnx.commit()

                # Get the newly created playlist details to update it
                # Make sure podcast_ids is always a list for update_playlist_contents
                update_podcast_ids = playlist_data.podcast_ids
                if update_podcast_ids is None:
                    update_podcast_ids = []
                elif not isinstance(update_podcast_ids, (list, tuple)):
                    update_podcast_ids = [update_podcast_ids]

                playlist_dict = {
                    'playlistid': playlist_id,
                    'userid': playlist_data.user_id,
                    'name': playlist_data.name,
                    'description': playlist_data.description,
                    'issystemplaylist': False,
                    'podcastids': update_podcast_ids,
                    'includeunplayed': playlist_data.include_unplayed,
                    'includepartiallyplayed': playlist_data.include_partially_played,
                    'includeplayed': playlist_data.include_played,
                    'minduration': min_duration,
                    'maxduration': max_duration,
                    'sortorder': playlist_data.sort_order,
                    'groupbypodcast': playlist_data.group_by_podcast,
                    'maxepisodes': playlist_data.max_episodes,
                    'playprogressmin': playlist_data.play_progress_min,
                    'playprogressmax': playlist_data.play_progress_max,
                    'timefilterhours': playlist_data.time_filter_hours
                }

                # Update the playlist contents immediately
                update_playlist_contents(cnx, database_type, playlist_dict)

                return playlist_id

        except Exception as sql_e:
            logging.error(f"SQL execution error: {sql_e}")
            if hasattr(sql_e, 'pgerror'):
                logging.error(f"PG Error: {sql_e.pgerror}")
            if hasattr(sql_e, 'diag'):
                logging.error(f"Diagnostics: {sql_e.diag.message_detail}")
            raise

    except Exception as e:
        cnx.rollback()
        logging.error(f"Detailed error creating playlist: {str(e)}")
        logging.error(f"Error type: {type(e)}")
        logging.error(f"Error args: {getattr(e, 'args', None)}")
        raise Exception(f"Failed to create playlist: {str(e)}\nPlaylist data: {playlist_data}")
    finally:
        cursor.close()

def delete_playlist(cnx, database_type, user_id, playlist_id):
    """
    Delete a playlist if it belongs to the user and is not a system playlist
    """
    cursor = cnx.cursor()
    try:
        # Check if playlist exists and belongs to user
        if database_type == "postgresql":
            cursor.execute("""
                SELECT IsSystemPlaylist, UserID
                FROM "Playlists"
                WHERE PlaylistID = %s
            """, (playlist_id,))
        else:  # MySQL
            cursor.execute("""
                SELECT IsSystemPlaylist, UserID
                FROM Playlists
                WHERE PlaylistID = %s
            """, (playlist_id,))

        result = cursor.fetchone()
        if not result:
            raise Exception("Playlist not found")

        # Handle different result formats (tuple vs dict)
        if isinstance(result, tuple):
            is_system = result[0]
            playlist_user_id = result[1]
        else:
            # For dict results, check for both capitalized and lowercase keys
            if 'issystemplaylist' in result:
                is_system = result['issystemplaylist']
            else:
                is_system = result['IsSystemPlaylist']

            if 'userid' in result:
                playlist_user_id = result['userid']
            else:
                playlist_user_id = result['UserID']

        if is_system:
            raise Exception("Cannot delete system playlists")
        if playlist_user_id != user_id:
            raise Exception("Unauthorized to delete this playlist")

        # Delete the playlist
        if database_type == "postgresql":
            cursor.execute("""
                DELETE FROM "Playlists"
                WHERE PlaylistID = %s
            """, (playlist_id,))
        else:  # MySQL
            cursor.execute("""
                DELETE FROM Playlists
                WHERE PlaylistID = %s
            """, (playlist_id,))

        cnx.commit()

    except Exception as e:
        cnx.rollback()
        raise Exception(f"Failed to delete playlist: {str(e)}")
    finally:
        cursor.close()

def normalize_playlist_data(playlist_record):
    """Normalize playlist data regardless of whether it's a tuple or dict."""
    if isinstance(playlist_record, tuple):
        result = {
            'playlist_id': playlist_record[0],
            'user_id': playlist_record[1],
            'name': playlist_record[2],
            'description': playlist_record[3],
            'is_system_playlist': playlist_record[4],
            'podcast_ids': playlist_record[5],
            'include_unplayed': playlist_record[6],
            'include_partially_played': playlist_record[7],
            'include_played': playlist_record[8],
            'min_duration': playlist_record[9],
            'max_duration': playlist_record[10],
            'sort_order': playlist_record[11],
            'group_by_podcast': playlist_record[12],
            'max_episodes': playlist_record[13],
            'last_updated': playlist_record[14],
            'created': playlist_record[15],
            'icon_name': playlist_record[16],
            'episode_count': playlist_record[17]
        }
    else:
        result = {
            'playlist_id': playlist_record['playlistid'],
            'user_id': playlist_record['userid'],
            'name': playlist_record['name'],
            'description': playlist_record['description'],
            'is_system_playlist': playlist_record['issystemplaylist'],
            'podcast_ids': playlist_record['podcastids'],
            'include_unplayed': playlist_record['includeunplayed'],
            'include_partially_played': playlist_record['includepartiallyplayed'],
            'include_played': playlist_record['includeplayed'],
            'min_duration': playlist_record['minduration'],
            'max_duration': playlist_record['maxduration'],
            'sort_order': playlist_record['sortorder'],
            'group_by_podcast': playlist_record['groupbypodcast'],
            'max_episodes': playlist_record['maxepisodes'],
            'last_updated': playlist_record['lastupdated'],
            'created': playlist_record['created'],
            'icon_name': playlist_record['iconname'],
            'episode_count': playlist_record['episode_count']
        }

    # Convert null values to appropriate string representations or default values
    if result['last_updated'] is None:
        result['last_updated'] = ""

    if result['created'] is None:
        result['created'] = ""

    if result['icon_name'] is None:
        result['icon_name'] = ""  # Or a default icon name like "ph-playlist"

    # Handle episode_count - ensure it's an integer
    if isinstance(result['episode_count'], str):  # It's coming back as a timestamp string
        result['episode_count'] = 0

    return result

def normalize_preview_episode(episode_record):
    """Normalize episode preview data regardless of whether it's a tuple or dict."""
    if isinstance(episode_record, tuple):
        return {
            'title': episode_record[0],
            'artwork': episode_record[1]
        }
    return {
        'title': episode_record.get('episodetitle', episode_record.get('EpisodeTitle')),
        'artwork': episode_record.get('episodeartwork', episode_record.get('EpisodeArtwork'))
    }

def get_playlists(cnx, database_type, user_id):
    """
    Get all playlists (system playlists and user's custom playlists)
    Returns consistently formatted dict results regardless of database response format
    """
    try:
        if database_type == "postgresql":
            # Create a cursor that returns dictionaries for PostgreSQL
            cursor = cnx.cursor(row_factory=psycopg.rows.dict_row)

            # PostgreSQL query
            cursor.execute("""
                WITH filtered_episodes AS (
                    SELECT pc.PlaylistID, pc.EpisodeID
                    FROM "PlaylistContents" pc
                    JOIN "Episodes" e ON pc.EpisodeID = e.EpisodeID
                    JOIN "Podcasts" p ON e.PodcastID = p.PodcastID
                    WHERE p.UserID = %s
                )
                SELECT
                    p.*,
                    COUNT(fe.EpisodeID)::INTEGER as episode_count,
                    p.IconName as icon_name
                FROM "Playlists" p
                LEFT JOIN filtered_episodes fe ON p.PlaylistID = fe.PlaylistID
                WHERE p.IsSystemPlaylist = TRUE
                    OR p.UserID = %s
                GROUP BY p.PlaylistID
                ORDER BY p.IsSystemPlaylist DESC, p.Name ASC
            """, (user_id, user_id))

            playlists = cursor.fetchall()

        else:  # MySQL
            # Create a cursor for MySQL
            cursor = cnx.cursor(dictionary=True)

            # MySQL query
            cursor.execute("""
                WITH filtered_episodes AS (
                    SELECT pc.PlaylistID, pc.EpisodeID
                    FROM PlaylistContents pc
                    JOIN Episodes e ON pc.EpisodeID = e.EpisodeID
                    JOIN Podcasts p ON e.PodcastID = p.PodcastID
                    WHERE p.UserID = %s
                )
                SELECT
                    p.*,
                    COUNT(fe.EpisodeID) as episode_count,
                    p.IconName as icon_name
                FROM Playlists p
                LEFT JOIN filtered_episodes fe ON p.PlaylistID = fe.PlaylistID
                WHERE p.IsSystemPlaylist = TRUE
                    OR p.UserID = %s
                GROUP BY p.PlaylistID
                ORDER BY p.IsSystemPlaylist DESC, p.Name ASC
            """, (user_id, user_id))

            playlists = cursor.fetchall()

        playlist_list = []
        for playlist_record in playlists:
            # Get the podcast_ids field
            raw_podcast_ids = playlist_record.get('podcastids', playlist_record.get('PodcastIDs'))

            # Process podcast_ids based on the data type and database
            processed_podcast_ids = None
            if raw_podcast_ids is not None:
                if database_type == "postgresql":
                    # PostgreSQL returns a list directly
                    processed_podcast_ids = raw_podcast_ids
                else:
                    # MySQL: Handle different formats
                    import json

                    # If it's a single integer, wrap it in a list
                    if isinstance(raw_podcast_ids, int):
                        processed_podcast_ids = [raw_podcast_ids]
                    # If it's a single string that can be parsed as an integer
                    elif isinstance(raw_podcast_ids, str) and raw_podcast_ids.strip().isdigit():
                        processed_podcast_ids = [int(raw_podcast_ids.strip())]
                    # If it's a string, try to parse it
                    elif isinstance(raw_podcast_ids, str):
                        try:
                            # Try to parse as JSON string
                            processed_podcast_ids = json.loads(raw_podcast_ids)
                        except json.JSONDecodeError:
                            # If that fails, try to handle quoted strings
                            try:
                                # Strip quotes if present
                                cleaned = raw_podcast_ids.strip('"\'')
                                # Manual parsing for array-like strings
                                if cleaned.startswith('[') and cleaned.endswith(']'):
                                    items = cleaned[1:-1].split(',')
                                    processed_podcast_ids = [int(item.strip()) for item in items if item.strip()]
                                else:
                                    # For comma-separated list without brackets
                                    processed_podcast_ids = [int(item.strip()) for item in cleaned.split(',') if item.strip()]
                            except (ValueError, AttributeError):
                                # Last resort: empty list
                                processed_podcast_ids = []
                    else:
                        # If it's none of the above, keep as is
                        processed_podcast_ids = raw_podcast_ids

                # Make sure we always return a list
                if processed_podcast_ids is not None and not isinstance(processed_podcast_ids, list):
                    processed_podcast_ids = [processed_podcast_ids]

            # Normalize field names to handle both PostgreSQL's lowercase and MySQL's capitalized names
            playlist_dict = {
                'playlist_id': playlist_record.get('playlistid', playlist_record.get('PlaylistID')),
                'user_id': playlist_record.get('userid', playlist_record.get('UserID')),
                'name': playlist_record.get('name', playlist_record.get('Name')),
                'description': playlist_record.get('description', playlist_record.get('Description')),
                'is_system_playlist': bool(playlist_record.get('issystemplaylist', playlist_record.get('IsSystemPlaylist'))),
                'podcast_ids': processed_podcast_ids,  # Use our processed value
                'include_unplayed': bool(playlist_record.get('includeunplayed', playlist_record.get('IncludeUnplayed'))),
                'include_partially_played': bool(playlist_record.get('includepartiallyplayed', playlist_record.get('IncludePartiallyPlayed'))),
                'include_played': bool(playlist_record.get('includeplayed', playlist_record.get('IncludePlayed'))),
                'min_duration': playlist_record.get('minduration', playlist_record.get('MinDuration')),
                'max_duration': playlist_record.get('maxduration', playlist_record.get('MaxDuration')),
                'sort_order': playlist_record.get('sortorder', playlist_record.get('SortOrder')),
                'group_by_podcast': bool(playlist_record.get('groupbypodcast', playlist_record.get('GroupByPodcast'))),
                'max_episodes': playlist_record.get('maxepisodes', playlist_record.get('MaxEpisodes')),
                'last_updated': playlist_record.get('lastupdated', playlist_record.get('LastUpdated', "")),
                'created': playlist_record.get('created', playlist_record.get('Created', "")),
                'icon_name': playlist_record.get('iconname', playlist_record.get('IconName', "")),
                'episode_count': int(playlist_record.get('episode_count', 0) or 0)
            }

            # Get preview episodes with error handling
            try:
                if database_type == "postgresql":
                    # Use dict cursor for PostgreSQL
                    preview_cursor = cnx.cursor(row_factory=psycopg.rows.dict_row)
                    preview_cursor.execute("""
                        SELECT e.EpisodeTitle as episodetitle, e.EpisodeArtwork as episodeartwork
                        FROM "PlaylistContents" pc
                        JOIN "Episodes" e ON pc.EpisodeID = e.EpisodeID
                        JOIN "Podcasts" p ON e.PodcastID = p.PodcastID
                        WHERE pc.PlaylistID = %s
                        AND p.UserID = %s
                        ORDER BY pc.Position
                        LIMIT 3
                    """, (playlist_dict['playlist_id'], user_id))
                else:  # MySQL
                    # Use dict cursor for MySQL
                    preview_cursor = cnx.cursor(dictionary=True)
                    preview_cursor.execute("""
                        SELECT e.EpisodeTitle as episodetitle, e.EpisodeArtwork as episodeartwork
                        FROM PlaylistContents pc
                        JOIN Episodes e ON pc.EpisodeID = e.EpisodeID
                        JOIN Podcasts p ON e.PodcastID = p.PodcastID
                        WHERE pc.PlaylistID = %s
                        AND p.UserID = %s
                        ORDER BY pc.Position
                        LIMIT 3
                    """, (playlist_dict['playlist_id'], user_id))

                preview_episodes = preview_cursor.fetchall()

                # Normalize field names for preview episodes
                playlist_dict['preview_episodes'] = []
                for ep in preview_episodes:
                    # Handle both PostgreSQL and MySQL column naming
                    title = ep.get('episodetitle', ep.get('EpisodeTitle', ''))
                    artwork = ep.get('episodeartwork', ep.get('EpisodeArtwork', ''))
                    playlist_dict['preview_episodes'].append({
                        'title': title,
                        'artwork': artwork
                    })

                preview_cursor.close()
            except Exception as e:
                print(f"Error fetching preview episodes for playlist {playlist_dict['playlist_id']}: {e}")
                playlist_dict['preview_episodes'] = []

            playlist_list.append(playlist_dict)

        return playlist_list
    except Exception as e:
        raise Exception(f"Failed to get playlists: {str(e)}")
    finally:
        if 'cursor' in locals():
            cursor.close()

def normalize_episode(episode):
    """Normalize episode data regardless of tuple or dict format"""
    if isinstance(episode, tuple):
        return {
            'episodeid': episode[0],
            'episodetitle': episode[1],
            'episodedescription': episode[2],
            'episodeartwork': episode[3],
            'episodepubdate': episode[4],
            'episodeurl': episode[5],
            'episodeduration': episode[6],
            'listenduration': episode[7],
            'completed': bool(episode[8]) if episode[8] is not None else False,
            'saved': bool(episode[9]) if episode[9] is not None else False,
            'queued': bool(episode[10]) if episode[10] is not None else False,
            'is_youtube': bool(episode[11]) if episode[11] is not None else False,
            'downloaded': bool(episode[12]) if episode[12] is not None else False,
            'podcastname': episode[13]
        }

    # For dict case, map field names explicitly
    field_mappings = {
        'episodeid': ['episodeid', 'EpisodeID'],
        'episodetitle': ['episodetitle', 'EpisodeTitle'],
        'episodedescription': ['episodedescription', 'EpisodeDescription'],
        'episodeartwork': ['episodeartwork', 'EpisodeArtwork'],
        'episodepubdate': ['episodepubdate', 'EpisodePubDate'],
        'episodeurl': ['episodeurl', 'EpisodeURL'],
        'episodeduration': ['episodeduration', 'EpisodeDuration'],
        'listenduration': ['listenduration', 'ListenDuration'],
        'completed': bool(episode['completed']) if episode['completed'] is not None else False,
        'saved': bool(episode['saved']) if episode['saved'] is not None else False,
        'queued': bool(episode['queued']) if episode['queued'] is not None else False,
        'is_youtube': bool(episode.get('isyoutube', False)),  # Use get() with default False
        'downloaded': bool(episode['downloaded']) if episode['downloaded'] is not None else False,
        'podcastname': ['podcastname', 'PodcastName']
    }

    result = {}
    for field, possible_keys in field_mappings.items():
        # Try all possible keys for each field
        value = None
        for key in possible_keys:
            value = episode.get(key)
            if value is not None:
                break

        # Handle booleans
        if field in ['completed', 'saved', 'queued', 'is_youtube', 'downloaded']:
            value = value or False

        result[field] = value

    return result

def normalize_playlist_info(playlist_info):
    """Normalize playlist info data regardless of tuple or dict format"""
    if isinstance(playlist_info, tuple):
        return {
            'name': playlist_info[0],
            'description': playlist_info[1],
            'episode_count': playlist_info[2],
            'icon_name': playlist_info[3]
        }
    # For dict case, first try lowercase keys (most common)
    name = playlist_info.get('name')
    description = playlist_info.get('description')
    episode_count = playlist_info.get('episode_count')
    icon_name = playlist_info.get('iconname')  # Note: this comes back as 'iconname' not 'icon_name'

    # If any are None, try uppercase keys as fallback
    if name is None:
        name = playlist_info.get('Name')
    if description is None:
        description = playlist_info.get('Description')
    if episode_count is None:
        episode_count = playlist_info.get('EpisodeCount')
    if icon_name is None:
        icon_name = playlist_info.get('IconName')

    return {
        'name': name,
        'description': description,
        'episode_count': episode_count,
        'icon_name': icon_name
    }

def get_playlist_episodes(cnx, database_type, user_id, playlist_id):
    """
    Get all episodes in a playlist, applying the playlist's filters
    Returns both playlist info and episodes in format matching Rust structs
    """
    print(f"Starting playlist episodes fetch for playlist_id={playlist_id}")
    cursor = cnx.cursor()
    try:
        # Get playlist info
        # Get playlist info with user-specific episode count
        if database_type == "postgresql":
            cursor.execute("""
                SELECT
                    p.Name,
                    p.Description,
                    (SELECT COUNT(*)
                     FROM "PlaylistContents" pc
                     JOIN "Episodes" e ON pc.EpisodeID = e.EpisodeID
                     JOIN "Podcasts" pod ON e.PodcastID = pod.PodcastID
                     LEFT JOIN "UserEpisodeHistory" h ON e.EpisodeID = h.EpisodeID AND h.UserID = %s
                     WHERE pc.PlaylistID = p.PlaylistID
                     AND (p.IsSystemPlaylist = FALSE OR
                          (p.IsSystemPlaylist = TRUE AND
                           (h.EpisodeID IS NOT NULL OR pod.UserID = %s)))
                    ) as episode_count,
                    p.IconName,
                    p.IsSystemPlaylist
                FROM "Playlists" p
                WHERE p.PlaylistID = %s AND (p.UserID = %s OR p.IsSystemPlaylist = TRUE)
                GROUP BY p.PlaylistID, p.Name, p.Description, p.IconName, p.IsSystemPlaylist
            """, (user_id, user_id, playlist_id, user_id))
            # Get playlist info with user-specific episode count
        else:  # MySQL
            cursor.execute("""
                SELECT
                    p.Name,
                    p.Description,
                    (SELECT COUNT(*)
                        FROM PlaylistContents pc
                        JOIN Episodes e ON pc.EpisodeID = e.EpisodeID
                        JOIN Podcasts pod ON e.PodcastID = pod.PodcastID
                        LEFT JOIN UserEpisodeHistory h ON e.EpisodeID = h.EpisodeID AND h.UserID = %s
                        WHERE pc.PlaylistID = p.PlaylistID
                        AND (p.IsSystemPlaylist = 0 OR
                            (p.IsSystemPlaylist = 1 AND
                            (h.EpisodeID IS NOT NULL OR pod.UserID = %s)))
                    ) as episode_count,
                    p.IconName,
                    p.IsSystemPlaylist
                FROM Playlists p
                WHERE p.PlaylistID = %s AND (p.UserID = %s OR p.IsSystemPlaylist = 1)
            """, (user_id, user_id, playlist_id, user_id))

        playlist_info = cursor.fetchone()

        if not playlist_info:
            raise Exception(f"Playlist {playlist_id} not found or access denied")

        # Handle both tuple and dict formats for playlist info
        is_system_playlist = False
        if isinstance(playlist_info, tuple):
            normalized_info = {
                'name': playlist_info[0],
                'description': playlist_info[1],
                'episode_count': playlist_info[2],
                'icon_name': playlist_info[3]
            }
            is_system_playlist = playlist_info[4]
        else:
            # Handle both upper and lower case keys
            normalized_info = {
                'name': playlist_info.get('name') or playlist_info.get('Name'),
                'description': playlist_info.get('description') or playlist_info.get('Description'),
                'episode_count': playlist_info.get('episode_count') or playlist_info.get('episode_count'),
                'icon_name': playlist_info.get('iconname') or playlist_info.get('IconName')
            }
            is_system_playlist = playlist_info.get('issystemplaylist') or playlist_info.get('IsSystemPlaylist')

        print(f"Debug - playlist_info type: {type(playlist_info)}")
        print(f"Debug - playlist_info content: {playlist_info}")
        print(f"Debug - normalized playlist info: {normalized_info}")
        print(f"Debug - is_system_playlist: {is_system_playlist}")

        # Get playlist settings
        if database_type == "postgresql":
            cursor.execute("""
                SELECT
                    IncludeUnplayed,
                    IncludePartiallyPlayed,
                    IncludePlayed,
                    MinDuration,
                    MaxDuration,
                    SortOrder,
                    GroupByPodcast,
                    MaxEpisodes,
                    PodcastIDs
                FROM "Playlists"
                WHERE PlaylistID = %s AND (UserID = %s OR IsSystemPlaylist = TRUE)
            """, (playlist_id, user_id))
        else:  # MySQL
            cursor.execute("""
                SELECT
                    IncludeUnplayed,
                    IncludePartiallyPlayed,
                    IncludePlayed,
                    MinDuration,
                    MaxDuration,
                    SortOrder,
                    GroupByPodcast,
                    MaxEpisodes,
                    PodcastIDs
                FROM Playlists
                WHERE PlaylistID = %s AND (UserID = %s OR IsSystemPlaylist = 1)
            """, (playlist_id, user_id))

        playlist_settings = cursor.fetchone()
        if isinstance(playlist_settings, dict):
            # Handle both uppercase and lowercase keys
            settings = [
                playlist_settings.get('includeunplayed', playlist_settings.get('IncludeUnplayed')),
                playlist_settings.get('includepartiallyplayed', playlist_settings.get('IncludePartiallyPlayed')),
                playlist_settings.get('includeplayed', playlist_settings.get('IncludePlayed')),
                playlist_settings.get('minduration', playlist_settings.get('MinDuration')),
                playlist_settings.get('maxduration', playlist_settings.get('MaxDuration')),
                playlist_settings.get('sortorder', playlist_settings.get('SortOrder')),
                playlist_settings.get('groupbypodcast', playlist_settings.get('GroupByPodcast')),
                playlist_settings.get('maxepisodes', playlist_settings.get('MaxEpisodes')),
                playlist_settings.get('podcastids', playlist_settings.get('PodcastIDs'))
            ]
        else:  # tuple
            settings = playlist_settings
        print(f"Debug - playlist_settings type: {type(playlist_settings)}")
        print(f"Debug - playlist_settings content: {playlist_settings}")

        (include_unplayed, include_partially_played, include_played,
         min_duration, max_duration, sort_order, group_by_podcast,
         max_episodes, podcast_ids) = settings

        # Build episode query with appropriate table names for each database
        if database_type == "postgresql":
            query = """
                SELECT DISTINCT
                    e.EpisodeID,
                    e.EpisodeTitle,
                    e.EpisodeDescription,
                    e.EpisodeArtwork,
                    e.EpisodePubDate,
                    e.EpisodeURL,
                    e.EpisodeDuration,
                    el.ListenDuration as ListenDuration,
                    CASE
                        WHEN el.ListenDuration >= e.EpisodeDuration THEN TRUE
                        ELSE FALSE
                    END as Completed,
                    es.SaveID IS NOT NULL as Saved,
                    eq.QueueID IS NOT NULL as Queued,
                    eq.is_youtube as IsYouTube,
                    ed.DownloadID IS NOT NULL as Downloaded,
                    p.PodcastName
                FROM "PlaylistContents" pc
                JOIN "Episodes" e ON pc.EpisodeID = e.EpisodeID
                JOIN "Podcasts" p ON e.PodcastID = p.PodcastID
                LEFT JOIN "UserEpisodeHistory" el ON e.EpisodeID = el.EpisodeID AND el.UserID = %s
                LEFT JOIN "SavedEpisodes" es ON e.EpisodeID = es.EpisodeID AND es.UserID = %s
                LEFT JOIN "EpisodeQueue" eq ON e.EpisodeID = eq.EpisodeID AND eq.UserID = %s
                LEFT JOIN "DownloadedEpisodes" ed ON e.EpisodeID = ed.EpisodeID AND ed.UserID = %s
                WHERE pc.PlaylistID = %s
                AND (p.UserID = %s OR NOT %s)
            """
        else:  # MySQL
            query = """
                SELECT DISTINCT
                    e.EpisodeID,
                    e.EpisodeTitle,
                    e.EpisodeDescription,
                    e.EpisodeArtwork,
                    e.EpisodePubDate,
                    e.EpisodeURL,
                    e.EpisodeDuration,
                    el.ListenDuration as ListenDuration,
                    CASE
                        WHEN el.ListenDuration >= e.EpisodeDuration THEN 1
                        ELSE 0
                    END as Completed,
                    es.SaveID IS NOT NULL as Saved,
                    eq.QueueID IS NOT NULL as Queued,
                    eq.is_youtube as IsYouTube,
                    ed.DownloadID IS NOT NULL as Downloaded,
                    p.PodcastName
                FROM PlaylistContents pc
                JOIN Episodes e ON pc.EpisodeID = e.EpisodeID
                JOIN Podcasts p ON e.PodcastID = p.PodcastID
                LEFT JOIN UserEpisodeHistory el ON e.EpisodeID = el.EpisodeID AND el.UserID = %s
                LEFT JOIN SavedEpisodes es ON e.EpisodeID = es.EpisodeID AND es.UserID = %s
                LEFT JOIN EpisodeQueue eq ON e.EpisodeID = eq.EpisodeID AND eq.UserID = %s
                LEFT JOIN DownloadedEpisodes ed ON e.EpisodeID = ed.EpisodeID AND ed.UserID = %s
                WHERE pc.PlaylistID = %s
                AND (p.UserID = %s OR NOT %s)
            """
        params = [user_id, user_id, user_id, user_id, playlist_id, user_id, is_system_playlist]

        # Add sorting logic
        if sort_order == "date_desc":
            query += " ORDER BY e.EpisodePubDate DESC"
        elif sort_order == "date_asc":
            query += " ORDER BY e.EpisodePubDate ASC"
        elif sort_order == "duration_desc":
            query += " ORDER BY e.EpisodeDuration DESC"
        elif sort_order == "duration_asc":
            query += " ORDER BY e.EpisodeDuration ASC"

        # Add limit if specified
        if max_episodes:
            query += " LIMIT %s"
            params.append(max_episodes)

        print(f"Debug - final query: {query}")
        print(f"Debug - final params: {params}")

        cursor.execute(query, tuple(params))
        episodes = cursor.fetchall()
        print(f"Debug - episodes type: {type(episodes)}")
        print(f"Debug - first episode content: {episodes[0] if episodes else None}")
        print(f"Debug - number of episodes: {len(episodes)}")

        # Normalize all episodes
        episode_list = []
        for episode in episodes:
            if isinstance(episode, tuple):
                episode_dict = {
                    'episodeid': episode[0],
                    'episodetitle': episode[1],
                    'episodedescription': episode[2],
                    'episodeartwork': episode[3],
                    'episodepubdate': episode[4],
                    'episodeurl': episode[5],
                    'episodeduration': episode[6],
                    'listenduration': episode[7],
                    'completed': bool(episode[8]) if episode[8] is not None else False,
                    'saved': bool(episode[9]) if episode[9] is not None else False,
                    'queued': bool(episode[10]) if episode[10] is not None else False,
                    'is_youtube': bool(episode[11]) if episode[11] is not None else False,
                    'downloaded': bool(episode[12]) if episode[12] is not None else False,
                    'podcastname': episode[13]
                }
            else:
                # Handle both upper and lower case dictionary keys
                episode_dict = {
                    'episodeid': episode.get('episodeid', episode.get('EpisodeID')),
                    'episodetitle': episode.get('episodetitle', episode.get('EpisodeTitle')),
                    'episodedescription': episode.get('episodedescription', episode.get('EpisodeDescription')),
                    'episodeartwork': episode.get('episodeartwork', episode.get('EpisodeArtwork')),
                    'episodepubdate': episode.get('episodepubdate', episode.get('EpisodePubDate')),
                    'episodeurl': episode.get('episodeurl', episode.get('EpisodeURL')),
                    'episodeduration': episode.get('episodeduration', episode.get('EpisodeDuration')),
                    'listenduration': episode.get('listenduration', episode.get('ListenDuration')),
                    'completed': bool(episode.get('completed', episode.get('Completed'))) if episode.get('completed', episode.get('Completed')) is not None else False,
                    'saved': bool(episode.get('saved', episode.get('Saved'))) if episode.get('saved', episode.get('Saved')) is not None else False,
                    'queued': bool(episode.get('queued', episode.get('Queued'))) if episode.get('queued', episode.get('Queued')) is not None else False,
                    'is_youtube': bool(episode.get('isyoutube', episode.get('IsYouTube'))) if episode.get('isyoutube', episode.get('IsYouTube')) is not None else False,
                    'downloaded': bool(episode.get('downloaded', episode.get('Downloaded'))) if episode.get('downloaded', episode.get('Downloaded')) is not None else False,
                    'podcastname': episode.get('podcastname', episode.get('PodcastName'))
                }
            episode_list.append(episode_dict)

        # Return directly matching Rust struct - no extra nesting
        print(f"Debug - final response structure: {dict(playlist_info=normalized_info, episodes=episode_list)}")
        return {
            "playlist_info": normalized_info,
            "episodes": episode_list
        }

    except Exception as e:
        raise Exception(f"Failed to get playlist episodes: {str(e)}")
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
    """Get the GPodder settings for a user with improved error handling"""
    import logging

    logger = logging.getLogger(__name__)

    # Check if cnx is a valid connection object
    if not hasattr(cnx, 'cursor'):
        logger.error(f"Invalid database connection object: {type(cnx)}")
        return {}

    cursor = cnx.cursor()
    try:
        query = (
            'SELECT GpodderUrl, GpodderToken, GpodderLoginName FROM "Users" WHERE UserID = %s' if database_type == "postgresql" else
            "SELECT GpodderUrl, GpodderToken, GpodderLoginName FROM Users WHERE UserID = %s"
        )
        cursor.execute(query, (user_id,))
        result = cursor.fetchone()

        # Ensure result is consistent
        if result:
            if isinstance(result, tuple):
                # Convert tuple result to a dictionary
                result = {
                    "gpodderurl": result[0],
                    "gpoddertoken": result[1],
                    "gpodderloginname": result[2]
                }
            elif isinstance(result, dict):
                # Normalize keys to lower case if necessary
                result = {k.lower(): v for k, v in result.items()}
        else:
            result = {}

        # Apply lowercase keys if needed
        if 'lowercase_keys' in globals():
            return lowercase_keys(result)
        return result
    except Exception as e:
        logger.error(f"Error in get_gpodder_settings: {str(e)}")
        return {}
    finally:
        cursor.close()




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
    """Remove GPodder sync settings for a user"""
    import logging
    logger = logging.getLogger(__name__)

    cursor = cnx.cursor()
    try:
        # First delete any device records
        if database_type == "postgresql":
            devices_query = 'DELETE FROM "GpodderDevices" WHERE UserID = %s'
            sync_state_query = 'DELETE FROM "GpodderSyncState" WHERE UserID = %s'
        else:
            devices_query = "DELETE FROM GpodderDevices WHERE UserID = %s"
            sync_state_query = "DELETE FROM GpodderSyncState WHERE UserID = %s"

        cursor.execute(devices_query, (user_id,))
        cursor.execute(sync_state_query, (user_id,))

        # Then clear GPodder settings from user record
        if database_type == "postgresql":
            user_query = '''
                UPDATE "Users"
                SET GpodderUrl = '', GpodderLoginName = '', GpodderToken = '', Pod_Sync_Type = 'None'
                WHERE UserID = %s
            '''
        else:
            user_query = '''
                UPDATE Users
                SET GpodderUrl = '', GpodderLoginName = '', GpodderToken = '', Pod_Sync_Type = 'None'
                WHERE UserID = %s
            '''

        cursor.execute(user_query, (user_id,))
        cnx.commit()
        return True
    except Exception as e:
        logger.error(f"Error removing GPodder settings: {e}")
        cnx.rollback()
        return False
    finally:
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
    # Query to select users with either external sync configuration OR internal gpodder API enabled
    if database_type == "postgresql":
        query = """
            SELECT UserID, GpodderUrl, GpodderToken, GpodderLoginName, Pod_Sync_Type
            FROM "Users"
            WHERE (GpodderUrl <> '' AND GpodderToken <> '' AND GpodderLoginName <> '')
               OR Pod_Sync_Type IN ('gpodder', 'both')
        """
    else:  # MySQL or MariaDB
        query = """
            SELECT UserID, GpodderUrl, GpodderToken, GpodderLoginName, Pod_Sync_Type
            FROM Users
            WHERE (GpodderUrl <> '' AND GpodderToken <> '' AND GpodderLoginName <> '')
               OR Pod_Sync_Type IN ('gpodder', 'both')
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
    import requests

    try:
        # Decrypt the token
        encryption_key = get_encryption_key(cnx, database_type)
        encryption_key_bytes = base64.b64decode(encryption_key)
        cipher_suite = Fernet(encryption_key_bytes)

        if encrypted_gpodder_token is not None:
            decrypted_token_bytes = cipher_suite.decrypt(encrypted_gpodder_token.encode())
            gpodder_token = decrypted_token_bytes.decode()
        else:
            gpodder_token = None

        # Create a session for cookie-based auth
        session = requests.Session()
        auth = HTTPBasicAuth(gpodder_login, gpodder_token)

        # Try to establish a session first (for PodFetch)
        try:
            login_url = f"{gpodder_url}/api/2/auth/{gpodder_login}/login.json"
            login_response = session.post(login_url, auth=auth)
            login_response.raise_for_status()
            print("Session login successful for podcast add")

            # Use the session to add the podcast
            url = f"{gpodder_url}/api/2/subscriptions/{gpodder_login}/{device_id}.json"
            data = {
                "add": [podcast_url],
                "remove": []
            }
            headers = {
                "Content-Type": "application/json"
            }
            response = session.post(url, json=data, headers=headers)
            response.raise_for_status()
            print(f"Podcast added to oPodSync successfully using session: {response.text}")
            return response.json()

        except Exception as e:
            print(f"Session auth failed, trying basic auth: {str(e)}")

            # Fall back to basic auth
            url = f"{gpodder_url}/api/2/subscriptions/{gpodder_login}/{device_id}.json"
            auth = HTTPBasicAuth(gpodder_login, gpodder_token)
            data = {
                "add": [podcast_url],
                "remove": []
            }
            headers = {
                "Content-Type": "application/json"
            }
            response = requests.post(url, json=data, headers=headers, auth=auth)
            response.raise_for_status()
            print(f"Podcast added to oPodSync successfully with basic auth: {response.text}")
            return response.json()

    except Exception as e:
        print(f"Failed to add podcast to oPodSync: {e}")
        print(f"Response body: {getattr(response, 'text', 'No response')}")
        return None


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
    import requests

    try:
        # Decrypt the token
        encryption_key = get_encryption_key(cnx, database_type)
        encryption_key_bytes = base64.b64decode(encryption_key)
        cipher_suite = Fernet(encryption_key_bytes)

        if encrypted_gpodder_token is not None:
            decrypted_token_bytes = cipher_suite.decrypt(encrypted_gpodder_token.encode())
            gpodder_token = decrypted_token_bytes.decode()
        else:
            gpodder_token = None

        # Create a session for cookie-based auth
        session = requests.Session()
        auth = HTTPBasicAuth(gpodder_login, gpodder_token)

        # Try to establish a session first (for PodFetch)
        try:
            login_url = f"{gpodder_url}/api/2/auth/{gpodder_login}/login.json"
            login_response = session.post(login_url, auth=auth)
            login_response.raise_for_status()
            print("Session login successful for podcast removal")

            # Use the session to remove the podcast
            url = f"{gpodder_url}/api/2/subscriptions/{gpodder_login}/{device_id}.json"
            data = {
                "add": [],
                "remove": [podcast_url]
            }
            headers = {
                "Content-Type": "application/json"
            }
            response = session.post(url, json=data, headers=headers)
            response.raise_for_status()
            print(f"Podcast removed from oPodSync successfully using session: {response.text}")
            return response.json()

        except Exception as e:
            print(f"Session auth failed, trying basic auth: {str(e)}")

            # Fall back to basic auth
            url = f"{gpodder_url}/api/2/subscriptions/{gpodder_login}/{device_id}.json"
            auth = HTTPBasicAuth(gpodder_login, gpodder_token)
            data = {
                "add": [],
                "remove": [podcast_url]
            }
            headers = {
                "Content-Type": "application/json"
            }
            response = requests.post(url, json=data, headers=headers, auth=auth)
            response.raise_for_status()
            print(f"Podcast removed from oPodSync successfully with basic auth: {response.text}")
            return response.json()

    except Exception as e:
        print(f"Failed to remove podcast from oPodSync: {e}")
        print(f"Response body: {getattr(response, 'text', 'No response')}")
        return None

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

def get_user_devices(cnx, database_type, user_id):
    """Get all GPodder devices for a user with proper datetime conversion"""
    import logging
    logger = logging.getLogger(__name__)
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            query = '''
                SELECT DeviceID, DeviceName, DeviceType, DeviceCaption, LastSync, IsActive, IsDefault
                FROM "GpodderDevices"
                WHERE UserID = %s
            '''
        else:
            query = '''
                SELECT DeviceID, DeviceName, DeviceType, DeviceCaption, LastSync, IsActive, IsDefault
                FROM GpodderDevices
                WHERE UserID = %s
            '''
        cursor.execute(query, (user_id,))
        devices = []
        for row in cursor.fetchall():
            if isinstance(row, dict):
                # Handle dict-style result (depends on the driver)
                # Convert datetime to string
                last_sync = row["lastsync"].isoformat() if row["lastsync"] else None
                device = {
                    "id": row["deviceid"],
                    "name": row["devicename"],
                    "type": row["devicetype"],
                    "caption": row["devicecaption"],
                    "last_sync": last_sync,
                    "is_active": row["isactive"],
                    "is_remote": False,
                    "is_default": row["isdefault"]
                }
            else:
                # Handle tuple-style result
                # Convert datetime to string
                last_sync = row[4].isoformat() if row[4] else None
                device = {
                    "id": row[0],
                    "name": row[1],
                    "type": row[2],
                    "caption": row[3],
                    "last_sync": last_sync,
                    "is_active": row[5],
                    "is_remote": False,
                    "is_default": row[6] if len(row) > 6 else False
                }
            devices.append(device)
        return devices
    except Exception as e:
        logger.error(f"Error getting user devices: {e}")
        return []
    finally:
        cursor.close()

# Add this to your database_functions/functions.py file

def handle_remote_device(cnx, database_type, user_id, device_name):
    """
    Handles setting a remote device (with negative ID) as default by creating
    a local representation or using an existing one.

    Args:
        cnx: Database connection
        database_type: Type of database ('postgresql' or other)
        user_id: User ID
        device_name: Name of the remote device

    Returns:
        tuple: (success: bool, message: str, device_id: int)
    """
    import logging
    logger = logging.getLogger(__name__)

    try:
        # First check if device exists - if so, set it as default
        existing_id = find_device_by_name(cnx, database_type, user_id, device_name)

        if existing_id:
            # Device exists, set it as default
            logger.info(f"Found existing device with name {device_name}, ID: {existing_id}")
            success = set_default_gpodder_device(cnx, database_type, user_id, existing_id)
            return (success, "Existing device set as default", existing_id)

        # Create new device
        new_device_id = create_or_update_device(
            cnx,
            database_type,
            user_id,
            device_name,
            "remote",  # Type for remote devices
            f"Remote device from GPodder server"
        )

        if not new_device_id:
            logger.error("Failed to create device for remote device")
            return (False, "Failed to create local representation of remote device", None)

        # Set as default
        success = set_default_gpodder_device(cnx, database_type, user_id, new_device_id)
        return (success, "Remote device created and set as default", new_device_id)

    except Exception as e:
        logger.error(f"Error handling remote device: {e}")
        return (False, f"Error: {str(e)}", None)


def find_device_by_name(cnx, database_type, user_id, device_name):
    """
    Find a device by name for a specific user

    Args:
        cnx: Database connection
        database_type: Type of database
        user_id: User ID
        device_name: Device name to find

    Returns:
        int: Device ID or None if not found
    """
    try:
        cursor = cnx.cursor()
        if database_type == "postgresql":
            query = 'SELECT DeviceID FROM "GpodderDevices" WHERE UserID = %s AND DeviceName = %s'
        else:
            query = 'SELECT DeviceID FROM GpodderDevices WHERE UserID = %s AND DeviceName = %s'

        cursor.execute(query, (user_id, device_name))
        result = cursor.fetchone()

        if result:
            if isinstance(result, tuple):
                return result[0]
            else:
                return result["deviceid"]
        return None
    except Exception as e:
        print(f"Error finding device by name: {e}")
        return None
    finally:
        cursor.close()

def create_or_update_device(cnx, database_type, user_id, device_name, device_type="desktop", device_caption=None, is_default=False):
    """
    Creates a new device or updates an existing one.
    If is_default is True, this device will be set as the default.
    """
    try:
        cursor = cnx.cursor()

        # Check if device exists
        if database_type == "postgresql":
            query = """
                SELECT DeviceID FROM "GpodderDevices"
                WHERE UserID = %s AND DeviceName = %s
            """
        else:
            query = """
                SELECT DeviceID FROM GpodderDevices
                WHERE UserID = %s AND DeviceName = %s
            """

        cursor.execute(query, (user_id, device_name))
        result = cursor.fetchone()

        if result:
            # Device exists, update it
            device_id = result[0] if isinstance(result, tuple) else result["deviceid"]

            if database_type == "postgresql":
                query = """
                    UPDATE "GpodderDevices"
                    SET DeviceType = %s, DeviceCaption = %s, LastSync = CURRENT_TIMESTAMP
                    WHERE DeviceID = %s
                """
            else:
                query = """
                    UPDATE GpodderDevices
                    SET DeviceType = %s, DeviceCaption = %s, LastSync = CURRENT_TIMESTAMP
                    WHERE DeviceID = %s
                """

            cursor.execute(query, (device_type, device_caption, device_id))

            # If this should be the default device, set it
            if is_default:
                set_default_gpodder_device(cnx, database_type, user_id, device_id)

            cnx.commit()
            return device_id
        else:
            # Device doesn't exist, create it
            if database_type == "postgresql":
                query = """
                    INSERT INTO "GpodderDevices" (UserID, DeviceName, DeviceType, DeviceCaption, IsDefault)
                    VALUES (%s, %s, %s, %s, %s)
                    RETURNING DeviceID
                """
            else:
                query = """
                    INSERT INTO GpodderDevices (UserID, DeviceName, DeviceType, DeviceCaption, IsDefault)
                    VALUES (%s, %s, %s, %s, %s)
                """

            # If this is the first device for the user, make it the default
            if is_default:
                cursor.execute(query, (user_id, device_name, device_type, device_caption, True))
            else:
                # Check if this is the first device
                if database_type == "postgresql":
                    count_query = 'SELECT COUNT(*) as count FROM "GpodderDevices" WHERE UserID = %s'
                else:
                    count_query = 'SELECT COUNT(*) as count FROM GpodderDevices WHERE UserID = %s'

                cursor.execute(count_query, (user_id,))
                result = cursor.fetchone()

                # Handle different result formats from different database types
                if result is None:
                    count = 0
                elif isinstance(result, tuple):
                    count = result[0]
                elif isinstance(result, dict) and "count" in result:
                    count = result["count"]
                else:
                    # Try to get value safely
                    try:
                        count = list(result.values())[0] if result else 0
                    except:
                        count = 0

                # If this is the first device, make it the default
                is_first_device = count == 0
                cursor.execute(query, (user_id, device_name, device_type, device_caption, is_first_device))

            if database_type == "postgresql":
                result = cursor.fetchone()
                device_id = result[0] if result and isinstance(result, tuple) else (result['deviceid'] if result else None)
            else:
                device_id = cursor.lastrowid

            cnx.commit()
            return device_id
    except Exception as e:
        print(f"Error creating/updating device: {e}")
        cnx.rollback()
        return None
    finally:
        cursor.close()

def get_sync_timestamps(cnx, database_type, user_id, device_id):
    """Get sync timestamps for a device, with default values if not found"""
    try:
        cursor = cnx.cursor()

        # Handle negative device IDs (remote devices)
        if device_id and device_id < 0:
            print(f"Error getting sync timestamps: Device ID {device_id} is negative (remote device)")
            # Return default timestamps for remote devices
            return {"last_timestamp": 0, "episodes_timestamp": 0}

        if database_type == "postgresql":
            query = '''
                SELECT LastTimestamp, EpisodesTimestamp
                FROM "GpodderSyncState"
                WHERE UserID = %s AND DeviceID = %s
            '''
        else:
            query = '''
                SELECT LastTimestamp, EpisodesTimestamp
                FROM GpodderSyncState
                WHERE UserID = %s AND DeviceID = %s
            '''

        cursor.execute(query, (user_id, device_id))
        result = cursor.fetchone()

        if result:
            if isinstance(result, tuple):
                return {
                    "last_timestamp": result[0] or 0,
                    "episodes_timestamp": result[1] or 0
                }
            else:
                return {
                    "last_timestamp": result.get("lasttimestamp", 0) or 0,
                    "episodes_timestamp": result.get("episodestimestamp", 0) or 0
                }
        else:
            # No timestamps found, create default record
            if database_type == "postgresql":
                insert_query = '''
                    INSERT INTO "GpodderSyncState" (UserID, DeviceID, LastTimestamp, EpisodesTimestamp)
                    VALUES (%s, %s, 0, 0)
                    ON CONFLICT (UserID, DeviceID) DO NOTHING
                '''
            else:
                insert_query = '''
                    INSERT INTO GpodderSyncState (UserID, DeviceID, LastTimestamp, EpisodesTimestamp)
                    VALUES (%s, %s, 0, 0)
                    ON CONFLICT (UserID, DeviceID) DO NOTHING
                '''

            try:
                cursor.execute(insert_query, (user_id, device_id))
                cnx.commit()
            except Exception as e:
                print(f"Error creating sync timestamps: {e}")
                # Don't let this error abort everything
                cnx.rollback()

            return {"last_timestamp": 0, "episodes_timestamp": 0}
    except Exception as e:
        print(f"Error getting sync timestamps: {e}")
        return {"last_timestamp": 0, "episodes_timestamp": 0}
    finally:
        cursor.close()

def update_sync_timestamp(cnx, database_type, user_id, device_id, timestamp_type, new_timestamp):
    """Update the sync timestamp for a particular user and device"""
    if timestamp_type not in ["last_timestamp", "episodes_timestamp"]:
        raise ValueError("Invalid timestamp_type. Must be 'last_timestamp' or 'episodes_timestamp'")

    cursor = cnx.cursor()
    try:
        db_column = "LastTimestamp" if timestamp_type == "last_timestamp" else "EpisodesTimestamp"

        if database_type == "postgresql":
            query = f'''
                UPDATE "GpodderSyncState"
                SET {db_column} = %s
                WHERE UserID = %s AND DeviceID = %s
            '''
        else:
            query = f'''
                UPDATE GpodderSyncState
                SET {db_column} = %s
                WHERE UserID = %s AND DeviceID = %s
            '''

        cursor.execute(query, (new_timestamp, user_id, device_id))
        cnx.commit()
        return True
    except Exception as e:
        print(f"Error updating sync timestamp: {e}")
        cnx.rollback()
        return False
    finally:
        cursor.close()

def get_or_create_default_device(cnx, database_type, user_id):
    """Get the default device for a user or create it if it doesn't exist"""
    default_device_name = "pinepods_default"

    # Try to find existing default device
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            query = '''
                SELECT DeviceID FROM "GpodderDevices"
                WHERE UserID = %s AND DeviceName = %s
            '''
        else:
            query = '''
                SELECT DeviceID FROM GpodderDevices
                WHERE UserID = %s AND DeviceName = %s
            '''

        cursor.execute(query, (user_id, default_device_name))
        result = cursor.fetchone()

        if result:
            # Default device exists
            return result[0] if isinstance(result, tuple) else result["deviceid"]
        else:
            # Create default device
            return create_or_update_device(
                cnx,
                database_type,
                user_id,
                default_device_name,
                "desktop",
                "Pinepods Default Device"
            )
    except Exception as e:
        logger.error(f"Error getting/creating default device: {e}")
        return None
    finally:
        cursor.close()

def get_current_timestamp():
    """Get current timestamp in format expected by gpodder API"""
    return int(time.time())


def create_or_get_gpodder_device(cnx, database_type, user_id, device_name, device_type, device_caption):
    """
    Create a gpodder device if it doesn't exist, or get its ID if it does

    Args:
        cnx: Database connection
        database_type: Type of database (postgresql or mysql)
        user_id: User ID
        device_name: Device name
        device_type: Device type (server, desktop, mobile, etc.)
        device_caption: Human-readable device caption

    Returns:
        Device ID if successful, None if failed
    """
    try:
        cursor = cnx.cursor()

        # Check if device exists
        if database_type == "postgresql":
            query = 'SELECT DeviceID FROM "GpodderDevices" WHERE UserID = %s AND DeviceName = %s'
        else:
            query = "SELECT DeviceID FROM GpodderDevices WHERE UserID = %s AND DeviceName = %s"

        cursor.execute(query, (user_id, device_name))
        device_result = cursor.fetchone()

        if device_result:
            # Device exists, return its ID
            if isinstance(device_result, tuple):
                device_id = device_result[0]
            else:
                # For dict result, use the correct column name case
                device_id = device_result["DeviceID"]
            print(f"Using existing gpodder device with ID: {device_id}")
        else:
            # Create device record
            if database_type == "postgresql":
                query = '''
                    INSERT INTO "GpodderDevices"
                    (UserID, DeviceName, DeviceType, DeviceCaption, IsActive, LastSync)
                    VALUES (%s, %s, %s, %s, TRUE, CURRENT_TIMESTAMP)
                    RETURNING DeviceID
                '''
            else:
                query = '''
                    INSERT INTO GpodderDevices
                    (UserID, DeviceName, DeviceType, DeviceCaption, IsActive, LastSync)
                    VALUES (%s, %s, %s, %s, TRUE, NOW())
                '''

            cursor.execute(query, (user_id, device_name, device_type, device_caption))

            if database_type == "postgresql":
                device_id = cursor.fetchone()[0]
            else:
                device_id = cursor.lastrowid

            print(f"Created gpodder device with ID: {device_id}")

            # Also create device sync state entry
            if database_type == "postgresql":
                state_query = '''
                    INSERT INTO "GpodderSyncDeviceState" (UserID, DeviceID)
                    VALUES (%s, %s)
                    ON CONFLICT (UserID, DeviceID) DO NOTHING
                '''
            else:
                state_query = '''
                    INSERT IGNORE INTO GpodderSyncDeviceState (UserID, DeviceID)
                    VALUES (%s, %s)
                '''

            cursor.execute(state_query, (user_id, device_id))

        cnx.commit()
        cursor.close()
        return device_id

    except Exception as e:
        print(f"Error in create_or_get_gpodder_device: {e}")
        if 'cursor' in locals():
            cursor.close()
        return None

def generate_secure_token(length=64):
    """
    Generate a secure random token for internal authentication

    Args:
        length: Length of the token (default: 64)

    Returns:
        Secure random token string
    """
    import secrets
    import string

    alphabet = string.ascii_letters + string.digits
    return ''.join(secrets.choice(alphabet) for _ in range(length))

def set_gpodder_internal_sync(cnx, database_type, user_id):
    """
    Set up internal gpodder sync for a user with a plain, unencrypted token
    """
    try:
        # Get the username
        cursor = cnx.cursor()
        if database_type == "postgresql":
            query = 'SELECT Username, Pod_Sync_Type FROM "Users" WHERE UserID = %s'
        else:
            query = "SELECT Username, Pod_Sync_Type FROM Users WHERE UserID = %s"
        cursor.execute(query, (user_id,))
        user_info = cursor.fetchone()
        cursor.close()
        if not user_info:
            print(f"User not found for ID: {user_id}")
            return None
        username = user_info[0] if isinstance(user_info, tuple) else user_info["username"]
        current_sync_type = user_info[1] if isinstance(user_info, tuple) else user_info["pod_sync_type"]

        # Generate a new sync type based on current
        new_sync_type = current_sync_type
        if current_sync_type == "external":
            new_sync_type = "both"
        elif current_sync_type == "None" or current_sync_type is None:
            new_sync_type = "gpodder"

        # Generate a secure internal token - PLAIN TEXT, NO ENCRYPTION
        import secrets
        import string
        alphabet = string.ascii_letters + string.digits
        internal_token = ''.join(secrets.choice(alphabet) for _ in range(64))

        # Set up the local gpodder API details
        local_gpodder_url = "http://localhost:8042"  # Internal API URL

        # Store the plain token in the database
        if database_type == "postgresql":
            query = '''
                UPDATE "Users"
                SET GpodderUrl = %s, GpodderToken = %s, GpodderLoginName = %s, Pod_Sync_Type = %s
                WHERE UserID = %s
            '''
        else:
            query = '''
                UPDATE Users
                SET GpodderUrl = %s, GpodderToken = %s, GpodderLoginName = %s, Pod_Sync_Type = %s
                WHERE UserID = %s
            '''
        cursor = cnx.cursor()
        cursor.execute(query, (local_gpodder_url, internal_token, username, new_sync_type, user_id))
        cnx.commit()
        cursor.close()

        # Create a default device for this user using the gPodder API
        default_device_name = f"pinepods-internal-{user_id}"

        # Create the device using the gPodder API
        import requests
        from requests.auth import HTTPBasicAuth

        # Use the API to register a device
        device_data = {
            "caption": f"PinePods Internal Device {user_id}",
            "type": "server"
        }

        try:
            # First, check if the device already exists
            device_list_url = f"{local_gpodder_url}/api/2/devices/{username}.json"
            response = requests.get(
                device_list_url,
                auth=HTTPBasicAuth(username, internal_token)
            )

            # If we can't get device list, create a new one anyway
            existing_device_id = None
            if response.status_code == 200:
                devices = response.json()
                for device in devices:
                    if device.get("id") == default_device_name:
                        existing_device_id = device.get("id")
                        print(f"Found existing device with ID: {existing_device_id}")
                        break

            # If device doesn't exist, create it
            if not existing_device_id:
                device_url = f"{local_gpodder_url}/api/2/devices/{username}/{default_device_name}.json"
                response = requests.post(
                    device_url,
                    json=device_data,
                    auth=HTTPBasicAuth(username, internal_token)
                )

                if response.status_code in [200, 201]:
                    print(f"Created device with ID: {default_device_name}")
                else:
                    print(f"Failed to create device: {response.status_code} - {response.text}")
                    # Continue anyway - the API might create the device on first sync

            # Return the device info
            return {
                "device_name": default_device_name,
                "device_id": user_id,  # Use user_id as a fallback/reference
                "success": True
            }

        except Exception as device_err:
            print(f"Error creating device via API: {device_err}")
            # Even if device creation fails, still return success
            return {
                "device_name": default_device_name,
                "device_id": user_id,
                "success": True
            }

    except Exception as e:
        print(f"Error in set_gpodder_internal_sync: {e}")
        return None

def disable_gpodder_internal_sync(cnx, database_type, user_id):
    """
    Disable internal gpodder sync for a user

    Args:
        cnx: Database connection
        database_type: Type of database (postgresql or mysql)
        user_id: User ID

    Returns:
        True if successful, False if failed
    """
    try:
        # Get current gpodder settings
        user_data = get_user_gpodder_status(cnx, database_type, user_id)
        if not user_data:
            print(f"User data not found for ID: {user_id}")
            return False

        current_sync_type = user_data["sync_type"]

        # Determine new sync type
        new_sync_type = current_sync_type
        if current_sync_type == "both":
            new_sync_type = "external"
        elif current_sync_type == "gpodder":
            new_sync_type = "None"

        # If internal API is being used, clear the settings
        if user_data.get("gpodder_url") == "http://localhost:8042":
            success = add_gpodder_settings(
                database_type,
                cnx,
                user_id,
                "",  # Clear URL
                "",  # Clear token
                "",  # Clear login
                new_sync_type
            )

            if not success:
                print(f"Failed to clear gpodder settings for user: {user_id}")
                return False
        else:
            # Just update the sync type
            success = update_user_gpodder_sync(cnx, database_type, user_id, new_sync_type)
            if not success:
                print(f"Failed to update gpodder sync type for user: {user_id}")
                return False

        return True

    except Exception as e:
        print(f"Error in disable_gpodder_internal_sync: {e}")
        return False

def refresh_gpodder_subscription(database_type, cnx, user_id, gpodder_url, encrypted_gpodder_token,
                              gpodder_login, pod_sync_type, device_id=None, device_name=None, is_remote=False):
    """Refreshes podcasts from GPodder with proper device handling"""
    from cryptography.fernet import Fernet
    import logging
    import requests
    import base64
    from requests.auth import HTTPBasicAuth

    # Set up logging
    logging.basicConfig(level=logging.INFO)
    logger = logging.getLogger(__name__)

    try:
        # More detailed logging for debugging
        print(f"Starting refresh with parameters: user_id={user_id}, gpodder_url={gpodder_url}, " +
              f"pod_sync_type={pod_sync_type}, device_id={device_id}, device_name={device_name}, " +
              f"is_remote={is_remote}")

        # Flag to identify internal API calls
        is_internal_api = (gpodder_url == "http://localhost:8042")
        print(f"Is internal API: {is_internal_api}")

        # Determine which device to use for GPodder API calls
        actual_device_name = None

        # Handle device name/id logic
        if is_remote and device_name:
            # If it's a remote device, use the provided device name directly
            print(f"Using remote device name: {device_name}")

            # Create a local representation of the remote device
            success, message, local_device_id = handle_remote_device(cnx, database_type, user_id, device_name)
            if success:
                print(f"Created/found local device for remote device: {local_device_id}")
                # Use the local device ID instead of -1
                device_id = local_device_id
                actual_device_name = device_name
            else:
                print(f"Failed to handle remote device: {message}")
                # Proceed with just the name, but device_id will still be -1 which might cause problems
                actual_device_name = device_name
        elif device_id:
            # If a specific device ID is provided, look it up in the database
            cursor = cnx.cursor()
            if database_type == "postgresql":
                query = 'SELECT DeviceName FROM "GpodderDevices" WHERE DeviceID = %s'
            else:
                query = "SELECT DeviceName FROM GpodderDevices WHERE DeviceID = %s"
            cursor.execute(query, (device_id,))
            result = cursor.fetchone()
            cursor.close()

            if result:
                actual_device_name = result[0] if isinstance(result, tuple) else result["devicename"]
                logger.info(f"Using device from database: {actual_device_name} (ID: {device_id})")
            else:
                logger.warning(f"Device ID {device_id} not found in database, falling back to default")
                default_device = get_default_gpodder_device(cnx, database_type, user_id)
                if default_device:
                    device_id = default_device["id"]
                    actual_device_name = default_device["name"]
                    print(f"Using default device: {actual_device_name} (ID: {device_id})")
                else:
                    # No default device, create one
                    device_id = create_or_update_device(
                        cnx,
                        database_type,
                        user_id,
                        "pinepods_default",
                        "desktop",
                        "Pinepods Default Device",
                        True  # Set as default
                    )
                    actual_device_name = "pinepods_default"
                    print(f"Created new default device: {actual_device_name} (ID: {device_id})")
        else:
            # No device specified, use default
            default_device = get_default_gpodder_device(cnx, database_type, user_id)
            if default_device:
                device_id = default_device["id"]
                actual_device_name = default_device["name"]
                print(f"Using default device: {actual_device_name} (ID: {device_id})")
            else:
                # No devices exist, create a default one
                device_id = create_or_update_device(
                    cnx,
                    database_type,
                    user_id,
                    "pinepods_default",
                    "desktop",
                    "Pinepods Default Device",
                    True  # Set as default
                )
                actual_device_name = "pinepods_default"
                print(f"Created new default device: {actual_device_name} (ID: {device_id})")

        # For remote devices, we might need to skip checking local timestamps
        # and force a full sync from the GPodder server
        if is_remote:
            # Force a full sync by setting timestamp to 0
            timestamps = {"last_timestamp": 0}
            print("Remote device selected - forcing full sync with timestamp 0")
        else:
            # Get sync timestamps for local device
            timestamps = get_sync_timestamps(cnx, database_type, user_id, device_id)

        # Get encryption key and decrypt the GPodder token
        print("Getting encryption key...")
        encryption_key = get_encryption_key(cnx, database_type)

        if not encryption_key:
            logger.error("Failed to retrieve encryption key")
            return False

        try:
            encryption_key_bytes = base64.b64decode(encryption_key)
            cipher_suite = Fernet(encryption_key_bytes)
            print("Successfully created cipher suite for decryption")
        except Exception as e:
            logger.error(f"Error preparing encryption key: {str(e)}")
            return False

        # Special handling for encrypted_gpodder_token based on input type
        if isinstance(encrypted_gpodder_token, dict):
            if "data" in encrypted_gpodder_token:
                print("Extracting token from dictionary input")
                encrypted_gpodder_token = encrypted_gpodder_token.get("data", {}).get("gpoddertoken", "")
            else:
                encrypted_gpodder_token = encrypted_gpodder_token.get("gpoddertoken", "")

        # Decrypt the token - with improved error handling
        gpodder_token = None
        if encrypted_gpodder_token is not None and encrypted_gpodder_token != "":
            try:
                # Handle both string and bytes formats
                if isinstance(encrypted_gpodder_token, bytes):
                    decrypted_token_bytes = cipher_suite.decrypt(encrypted_gpodder_token)
                else:
                    # Make sure we're working with a valid token
                    token_to_decrypt = encrypted_gpodder_token
                    # If the token isn't in the right format for decryption, try to fix it
                    if not (token_to_decrypt.startswith(b'gAAAAA') if isinstance(token_to_decrypt, bytes)
                            else token_to_decrypt.startswith('gAAAAA')):
                        print("Token doesn't appear to be in Fernet format, using raw token instead")
                        gpodder_token = encrypted_gpodder_token
                    else:
                        print("Decrypting token in Fernet format")
                        decrypted_token_bytes = cipher_suite.decrypt(token_to_decrypt.encode())
                        gpodder_token = decrypted_token_bytes.decode()
                        print("Successfully decrypted gpodder token")
            except Exception as e:
                logger.error(f"Error decrypting token: {str(e)}")
                # For non-internal servers, we might still want to continue with whatever token we have
                if is_internal_api:
                    # For internal server, fall back to using the raw token if decryption fails
                    logger.warning("Using raw token as fallback for internal server")
                    gpodder_token = encrypted_gpodder_token
                else:
                    # For external servers, continue with the encrypted token
                    print("Using encrypted token for external server")
                    gpodder_token = encrypted_gpodder_token
        else:
            logger.warning("No token provided")
            if is_internal_api:
                logger.error("Token required for internal gpodder server")
                return False

        print(f"Final token established: {'[OBSCURED FOR SECURITY]' if gpodder_token else 'None'}")
        print(f"Using {'internal' if is_internal_api else 'external'} gpodder API at {gpodder_url}")

        # Create a session for cookie-based auth
        session = requests.Session()

        # Handle authentication for internal API calls
        if is_internal_api:
            print("Using token-based auth for internal API")
            # Use the token directly with the gPodder API
            auth = HTTPBasicAuth(gpodder_login, encrypted_gpodder_token)

            # Try to access API using Basic Auth
            try:
                # First, create or update the device if needed
                device_data = {
                    "caption": f"PinePods Internal Device {user_id}",
                    "type": "server"
                }
                device_url = f"{gpodder_url}/api/2/devices/{gpodder_login}/{actual_device_name}.json"

                try:
                    response = requests.post(
                        device_url,
                        json=device_data,
                        auth=auth
                    )
                    if response.status_code in [200, 201]:
                        print(f"Updated device: {actual_device_name}")
                    else:
                        print(f"Note: Device update returned {response.status_code}")
                except Exception as device_err:
                    print(f"Warning: Device update failed: {device_err}")
                    # Continue anyway

                # Now get subscriptions
                subscription_url = f"{gpodder_url}/api/2/subscriptions/{gpodder_login}/{actual_device_name}.json?since={timestamps['last_timestamp']}"
                print(f"Requesting subscriptions from internal API at {subscription_url}")
                response = requests.get(subscription_url, auth=auth)
                response.raise_for_status()
                gpodder_data = response.json()
                print("Successfully retrieved data from internal API")
                use_session = False
            except Exception as e:
                logger.error(f"Failed to get subscriptions from internal API: {str(e)}")
                raise
        else:
            # For external API, use regular basic auth as before
            print("Using regular basic auth for external API")
            auth = HTTPBasicAuth(gpodder_login, gpodder_token)

        # Try session-based authentication (for PodFetch)
        gpodder_data = None
        use_session = False

        try:
            # First try to login to establish a session
            login_url = f"{gpodder_url}/api/2/auth/{gpodder_login}/login.json"
            print(f"Trying session-based authentication at {login_url}")
            login_response = session.post(login_url, auth=auth)
            login_response.raise_for_status()
            print("Session login successful")

            # Use the session to get subscriptions with the since parameter
            subscription_url = f"{gpodder_url}/api/2/subscriptions/{gpodder_login}/{actual_device_name}.json?since={timestamps['last_timestamp']}"
            response = session.get(subscription_url)
            response.raise_for_status()
            gpodder_data = response.json()
            use_session = True
            print("Using session-based authentication")

        except Exception as e:
            logger.warning(f"Session-based authentication failed: {str(e)}. Falling back to basic auth.")
            # Fall back to standard auth if session auth fails
            try:
                subscription_url = f"{gpodder_url}/api/2/subscriptions/{gpodder_login}/{actual_device_name}.json?since={timestamps['last_timestamp']}"
                print(f"Trying basic authentication at {subscription_url}")
                response = requests.get(subscription_url, auth=auth)
                response.raise_for_status()
                gpodder_data = response.json()
                print("Using basic authentication")
            except Exception as e2:
                logger.error(f"Basic auth also failed: {str(e2)}")
                raise

        # Store timestamp for next sync if present
        if gpodder_data and "timestamp" in gpodder_data:
            update_sync_timestamp(cnx, database_type, user_id, device_id, "last_timestamp", gpodder_data["timestamp"])
            logger.info(f"Stored timestamp: {gpodder_data['timestamp']}")

        # Extract subscription data
        gpodder_podcasts_add = gpodder_data.get("add", [])
        gpodder_podcasts_remove = gpodder_data.get("remove", [])

        print(f"gPodder podcasts to add: {gpodder_podcasts_add}")
        print(f"gPodder podcasts to remove: {gpodder_podcasts_remove}")

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

        podcasts_to_add = set(gpodder_podcasts_add) - local_podcasts
        podcasts_to_remove = set(gpodder_podcasts_remove) & local_podcasts

        # Track successful additions and removals for sync
        successful_additions = set()
        successful_removals = set()

        # Add new podcasts with individual error handling
        print("Adding new podcasts...")
        for feed_url in podcasts_to_add:
            try:
                podcast_values = get_podcast_values(feed_url, user_id)
                return_value = add_podcast(cnx, database_type, podcast_values, user_id)
                if return_value:
                    print(f"Successfully added {feed_url}")
                    successful_additions.add(feed_url)
                else:
                    logger.error(f"Failed to add {feed_url}")
            except Exception as e:
                logger.error(f"Error processing {feed_url}: {str(e)}")
                continue  # Continue with next podcast even if this one fails

        # Remove podcasts with individual error handling
        print("Removing podcasts...")
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
                        print(f"Successfully removed {feed_url}")
                    else:
                        logger.error(f"Failed to remove {feed_url}")
                else:
                    logger.warning(f"No podcast found with URL: {feed_url}")
            except Exception as e:
                logger.error(f"Error removing {feed_url}: {str(e)}")
                continue

        cnx.commit()
        cursor.close()

        # Process episode actions using the correct device
        try:
            print(f"Authentication method: {'session' if use_session else 'basic auth'}")
            if use_session:
                print("Using SESSION authentication for episode actions")
                process_episode_actions_session(
                    session,
                    gpodder_url,
                    gpodder_login,
                    cnx,
                    database_type,
                    user_id,
                    actual_device_name,
                    device_id
                )
            else:
                print("Using BASIC authentication for episode actions")
                process_episode_actions(
                    gpodder_url,
                    gpodder_login,
                    auth,
                    cnx,
                    database_type,
                    user_id,
                    actual_device_name,
                    device_id
                )
        except Exception as e:
            logger.error(f"Error processing episode actions: {str(e)}")

        # Sync local episode times
        try:
            print(f"Authentication method for ep times: {'session' if use_session else 'basic auth'}")
            if use_session:
                sync_local_episode_times_session(
                    session,
                    gpodder_url,
                    gpodder_login,
                    cnx,
                    database_type,
                    user_id,
                    actual_device_name
                )
            else:
                sync_local_episode_times(
                    gpodder_url,
                    gpodder_login,
                    auth,
                    cnx,
                    database_type,
                    user_id,
                    actual_device_name
                )
        except Exception as e:
            logger.error(f"Error syncing local episode times: {str(e)}")

        return True
    except Exception as e:
        logger.error(f"Major error in refresh_gpodder_subscription: {str(e)}")
        return False

def sync_local_episode_times_session(session, gpodder_url, gpodder_login, cnx, database_type, user_id, device_name=None, UPLOAD_BULK_SIZE=30):
    """Sync local episode times using session-based authentication"""
    import logging
    import json
    from datetime import datetime

    logger = logging.getLogger(__name__)
    print(f"Starting episode time sync with device_name={device_name}")

    try:
        # If no device name is provided, get the user's default device
        if not device_name:
            default_device = get_default_gpodder_device(cnx, database_type, user_id)
            if default_device:
                device_name = default_device["name"]
                print(f"Using default device for episode actions: {device_name}")
            else:
                print("WARNING: No devices found for user, episode actions will fail")
                return

        # Get local episode times
        local_episode_times = get_local_episode_times(cnx, database_type, user_id)

        # Skip if no episodes to sync
        if not local_episode_times:
            print("No episodes to sync")
            return

        # Format actions with all the required fields
        actions = []

        # Format timestamp as ISO string
        current_time = datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%S")

        for episode_time in local_episode_times:
            # Only include episodes with valid duration data
            if episode_time.get("episode_duration") and episode_time.get("listen_duration"):
                if not episode_time.get("podcast_url") or not episode_time.get("episode_url"):
                    print(f"Skipping episode with missing URL data")
                    continue

                # If episode is completed, set position to total duration
                position = (episode_time["episode_duration"]
                        if episode_time.get("completed", False)
                        else episode_time["listen_duration"])

                # Add all required fields including device
                action = {
                    "podcast": episode_time["podcast_url"],
                    "episode": episode_time["episode_url"],
                    "action": "play",
                    "position": int(position),
                    "total": int(episode_time["episode_duration"]),
                    "timestamp": current_time,
                    "device": device_name,
                    "started": 0  # Required by some implementations
                }

                # Add guid if available
                if episode_time.get("guid"):
                    action["guid"] = episode_time["guid"]

                actions.append(action)

        if not actions:
            print("No valid actions to send")
            return

        print(f"Prepared {len(actions)} actions to send")
        print(f"First action device name: {actions[0]['device']}")

        # Split into chunks and process
        actions_chunks = [
            actions[i:i + UPLOAD_BULK_SIZE]
            for i in range(0, len(actions), UPLOAD_BULK_SIZE)
        ]

        for chunk in actions_chunks:
            try:
                response = session.post(
                    f"{gpodder_url}/api/2/episodes/{gpodder_login}.json",
                    json=chunk,  # Send as array
                    headers={"Content-Type": "application/json"}
                )

                if response.status_code < 300:
                    print(f"Successfully synced {len(chunk)} episode actions")
                else:
                    print(f"Error syncing episode actions: {response.status_code} - {response.text}")

                    # Debug the request
                    print(f"Request URL: {gpodder_url}/api/2/episodes/{gpodder_login}.json")
                    print(f"Request headers: {session.headers}")
                    print(f"First few actions in chunk: {chunk[:2]}")
            except Exception as e:
                print(f"Error sending actions: {str(e)}")
                continue

    except Exception as e:
        print(f"Error in sync_local_episode_times_session: {str(e)}")


def set_default_gpodder_device(cnx, database_type, user_id, device_id):
    """
    Sets a device as the user's default GPodder device.
    This will unset any previous default device.

    Args:
        cnx: Database connection
        database_type: "postgresql" or "mariadb"
        user_id: User ID
        device_id: Device ID to set as default

    Returns:
        bool: Success or failure
    """
    try:
        cursor = cnx.cursor()

        # First verify the device exists and belongs to the user
        if database_type == "postgresql":
            query = 'SELECT DeviceID FROM "GpodderDevices" WHERE DeviceID = %s AND UserID = %s'
        else:
            query = 'SELECT DeviceID FROM GpodderDevices WHERE DeviceID = %s AND UserID = %s'

        cursor.execute(query, (device_id, user_id))
        if not cursor.fetchone():
            print(f"Device ID {device_id} does not exist or doesn't belong to user {user_id}")
            return False

        # Start a transaction
        if database_type == "postgresql":
            # First, unset the current default device if any
            cursor.execute("""
                UPDATE "GpodderDevices"
                SET IsDefault = FALSE
                WHERE UserID = %s AND IsDefault = TRUE
            """, (user_id,))

            # Then set the new default device
            cursor.execute("""
                UPDATE "GpodderDevices"
                SET IsDefault = TRUE
                WHERE DeviceID = %s
            """, (device_id,))
        else:
            # First, unset the current default device if any
            cursor.execute("""
                UPDATE GpodderDevices
                SET IsDefault = FALSE
                WHERE UserID = %s AND IsDefault = TRUE
            """, (user_id,))

            # Then set the new default device
            cursor.execute("""
                UPDATE GpodderDevices
                SET IsDefault = TRUE
                WHERE DeviceID = %s
            """, (device_id,))

        cnx.commit()
        print(f"Set default GPodder device {device_id} for user {user_id}")
        return True
    except Exception as e:
        print(f"Error setting default GPodder device: {e}")
        cnx.rollback()
        return False
    finally:
        cursor.close()

def get_default_gpodder_device(cnx, database_type, user_id):
    """
    Gets the user's default GPodder device.
    If no default is set, returns the oldest device.

    Args:
        cnx: Database connection
        database_type: "postgresql" or "mariadb"
        user_id: User ID

    Returns:
        dict: Device information or None if no devices exist
    """
    try:
        cursor = cnx.cursor()

        # First try to get the default device
        if database_type == "postgresql":
            query = """
                SELECT DeviceID, DeviceName, DeviceType, DeviceCaption, LastSync, IsActive
                FROM "GpodderDevices"
                WHERE UserID = %s AND IsDefault = TRUE
                LIMIT 1
            """
        else:
            query = """
                SELECT DeviceID, DeviceName, DeviceType, DeviceCaption, LastSync, IsActive
                FROM GpodderDevices
                WHERE UserID = %s AND IsDefault = TRUE
                LIMIT 1
            """

        cursor.execute(query, (user_id,))
        result = cursor.fetchone()

        if result:
            # Return the default device
            if isinstance(result, dict):
                return {
                    "id": result["deviceid"],
                    "name": result["devicename"],
                    "type": result["devicetype"],
                    "caption": result["devicecaption"],
                    "last_sync": result["lastsync"],
                    "is_active": result["isactive"],
                    "is_remote": False,
                    "is_default": True
                }
            else:
                return {
                    "id": result[0],
                    "name": result[1],
                    "type": result[2],
                    "caption": result[3],
                    "last_sync": result[4],
                    "is_active": result[5],
                    "is_remote": False,
                    "is_default": True
                }

        # If no default device is set, get the oldest device
        if database_type == "postgresql":
            query = """
                SELECT DeviceID, DeviceName, DeviceType, DeviceCaption, LastSync, IsActive
                FROM "GpodderDevices"
                WHERE UserID = %s
                ORDER BY DeviceID ASC
                LIMIT 1
            """
        else:
            query = """
                SELECT DeviceID, DeviceName, DeviceType, DeviceCaption, LastSync, IsActive
                FROM GpodderDevices
                WHERE UserID = %s
                ORDER BY DeviceID ASC
                LIMIT 1
            """

        cursor.execute(query, (user_id,))
        result = cursor.fetchone()

        if result:
            # Return the oldest device
            if isinstance(result, dict):
                return {
                    "id": result["deviceid"],
                    "name": result["devicename"],
                    "type": result["devicetype"],
                    "caption": result["devicecaption"],
                    "last_sync": result["lastsync"],
                    "is_active": result["isactive"],
                    "is_remote": False,
                    "is_default": False
                }
            else:
                return {
                    "id": result[0],
                    "name": result[1],
                    "type": result[2],
                    "caption": result[3],
                    "last_sync": result[4],
                    "is_active": result[5],
                    "is_remote": False,
                    "is_default": False
                }

        # No devices found
        return None
    except Exception as e:
        print(f"Error getting default GPodder device: {e}")
        return None
    finally:
        cursor.close()


def sync_local_episode_times(gpodder_url, gpodder_login, auth, cnx, database_type, user_id, device_name="default", UPLOAD_BULK_SIZE=30):
    """Sync local episode times using basic authentication"""
    import logging
    from datetime import datetime
    import requests

    logger = logging.getLogger(__name__)

    try:
        local_episode_times = get_local_episode_times(cnx, database_type, user_id)
        update_actions = []

        for episode_time in local_episode_times:
            # Only include episodes with valid duration data
            if episode_time.get("episode_duration") and episode_time.get("listen_duration"):
                # If episode is completed, set position to total duration
                position = (episode_time["episode_duration"]
                          if episode_time.get("completed", False)
                          else episode_time["listen_duration"])

                action = {
                    "podcast": episode_time["podcast_url"],
                    "episode": episode_time["episode_url"],
                    "action": "play",
                    "timestamp": datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%S"),
                    "position": int(position),
                    "started": 0,
                    "total": int(episode_time["episode_duration"]),
                    "device": device_name  # Use the specified device name
                }

                # Add guid if available
                if episode_time.get("guid"):
                    action["guid"] = episode_time["guid"]

                update_actions.append(action)

        # Skip if no actions to send
        if not update_actions:
            logger.info("No episode actions to upload")
            return

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
                    headers={"Accept": "application/json", "Content-Type": "application/json"}
                )
                response.raise_for_status()
                logger.info(f"Successfully synced {len(chunk)} episode actions")
            except Exception as e:
                logger.error(f"Error uploading chunk: {str(e)}")
                continue

    except Exception as e:
        logger.error(f"Error syncing local episode times: {str(e)}")
        raise

def process_episode_actions_session(session, gpodder_url, gpodder_login, cnx, database_type, user_id, device_name, device_id):
    """Process incoming episode actions from gPodder using session-based authentication"""
    logger = logging.getLogger(__name__)
    print('running episode actions')

    try:
        # Get timestamp for since parameter
        timestamps = get_sync_timestamps(cnx, database_type, user_id, device_id)
        episodes_timestamp = timestamps["episodes_timestamp"]
        print('got timestamps')

        # Get episode actions with session and since parameter
        episode_actions_response = session.get(
            f"{gpodder_url}/api/2/episodes/{gpodder_login}.json?since={episodes_timestamp}&device={device_name}"
        )
        episode_actions_response.raise_for_status()
        episode_actions = episode_actions_response.json()
        print('got actions')

        # Store timestamp for future requests
        if "timestamp" in episode_actions:
            update_sync_timestamp(cnx, database_type, user_id, device_id, "episodes_timestamp", episode_actions["timestamp"])
        print('stamp stored')
        # Process each action
        cursor = cnx.cursor()
        for action in episode_actions.get('actions', []):
            print('processing')
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
                                print(f"Marked episode {episode_id} as completed")
            except Exception as e:
                logger.error(f"Error processing episode action {action}: {str(e)}")
                continue
        cursor.close()
    except Exception as e:
        logger.error(f"Error fetching episode actions with session: {str(e)}")
        raise

def process_episode_actions(gpodder_url, gpodder_login, auth, cnx, database_type, user_id, device_name, device_id):
    """Process incoming episode actions from gPodder using basic authentication"""
    logger = logging.getLogger(__name__)
    print('Running episode actions with basic auth')
    try:
        # Get timestamp for since parameter
        timestamps = get_sync_timestamps(cnx, database_type, user_id, device_id)
        episodes_timestamp = timestamps["episodes_timestamp"]
        print(f'Got timestamps: {episodes_timestamp}')

        # Always include device parameter, even if it's empty
        url = f"{gpodder_url}/api/2/episodes/{gpodder_login}.json?since={episodes_timestamp}"
        if device_name:
            url += f"&device={device_name}"

        print(f"Episode actions API URL: {url}")

        # Get episode actions with basic auth
        episode_actions_response = requests.get(url, auth=auth)
        print(f"Episode actions response status: {episode_actions_response.status_code}")

        # Log the raw response for debugging
        response_text = episode_actions_response.text
        print(f"Raw response: {response_text[:200]}...")  # Log first 200 chars

        episode_actions_response.raise_for_status()

        # Parse the JSON response
        episode_actions = episode_actions_response.json()
        print(f"Response keys: {episode_actions.keys()}")

        # Store timestamp for future requests
        if "timestamp" in episode_actions:
            update_sync_timestamp(cnx, database_type, user_id, device_id, "episodes_timestamp", episode_actions["timestamp"])
            print(f'Updated timestamp to {episode_actions["timestamp"]}')

        # Check if 'actions' key exists before processing
        if 'actions' not in episode_actions:
            print("No 'actions' key in response. Response structure: %s", episode_actions)
            return  # Exit early if no actions to process

        # Process each action - same as in session version
        cursor = cnx.cursor()
        for action in episode_actions.get('actions', []):
            try:
                print(f"Processing action: {action}")

                if "action" not in action:
                    print(f"Action missing 'action' key: {action}")
                    continue

                if action["action"].lower() in ["play", "update_time"]:
                    if "position" in action and action["position"] != -1:
                        # Check if episode key exists
                        if "episode" not in action:
                            print(f"Action missing 'episode' key: {action}")
                            continue

                        episode_id = get_episode_id_by_url(cnx, database_type, action["episode"])

                        if not episode_id:
                            print(f"No episode found for URL: {action['episode']}")
                            continue

                        # Update listen duration
                        record_listen_duration(cnx, database_type, episode_id, user_id, int(action["position"]))
                        print(f"Updated listen duration for episode {episode_id}")

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
                            print(f"Marked episode {episode_id} as completed")
            except Exception as e:
                logger.error(f"Error processing episode action {action}: {str(e)}")
                # Continue with next action rather than breaking
                continue
        cursor.close()
    except Exception as e:
        logger.error(f"Error fetching episode actions with basic auth: {str(e)}", exc_info=True)
        raise

def force_full_sync_to_gpodder(database_type, cnx, user_id, gpodder_url, encrypted_gpodder_token, gpodder_login, device_id=None, device_name=None, is_remote=False):
    """Force a full sync of all local podcasts to the GPodder server"""
    from cryptography.fernet import Fernet
    from requests.auth import HTTPBasicAuth
    import requests
    import logging

    print(f"Starting GPodder sync with: device_id={device_id}, device_name={device_name}, is_remote={is_remote}")

    try:
        # Use provided device_id or get/create default
        if device_id is None or device_id <= 0:  # Handle negative IDs for remote devices
            device_id = get_or_create_default_device(cnx, database_type, user_id)
            print(f"Using default device with ID: {device_id}")
        else:
            print(f"Using provided device ID: {device_id}")

        # Use provided device_name or get from database
        if device_name is None:
            cursor = cnx.cursor()
            if database_type == "postgresql":
                query = 'SELECT DeviceName FROM "GpodderDevices" WHERE DeviceID = %s'
            else:
                query = "SELECT DeviceName FROM GpodderDevices WHERE DeviceID = %s"
            cursor.execute(query, (device_id,))
            result = cursor.fetchone()
            if result:
                device_name = result[0] if isinstance(result, tuple) else result["devicename"]
                print(f"Found device name from database: {device_name}")
            else:
                # Fallback to default name if query returns nothing
                device_name = "pinepods_default"
                print(f"No device name found, using default: {device_name}")
            cursor.close()
        else:
            print(f"Using provided device name: {device_name}")

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
            print("Warning: No GPodder token provided")

        # Create auth
        auth = HTTPBasicAuth(gpodder_login, gpodder_token)

        # Get all local podcasts
        cursor = cnx.cursor()
        if database_type == "postgresql":
            query = 'SELECT FeedURL FROM "Podcasts" WHERE UserID = %s'
        else:
            query = "SELECT FeedURL FROM Podcasts WHERE UserID = %s"
        cursor.execute(query, (user_id,))

        local_podcasts = []
        for row in cursor.fetchall():
            if isinstance(row, dict):
                local_podcasts.append(row["feedurl"])
            else:
                local_podcasts.append(row[0])

        print(f"Found {len(local_podcasts)} local podcasts to sync")

        # Create payload for PUT request
        try:
            # Try to login first to establish a session
            session = requests.Session()
            login_url = f"{gpodder_url}/api/2/auth/{gpodder_login}/login.json"
            print(f"Logging in to GPodder at: {login_url}")
            login_response = session.post(login_url, auth=auth)
            login_response.raise_for_status()
            print("Session login successful for full sync")

            # Use PUT request to update subscriptions
            subscription_url = f"{gpodder_url}/api/2/subscriptions/{gpodder_login}/{device_name}.json"
            print(f"Sending PUT request to: {subscription_url}")

            # Debug the payload
            print(f"Sending payload: {local_podcasts[:3]}... (showing first 3 of {len(local_podcasts)})")

            response = session.put(
                subscription_url,
                json=local_podcasts,
                headers={"Content-Type": "application/json"}
            )

            # Check response
            print(f"PUT response status: {response.status_code}")
            print(f"PUT response text: {response.text[:200]}...")  # Show first 200 chars

            response.raise_for_status()
            print(f"Successfully pushed all podcasts to GPodder")
            return True

        except Exception as e:
            print(f"Session-based sync failed: {str(e)}. Falling back to basic auth.")
            try:
                # Try a different method - POST with the update API
                try:
                    print("Trying POST to subscriptions-update API...")
                    update_url = f"{gpodder_url}/api/2/subscriptions/{gpodder_login}/{device_name}.json"
                    payload = {
                        "add": local_podcasts,
                        "remove": []
                    }
                    response = session.post(
                        update_url,
                        json=payload,
                        headers={"Content-Type": "application/json"}
                    )
                    response.raise_for_status()
                    print(f"Successfully updated podcasts using POST method")
                    return True
                except Exception as e3:
                    print(f"Failed with POST method: {str(e3)}")

                # Fall back to basic auth with PUT
                print("Falling back to basic auth with PUT...")
                subscription_url = f"{gpodder_url}/api/2/subscriptions/{gpodder_login}/{device_name}.json"
                response = requests.put(
                    subscription_url,
                    json=local_podcasts,
                    auth=auth,
                    headers={"Content-Type": "application/json"}
                )

                # Check response
                print(f"Basic auth PUT response status: {response.status_code}")
                print(f"Basic auth PUT response text: {response.text[:200]}...")  # Show first 200 chars

                response.raise_for_status()
                print(f"Successfully pushed all podcasts to GPodder using basic auth")
                return True
            except Exception as e2:
                print(f"Failed to push podcasts with basic auth: {str(e2)}")
                return False

    except Exception as e:
        print(f"Error in force_full_sync_to_gpodder: {str(e)}")
        return False

def sync_subscription_change_gpodder_with_device(gpodder_url, gpodder_login, auth, device_name, add=None, remove=None):
    """Sync subscription changes using device name"""
    import requests
    import logging

    logger = logging.getLogger(__name__)

    add = add or []
    remove = remove or []

    payload = {
        "add": add,
        "remove": remove
    }

    try:
        response = requests.post(
            f"{gpodder_url}/api/2/subscriptions/{gpodder_login}/{device_name}.json",
            json=payload,
            auth=auth
        )
        response.raise_for_status()
        logger.info(f"Subscription changes synced with gPodder: {response.text}")
        return response.json()
    except Exception as e:
        logger.error(f"Error syncing subscription changes: {str(e)}")
        return None

def sync_subscription_change_gpodder_session_with_device(session, gpodder_url, gpodder_login, device_name, add=None, remove=None):
    """Sync subscription changes using session-based authentication with device name"""
    import logging

    logger = logging.getLogger(__name__)

    add = add or []
    remove = remove or []

    payload = {
        "add": add,
        "remove": remove
    }

    try:
        response = session.post(
            f"{gpodder_url}/api/2/subscriptions/{gpodder_login}/{device_name}.json",
            json=payload,
            headers={"Content-Type": "application/json"}
        )
        response.raise_for_status()
        logger.info(f"Subscription changes synced with gPodder using session: {response.text}")
        return response.json()
    except Exception as e:
        logger.error(f"Error syncing subscription changes with session: {str(e)}")
        return None

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

# Functions for OIDC

def get_oidc_provider(cnx, database_type, client_id):
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            query = """
                SELECT ProviderID, ClientID, ClientSecret, TokenURL, UserInfoURL
                FROM "OIDCProviders"
                WHERE ClientID = %s AND Enabled = true
            """
        else:
            query = """
                SELECT ProviderID, ClientID, ClientSecret, TokenURL, UserInfoURL
                FROM OIDCProviders
                WHERE ClientID = %s AND Enabled = true
            """
        cursor.execute(query, (client_id,))
        result = cursor.fetchone()
        if result:
            if isinstance(result, dict):
                return (
                    result['providerid'],
                    result['clientid'],
                    result['clientsecret'],
                    result['tokenurl'],
                    result['userinfourl']
                )
            return result
        return None
    finally:
        cursor.close()

def get_user_by_email(cnx, database_type, email):
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            query = """
                SELECT UserID, Email, Username, Fullname, IsAdmin
                FROM "Users"
                WHERE Email = %s
            """
        else:
            query = """
                SELECT UserID, Email, Username, Fullname, IsAdmin
                FROM Users
                WHERE Email = %s
            """
        cursor.execute(query, (email,))
        result = cursor.fetchone()
        if result:
            if isinstance(result, dict):
                return (
                    result['userid'],
                    result['email'],
                    result['username'],
                    result['fullname'],
                    result['isadmin']
                )
            return result
        return None
    finally:
        cursor.close()

def create_oidc_user(cnx, database_type, email, fullname, base_username):
    cursor = cnx.cursor()
    try:
        print(f"Starting create_oidc_user for email: {email}, fullname: {fullname}, base_username: {base_username}")
        # Check if username exists and find a unique one
        username = base_username
        counter = 1
        while True:
            # Check if username exists
            check_query = """
                SELECT COUNT(*) FROM "Users" WHERE Username = %s
            """ if database_type == "postgresql" else """
                SELECT COUNT(*) FROM Users WHERE Username = %s
            """
            print(f"Checking if username '{username}' exists")
            cursor.execute(check_query, (username,))
            result = cursor.fetchone()
            print(f"Username check result: {result}, type: {type(result)}")

            count = 0
            if isinstance(result, tuple):
                count = result[0]
            elif isinstance(result, dict):
                count = result.get('count', 0)
            else:
                # Try to extract the count value safely
                try:
                    count = int(result)
                except (TypeError, ValueError):
                    print(f"Unable to extract count from result: {result}")
                    count = 1  # Assume username exists to be safe

            print(f"Username count: {count}")
            if count == 0:
                print(f"Username '{username}' is unique, proceeding")
                break  # Username is unique

            # Try with incremented counter
            print(f"Username '{username}' already exists, trying next")
            username = f"{base_username}{counter}"
            counter += 1
            if counter > 10:  # Limit attempts
                raise Exception("Could not find a unique username")

        # Create a random salt using base64 (which is what Argon2 expects)
        salt = base64.b64encode(secrets.token_bytes(16)).decode('utf-8')
        # Create an impossible-to-match hash that's clearly marked as OIDC
        # Using proper Argon2id format but with an impossible hash
        hashed_password = f"$argon2id$v=19$m=65536,t=3,p=4${salt}${'X' * 43}_OIDC_ACCOUNT_NO_PASSWORD"

        print(f"Inserting new user with username: {username}, email: {email}")
        # Insert user
        if database_type == "postgresql":
            query = """
                INSERT INTO "Users"
                (Fullname, Username, Email, Hashed_PW, IsAdmin)
                VALUES (%s, %s, %s, %s, false)
                RETURNING UserID
            """
        else:
            query = """
                INSERT INTO Users
                (Fullname, Username, Email, Hashed_PW, IsAdmin)
                VALUES (%s, %s, %s, %s, 0)
            """
        cursor.execute(query, (fullname, username, email, hashed_password))

        # Get user ID
        if database_type == "postgresql":
            result = cursor.fetchone()
            print(f"PostgreSQL INSERT result: {result}, type: {type(result)}")

            if result is None:
                print("ERROR: No result returned from INSERT RETURNING")
                raise Exception("No user ID returned from database after insertion")

            # Handle different result types
            if isinstance(result, tuple):
                print(f"Result is tuple: {result}")
                user_id = result[0]
            elif isinstance(result, dict):
                print(f"Result is dict: {result}")
                # Note: PostgreSQL column names are lowercase by default
                user_id = result.get('userid')
                if user_id is None:
                    # Try other possible key variations
                    user_id = result.get('UserID') or result.get('userID') or result.get('user_id')
            else:
                print(f"Unexpected result type: {type(result)}, value: {result}")
                # Try to extract user_id safely
                try:
                    # Try accessing as a number
                    user_id = int(result)
                except (TypeError, ValueError):
                    # If that fails, convert to string and raise exception
                    result_str = str(result)
                    print(f"Result as string: {result_str}")
                    raise Exception(f"Unable to extract user_id from result: {result_str}")
        else:
            user_id = cursor.lastrowid
            print(f"MySQL lastrowid: {user_id}")

        print(f"Extracted user_id: {user_id}, type: {type(user_id)}")

        if not user_id:
            print("ERROR: user_id is empty or zero")
            raise Exception("Invalid user_id after user creation")

        # Add default user settings
        print(f"Inserting default user settings for user_id: {user_id}")
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

        # Add default user stats
        print(f"Inserting default user stats for user_id: {user_id}")
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

        print(f"Committing transaction")
        cnx.commit()
        print(f"User creation complete, returning user_id: {user_id}")
        return user_id
    except Exception as e:
        print(f"Error in create_oidc_user: {str(e)}")
        import traceback
        print(f"Traceback: {traceback.format_exc()}")
        cnx.rollback()
        raise
    finally:
        cursor.close()

def get_user_startpage(cnx, database_type, user_id):
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            query = """
                SELECT StartPage
                FROM "UserSettings"
                WHERE UserID = %s
            """
        else:
            query = """
                SELECT StartPage
                FROM UserSettings
                WHERE UserID = %s
            """

        cursor.execute(query, (user_id,))
        result = cursor.fetchone()

        # Return 'home' as default if no setting is found
        if result:
            return result[0] if isinstance(result, tuple) else result['startpage']
        return 'home'

    except Exception as e:
        raise
    finally:
        cursor.close()

def set_user_startpage(cnx, database_type, user_id, startpage):
    cursor = cnx.cursor()
    try:
        if database_type == "postgresql":
            query = """
                UPDATE "UserSettings"
                SET StartPage = %s
                WHERE UserID = %s
            """
        else:
            query = """
                UPDATE UserSettings
                SET StartPage = %s
                WHERE UserID = %s
            """

        cursor.execute(query, (startpage, user_id))
        cnx.commit()
        return True

    except Exception as e:
        cnx.rollback()
        raise
    finally:
        cursor.close()


def convert_booleans(data):
    boolean_fields = ['completed', 'saved', 'queued', 'downloaded', 'is_youtube', 'explicit', 'is_system_playlist', 'include_unplayed', 'include_partially_played', 'include_played']

    if isinstance(data, dict):
        for key, value in data.items():
            if key in boolean_fields and value is not None:
                # Convert 0/1 to False/True for known boolean fields
                data[key] = bool(value)
            elif isinstance(value, (dict, list)):
                # Recursively process nested dictionaries and lists
                data[key] = convert_booleans(value)
    elif isinstance(data, list):
        # Process each item in the list
        for i, item in enumerate(data):
            data[i] = convert_booleans(item)

    return data

def get_home_overview(database_type, cnx, user_id):
    if database_type == "postgresql":
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
    else:
        cursor = cnx.cursor(dictionary=True)

    home_data = {
        "recent_episodes": [],
        "in_progress_episodes": [],
        "top_podcasts": [],
        "saved_count": 0,
        "downloaded_count": 0,
        "queue_count": 0
    }

    # Recent Episodes query with is_youtube field
    if database_type == "postgresql":
        recent_query = """
            SELECT
                "Episodes".EpisodeID,
                "Episodes".EpisodeTitle,
                "Episodes".EpisodePubDate,
                "Episodes".EpisodeDescription,
                "Episodes".EpisodeArtwork,
                "Episodes".EpisodeURL,
                "Episodes".EpisodeDuration,
                "Episodes".Completed,
                "Podcasts".PodcastName,
                "Podcasts".PodcastID,
                "Podcasts".IsYouTubeChannel as is_youtube,
                "UserEpisodeHistory".ListenDuration,
                CASE WHEN "SavedEpisodes".EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                CASE WHEN "EpisodeQueue".EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS queued,
                CASE WHEN "DownloadedEpisodes".EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded
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
            WHERE "Podcasts".UserID = %s
                AND "Episodes".EpisodePubDate >= NOW() - INTERVAL '7 days'
            ORDER BY "Episodes".EpisodePubDate DESC
            LIMIT 10
        """
    else:  # MySQL or MariaDB
        recent_query = """
            SELECT
                Episodes.EpisodeID,
                Episodes.EpisodeTitle,
                Episodes.EpisodePubDate,
                Episodes.EpisodeDescription,
                Episodes.EpisodeArtwork,
                Episodes.EpisodeURL,
                Episodes.EpisodeDuration,
                Episodes.Completed,
                Podcasts.PodcastName,
                Podcasts.PodcastID,
                Podcasts.IsYouTubeChannel as is_youtube,
                UserEpisodeHistory.ListenDuration,
                CASE WHEN SavedEpisodes.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                CASE WHEN EpisodeQueue.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS queued,
                CASE WHEN DownloadedEpisodes.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded
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
            WHERE Podcasts.UserID = %s
                AND Episodes.EpisodePubDate >= DATE_SUB(NOW(), INTERVAL 7 DAY)
            ORDER BY Episodes.EpisodePubDate DESC
            LIMIT 10
        """

    # In Progress Episodes query with is_youtube field
    in_progress_query = """
        SELECT
            "Episodes".*,
            "Podcasts".PodcastName,
            "Podcasts".IsYouTubeChannel as is_youtube,
            "UserEpisodeHistory".ListenDuration,
            CASE WHEN "SavedEpisodes".EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
            CASE WHEN "EpisodeQueue".EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS queued,
            CASE WHEN "DownloadedEpisodes".EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded
        FROM "UserEpisodeHistory"
        JOIN "Episodes" ON "UserEpisodeHistory".EpisodeID = "Episodes".EpisodeID
        JOIN "Podcasts" ON "Episodes".PodcastID = "Podcasts".PodcastID
        LEFT JOIN "SavedEpisodes" ON
            "Episodes".EpisodeID = "SavedEpisodes".EpisodeID
            AND "SavedEpisodes".UserID = %s
        LEFT JOIN "EpisodeQueue" ON
            "Episodes".EpisodeID = "EpisodeQueue".EpisodeID
            AND "EpisodeQueue".UserID = %s
        LEFT JOIN "DownloadedEpisodes" ON
            "Episodes".EpisodeID = "DownloadedEpisodes".EpisodeID
            AND "DownloadedEpisodes".UserID = %s
        WHERE "UserEpisodeHistory".UserID = %s
        AND "UserEpisodeHistory".ListenDuration > 0
        AND "Episodes".Completed = FALSE
        ORDER BY "UserEpisodeHistory".ListenDate DESC
        LIMIT 10
    """ if database_type == "postgresql" else """
        SELECT
            Episodes.*,
            Podcasts.PodcastName,
            Podcasts.IsYouTubeChannel as is_youtube,
            UserEpisodeHistory.ListenDuration,
            CASE WHEN SavedEpisodes.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
            CASE WHEN EpisodeQueue.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS queued,
            CASE WHEN DownloadedEpisodes.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded
        FROM UserEpisodeHistory
        JOIN Episodes ON UserEpisodeHistory.EpisodeID = Episodes.EpisodeID
        JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID
        LEFT JOIN SavedEpisodes ON
            Episodes.EpisodeID = SavedEpisodes.EpisodeID
            AND SavedEpisodes.UserID = %s
        LEFT JOIN EpisodeQueue ON
            Episodes.EpisodeID = EpisodeQueue.EpisodeID
            AND EpisodeQueue.UserID = %s
        LEFT JOIN DownloadedEpisodes ON
            Episodes.EpisodeID = DownloadedEpisodes.EpisodeID
            AND DownloadedEpisodes.UserID = %s
        WHERE UserEpisodeHistory.UserID = %s
        AND UserEpisodeHistory.ListenDuration > 0
        AND Episodes.Completed = FALSE
        ORDER BY UserEpisodeHistory.ListenDate DESC
        LIMIT 10
    """

    # Top Podcasts query with all needed fields
    top_podcasts_query = """
        SELECT
            "Podcasts".PodcastID,
            "Podcasts".PodcastName,
            "Podcasts".PodcastIndexID,
            "Podcasts".ArtworkURL,
            "Podcasts".Author,
            "Podcasts".Categories,
            "Podcasts".Description,
            "Podcasts".EpisodeCount,
            "Podcasts".FeedURL,
            "Podcasts".WebsiteURL,
            "Podcasts".Explicit,
            "Podcasts".IsYouTubeChannel as is_youtube,
            COUNT(DISTINCT "UserEpisodeHistory".EpisodeID) as play_count,
            SUM("UserEpisodeHistory".ListenDuration) as total_listen_time
        FROM "Podcasts"
        LEFT JOIN "Episodes" ON "Podcasts".PodcastID = "Episodes".PodcastID
        LEFT JOIN "UserEpisodeHistory" ON "Episodes".EpisodeID = "UserEpisodeHistory".EpisodeID
        WHERE "Podcasts".UserID = %s
        GROUP BY "Podcasts".PodcastID
        ORDER BY total_listen_time DESC NULLS LAST
        LIMIT 6
    """ if database_type == "postgresql" else """
        SELECT
            Podcasts.PodcastID,
            Podcasts.PodcastName,
            Podcasts.PodcastIndexID,
            Podcasts.ArtworkURL,
            Podcasts.Author,
            Podcasts.Categories,
            Podcasts.Description,
            Podcasts.EpisodeCount,
            Podcasts.FeedURL,
            Podcasts.WebsiteURL,
            Podcasts.Explicit,
            Podcasts.IsYouTubeChannel as is_youtube,
            COUNT(DISTINCT UserEpisodeHistory.EpisodeID) as play_count,
            SUM(UserEpisodeHistory.ListenDuration) as total_listen_time
        FROM Podcasts
        LEFT JOIN Episodes ON Podcasts.PodcastID = Episodes.PodcastID
        LEFT JOIN UserEpisodeHistory ON Episodes.EpisodeID = UserEpisodeHistory.EpisodeID
        WHERE Podcasts.UserID = %s
        GROUP BY Podcasts.PodcastID
        ORDER BY total_listen_time DESC
        LIMIT 5
    """

    try:
        # Get recent episodes - need to pass 5 parameters as we have 5 placeholders
        cursor.execute(recent_query, (user_id, user_id, user_id, user_id, user_id))
        recent_results = cursor.fetchall()
        if recent_results is not None:
            home_data["recent_episodes"] = lowercase_keys(recent_results)

        # Get in progress episodes - need to pass 4 parameters as we have 4 placeholders
        cursor.execute(in_progress_query, (user_id, user_id, user_id, user_id))
        in_progress_results = cursor.fetchall()
        if in_progress_results is not None:
            home_data["in_progress_episodes"] = lowercase_keys(in_progress_results)

        # Get top podcasts
        cursor.execute(top_podcasts_query, (user_id,))
        top_podcasts_results = cursor.fetchall()
        if top_podcasts_results is not None:
            home_data["top_podcasts"] = lowercase_keys(top_podcasts_results)

        # Get counts
        if database_type == "postgresql":
            for table, key in [
                ("SavedEpisodes", "saved_count"),
                ("DownloadedEpisodes", "downloaded_count"),
                ("EpisodeQueue", "queue_count")
            ]:
                count_query = f'SELECT COUNT(*) FROM "{table}" WHERE userid = %s'
                cursor.execute(count_query, (user_id,))
                count_result = cursor.fetchone()
                if count_result is not None:
                    home_data[key] = count_result[0] if isinstance(count_result, tuple) else count_result.get('count', 0)

    except Exception as e:
        print(f"Error fetching home overview: {e}")
        print(f"Error type: {type(e)}")
        import traceback
        traceback.print_exc()
        return None
    finally:
        cursor.close()

    if database_type != "postgresql":
        home_data = convert_booleans(home_data)

    return lowercase_keys(home_data)
