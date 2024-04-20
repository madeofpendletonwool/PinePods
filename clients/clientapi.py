# Fast API
from fastapi import FastAPI, Depends, HTTPException, status, Header, Body, Path, Form, Query, \
    security, BackgroundTasks
from fastapi.security import APIKeyHeader, HTTPBasic, HTTPBasicCredentials
from fastapi.responses import PlainTextResponse, JSONResponse, Response, FileResponse
from fastapi.middleware.cors import CORSMiddleware
from starlette.concurrency import run_in_threadpool
import smtplib
from email.mime.text import MIMEText
from email.mime.multipart import MIMEMultipart

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
from pydantic import BaseModel, Field, HttpUrl
from typing import Dict
from typing import List
from typing import Optional
from typing import Generator
import json
import logging
import argparse
import sys
from pyotp import TOTP, random_base32
import base64
import traceback
import time
import httpx
import asyncio
import io
import qrcode
import qrcode.image.svg

# Internal Modules
sys.path.append('/pinepods')

import database_functions.functions
import database_functions.auth_functions

database_type = str(os.getenv('DB_TYPE', 'mariadb'))
if database_type == "postgresql":
    print(f"You've selected a postgresql database.")
else:
    print("You've selected a mariadb database")

secret_key_middle = secrets.token_hex(32)

print('Client API Server is Starting!')

# Temporary storage for MFA secrets
temp_mfa_secrets = {}

app = FastAPI()
security = HTTPBasic()
origins = [
    "http://localhost",
    "http://localhost:8080",
    "http://127.0.0.1:8080",
    "http://127.0.0.1",
    "*"
]

# app.add_middleware(
#     CORSMiddleware,
#     allow_origins=origins,
#     allow_credentials=True,
#     allow_methods=["*"],
#     allow_headers=["*"],
# )

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

def create_database_connection():
    try:
        db = connection_pool.getconn() if database_type == "postgresql" else connection_pool.get_connection()
        return db
    except Exception as e:
        logger.error(f"Database connection error of type {type(e).__name__} with arguments: {e.args}")
        logger.error(traceback.format_exc())
        raise HTTPException(500, "Unable to connect to the database")


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


async def get_current_user(credentials: HTTPBasicCredentials = Depends(security)):
    # Use credentials.username and credentials.password where needed
    return credentials


# Use the non-generator version in your script initialization
cnx = direct_database_connection()
base_webkey.get_web_key(cnx)


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


