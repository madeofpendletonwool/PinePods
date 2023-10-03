# Fast API
from fastapi import FastAPI, Depends, HTTPException, status, Header, Body, Path, Form, Query, \
    BackgroundTasks
from fastapi.security import APIKeyHeader
from fastapi.responses import PlainTextResponse

# Needed Modules
from passlib.context import CryptContext
import mysql.connector
from mysql.connector import pooling
from mysql.connector.pooling import MySQLConnectionPool
from mysql.connector import Error
import psycopg2
from psycopg2 import pool as pg_pool
from psycopg2.extras import RealDictCursor
import os
from fastapi.middleware.gzip import GZipMiddleware
from starlette.middleware.sessions import SessionMiddleware
from starlette.requests import Request
import secrets
from pydantic import BaseModel, Field
from typing import Dict
from typing import List
from typing import Optional
from typing import Generator
import json
import logging
import argparse
import sys
from pyotp import TOTP
import base64
import traceback

# Internal Modules
sys.path.append('/pinepods')

import database_functions.functions
import Auth.Passfunctions

database_type = str(os.getenv('DB_TYPE', 'mariadb'))
if database_type == "postgresql":
    print(f"You've selected a postgresql database.")
else:
    print("You've selected a mariadb database")

secret_key_middle = secrets.token_hex(32)
debug_mode = os.environ.get("DEBUG_MODE", "False") == "True"
if debug_mode == "True":
    logging.basicConfig(level=logging.INFO)
else:
    logging.basicConfig(level=logging.ERROR)

print('Client API Server is Starting!')

app = FastAPI()
app.add_middleware(GZipMiddleware, minimum_size=1000)
app.add_middleware(SessionMiddleware, secret_key=secret_key_middle)

API_KEY_NAME = "pinepods_api"
api_key_header = APIKeyHeader(name=API_KEY_NAME, auto_error=False)

pwd_context = CryptContext(schemes=["bcrypt"], deprecated="auto")

# Proxy variables
proxy_host = os.environ.get("HOSTNAME", "localhost")
proxy_port = os.environ.get("PINEPODS_PORT", "8040")
proxy_protocol = os.environ.get("PROXY_PROTOCOL", "http")
reverse_proxy = os.environ.get("REVERSE_PROXY", "False")

# Podcast Index API url
api_url = os.environ.get("SEARCH_API_URL", "https://api.pinepods.online/api/search")
print(f'Search API URL: {api_url}')

# Initial Vars needed to start and used throughout
if reverse_proxy == "True":
    proxy_url = f'{proxy_protocol}://{proxy_host}/mover/?url='
else:
    proxy_url = f'{proxy_protocol}://{proxy_host}:{proxy_port}/mover/?url='
print(f'Proxy url is configured to {proxy_url}')

logger = logging.getLogger(__name__)


def get_database_connection():
    try:
        db = connection_pool.getconn() if database_type == "postgresql" else connection_pool.get_connection()
        yield db
    except HTTPException:
        raise  # Re-raise the HTTPException to let FastAPI handle it properly
    except Exception as e:
        logger.error(f"Database connection error of type {type(e).__name__} with arguments: {e.args}")
        logger.error(traceback.format_exc())
        raise HTTPException(500, "Unable to connect to the database")
    finally:
        if database_type == "postgresql":
            connection_pool.putconn(db)
        else:
            db.close()


def setup_connection_pool():
    db_host = os.environ.get("DB_HOST", "127.0.0.1")
    db_port = os.environ.get("DB_PORT", "3306")
    db_user = os.environ.get("DB_USER", "root")
    db_password = os.environ.get("DB_PASSWORD", "password")
    db_name = os.environ.get("DB_NAME", "pypods_database")

    if database_type == "postgresql":
        return pg_pool.SimpleConnectionPool(
            1,  # minconn
            32,  # maxconn
            host=db_host,
            port=db_port,
            user=db_user,
            password=db_password,
            dbname=db_name
        )
    else:  # Default to MariaDB/MySQL
        return pooling.MySQLConnectionPool(
            pool_name="pinepods_api_pool",
            pool_size=32,
            pool_reset_session=True,
            host=db_host,
            port=db_port,
            user=db_user,
            password=db_password,
            database=db_name,
        )


connection_pool = setup_connection_pool()


def get_api_keys(cnx):
    logging.info("Executing get_api_keys function...")
    if database_type == "postgresql":
        cursor = cnx.cursor(cursor_factory=RealDictCursor)
    else:  # Assuming MariaDB/MySQL if not PostgreSQL
        cursor = cnx.cursor(dictionary=True)

    query = "SELECT * FROM APIKeys"
    try:
        cursor.execute(query)
        rows = cursor.fetchall()
    except Exception as e:
        logging.error(f"Database error: {e}")
        raise
    logging.info(f"Retrieved API keys: {rows}")

    cursor.close()
    return rows


def get_api_key(request: Request, api_key: str = Depends(api_key_header),
                cnx: Generator = Depends(get_database_connection)):
    if api_key is None:
        raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED, detail="API key is missing")

    api_keys = get_api_keys(cnx)

    for api_key_entry in api_keys:
        stored_key = api_key_entry.get("APIKey".lower(), None)
        client_id = api_key_entry.get("APIKeyID".lower(), None)

        if api_key == stored_key:  # Direct comparison instead of using Passlib
            request.session["api_key"] = api_key  # Store the API key in the session
            return client_id

    raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED, detail="Invalid API key")