@app.get('/api/data/verify_key')
async def verify_key(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if is_valid_key:
        database_functions.functions.clean_expired_sessions(cnx)
        return {"status": "success"}
    else:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

@app.get('/api/data/get_user')
async def verify_key(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if is_valid_key:
        retrieved_id = database_functions.functions.get_api_user(cnx, api_key)
        logging.error(f"here's id: {retrieved_id}")
        return {"status": "success", "retrieved_id": retrieved_id}
    else:
        raise HTTPException(status_code=403,
                            detail="Your api-key appears to be incorrect.")

@app.get('/api/data/get_key')
async def verify_key(cnx=Depends(get_database_connection),
                     credentials: HTTPBasicCredentials = Depends(get_current_user)):
    logging.info(f"creds: {credentials.username}, {credentials.password}")
    is_password_valid = database_functions.auth_functions.verify_password(cnx, credentials.username, credentials.password)
    if is_password_valid:
        retrieved_key = database_functions.functions.get_api_key(cnx, credentials.username)
        return {"status": "success", "retrieved_key": retrieved_key}
    else:
        raise HTTPException(status_code=403,
                            detail="Your credentials appear to be incorrect.")


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
            is_password_valid = database_functions.auth_functions.verify_password(cnx, data.username, data.password)
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


class PodcastData(BaseModel):
    podcast_id: int
    user_id: int
@app.post("/api/data/podcast_episodes")
async def api_podcast_episodes(data: PodcastData, cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user, or it's the web API key
    if key_id == data.user_id or is_web_key:
        episodes = database_functions.functions.return_podcast_episodes(database_type, cnx, data.user_id, data.podcast_id)
        if episodes is None:
            episodes = []  # Return an empty list instead of raising an exception
        return {"episodes": episodes}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only return episodes of your own!")

class PodcastIDData(BaseModel):
    podcast_feed: str
    user_id: int
@app.get("/api/data/get_podcast_id")
async def api_podcast_id(data: PodcastData, cnx=Depends(get_database_connection),
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
        episodes = database_functions.functions.get_podcast_id(database_type, cnx, data.user_id, data.podcast_feed)
        if episodes is None:
            episodes = []  # Return an empty list instead of raising an exception
        return {"episodes": episodes}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only return pocast ids of your own podcasts!")

class PodcastFeedData(BaseModel):
    podcast_feed: str

@app.get("/api/data/fetch_podcast_feed")
async def fetch_podcast_feed(podcast_feed: str = Query(...), cnx=Depends(get_database_connection),
                             api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Invalid API key or insufficient permissions")
    
    # Fetch the podcast feed data using httpx
    async with httpx.AsyncClient() as client:
        response = await client.get(podcast_feed)
        response.raise_for_status()  # Will raise an httpx.HTTPStatusError for 4XX/5XX responses
        return Response(content=response.content, media_type="application/xml")


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


class PodcastValuesModel(BaseModel):
    pod_title: str
    pod_artwork: str
    pod_author: str
    categories: dict
    pod_description: str
    pod_episode_count: int
    pod_feed_url: str
    pod_website: str
    pod_explicit: bool
    user_id: int

# app = FastAPI()


@app.post("/api/data/add_podcast")
async def api_add_podcast(podcast_values: PodcastValuesModel,
                          cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header)):
                          
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    logging.error(f"{podcast_values}")
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == podcast_values.user_id or is_web_key:
        result = database_functions.functions.add_podcast(cnx, podcast_values.dict(), podcast_values.user_id)
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
async def api_self_service_status(cnx=Depends(get_database_connection)):
    status = database_functions.functions.self_service_status(cnx)
    return {"status": status}

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
    episode_id: int
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
        database_functions.functions.record_podcast_history(cnx, data.episode_id, data.user_id, data.episode_pos)
        return {"detail": "Podcast history recorded."}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only make sessions for yourself!")


class DownloadPodcastData(BaseModel):
    episode_id: int
    user_id: int


@app.post("/api/data/download_podcast")
async def api_download_podcast(data: DownloadPodcastData, background_tasks: BackgroundTasks, cnx=Depends(get_database_connection),
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
        ep_status = database_functions.functions.check_downloaded(cnx, data.user_id, data.episode_id)
        if ep_status:
            return {"detail": "Podcast already downloaded."}
        else:
            background_tasks.add_task(download_podcast_fun, data.episode_id, data.user_id)
            return {"detail": "Podcast download started."}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only download podcasts for yourself!")
    
def download_podcast_fun(episode_id: int, user_id: int):
    cnx = create_database_connection()  # replace with your function to create a new database connection
    try:
        database_functions.functions.download_podcast(cnx, episode_id, user_id)
    finally:
        cnx.close()  # make sure to close the connection when you're done

def test_download_podcast_fun(episode_id: int, user_id: int):
    print(f"Downloading podcast {episode_id} for user {user_id}")
    cnx = create_database_connection()
    try:
        database_functions.functions.download_podcast(cnx, episode_id, user_id)
    finally:
        if database_type == "postgresql":
            connection_pool.putconn(cnx)
        else:
            cnx.close()


class DeletePodcastData(BaseModel):
    episode_id: int
    user_id: int


@app.post("/api/data/delete_episode")
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
        database_functions.functions.delete_episode(cnx, data.episode_id, data.user_id)
        return {"detail": "Podcast deleted."}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only delete podcasts for yourself!")


class SaveEpisodeData(BaseModel):
    episode_id: int
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
        ep_status = database_functions.functions.check_saved(cnx, data.user_id, data.episode_id)
        if ep_status:
            return {"detail": "Episode already saved."}
        else:
            success = database_functions.functions.save_episode(cnx, data.episode_id, data.user_id)
        if success:
            return {"detail": "Episode saved!"}
        else:
            raise HTTPException(status_code=400, detail="Error saving episode.")
    else:
        raise HTTPException(status_code=403,
                            detail="You can only save episodes of your own!")


class RemoveSavedEpisodeData(BaseModel):
    episode_id: int
    user_id: int


@app.post("/api/data/remove_saved_episode")
async def api_remove_saved_episode(data: RemoveSavedEpisodeData, cnx=Depends(get_database_connection),
                                   api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if is_valid_key:
        key_id = database_functions.functions.id_from_api_key(cnx, api_key)
        if key_id == data.user_id:
            database_functions.functions.remove_saved_episode(cnx, data.episode_id, data.user_id)
            return {"detail": "Saved episode removed."}
        else:
            raise HTTPException(status_code=403,
                                detail="You can only return episodes of your own!")
    else:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")


class RecordListenDurationData(BaseModel):
    episode_id: int
    user_id: int
    listen_duration: float


@app.post("/api/data/record_listen_duration")
async def api_record_listen_duration(data: RecordListenDurationData, cnx=Depends(get_database_connection),
                                     api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Ignore listen duration for episodes with ID 0
    if data.episode_id == 0:
        return {"detail": "Listen duration for episode ID 0 is ignored."}

    # Continue as normal for all other episode IDs
    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, api_key)

    if key_id == data.user_id or is_web_key:
        database_functions.functions.record_listen_duration(cnx, data.episode_id, data.user_id, data.listen_duration)
        return {"detail": "Listen duration recorded."}
    else:
        raise HTTPException(status_code=403, detail="You can only record your own listen duration")



@app.get("/api/data/refresh_pods")
async def api_refresh_pods(background_tasks: BackgroundTasks, is_admin: bool = Depends(check_if_admin)):
    background_tasks.add_task(refresh_pods_task)
    return {"detail": "Refresh initiated."}

def refresh_pods_task():
    cnx = create_database_connection()
    try:
        database_functions.functions.refresh_pods(cnx)
    finally:
        if database_type == "postgresql":
            connection_pool.putconn(cnx)
        else:
            cnx.close()

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
        logging.error(f"not valid key")
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
        print(episode_count)
        logging.error(f"here's count: {episode_count}")
        return episode_count
    else:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="User not found")


@app.get("/api/data/get_user_info")
async def api_get_user_info(is_admin: bool = Depends(check_if_admin), cnx=Depends(get_database_connection)):
    user_info = database_functions.functions.get_user_info(database_type, cnx)
    return user_info


@app.get("/api/data/check_podcast", response_model=Dict[str, bool])
async def api_check_podcast(
    user_id: int, 
    podcast_name: str, 
    podcast_url: str,
    cnx=Depends(get_database_connection), 
    api_key: str = Depends(get_api_key_from_header)
):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if is_valid_key:
        exists = database_functions.functions.check_podcast(cnx, user_id, podcast_name, podcast_url)
        return {"exists": exists}
    else:
        raise HTTPException(status_code=403, detail="Your API key is either invalid or does not have correct permission")

@app.get("/api/data/user_admin_check/{user_id}")
async def api_user_admin_check_route(user_id: int, api_key: str = Depends(get_api_key_from_header),
                                     cnx=Depends(get_database_connection)):
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
                                detail="You are not authorized to check admin status for other users")
    is_admin = database_functions.functions.user_admin_check(cnx, user_id)
    return {"is_admin": is_admin}


class RemovePodcastData(BaseModel):
    user_id: int
    podcast_name: str
    podcast_url: str


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
    database_functions.functions.remove_podcast(cnx, data.podcast_name, data.podcast_url, data.user_id)
    return {"success": True}

class RemovePodcastIDData(BaseModel):
    user_id: int
    podcast_id: int


@app.post("/api/data/remove_podcast_id")
async def api_remove_podcast_route(data: RemovePodcastIDData = Body(...), cnx=Depends(get_database_connection),
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
    database_functions.functions.remove_podcast_id(cnx, data.podcast_id, data.user_id)
    return {"success": True}


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
        return {"data": history}
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


@app.get("/api/data/download_episode_list")
async def api_download_episode_list(cnx=Depends(get_database_connection),
                                    api_key: str = Depends(get_api_key_from_header), 
                                    user_id: int = Query(...)):
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
    hash_pw: str



@app.post("/api/data/add_user")
async def api_add_user(is_admin: bool = Depends(check_if_admin), cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header),
                       user_values: UserValues = Body(...)):
    database_functions.functions.add_user(cnx, (
        user_values.fullname, user_values.username, user_values.email, user_values.hash_pw))
    return {"detail": "User added."}


@app.post("/api/data/add_login_user")
async def api_add_user(cnx=Depends(get_database_connection),
                       user_values: UserValues = Body(...)):
    self_service = database_functions.functions.check_self_service(cnx)
    if self_service:
        database_functions.functions.add_user(cnx, (
            user_values.fullname, user_values.username, user_values.email, user_values.hash_pw))
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
async def api_set_password(user_id: int, hash_pw: str = Body(...),
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
        database_functions.functions.set_password(cnx, user_id, hash_pw)
        return {"detail": "Password updated."}
    except Exception as e:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail=f"User not found. Error: {str(e)}")


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

class SendTestEmailValues(BaseModel):
    server_name: str
    server_port: str
    from_email: str
    send_mode: str
    encryption: str
    auth_required: bool
    email_username: str
    email_password: str
    to_email: str
    message: str  # Add this line


def send_email(payload: SendTestEmailValues):
    # This is now a synchronous function
    msg = MIMEMultipart()
    msg['From'] = payload.from_email
    msg['To'] = payload.to_email
    msg['Subject'] = "Test Email"
    msg.attach(MIMEText(payload.message, 'plain'))
    try:
        port = int(payload.server_port)  # Convert port to int here
        if payload.encryption == "SSL/TLS":
            server = smtplib.SMTP_SSL(payload.server_name, port)
        else:
            server = smtplib.SMTP(payload.server_name, port)
            if payload.encryption == "StartTLS":
                server.starttls()
        if payload.auth_required:
            server.login(payload.email_username, payload.email_password)
        server.send_message(msg)
        server.quit()
        return "Email sent successfully"
    except Exception as e:
        raise Exception(f"Failed to send email: {str(e)}")

@app.post("/api/data/send_test_email")
async def api_send_email(payload: SendTestEmailValues, is_admin: bool = Depends(check_if_admin), cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header)):
    # Assume API key validation logic here
    try:
        # Use run_in_threadpool to execute the synchronous send_email function
        send_status = await run_in_threadpool(send_email, payload)
        return {"email_status": send_status}
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Failed to send email: {str(e)}")

class SendEmailValues(BaseModel):
    to_email: str
    subject : str
    message: str  # Add this line

def send_email_with_settings(email_values, payload: SendEmailValues):

    try:
        msg = MIMEMultipart()
        msg['From'] = email_values['From_Email']
        msg['To'] = payload.to_email
        msg['Subject'] = payload.subject
        msg.attach(MIMEText(payload.message, 'plain'))
        
        try:
            port = int(email_values['Server_Port'])
            if email_values['Encryption'] == "SSL/TLS":
                server = smtplib.SMTP_SSL(email_values['Server_Name'], port)
            elif email_values['Encryption'] == "StartTLS":
                server = smtplib.SMTP(email_values['Server_Name'], port)
                server.starttls()
            else:
                server = smtplib.SMTP(email_values['Server_Name'], port)
                
            if email_values['Auth_Required']:
                server.login(email_values['Username'], email_values['Password'])
                
            server.send_message(msg)
            server.quit()
            return "Email sent successfully"
        except Exception as e:
            raise Exception(f"Failed to send email: {str(e)}")
    except Exception as e:
        logging.error(f"Failed to send email: {str(e)}", exc_info=True)
        raise Exception(f"Failed to send email: {str(e)}")



@app.post("/api/data/send_email")
async def api_send_email(payload: SendEmailValues, cnx=Depends(get_database_connection),
                         api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Invalid API key")

    email_values = database_functions.functions.get_email_settings(cnx)
    if not email_values:
        raise HTTPException(status_code=404, detail="Email settings not found")

    try:
        send_status = await run_in_threadpool(send_email_with_settings, email_values, payload)
        return {"email_status": send_status}
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Failed to send email: {str(e)}")


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
                                detail="You are not authorized to access or remove other users api-keys.")
    # Check if the API key to be deleted is the same as the one used in the current request
    if database_functions.functions.is_same_api_key(cnx, payload.api_id, api_key):
        raise HTTPException(status_code=403,
                            detail="You cannot delete the API key that is currently in use.")

    # Check if the API key belongs to the guest user (user_id 1)
    if database_functions.functions.belongs_to_guest_user(cnx, payload.api_id):
        raise HTTPException(status_code=403,
                            detail="Cannot delete guest user api.")

    # Proceed with deletion if the checks pass
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
    api_information = database_functions.functions.get_api_info(database_type, cnx, user_id)
    if api_information:
        return {"api_info": api_information}
    else:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="User not found")


class ResetCodePayload(BaseModel):
    email: str
    username: str


class ResetPasswordPayload(BaseModel):
    email: str
    hashed_pw: str


@app.post("/api/data/reset_password_create_code")
async def api_reset_password_route(payload: ResetCodePayload, cnx=Depends(get_database_connection)):
    email_setup = database_functions.functions.get_email_settings(cnx)
    if email_setup['Server_Name'] == "default_server":
        raise HTTPException(status_code=403,
                            detail="Email settings not configured. Please contact your administrator.")
    else:
        check_user = database_functions.functions.check_reset_user(cnx, payload.username, payload.email)
        if check_user:
            create_code = database_functions.functions.reset_password_create_code(cnx, payload.email)
                              
                                          # Create a SendTestEmailValues instance with the email setup values and the password reset code
            email_payload = SendEmailValues(
                to_email=payload.email,
                subject="Pinepods Password Reset Code",
                message=f"Your password reset code is {create_code}"
            )
            # Send the email with the password reset code
            email_send = send_email_with_settings(email_setup, email_payload)
            if email_send:
                return {"code_created": True}
            else:
                database_functions.functions.reset_password_remove_code(cnx, payload.email)
                raise HTTPException(status_code=500, detail="Failed to send email")
            
            return {"user_exists": user_exists}
        else:
            raise HTTPException(status_code=404, detail="User not found")

class ResetVerifyCodePayload(BaseModel):
    reset_code: str
    email: str
    new_password: str

@app.post("/api/data/verify_and_reset_password")
async def api_verify_and_reset_password_route(payload: ResetVerifyCodePayload, cnx=Depends(get_database_connection)):
    code_valid = database_functions.functions.verify_reset_code(cnx, payload.email, payload.reset_code)
    if code_valid is None:
        raise HTTPException(status_code=404, detail="User not found")
    elif not code_valid:
        raise HTTPException(status_code=400, detail="Code is invalid")
        # return {"code_valid": False}

    message = database_functions.functions.reset_password_prompt(cnx, payload.email, payload.new_password)
    if message is None:
        raise HTTPException(status_code=500, detail="Failed to reset password")
    return {"message": message}


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
    episode_id: int
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
        episode = database_functions.functions.get_episode_metadata(database_type, cnx, data.episode_id, data.user_id)
        return {"episode": episode}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only get metadata for yourself!")

@app.get("/api/data/generate_mfa_secret/{user_id}")
async def generate_mfa_secret(user_id: int, cnx=Depends(get_database_connection),
                              api_key: str = Depends(get_api_key_from_header)):
    # Perform API key validation and user authorization checks as before
    logging.error(f"Running Save mfa")
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        logging.warning(f"Invalid API key: {api_key}")
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key
    logging.info(f"Is web key: {is_web_key}")

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)
    logging.info(f"Key ID from API key: {key_id}")

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == user_id or is_web_key:
        user_details = database_functions.functions.get_user_details_id(cnx, user_id)
        if not user_details:
            raise HTTPException(status_code=404, detail="User not found")

        email = user_details['Email']
        secret = random_base32()  # Correctly generate a random base32 secret
        # Store the secret in temporary storage
        temp_mfa_secrets[user_id] = secret
        totp = TOTP(secret)
        provisioning_uri = totp.provisioning_uri(name=email, issuer_name="Pinepods")

        # Generate QR code as SVG
        qr = qrcode.QRCode(
            version=1,
            error_correction=qrcode.constants.ERROR_CORRECT_L,
            box_size=10,
            border=4,
        )
        qr.add_data(provisioning_uri)
        qr.make(fit=True)

        # Convert the QR code to an SVG string
        factory = qrcode.image.svg.SvgPathImage
        img = qr.make_image(fill_color="black", back_color="white", image_factory=factory)
        buffered = io.BytesIO()
        img.save(buffered)
        qr_code_svg = buffered.getvalue().decode("utf-8")
        logging.info(f"Generated MFA secret for user {user_id}")
        logging.info(f"Secret: {secret}")

        return {
            "secret": secret,
            "qr_code_svg": qr_code_svg  # Directly return the SVG string
        }
    else:
        logging.warning("Attempted to generate MFA secret for another user")
        raise HTTPException(status_code=403,
                            detail="You can only generate MFA secrets for yourself!")
    
class VerifyTempMFABody(BaseModel):
    user_id: int
    mfa_code: str

@app.post("/api/data/verify_temp_mfa")
async def verify_temp_mfa(body: VerifyTempMFABody, cnx=Depends(get_database_connection),
                              api_key: str = Depends(get_api_key_from_header)):
    # Perform API key validation and user authorization checks as before
    logging.error(f"Running Save mfa")
    logging.info(f"Verifying MFA code for user_id: {body.user_id} with code: {body.mfa_code}")

    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        logging.warning(f"Invalid API key: {api_key}")
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key
    logging.info(f"Is web key: {is_web_key}")

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)
    logging.info(f"Key ID from API key: {key_id}")

    if key_id == body.user_id or is_web_key:
        secret = temp_mfa_secrets.get(body.user_id)
        if secret is None:
            raise HTTPException(status_code=status.HTTP_404_NOT_FOUND,
                                detail="MFA setup not initiated or expired.")
        if secret:
            logging.info(f"Retrieved secret for user_id: {body.user_id}: {secret}")
        else:
            logging.warning(f"No secret found for user_id: {body.user_id}")
            raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="MFA setup not initiated or expired.")

        totp = TOTP(secret)
        if totp.verify(body.mfa_code):
            try:
                # Attempt to save the MFA secret to permanent storage
                success = database_functions.functions.save_mfa_secret(database_type, cnx, body.user_id, secret)
                if success:
                    # Remove the temporary secret upon successful verification and storage
                    del temp_mfa_secrets[body.user_id]
                    logging.info(f"MFA secret successfully saved for user_id: {body.user_id}")
                    return {"verified": True}
                else:
                    # Handle unsuccessful save attempt (e.g., database error)
                    logging.error("Failed to save MFA secret to database.")
                    logging.error(f"Failed to save MFA secret for user_id: {body.user_id}")
                    return JSONResponse(status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
                                        content={"message": "Failed to save MFA secret. Please try again."})
            except Exception as e:
                logging.error(f"Exception saving MFA secret: {e}")
                return JSONResponse(status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
                                    content={"message": "An error occurred. Please try again."})
        else:
            return {"verified": False}
    else:
        raise HTTPException(status_code=status.HTTP_403_FORBIDDEN,
                            detail="You are not authorized to verify MFA for this user.")