def get_api_key_from_header(api_key: str = Header(None, name="Api-Key")):
    if not api_key:
        raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED, detail="Not authenticated")
    return api_key


class Web_Key:
    def __init__(self):
        self.web_key = None

    def get_web_key(self, cnx):
        self.web_key = database_functions.functions.get_web_key(cnx)


base_webkey = Web_Key()


# Get a direct database connection
def direct_database_connection():
    try:
        if database_type == "postgresql":
            return connection_pool.getconn()
        else:
            return connection_pool.get_connection()
    except Exception as e:
        logger.error(f"Database connection error of type {type(e).__name__} with arguments: {e.args}")
        logger.error(traceback.format_exc())
        raise RuntimeError("Unable to connect to the database")


# Use the non-generator version in your script initialization
cnx = direct_database_connection()
base_webkey.get_web_key(cnx)


# Close the connection if needed, or manage it accordingly


# @app.get('/api/data')
# async def get_data(client_id: str = Depends(get_api_key)):
#     try:
#         return {"status": "success", "data": "Your data"}
#     except Exception as e:
#         logging.error(f"Error in /api/data endpoint: {e}")
#         raise

async def check_if_admin(api_key: str = Depends(get_api_key_from_header), cnx=Depends(get_database_connection)):
    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key  # Ensure base_webkey.web_key is defined elsewhere

    # If it's the web key, allow the request (return True)
    if is_web_key:
        return True

    # Get user ID associated with the API key
    user_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # If no user ID found, throw an exception
    if not user_id:
        raise HTTPException(status_code=403, detail="Invalid API key.")

    # Check if the user is an admin
    is_admin = database_functions.functions.user_admin_check(cnx, user_id)

    # If the user is not an admin, throw an exception
    if not is_admin:
        raise HTTPException(status_code=403, detail="User not authorized.")

    # If all checks pass, allow the request (return True)
    return True


async def check_if_admin_inner(api_key: str, cnx):
    user_id = database_functions.functions.id_from_api_key(cnx, api_key)

    if not user_id:
        return False

    return database_functions.functions.user_admin_check(cnx, user_id)


async def has_elevated_access(api_key: str, cnx):
    # Check if it's an admin
    is_admin = await check_if_admin_inner(api_key, cnx)

    # Check if it's the web key
    web_key = base_webkey.web_key
    is_web_key = api_key == web_key

    return is_admin or is_web_key


@app.get('/api/pinepods_check')
async def pinepods_check():
    return {"status_code": 200, "pinepods_instance": True}


@app.post("/api/data/clean_expired_sessions/")
async def api_clean_expired_sessions(cnx=Depends(get_database_connection),
                                     api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if is_valid_key:
        database_functions.functions.clean_expired_sessions(cnx)
        return {"status": "success"}
    else:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")


@app.get("/api/data/check_saved_session/{session_value}", response_model=int)
async def api_check_saved_session(session_value: str, cnx=Depends(get_database_connection),
                                  api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if is_valid_key:
        result = database_functions.functions.check_saved_session(cnx, session_value)
        if result:
            return result
        else:
            raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="No saved session found")
    else:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")


@app.get("/api/data/config")
async def api_config(api_key: str = Depends(get_api_key_from_header), cnx=Depends(get_database_connection)):
    global api_url, proxy_url, proxy_host, proxy_port, proxy_protocol, reverse_proxy

    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if is_valid_key:
        return {
            "api_url": api_url,
            "proxy_url": proxy_url,
            "proxy_host": proxy_host,
            "proxy_port": proxy_port,
            "proxy_protocol": proxy_protocol,
            "reverse_proxy": reverse_proxy,
        }
    else:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")


@app.get("/api/data/guest_status", response_model=bool)
async def api_guest_status(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if is_valid_key:
        result = database_functions.functions.guest_status(cnx)
        return result
    else:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")


@app.get("/api/data/download_status", response_model=bool)
async def api_download_status(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if is_valid_key:
        result = database_functions.functions.download_status(cnx)
        return result
    else:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")


@app.get("/api/data/user_details/{username}")
async def api_get_user_details(username: str, cnx=Depends(get_database_connection),
                               api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)

    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    elevated_access = await has_elevated_access(api_key, cnx)

    if not elevated_access:
        # Get user ID from username
        user_id_from_username = database_functions.functions.get_user_id(cnx, username)

        # Get user ID from API key
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, api_key)

        if user_id_from_username != user_id_from_api_key:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN,
                                detail="You are not authorized to access these user details")

    result = database_functions.functions.get_user_details(cnx, username)
    if result:
        return result
    else:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="User not found")


class SessionData(BaseModel):
    session_token: str


@app.post("/api/data/create_session/{user_id}")
async def api_create_session(user_id: int, session_data: SessionData, cnx=Depends(get_database_connection),
                             api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == user_id or is_web_key:
        database_functions.functions.create_session(cnx, user_id, session_data.session_token)
        return {"status": "success"}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only make sessions for yourself!")


class VerifyPasswordInput(BaseModel):
    username: str
    password: str


@app.post("/api/data/verify_password/")
async def api_verify_password(data: VerifyPasswordInput, cnx=Depends(get_database_connection),
                              api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if is_valid_key:
        if database_type == 'postgresql':
            print('run in postgres')
            is_password_valid = database_functions.functions.verify_password(cnx, data.username, data.password)
        else:
            is_password_valid = Auth.Passfunctions.verify_password(cnx, data.username, data.password)
        return {"is_password_valid": is_password_valid}
    else:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")


@app.get("/api/data/return_episodes/{user_id}")
async def api_return_episodes(user_id: int, cnx=Depends(get_database_connection),
                              api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user, or it's the web API key
    if key_id == user_id or is_web_key:
        episodes = database_functions.functions.return_episodes(database_type, cnx, user_id)
        if episodes is None:
            episodes = []  # Return an empty list instead of raising an exception
        return {"episodes": episodes}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only return episodes of your own!")


@app.post("/api/data/check_episode_playback")
async def api_check_episode_playback(
        user_id: int = Form(...),
        episode_title: Optional[str] = Form(None),
        episode_url: Optional[str] = Form(None),
        cnx=Depends(get_database_connection),
        api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == user_id or is_web_key:
        logging.info(f"Received: user_id={user_id}, episode_title={episode_title}, episode_url={episode_url}")

        has_playback, listen_duration = database_functions.functions.check_episode_playback(
            cnx, user_id, episode_title, episode_url
        )
        if has_playback:
            logging.info("Playback found, listen_duration={}".format(listen_duration))
            return {"has_playback": True, "listen_duration": listen_duration}
        else:
            logging.info("No playback found")
            return {"has_playback": False, "listen_duration": 0}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only check playback for yourself!")


@app.get("/api/data/user_details_id/{user_id}")
async def api_get_user_details_id(user_id: int, cnx=Depends(get_database_connection),
                                  api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)

    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    elevated_access = await has_elevated_access(api_key, cnx)

    if not elevated_access:
        # Get user ID from API key
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, api_key)

        if user_id != user_id_from_api_key:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN,
                                detail="You are not authorized to access these user details")
    result = database_functions.functions.get_user_details_id(cnx, user_id)
    if result:
        return result
    else:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="User not found")


@app.get("/api/data/get_theme/{user_id}")
async def api_get_theme(user_id: int, cnx=Depends(get_database_connection),
                        api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user, or it's the web API key
    if key_id == user_id or is_web_key:
        theme = database_functions.functions.get_theme(cnx, user_id)
        return {"theme": theme}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only make sessions for yourself!")


@app.post("/api/data/add_podcast")
async def api_add_podcast(podcast_values: str = Form(...), user_id: int = Form(...),
                          cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == user_id or is_web_key:
        podcast_values = json.loads(podcast_values)
        result = database_functions.functions.add_podcast(cnx, podcast_values, user_id)
        if result:
            return {"success": True}
        else:
            return {"success": False}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only make sessions for yourself!")


@app.post("/api/data/enable_disable_guest")
async def api_enable_disable_guest(is_admin: bool = Depends(check_if_admin), cnx=Depends(get_database_connection)):
    database_functions.functions.enable_disable_guest(cnx)
    return {"success": True}


@app.post("/api/data/enable_disable_downloads")
async def api_enable_disable_downloads(is_admin: bool = Depends(check_if_admin), cnx=Depends(get_database_connection)):
    database_functions.functions.enable_disable_downloads(cnx)
    return {"success": True}


@app.post("/api/data/enable_disable_self_service")
async def api_enable_disable_self_service(is_admin: bool = Depends(check_if_admin),
                                          cnx=Depends(get_database_connection)):
    database_functions.functions.enable_disable_self_service(cnx)
    return {"success": True}


@app.get("/api/data/self_service_status")
async def api_self_service_status(cnx=Depends(get_database_connection),
                                  api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if is_valid_key:
        status = database_functions.functions.self_service_status(cnx)
        return {"status": status}
    else:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")


@app.put("/api/data/increment_listen_time/{user_id}")
async def api_increment_listen_time(user_id: int, cnx=Depends(get_database_connection),
                                    api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user, or it's the web API key
    if key_id == user_id or is_web_key:
        database_functions.functions.increment_listen_time(cnx, user_id)
        return {"detail": "Listen time incremented."}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only increment your own listen time.")


@app.put("/api/data/increment_played/{user_id}")
async def api_increment_played(user_id: int, cnx=Depends(get_database_connection),
                               api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == user_id or is_web_key:
        database_functions.functions.increment_played(cnx, user_id)
        return {"detail": "Played count incremented."}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only increment your own play count.")


class RecordHistoryData(BaseModel):
    episode_title: str
    user_id: int
    episode_pos: float


@app.post("/api/data/record_podcast_history")
async def api_record_podcast_history(data: RecordHistoryData, cnx=Depends(get_database_connection),
                                     api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user, or it's the web API key
    if key_id == data.user_id or is_web_key:
        database_functions.functions.record_podcast_history(cnx, data.episode_title, data.user_id, data.episode_pos)
        return {"detail": "Podcast history recorded."}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only make sessions for yourself!")


class DownloadPodcastData(BaseModel):
    episode_url: str
    title: str
    user_id: int


@app.post("/api/data/download_podcast")
async def api_download_podcast(data: DownloadPodcastData, cnx=Depends(get_database_connection),
                               api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == data.user_id or is_web_key:
        result = database_functions.functions.download_podcast(cnx, data.episode_url, data.title, data.user_id)
        if result:
            return {"detail": "Podcast downloaded."}
        else:
            raise HTTPException(status_code=400, detail="Error downloading podcast.")
    else:
        raise HTTPException(status_code=403,
                            detail="You can only make sessions for yourself!")


class DeletePodcastData(BaseModel):
    episode_url: str
    title: str
    user_id: int


@app.post("/api/data/delete_podcast")
async def api_delete_podcast(data: DeletePodcastData, cnx=Depends(get_database_connection),
                             api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == data.user_id or is_web_key:
        database_functions.functions.delete_podcast(cnx, data.episode_url, data.title, data.user_id)
        return {"detail": "Podcast deleted."}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only delete podcasts for yourself!")


class SaveEpisodeData(BaseModel):
    episode_url: str
    title: str
    user_id: int


@app.post("/api/data/save_episode")
async def api_save_episode(data: SaveEpisodeData, cnx=Depends(get_database_connection),
                           api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == data.user_id or is_web_key:
        success = database_functions.functions.save_episode(cnx, data.episode_url, data.title, data.user_id)
        if success:
            return {"detail": "Episode saved."}
        else:
            raise HTTPException(status_code=400, detail="Error saving episode.")
    else:
        raise HTTPException(status_code=403,
                            detail="You can only save episodes of your own!")


class RemoveSavedEpisodeData(BaseModel):
    episode_url: str
    title: str
    user_id: int


@app.post("/api/data/remove_saved_episode")
async def api_remove_saved_episode(data: RemoveSavedEpisodeData, cnx=Depends(get_database_connection),
                                   api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if is_valid_key:
        key_id = database_functions.functions.id_from_api_key(cnx, api_key)
        if key_id == data.user_id:
            database_functions.functions.remove_saved_episode(cnx, data.episode_url, data.title, data.user_id)
            return {"detail": "Saved episode removed."}
        else:
            raise HTTPException(status_code=403,
                                detail="You can only return episodes of your own!")
    else:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")


class RecordListenDurationData(BaseModel):
    episode_url: str
    title: str
    user_id: int
    listen_duration: float


@app.post("/api/data/record_listen_duration")
async def api_record_listen_duration(data: RecordListenDurationData, cnx=Depends(get_database_connection),
                                     api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == data.user_id or is_web_key:
        database_functions.functions.record_listen_duration(cnx, data.episode_url, data.title, data.user_id,
                                                            data.listen_duration)
        return {"detail": "Listen duration recorded."}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only record your own listen duration")


@app.get("/api/data/refresh_pods")
async def api_refresh_pods(background_tasks: BackgroundTasks, is_admin: bool = Depends(check_if_admin),
                           cnx=Depends(get_database_connection)):
    background_tasks.add_task(database_functions.functions.refresh_pods, cnx)
    return {"detail": "Refresh initiated."}


@app.get("/api/data/get_stats")
async def api_get_stats(user_id: int, cnx=Depends(get_database_connection),
                        api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == user_id or is_web_key:
        stats = database_functions.functions.get_stats(cnx, user_id)
        return stats
    else:
        raise HTTPException(status_code=403,
                            detail="You can only get stats for your own account.")


@app.get("/api/data/get_user_episode_count")
async def api_get_user_episode_count(user_id: int, cnx=Depends(get_database_connection),
                                     api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)

    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    elevated_access = await has_elevated_access(api_key, cnx)

    if not elevated_access:
        # Get user ID from API key
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, api_key)

        if user_id != user_id_from_api_key:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN,
                                detail="You are not authorized to access these user details")
        episode_count = database_functions.functions.get_user_episode_count(cnx, user_id)
        if episode_count:
            return episode_count
        else:
            raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="User not found")


@app.get("/api/data/get_user_info")
async def api_get_user_info(is_admin: bool = Depends(check_if_admin), cnx=Depends(get_database_connection)):
    user_info = database_functions.functions.get_user_info(database_type, cnx)
    return user_info


class CheckPodcastData(BaseModel):
    user_id: int
    podcast_name: str


@app.post("/api/data/check_podcast", response_model=Dict[str, bool])
async def api_check_podcast(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header),
                            data: CheckPodcastData = Body(...)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if is_valid_key:
        exists = database_functions.functions.check_podcast(cnx, data.user_id, data.podcast_name)
        return {"exists": exists}
    else:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")


@app.get("/api/data/user_admin_check/{user_id}")
async def api_user_admin_check_route(user_id: int, is_admin: bool = Depends(check_if_admin),
                                     cnx=Depends(get_database_connection)):
    is_admin = database_functions.functions.user_admin_check(cnx, user_id)
    return {"is_admin": is_admin}


class RemovePodcastData(BaseModel):
    user_id: int
    podcast_name: str


@app.post("/api/data/remove_podcast")
async def api_remove_podcast_route(data: RemovePodcastData = Body(...), cnx=Depends(get_database_connection),
                                   api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)

    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    elevated_access = await has_elevated_access(api_key, cnx)

    if not elevated_access:
        # Get user ID from API key
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, api_key)

        if data.user_id != user_id_from_api_key:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN,
                                detail="You are not authorized to remove podcasts for other users")
    database_functions.functions.remove_podcast(cnx, data.podcast_name, data.user_id)
    return {"status": "Podcast removed"}


@app.get("/api/data/return_pods/{user_id}")
async def api_return_pods(user_id: int, cnx=Depends(get_database_connection),
                          api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == user_id or is_web_key:
        pods = database_functions.functions.return_pods(database_type, cnx, user_id)
        return {"pods": pods}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only return pods for yourself!")


@app.get("/api/data/user_history/{user_id}")
async def api_user_history(user_id: int, cnx=Depends(get_database_connection),
                           api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == user_id or is_web_key:
        history = database_functions.functions.user_history(cnx, user_id)
        return {"history": history}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only return history for yourself!")


@app.get("/api/data/saved_episode_list/{user_id}")
async def api_saved_episode_list(user_id: int, cnx=Depends(get_database_connection),
                                 api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == user_id or is_web_key:
        saved_episodes = database_functions.functions.saved_episode_list(database_type, cnx, user_id)
        return {"saved_episodes": saved_episodes}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only return saved episodes for yourself!")


@app.post("/api/data/download_episode_list")
async def api_download_episode_list(cnx=Depends(get_database_connection),
                                    api_key: str = Depends(get_api_key_from_header), user_id: int = Form(...)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == user_id or is_web_key:
        downloaded_episodes = database_functions.functions.download_episode_list(database_type, cnx, user_id)
        return {"downloaded_episodes": downloaded_episodes}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only return downloaded episodes for yourself!")


@app.post("/api/data/return_selected_episode")
async def api_return_selected_episode(cnx=Depends(get_database_connection),
                                      api_key: str = Depends(get_api_key_from_header), user_id: int = Body(...),
                                      title: str = Body(...), url: str = Body(...)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == user_id or is_web_key:
        episode_info = database_functions.functions.return_selected_episode(cnx, user_id, title, url)
        return {"episode_info": episode_info}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only return episode information for your own episodes!")


@app.post("/api/data/check_usernames")
async def api_check_usernames(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header),
                              username: str = Body(...)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if is_valid_key:
        result = database_functions.functions.check_usernames(cnx, username)
        return {"username_exists": result}
    else:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")


class UserValues(BaseModel):
    fullname: str
    username: str
    email: str
    hash_pw: bytes
    salt: bytes


@app.post("/api/data/add_user")
async def api_add_user(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header),
                       user_values: UserValues = Body(...)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if is_valid_key:
        # Convert base64 strings back to bytes
        hash_pw_bytes = base64.b64decode(user_values.hash_pw)
        salt_bytes = base64.b64decode(user_values.salt)
        database_functions.functions.add_user(cnx, (
            user_values.fullname, user_values.username, user_values.email, hash_pw_bytes, salt_bytes))
        return {"detail": "User added."}
    else:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")


@app.put("/api/data/set_fullname/{user_id}")
async def api_set_fullname(user_id: int, new_name: str = Query(...), cnx=Depends(get_database_connection),
                           api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)

    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    elevated_access = await has_elevated_access(api_key, cnx)

    if not elevated_access:
        # Get user ID from API key
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, api_key)

        if user_id != user_id_from_api_key:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN,
                                detail="You are not authorized to access these user details")
    try:
        database_functions.functions.set_fullname(cnx, user_id, new_name)
        return {"detail": "Fullname updated."}
    except:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="User not found")


@app.put("/api/data/set_password/{user_id}")
async def api_set_password(user_id: int, salt: str = Body(...), hash_pw: str = Body(...),
                           cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)

    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    elevated_access = await has_elevated_access(api_key, cnx)

    if not elevated_access:
        # Get user ID from API key
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, api_key)

        if user_id != user_id_from_api_key:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN,
                                detail="You are not authorized to access these user details")
    try:
        database_functions.functions.set_password(cnx, user_id, salt, hash_pw)
        return {"detail": "Password updated."}
    except:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="User not found")


@app.put("/api/data/user/set_email")
async def api_set_email(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header),
                        user_id: int = Body(...), new_email: str = Body(...)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)

    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    elevated_access = await has_elevated_access(api_key, cnx)

    if not elevated_access:
        # Get user ID from API key
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, api_key)

        if user_id != user_id_from_api_key:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN,
                                detail="You are not authorized to access these user details")
    try:
        database_functions.functions.set_email(cnx, user_id, new_email)
        return {"detail": "Email updated."}
    except:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="User not found")


@app.put("/api/data/user/set_username")
async def api_set_username(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header),
                           user_id: int = Body(...), new_username: str = Body(...)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)

    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    elevated_access = await has_elevated_access(api_key, cnx)

    if not elevated_access:
        # Get user ID from API key
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, api_key)

        if user_id != user_id_from_api_key:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN,
                                detail="You are not authorized to access these user details")
    try:
        database_functions.functions.set_username(cnx, user_id, new_username)
        return {"detail": "Username updated."}
    except:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="User not found")