# Cleanup task for temp_mfa_secrets
async def cleanup_temp_mfa_secrets():
    while True:
        # Wait for 1 hour before running cleanup
        await asyncio.sleep(3600)
        # Current timestamp
        current_time = time.time()
        # Iterate over the temp_mfa_secrets and remove entries older than 1 hour
        for user_id, (secret, timestamp) in list(temp_mfa_secrets.items()):
            if current_time - timestamp > 3600:
                del temp_mfa_secrets[user_id]
        logging.info("Cleanup task: Removed expired MFA setup entries.")


class MfaSecretData(BaseModel):
    user_id: int
    mfa_secret: str


@app.post("/api/data/save_mfa_secret")
async def api_save_mfa_secret(data: MfaSecretData, cnx=Depends(get_database_connection),
                              api_key: str = Depends(get_api_key_from_header)):
    logging.info(f"Received request to save MFA secret for user {data.user_id}")
    logging.error(f"Running Save mfa")
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        logging.warning(f"Invalid API key: {api_key}")
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key
    logging.info(f"Is web key: {is_web_key}")

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)
    logging.info(f"Key ID from API key: {key_id}")

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == data.user_id or is_web_key:
        success = database_functions.functions.save_mfa_secret(database_type, cnx, data.user_id, data.mfa_secret)
        if success:
            logging.info("MFA secret saved successfully")
            return {"status": "success"}
        else:
            logging.error("Failed to save MFA secret")
            return {"status": "error"}
    else:
        logging.warning("Attempted to save MFA secret for another user")
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
    date_format: str


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

        if data.user_id != user_id_from_api_key:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN,
                                detail="You are not authorized to access these user details")

    success = database_functions.functions.setup_timezone_info(database_type, cnx, data.user_id, data.timezone,
                                                               data.hour_pref, data.date_format)
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
    timezone, hour_pref, date_format = database_functions.functions.get_time_info(database_type, cnx, user_id)
    if timezone:
        return {"timezone": timezone, "hour_pref": hour_pref, "date_format": date_format}
    else:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="User not found")