@app.put("/api/data/user/set_isadmin")
async def api_set_isadmin(is_admin: bool = Depends(check_if_admin), cnx=Depends(get_database_connection),
                          user_id: int = Body(...), isadmin: bool = Body(...)):
    database_functions.functions.set_isadmin(cnx, user_id, isadmin)
    return {"detail": "IsAdmin status updated."}


@app.get("/api/data/user/final_admin/{user_id}")
async def api_final_admin(is_admin: bool = Depends(check_if_admin), cnx=Depends(get_database_connection),
                          user_id: int = Path(...)):
    is_final_admin = database_functions.functions.final_admin(cnx, user_id)
    return {"final_admin": is_final_admin}


@app.delete("/api/data/user/delete/{user_id}")
async def api_delete_user(is_admin: bool = Depends(check_if_admin), cnx=Depends(get_database_connection),
                          user_id: int = Path(...)):
    database_functions.functions.delete_user(cnx, user_id)
    return {"status": "User deleted"}


@app.put("/api/data/user/set_theme")
async def api_set_theme(user_id: int = Body(...), new_theme: str = Body(...), cnx=Depends(get_database_connection),
                        api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == user_id or is_web_key:
        database_functions.functions.set_theme(cnx, user_id, new_theme)
        return {"message": "Theme updated successfully"}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only set your own theme!")


@app.get("/api/data/user/check_downloaded")
async def api_check_downloaded(user_id: int, title: str, url: str, cnx=Depends(get_database_connection),
                               api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == user_id or is_web_key:
        is_downloaded = database_functions.functions.check_downloaded(cnx, user_id, title, url)
        return {"is_downloaded": is_downloaded}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only check your own episodes!")


@app.get("/api/data/user/check_saved")
async def api_check_saved(user_id: int, title: str, url: str, cnx=Depends(get_database_connection),
                          api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == user_id or is_web_key:
        is_saved = database_functions.functions.check_saved(cnx, user_id, title, url)
        return {"is_saved": is_saved}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only check your own episodes!")


@app.post("/api/data/create_api_key")
async def api_create_api_key(user_id: int = Body(..., embed=True), cnx=Depends(get_database_connection),
                             api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if is_valid_key:
        new_api_key = database_functions.functions.create_api_key(cnx, user_id)
        return {"api_key": new_api_key}
    else:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")


@app.post("/api/data/save_email_settings")
async def api_save_email_settings(email_settings: dict = Body(..., embed=True),
                                  is_admin: bool = Depends(check_if_admin), cnx=Depends(get_database_connection)):
    database_functions.functions.save_email_settings(cnx, email_settings)
    return {"message": "Email settings saved."}


@app.get("/api/data/get_encryption_key")
async def api_get_encryption_key(is_admin: bool = Depends(check_if_admin), cnx=Depends(get_database_connection)):
    encryption_key = database_functions.functions.get_encryption_key(cnx)
    return {"encryption_key": encryption_key}


@app.get("/api/data/get_email_settings")
async def api_get_email_settings(is_admin: bool = Depends(check_if_admin), cnx=Depends(get_database_connection)):
    email_settings = database_functions.functions.get_email_settings(cnx)
    return email_settings


class DeleteAPIKeyHeaders(BaseModel):
    api_id: str
    user_id: str


@app.delete("/api/data/delete_api_key")
async def api_delete_api_key(payload: DeleteAPIKeyHeaders, cnx=Depends(get_database_connection),
                             api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)

    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    elevated_access = await has_elevated_access(api_key, cnx)

    if not elevated_access:
        # Get user ID from API key
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, api_key)

        if payload.user_id != user_id_from_api_key:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN,
                                detail="You are not authorized to access these remove other users api-keys.")
    database_functions.functions.delete_api(cnx, payload.api_id)
    return {"detail": "API key deleted."}


@app.get("/api/data/get_api_info/{user_id}")
async def api_get_api_info(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header),
                           user_id: int = Path(...)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)

    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    elevated_access = await has_elevated_access(api_key, cnx)

    if not elevated_access:
        # Get user ID from API key
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, api_key)

        if user_id != user_id_from_api_key:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN,
                                detail="You are not authorized to access these user details")
    api_information = database_functions.functions.get_api_info(database_type, cnx)
    if api_information:
        return {"api_info": api_information}
    else:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="User not found")