@app.get("/api/data/first_login_done/{user_id}")
async def first_login_done(user_id: int, cnx=Depends(get_database_connection),
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
        first_login_status = database_functions.functions.first_login_done(database_type, cnx, user_id)
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
    episode_id: int
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
        ep_status = database_functions.functions.check_queued(database_type, cnx, data.episode_id, data.user_id)
        if ep_status:
            return {"data": "Episode already in queue"}
        else:
            result = database_functions.functions.queue_pod(database_type, cnx, data.episode_id, data.user_id)
            return {"data": result}

    else:
        raise HTTPException(status_code=403,
                            detail="You can only add episodes to your own queue!")


class QueueRmData(BaseModel):
    episode_id: int
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
        result = database_functions.functions.remove_queued_pod(database_type, cnx, data.episode_id, data.user_id)
        return {"data": result}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only remove episodes for your own queue!")


# class QueuedEpisodesData(BaseModel):
#     user_id: int


@app.get("/api/data/get_queued_episodes")
async def get_queued_episodes(user_id: int = Query(...), cnx=Depends(get_database_connection),
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
        result = database_functions.functions.get_queued_episodes(database_type, cnx, user_id)
        return {"data": result}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only get episodes from your own queue!")


@app.get("/api/data/check_episode_in_db/{user_id}")
async def check_episode_in_db(user_id: int, episode_title: str = Query(...), episode_url: str = Query(...), cnx=Depends(get_database_connection),
                              api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Your API key is either invalid or does not have correct permission")

    if database_functions.functions.id_from_api_key(cnx, api_key) != user_id:
        raise HTTPException(status_code=403, detail="You can only check episodes for your own account")

    episode_exists = database_functions.functions.check_episode_exists(cnx, user_id, episode_title, episode_url)
    return {"episode_in_db": episode_exists}

class GpodderSettings(BaseModel):
    user_id: int
    gpodder_url: str
    gpodder_token: str


@app.post("/api/data/add_gpodder_settings")
async def add_gpodder_settings(data: GpodderSettings, cnx=Depends(get_database_connection),
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
        result = database_functions.functions.add_gpodder_settings(database_type, cnx, data.user_id, data.gpodder_url, data.gpodder_token)
        return {"data": result}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only add your own gpodder data!")

class RemoveGpodderSettings(BaseModel):
    user_id: int

@app.post("/api/data/remove_gpodder_settings")
async def remove_gpodder_settings(data: RemoveGpodderSettings, cnx=Depends(get_database_connection),
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
        result = database_functions.functions.remove_gpodder_settings(database_type, cnx, data.user_id)
        return {"data": result}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only remove your own gpodder data!")

# class CheckGpodderSettings(BaseModel):
#     user_id: int

# @app.get("/api/data/get_gpodder_settings")
# async def get_gpodder_settings(data: CheckGpodderSettings, cnx=Depends(get_database_connection),
#                               api_key: str = Depends(get_api_key_from_header)):
#     is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
#     if not is_valid_key:
#         raise HTTPException(status_code=403,
#                             detail="Your API key is either invalid or does not have correct permission")

#     # Check if the provided API key is the web key
#     is_web_key = api_key == base_webkey.web_key

#     key_id = database_functions.functions.id_from_api_key(cnx, api_key)

#     # Allow the action if the API key belongs to the user or it's the web API key
#     if key_id == data.user_id or is_web_key:
#         result = database_functions.functions.check_gpodder_settings(database_type, cnx, data.user_id)
#         return {"data": result}
#     else:
#         raise HTTPException(status_code=403,
#                             detail="You can only remove your own gpodder data!")
    
@app.get("/api/data/check_gpodder_settings/{user_id}")
async def check_gpodder_settings(user_id: int, cnx=Depends(get_database_connection),
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
        result = database_functions.functions.check_gpodder_settings(database_type, cnx, user_id)
        return {"data": result}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only remove your own gpodder data!")
    
@app.get("/api/data/get_gpodder_settings/{user_id}")
async def get_gpodder_settings(user_id: int, cnx=Depends(get_database_connection),
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
        result = database_functions.functions.get_gpodder_settings(database_type, cnx, user_id)
        return {"data": result}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only remove your own gpodder data!")

class NextcloudAuthRequest(BaseModel):
    user_id: int
    token: str
    poll_endpoint: HttpUrl
    nextcloud_url: HttpUrl

@app.post("/api/data/add_nextcloud_server")
async def add_nextcloud_server(background_tasks: BackgroundTasks, data: NextcloudAuthRequest, cnx=Depends(get_database_connection),
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
                                detail="You are not authorized to access these user details")

    # Reset gPodder settings to default
    database_functions.functions.remove_gpodder_settings(database_type, cnx, data.user_id)

    # Add the polling task to the background tasks
    background_tasks.add_task(poll_for_auth_completion_background, data, database_type)

    # Return 200 status code before starting to poll
    return {"status": "polling"}

async def poll_for_auth_completion_background(data: NextcloudAuthRequest, database_type):
    # Create a new database connection
    cnx = create_database_connection()

    try:
        credentials = await poll_for_auth_completion(data.poll_endpoint, data.token)
        if credentials:
            logging.info(f"Nextcloud authentication successful: {credentials}")
            logging.info(f"Adding Nextcloud settings for user {data.user_id}")
            logging.info(f"Database Type: {database_type}, Connection: {cnx}, User ID: {data.user_id}")
            logging.info(f"Nextcloud URL: {data.nextcloud_url}, Token: {data.token}")
            result = database_functions.functions.add_gpodder_settings(database_type, cnx, data.user_id, str(data.nextcloud_url), credentials["appPassword"], credentials["loginName"])
            if not result:
                logging.error("User not found")
        else:
            logging.error("Nextcloud authentication failed.")
    finally:
        # Close the database connection
        cnx.close()

# Adjusted to use httpx for async HTTP requests
async def poll_for_auth_completion(endpoint: HttpUrl, token: str):
    payload = {"token": token}
    timeout = 20 * 60  # 20 minutes timeout for polling
    async with httpx.AsyncClient() as client:
        start_time = asyncio.get_event_loop().time()
        while asyncio.get_event_loop().time() - start_time < timeout:
            try:
                response = await client.post(str(endpoint), json=payload, headers={"Content-Type": "application/json"}) 
            except httpx.ConnectTimeout:
                logging.info("Connection timed out, retrying...")
                logging.info(f"endpoint: {endpoint}, token: {token}")
                continue
            if response.status_code == 200:
                credentials = response.json()
                logging.info(f"Authentication successful: {credentials}")
                return credentials
            elif response.status_code == 404:
                await asyncio.sleep(5)  # Non-blocking sleep
            else:
                logging.info(f"Polling failed with status code {response.status_code}")
                raise HTTPException(status_code=500, detail="Polling for Nextcloud authentication failed.")
    raise HTTPException(status_code=408, detail="Nextcloud authentication request timed out.")

@app.get("/api/data/refresh_nextcloud_subscriptions")
async def refresh_nextcloud_subscription(background_tasks: BackgroundTasks, is_admin: bool = Depends(check_if_admin), api_key: str = Depends(get_api_key_from_header)):

    cnx = create_database_connection()
    try:
        users = database_functions.functions.get_nextcloud_users(database_type, cnx)
    finally:
        if database_type == "postgresql":
            connection_pool.putconn(cnx)
        else:
            cnx.close()

    for user_id, gpodder_url, gpodder_token, gpodder_login in users:
        background_tasks.add_task(refresh_nextcloud_subscription_for_user, database_type, user_id, gpodder_url, gpodder_token, gpodder_login)

    return {"status": "success", "message": "Nextcloud subscriptions refresh initiated."}

def refresh_nextcloud_subscription_for_user(database_type, user_id, gpodder_url, gpodder_token, gpodder_login):
    cnx = create_database_connection()
    try:
        database_functions.functions.refresh_nextcloud_subscription(database_type, cnx, user_id, gpodder_url, gpodder_token, gpodder_login)
    finally:
        if database_type == "postgresql":
            connection_pool.putconn(cnx)
        else:
            cnx.close()

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
            result = database_functions.functions.queue_bump(database_type, cnx, data.ep_url, data.title, data.user_id)
        except Exception as e:
            raise HTTPException(status_code=400, detail=str(e))
        return {"data": result}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only bump the queue for yourself!")
    
@app.get("/api/data/stream/{episode_id}")
async def stream_episode(
    episode_id: int, 
    cnx=Depends(get_database_connection),
    api_key: str = Query(..., alias='api_key'),  # Change here
    user_id: int = Query(..., alias='user_id')   # Change here
):
    print(f"Episode ID: {episode_id}")
    print(f"API Key: {api_key}")
    print(f"User ID: {user_id}")
    is_valid_key = database_functions.functions.verify_api_key(cnx, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, api_key)
    print("About to start the episode stream")
    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == user_id or is_web_key:
        file_path = database_functions.functions.get_download_location(cnx, episode_id, user_id)
        if file_path:
            return FileResponse(path=file_path, media_type='audio/mpeg', filename=os.path.basename(file_path))
        else:
            raise HTTPException(status_code=404, detail="Episode not found or not downloaded")
    else:
        raise HTTPException(status_code=403, detail="You do not have permission to access this episode")


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


class BackupServerRequest(BaseModel):
    database_pass: str

@app.post("/api/data/backup_server", response_class=PlainTextResponse)
async def backup_server(request: BackupServerRequest, is_admin: bool = Depends(check_if_admin), cnx=Depends(get_database_connection)):
    # logging.info(f"request: {request}")
    if not is_admin:
        raise HTTPException(status_code=status.HTTP_403_FORBIDDEN, detail="Not authorized")
    try:
        dump_data = database_functions.functions.backup_server(cnx, request.database_pass)
    except Exception as e:
        raise HTTPException(status_code=status.HTTP_400_BAD_REQUEST, detail=str(e))
    return Response(content=dump_data, media_type="text/plain")

class RestoreServer(BaseModel):
    database_pass: str
    server_restore_data: str


@app.post("/api/data/restore_server")
async def api_restore_server(data: RestoreServer, background_tasks: BackgroundTasks, is_admin: bool = Depends(check_if_admin), cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header)):
    
    if not is_admin:
        raise HTTPException(status_code=403, detail="Not authorized")
    logging.info(f"Restoring server with data")
    # Proceed with restoration but in the background
    background_tasks.add_task(restore_server_fun, data.database_pass, data.server_restore_data)
    return JSONResponse(content={"detail": "Server restoration started."})

def restore_server_fun(database_pass: str, server_restore_data: str):
    # Assuming create_database_connection and restore_server are defined in database_functions.functions
    cnx = create_database_connection()  # Replace with your method to create a new DB connection
    try:
        # Restore server using the provided password and data
        database_functions.functions.restore_server(cnx, database_pass, server_restore_data)
    finally:
        cnx.close() 

async def async_tasks():
    # Start cleanup task
    logging.info("Starting cleanup tasks")
    asyncio.create_task(cleanup_temp_mfa_secrets())


if __name__ == '__main__':
    raw_debug_mode = os.environ.get("DEBUG_MODE", "False")
    DEBUG_MODE = raw_debug_mode.lower() == "true"
    if DEBUG_MODE:
        logging.info("Debug Mode Enabled")
    else:
        logging.info("Debug Mode Disabled")
    config_file = "/pinepods/startup/logging_config_debug.ini" if DEBUG_MODE else "/pinepods/startup/logging_config.ini"
    logging.info(config_file)
    parser = argparse.ArgumentParser()
    parser.add_argument('--port', type=int, default=8032, help='Port to run the server on')
    args = parser.parse_args()
    asyncio.run(async_tasks())

    import uvicorn

    uvicorn.run(
        "clientapi:app",
        host="0.0.0.0",
        port=args.port,
        log_config=config_file
        # ssl_keyfile="/opt/pinepods/certs/key.pem",
        # ssl_certfile="/opt/pinepods/certs/cert.pem"
    )