class ResetPasswordPayload(BaseModel):
    email: str
    reset_code: str
    user_id: int


@app.post("/api/data/reset_password_create_code")
async def api_reset_password_route(payload: ResetPasswordPayload, cnx=Depends(get_database_connection),
                                   api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == payload.user_id or is_web_key:
        user_exists = database_functions.functions.reset_password_create_code(cnx, payload.email,
                                                                              payload.reset_code)
        return {"user_exists": user_exists}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only create codes for yourself!")


@app.post("/api/data/verify_reset_code")
async def api_verify_reset_code_route(payload: ResetPasswordPayload, cnx=Depends(get_database_connection),
                                      api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == ResetPasswordPayload.user_id or is_web_key:
        code_valid = database_functions.functions.verify_reset_code(cnx, payload.email, payload.reset_code)
        if code_valid is None:
            raise HTTPException(status_code=404, detail="User not found")
        return {"code_valid": code_valid}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only create codes for yourself!")


class ResetPasswordPayloadVerify(BaseModel):
    email: str
    salt: str
    hashed_pw: str
    user_id: int


@app.post("/api/data/reset_password_prompt")
async def api_reset_password_verify_route(payload: ResetPasswordPayloadVerify, cnx=Depends(get_database_connection),
                                          api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == payload.user_id or is_web_key:
        message = database_functions.functions.reset_password_prompt(cnx, payload.email, payload.salt,
                                                                     payload.hashed_pw)
        if message is None:
            raise HTTPException(status_code=404, detail="User not found")
        return {"message": message}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only reset your own password!")


@app.post("/api/data/clear_guest_data")
async def api_clear_guest_data(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if is_valid_key:
        message = database_functions.functions.clear_guest_data(cnx)
        if message is None:
            raise HTTPException(status_code=404, detail="User not found")
        return {"message": message}
    else:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")


class EpisodeMetadata(BaseModel):
    episode_url: str
    episode_title: str
    user_id: int


@app.post("/api/data/get_episode_metadata")
async def api_get_episode_metadata(data: EpisodeMetadata, cnx=Depends(get_database_connection),
                                   api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == data.user_id or is_web_key:
        episode = database_functions.functions.get_episode_metadata(database_type, cnx, data.episode_url,
                                                                    data.episode_title, data.user_id)
        return {"episode": episode}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only get metadata for yourself!")


class MfaSecretData(BaseModel):
    user_id: int
    mfa_secret: str


@app.post("/api/data/save_mfa_secret")
async def api_save_mfa_secret(data: MfaSecretData, cnx=Depends(get_database_connection),
                              api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == data.user_id or is_web_key:
        success = database_functions.functions.save_mfa_secret(database_type, cnx, data.user_id, data.mfa_secret)
        if success:
            return {"status": "success"}
        else:
            return {"status": "error"}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only save MFA secrets for yourself!")


@app.get("/api/data/check_mfa_enabled/{user_id}")
async def api_check_mfa_enabled(user_id: int, cnx=Depends(get_database_connection),
                                api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)

    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    elevated_access = await has_elevated_access(api_key, cnx)

    if not elevated_access:
        # Get user ID from API key
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, api_key)

        if user_id != user_id_from_api_key:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN,
                                detail="You are not authorized to check mfa status for other users.")
    logging.info(f"Database Type: {database_type}, Connection: {cnx}, User ID: {user_id}")

    is_enabled = database_functions.functions.check_mfa_enabled(database_type, cnx, user_id)
    return {"mfa_enabled": is_enabled}


class VerifyMFABody(BaseModel):
    user_id: int
    mfa_code: str


@app.post("/api/data/verify_mfa")
async def api_verify_mfa(body: VerifyMFABody, cnx=Depends(get_database_connection),
                         api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == body.user_id or is_web_key:
        secret = database_functions.functions.get_mfa_secret(database_type, cnx, body.user_id)

        if secret is None:
            return {"verified": False}
        else:
            totp = TOTP(secret)
            verification_result = totp.verify(body.mfa_code)
            return {"verified": verification_result}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only verify your own login code!")


class UserIDBody(BaseModel):
    user_id: int


@app.delete("/api/data/delete_mfa")
async def api_delete_mfa(body: UserIDBody, cnx=Depends(get_database_connection),
                         api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)

    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    elevated_access = await has_elevated_access(api_key, cnx)

    if not elevated_access:
        # Get user ID from API key
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, api_key)

        if body.user_id != user_id_from_api_key:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN,
                                detail="You are not authorized to access these user details")

    result = database_functions.functions.delete_mfa_secret(database_type, cnx, body.user_id)
    if result:
        return {"deleted": result}
    else:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="User not found")


class AllEpisodes(BaseModel):
    pod_feed: str


@app.post("/api/data/get_all_episodes")
async def api_get_episodes(data: AllEpisodes, cnx=Depends(get_database_connection),
                           api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if is_valid_key:
        episodes = database_functions.functions.get_all_episodes(database_type, cnx, data.pod_feed)
        return {"episodes": episodes}
    else:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")


class EpisodeToRemove(BaseModel):
    url: str
    title: str
    user_id: int


@app.post("/api/data/remove_episode_history")
async def api_remove_episode_from_history(data: EpisodeToRemove, cnx=Depends(get_database_connection),
                                          api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == data.user_id or is_web_key:
        success = database_functions.functions.remove_episode_history(database_type, cnx, data.url, data.title,
                                                                      data.user_id)
        return {"success": success}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only remove your own history!")


# Model for request data
class TimeZoneInfo(BaseModel):
    user_id: int
    timezone: str
    hour_pref: int


# FastAPI endpoint
@app.post("/api/data/setup_time_info")
async def setup_timezone_info(data: TimeZoneInfo, cnx=Depends(get_database_connection),
                              api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)

    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    elevated_access = await has_elevated_access(api_key, cnx)

    if not elevated_access:
        # Get user ID from API key
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, api_key)

        if TimeZoneInfo.user_id != user_id_from_api_key:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN,
                                detail="You are not authorized to access these user details")

    success = database_functions.functions.setup_timezone_info(database_type, cnx, data.user_id, data.timezone,
                                                               data.hour_pref)
    if success:
        return {"success": success}
    else:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="User not found")


@app.get("/api/data/get_time_info")
async def get_time_info(user_id: int, cnx=Depends(get_database_connection),
                        api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)

    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    elevated_access = await has_elevated_access(api_key, cnx)

    if not elevated_access:
        # Get user ID from API key
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, api_key)

        if user_id != user_id_from_api_key:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN,
                                detail="You are not authorized to access these user details")
    timezone, hour_pref = database_functions.functions.get_time_info(database_type, cnx, user_id)
    if timezone:
        return {"timezone": timezone, "hour_pref": hour_pref}
    else:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="User not found")


class UserLoginUpdate(BaseModel):
    user_id: int


@app.post("/api/data/first_login_done")
async def first_login_done(data: UserLoginUpdate, cnx=Depends(get_database_connection),
                           api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == data.user_id or is_web_key:
        first_login_status = database_functions.functions.first_login_done(database_type, cnx, data.user_id)
        return {"FirstLogin": first_login_status}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only make sessions for yourself!")


class SelectedEpisodesDelete(BaseModel):
    selected_episodes: List[int] = Field(..., title="List of Episode IDs")
    user_id: int = Field(..., title="User ID")


@app.post("/api/data/delete_selected_episodes")
async def delete_selected_episodes(data: SelectedEpisodesDelete, cnx=Depends(get_database_connection),
                                   api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == data.user_id or is_web_key:
        if is_valid_key:
            delete_status = database_functions.functions.delete_selected_episodes(cnx, data.selected_episodes,
                                                                                  data.user_id)
            return {"status": delete_status}
        else:
            raise HTTPException(status_code=403,
                                detail="Your API key is either invalid or does not have correct permission")
    else:
        raise HTTPException(status_code=403,
                            detail="You can only delete your own selected episodes!")

class SelectedPodcastsDelete(BaseModel):
    delete_list: List[int] = Field(..., title="List of Podcast IDs")
    user_id: int = Field(..., title="User ID")


@app.post("/api/data/delete_selected_podcasts")
async def delete_selected_podcasts(data: SelectedPodcastsDelete, cnx=Depends(get_database_connection),
                                   api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == data.user_id or is_web_key:
        if is_valid_key:
            delete_status = database_functions.functions.delete_selected_podcasts(cnx, data.delete_list,
                                                                                  data.user_id)
            return {"status": delete_status}
        else:
            raise HTTPException(status_code=403,
                                detail="Your API key is either invalid or does not have correct permission")
    else:
        raise HTTPException(status_code=403,
                            detail="You can only delete your own selected podcasts!")

class SearchPodcastData(BaseModel):
    search_term: str
    user_id: int


@app.post("/api/data/search_data")
async def search_data(data: SearchPodcastData, cnx=Depends(get_database_connection),
                      api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if is_valid_key:
        result = database_functions.functions.search_data(database_type, cnx, data.search_term, data.user_id)
        return {"data": result}
    else:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")


class QueuePodData(BaseModel):
    episode_title: str
    ep_url: str
    user_id: int


@app.post("/api/data/queue_pod")
async def queue_pod(data: QueuePodData, cnx=Depends(get_database_connection),
                    api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == data.user_id or is_web_key:
        result = database_functions.functions.queue_pod(database_type, cnx, data.episode_title, data.ep_url,
                                                        data.user_id)
        return {"data": result}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only add episodes to your own queue!")
class QueueRmData(BaseModel):
    episode_title: str
    ep_url: str
    user_id: int


@app.post("/api/data/remove_queued_pod")
async def remove_queued_pod(data: QueueRmData, cnx=Depends(get_database_connection),
                            api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == data.user_id or is_web_key:
        result = database_functions.functions.remove_queued_pod(database_type, cnx, data.episode_title, data.ep_url,
                                                                data.user_id)
        return {"data": result}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only remove episodes for your own queue!")
class QueuedEpisodesData(BaseModel):
    user_id: int


@app.get("/api/data/get_queued_episodes")
async def get_queued_episodes(data: QueuedEpisodesData, cnx=Depends(get_database_connection),
                              api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == data.user_id or is_web_key:
        result = database_functions.functions.get_queued_episodes(database_type, cnx, data.user_id)
        return {"data": result}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only get episodes from your own queue!")

class QueueBump(BaseModel):
    ep_url: str
    title: str
    user_id: int


@app.post("/api/data/queue_bump")
async def queue_bump(data: QueueBump, cnx=Depends(get_database_connection),
                     api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == data.user_id or is_web_key:
        try:
            result = database_functions.functions.queue_bump(cnx, data.ep_url, data.title, data.user_id)
        except Exception as e:
            raise HTTPException(status_code=400, detail=str(e))
        return {"data": result}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only bump the queue for yourself!")
class BackupUser(BaseModel):
    user_id: int


@app.post("/api/data/backup_user", response_class=PlainTextResponse)
async def backup_user(data: BackupUser, cnx=Depends(get_database_connection),
                      api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == data.user_id or is_web_key:
        try:
            opml_data = database_functions.functions.backup_user(database_type, cnx, data.user_id)
        except Exception as e:
            raise HTTPException(status_code=400, detail=str(e))
        return opml_data
    else:
        raise HTTPException(status_code=403,
                            detail="You can only make backups for yourself!")

class BackupServer(BaseModel):
    backup_dir: str
    database_pass: str


@app.get("/api/data/backup_server", response_class=PlainTextResponse)
async def backup_server(data: BackupServer, is_admin: bool = Depends(check_if_admin),
                        cnx=Depends(get_database_connection)):
    try:
        dump_data = database_functions.functions.backup_server(cnx, data.backup_dir, data.database_pass)
    except Exception as e:
        raise HTTPException(status_code=400, detail=str(e))
    return dump_data


class RestoreServer(BaseModel):
    database_pass: str
    server_restore_data: str


@app.post("/api/data/restore_server", response_class=PlainTextResponse)
async def restore_server(data: RestoreServer, is_admin: bool = Depends(check_if_admin),
                         cnx=Depends(get_database_connection)):
    try:
        dump_data = database_functions.functions.restore_server(cnx, data.database_pass, data.server_restore_data)
    except Exception as e:
        raise HTTPException(status_code=400, detail=str(e))
    return dump_data


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--port', type=int, default=8032, help='Port to run the server on')
    args = parser.parse_args()

    import uvicorn

    uvicorn.run(
        "clientapi:app",
        host="0.0.0.0",
        port=args.port,
        # ssl_keyfile="/opt/pinepods/certs/key.pem",  # Replace with the path to your key.pem
        # ssl_certfile="/opt/pinepods/certs/cert.pem"  # Replace with the path to your cert.pem
    )
