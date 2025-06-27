# Fast API
from fastapi import FastAPI, WebSocket, WebSocketDisconnect, Depends, HTTPException, status, Header, Body, Path, Form, Query, \
    security, BackgroundTasks, UploadFile
from fastapi.security import APIKeyHeader, HTTPBasic, HTTPBasicCredentials
from fastapi.responses import PlainTextResponse, JSONResponse, Response, FileResponse, StreamingResponse, RedirectResponse
from fastapi.middleware.cors import CORSMiddleware
from starlette.concurrency import run_in_threadpool
from threading import Lock
import smtplib
from email.mime.text import MIMEText
from email.mime.multipart import MIMEMultipart
from functools import lru_cache, wraps
from yt_dlp import YoutubeDL
import subprocess
import threading

# Needed Modules
from passlib.context import CryptContext
import mysql.connector
from mysql.connector import pooling
from time import time
from mysql.connector.pooling import MySQLConnectionPool
from mysql.connector import Error
import psycopg
from psycopg_pool import ConnectionPool
from psycopg.rows import dict_row
from psycopg.errors import UniqueViolation, ForeignKeyViolation, OperationalError
import os
import xml.etree.ElementTree as ET
from fastapi.middleware.gzip import GZipMiddleware
from starlette.middleware.sessions import SessionMiddleware
from starlette.requests import Request
import secrets
from pydantic import BaseModel, Field, HttpUrl
from typing import Dict, List, Any, Optional, Generator, Tuple, Set, TypedDict, Callable
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
from urllib.parse import urlparse, urlunparse
import datetime
import feedparser
import dateutil.parser
import re
import requests
from requests.auth import HTTPBasicAuth
from contextlib import contextmanager
import signal

def sigterm_handler(_signo, _stack_frame):
    # Perform cleanup here
    print("Received SIGTERM. Shutting down...")
    sys.exit(0)

signal.signal(signal.SIGTERM, sigterm_handler)

# Internal Modules
sys.path.append('/pinepods')

import database_functions.functions
import database_functions.auth_functions
import database_functions.app_functions
import database_functions.import_progress
import database_functions.oidc_state_manager
import database_functions.valkey_client
import database_functions.youtube
import database_functions.tasks
from database_functions.gpodder_router import gpodder_router
from database_functions.db_client import create_database_connection, close_database_connection

# # Use a try-except to handle potential import errors
# try:
#     from database_functions.tasks import (
#         download_podcast_task,
#         download_youtube_video_task,
#         queue_podcast_downloads,
#         task_manager,  # Changed from download_manager to task_manager
#         download_manager,  # Keep this for backward compatibility
#         get_all_active_tasks,  # Add this new function
#         debug_task
#     )
#     CELERY_AVAILABLE = True
#     print('celery tasks imported')
# except ImportError as e:
#     print(f"Failed to import Celery tasks: {e}")
#     CELERY_AVAILABLE = False
#     # Define fallback functions if needed


database_type = str(os.getenv('DB_TYPE', 'mariadb'))
if database_type == "postgresql":
    print(f"You've selected a postgresql database.")
else:
    print("You've selected a mariadb database")

secret_key_middle = secrets.token_hex(32)

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

app.include_router(gpodder_router)

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
api_url = os.environ.get("SEARCH_API_URL", "https://search.pinepods.online/api/search")
people_url = os.environ.get("PEOPLE_API_URL", "https://people.pinepods.online")

# Initial Vars needed to start and used throughout
if reverse_proxy == "True":
    proxy_url = f'{proxy_protocol}://{proxy_host}/mover/?url='
else:
    proxy_url = f'{proxy_protocol}://{proxy_host}:{proxy_port}/mover/?url='

logger = logging.getLogger(__name__)


def get_database_connection():
    """FastAPI dependency for getting a database connection"""
    try:
        db = create_database_connection()
        yield db
    except HTTPException:
        raise  # Re-raise the HTTPException to let FastAPI handle it properly
    except Exception as e:
        logger.error(f"Database connection error of type {type(e).__name__} with arguments: {e.args}")
        logger.error(traceback.format_exc())
        raise HTTPException(500, "Unable to connect to the database")
    finally:
        try:
            close_database_connection(db)
        except Exception as e:
            logger.error(f"Error in connection cleanup: {str(e)}")


def get_api_keys(cnx):
    logging.info("Executing get_api_keys function...")
    if database_type == "postgresql":
        # Use dict_row row factory for PostgreSQL
        cnx.row_factory = dict_row
        cursor = cnx.cursor()
        query = 'SELECT * FROM "APIKeys"'
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
            request.session["api_key"] = api_key  # Store the API key
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
        self.web_key = database_functions.functions.get_web_key(cnx, database_type)
        return self.web_key

base_webkey = Web_Key()

async def initialize_web_key():
    cnx = create_database_connection()
    try:
        base_webkey.get_web_key(cnx)
    finally:
        close_database_connection(cnx)

def direct_database_connection():
    """Get a direct database connection - alias for create_database_connection"""
    return create_database_connection()

async def get_current_user(credentials: HTTPBasicCredentials = Depends(security)):
    # Use credentials.username and credentials.password where needed
    return credentials


# Modify check_if_admin to handle initialization
async def check_if_admin(api_key: str = Depends(get_api_key_from_header), cnx=Depends(get_database_connection)):
    # Initialize web key if not already set
    if base_webkey.web_key is None:
        await initialize_web_key()

    # Debug logging
    print(f"Checking admin access - API Key: {api_key}, Web Key: {base_webkey.web_key}")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key
    if is_web_key:
        return True

    user_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
    if not user_id:
        raise HTTPException(status_code=403, detail="Invalid API key.")

    if user_id == 1:
        return True

    is_admin = database_functions.functions.user_admin_check(cnx, database_type, user_id)
    if not is_admin:
        raise HTTPException(status_code=403, detail="User not authorized.")

    return True


def check_if_admin_inner(api_key: str, cnx):
    user_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if not user_id:
        return False
    return database_functions.functions.user_admin_check(cnx, database_type, user_id)

async def has_elevated_access(api_key: str, cnx):
    # Check if it's an admin
    is_admin = await run_in_threadpool(check_if_admin_inner, api_key, cnx)
    # Check if it's the web key
    web_key = base_webkey.web_key
    is_web_key = api_key == web_key

    return is_admin or is_web_key

@app.get('/api/pinepods_check')
async def pinepods_check():
    return {"status_code": 200, "pinepods_instance": True}


@app.get('/api/data/verify_key')
async def verify_key(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if is_valid_key:
        return {"status": "success"}
    else:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

@app.get('/api/data/get_user')
async def get_user(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if is_valid_key:
        retrieved_id = database_functions.functions.get_api_user(cnx, database_type, api_key)
        return {"status": "success", "retrieved_id": retrieved_id}
    else:
        raise HTTPException(status_code=403,
                            detail="Your api-key appears to be incorrect.")

@app.get('/api/data/get_key')
async def get_key(cnx=Depends(get_database_connection),
                     credentials: HTTPBasicCredentials = Depends(get_current_user)):
    is_password_valid = database_functions.auth_functions.verify_password(cnx, database_type, credentials.username.lower(), credentials.password)
    if is_password_valid:
        retrieved_key = database_functions.functions.get_api_key(cnx, database_type, credentials.username.lower())
        return {"status": "success", "retrieved_key": retrieved_key}
    else:
        raise HTTPException(status_code=403,
                            detail="Your credentials appear to be incorrect.")

@app.get("/api/data/config")
async def api_config(api_key: str = Depends(get_api_key_from_header), cnx=Depends(get_database_connection)):
    global api_url, proxy_url, proxy_host, proxy_port, proxy_protocol, reverse_proxy

    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if is_valid_key:
        return {
            "api_url": api_url,
            "proxy_url": proxy_url,
            "proxy_host": proxy_host,
            "proxy_port": proxy_port,
            "proxy_protocol": proxy_protocol,
            "reverse_proxy": reverse_proxy,
            "people_url": people_url,
        }
    else:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")


@app.get("/api/data/guest_status", response_model=bool)
async def api_guest_status(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if is_valid_key:
        result = database_functions.functions.guest_status(cnx, database_type)
        return result
    else:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")


@app.get("/api/data/download_status", response_model=bool)
async def api_download_status(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if is_valid_key:
        result = database_functions.functions.download_status(cnx, database_type)
        return result
    else:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

@app.get("/api/data/return_episodes/{user_id}")
async def api_return_episodes(user_id: int, cnx=Depends(get_database_connection),
                              api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    # Allow the action if the API key belongs to the user, or it's the web API key
    if key_id == user_id or is_web_key:
        episodes = database_functions.functions.return_episodes(database_type, cnx, user_id)
        if episodes is None:
            episodes = []  # Return an empty list instead of raising an exception
        return {"episodes": episodes}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only return episodes of your own!")


@app.get("/api/data/podcast_episodes")
async def api_podcast_episodes(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header), user_id: int = Query(...), podcast_id: int = Query(...)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    # Allow the action if the API key belongs to the user, or it's the web API key
    if key_id == user_id or is_web_key:
        episodes = database_functions.functions.return_podcast_episodes(database_type, cnx, user_id, podcast_id)
        if episodes is None:
            episodes = []  # Return an empty list instead of raising an exception
        # logging.error(f"Episodes returned: {episodes}")
        return {"episodes": episodes}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only return episodes of your own!")

@app.get("/api/data/home_overview")
async def api_home_overview(
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header),
    user_id: int = Query(...)
):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(
            status_code=403,
            detail="Your API key is either invalid or does not have correct permission"
        )

    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id == user_id or is_web_key:
        home_data = database_functions.functions.get_home_overview(database_type, cnx, user_id)
        return home_data
    else:
        raise HTTPException(
            status_code=403,
            detail="You can only view your own home overview!"
        )

@app.get("/api/data/startpage")
async def api_get_startpage(
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header),
    user_id: int = Query(...)
):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(
            status_code=403,
            detail="Your API key is either invalid or does not have correct permission"
        )
    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
    if key_id == user_id or is_web_key:
        startpage = database_functions.functions.get_user_startpage(cnx, database_type, user_id)
        return {"StartPage": startpage}
    else:
        raise HTTPException(
            status_code=403,
            detail="You can only view your own StartPage setting!"
        )

@app.post("/api/data/startpage")
async def api_set_startpage(
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header),
    user_id: int = Query(...),
    startpage: str = Query(...)
):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(
            status_code=403,
            detail="Your API key is either invalid or does not have correct permission"
        )
    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
    if key_id == user_id or is_web_key:
        success = database_functions.functions.set_user_startpage(cnx, database_type, user_id, startpage)
        return {"success": success, "message": "StartPage updated successfully"}
    else:
        raise HTTPException(
            status_code=403,
            detail="You can only modify your own StartPage setting!"
        )

@app.get("/api/data/youtube_episodes")
async def api_youtube_episodes(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header), user_id: int = Query(...), podcast_id: int = Query(...)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")
    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
    # Allow the action if the API key belongs to the user, or it's the web API key
    if key_id == user_id or is_web_key:
        episodes = database_functions.functions.return_youtube_episodes(database_type, cnx, user_id, podcast_id)
        if episodes is None:
            episodes = []  # Return an empty list instead of raising an exception
        return {"episodes": episodes}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only return episodes of your own!")


@app.get("/api/data/get_episode_id_ep_name")
async def api_episode_id(cnx=Depends(get_database_connection),
                              api_key: str = Depends(get_api_key_from_header),
                              user_id: int = Query(...), episode_title: str = Query(...), episode_url: str = Query(...)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    # Allow the action if the API key belongs to the user, or it's the web API key
    if key_id == user_id or is_web_key:
        print(episode_title)
        print(episode_url)
        ep_id = database_functions.functions.get_episode_id_ep_name(cnx, database_type, episode_title, episode_url)
        print(f"Episode ID: {ep_id}")
        return ep_id
    else:
        raise HTTPException(status_code=403,
                            detail="You can only return pocast ids of your own podcasts!")


@app.get("/api/data/get_podcast_id")
async def api_podcast_id(cnx=Depends(get_database_connection),
                              api_key: str = Depends(get_api_key_from_header),
                              user_id: int = Query(...), podcast_feed: str = Query(...), podcast_title: str = Query(...)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    # Allow the action if the API key belongs to the user, or it's the web API key
    if key_id == user_id or is_web_key:
        episodes = database_functions.functions.get_podcast_id(database_type, cnx, user_id, podcast_feed, podcast_title)
        if episodes is None:
            episodes = []  # Return an empty list instead of raising an exception
        return {"episodes": episodes}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only return pocast ids of your own podcasts!")

@app.get("/api/data/get_podcast_id_from_ep_id")
async def api_get_podcast_id(
    episode_id: int,
    user_id: int,
    is_youtube: bool = False,  # Add optional parameter
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    logging.info('Fetching API key')
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Your API key is either invalid or does not have correct permission")

    is_web_key = api_key == base_webkey.web_key
    logging.info('Getting key ID')
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
    logging.info(f'Got key ID: {key_id}')

    if key_id == user_id or is_web_key:
        podcast_id = database_functions.functions.get_podcast_id_from_episode(
            cnx, database_type, episode_id, user_id, is_youtube
        )
        if podcast_id is None:
            raise HTTPException(status_code=404, detail="Podcast ID not found for the given episode ID")
        return {"podcast_id": podcast_id}
    else:
        raise HTTPException(status_code=403, detail="You can only get podcast ID for your own episodes.")


@app.get("/api/data/get_podcast_id_from_ep_name")
async def api_get_podcast_id_name(episode_name: str, episode_url: str, user_id: int, cnx=Depends(get_database_connection),
                             api_key: str = Depends(get_api_key_from_header)):
    logging.info('Fetching API key')
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key
    logging.info('Getting key ID')
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
    logging.info(f'Got key ID: {key_id}')

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == user_id or is_web_key:
        podcast_id = database_functions.functions.get_podcast_id_from_episode_name(cnx, database_type, episode_name, episode_url, user_id)
        if podcast_id is None:
            raise HTTPException(status_code=404, detail="Podcast ID not found for the given episode name and URL")
        return {"podcast_id": podcast_id}
    else:
        raise HTTPException(status_code=403, detail="You can only get podcast ID for your own episodes.")


@app.get("/api/data/get_podcast_details")
async def api_podcast_details(podcast_id: str = Query(...), cnx=Depends(get_database_connection),
                              api_key: str = Depends(get_api_key_from_header),
                              user_id: int = Query(...)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")
    print('in pod details')
    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
    # Allow the action if the API key belongs to the user, or it's the web API key
    if key_id == user_id or is_web_key:
        details = database_functions.functions.get_podcast_details(database_type, cnx, user_id, podcast_id)
        print(f'got details {details}')
        if details is None:
            episodes = []  # Return an empty list instead of raising an exception
        return {"details": details}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only return pocast ids of your own podcasts!")

class ClickedFeedURL(BaseModel):
    podcastid: int
    podcastname: str
    feedurl: str
    description: str
    author: str
    artworkurl: str
    explicit: bool
    episodecount: int
    categories: Optional[Dict[str, str]]
    websiteurl: str
    podcastindexid: int
    is_youtube: Optional[bool]

@app.get("/api/data/get_podcast_details_dynamic", response_model=ClickedFeedURL)
async def get_podcast_details(
    user_id: int,
    podcast_title: str,
    podcast_url: str,
    podcast_index_id: int,
    added: bool,
    display_only: bool = False,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header),
):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Invalid API key or insufficient permissions")
    if added:
        podcast_id = database_functions.functions.get_podcast_id(database_type, cnx, user_id, podcast_url, podcast_title)
        details = database_functions.functions.get_podcast_details(database_type, cnx, user_id, podcast_id)
        if details is None:
            raise HTTPException(status_code=404, detail="Podcast not found")

        # Handle categories field with existence check
        categories = details.get("categories") if database_type != "postgresql" else details.get("categories")
        if not categories:
            categories_dict = {}
        elif categories.startswith('{'):
            try:
                categories = categories.replace("'", '"')
                categories_dict = json.loads(categories)
            except json.JSONDecodeError as e:
                print(f"JSON decode error: {e}")
                raise HTTPException(status_code=500, detail="Internal server error")
        else:
            categories_dict = {str(i): cat.strip() for i, cat in enumerate(categories.split(','))}


        pod_details = ClickedFeedURL(
            podcastid=0,
            podcastname=details["podcastname"],
            feedurl=details["feedurl"],
            description=details["description"],
            author=details["author"],
            artworkurl=details["artworkurl"],
            explicit=details["explicit"],
            episodecount=details["episodecount"],
            categories=categories_dict,
            websiteurl=details["websiteurl"],
            podcastindexid=details["podcastindexid"],
            is_youtube=details["isyoutubechannel"]
        )
        return pod_details
    else:
        podcast_values = database_functions.app_functions.get_podcast_values(podcast_url, user_id, None, None, display_only)
        categories = podcast_values['categories']
        print(f"heres the ep count: {podcast_values['pod_episode_count']}")

        if categories.startswith('{'):
            try:
                # Replace single quotes with double quotes
                categories = categories.replace("'", '"')
                categories_dict = json.loads(categories)
            except json.JSONDecodeError as e:
                print(f"JSON decode error: {e}")
                raise HTTPException(status_code=500, detail="Internal server error")
        else:
            categories_dict = {str(i): cat.strip() for i, cat in enumerate(categories.split(','))}


        return ClickedFeedURL(
            podcastid=0,
            podcastname=podcast_values['pod_title'],
            feedurl=podcast_values['pod_feed_url'],
            description=podcast_values['pod_description'],
            author=podcast_values['pod_author'],
            artworkurl=podcast_values['pod_artwork'],
            explicit=podcast_values['pod_explicit'],
            episodecount=podcast_values['pod_episode_count'],
            categories=categories_dict,
            websiteurl=podcast_values['pod_website'],
            podcastindexid=podcast_index_id,
            is_youtube=False
        )

class ImportProgressResponse(BaseModel):
    current: int
    current_podcast: str
    total: int

@app.get("/api/data/import_progress/{user_id}")
async def get_import_progress(
    user_id: int,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Invalid API key")

    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id == user_id or is_web_key:
        # Fetch the import progress from the database
        current, total, current_podcast = database_functions.import_progress.import_progress_manager.get_progress(user_id)
        return ImportProgressResponse(current=current, total=total, current_podcast=current_podcast)
    else:
        raise HTTPException(status_code=403, detail="You can only fetch import progress for yourself!")

class OPMLImportRequest(BaseModel):
    podcasts: List[str]
    user_id: int

@app.post("/api/data/import_opml")
async def api_import_opml(
    import_request: OPMLImportRequest,
    background_tasks: BackgroundTasks,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Invalid API key")

    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id == import_request.user_id or is_web_key:
        # Start the import process in the background
        background_tasks.add_task(process_opml_import, import_request, database_type)
        return {"success": True, "message": "Import process started"}
    else:
        raise HTTPException(status_code=403, detail="You can only import podcasts for yourself!")


@contextmanager
def get_db_connection():
    connection = None
    try:
        connection = create_database_connection()
        yield connection
    finally:
        if connection:
            close_database_connection(connection)

def process_opml_import(import_request: OPMLImportRequest, database_type):
    total_podcasts = len(import_request.podcasts)
    database_functions.import_progress.import_progress_manager.start_import(import_request.user_id, total_podcasts)
    for index, podcast_url in enumerate(import_request.podcasts, start=1):
        try:
            with get_db_connection() as cnx:
                podcast_values = database_functions.app_functions.get_podcast_values(podcast_url, import_request.user_id, None, None, False)
                database_functions.functions.add_podcast(cnx, database_type, podcast_values, import_request.user_id, 30)
                database_functions.import_progress.import_progress_manager.update_progress(import_request.user_id, index, podcast_url)
        except Exception as e:
            print(f"Error importing podcast {podcast_url}: {str(e)}")
        # Add a small delay to allow other requests to be processed
        time.sleep(0.1)
    database_functions.import_progress.import_progress_manager.clear_progress(import_request.user_id)

class PodcastFeedData(BaseModel):
    podcast_feed: str

@app.get("/api/data/fetch_podcast_feed")
async def fetch_podcast_feed(podcast_feed: str = Query(...), cnx=Depends(get_database_connection),
                             api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Invalid API key or insufficient permissions")

    # Define headers that mimic a standard web browser
    headers = {
        "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36",
        "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8",
        "Accept-Language": "en-US,en;q=0.5",
        "Connection": "keep-alive",
        "Upgrade-Insecure-Requests": "1",
        "Cache-Control": "max-age=0"
    }

    # Fetch the podcast feed data using httpx with browser-like headers
    try:
        async with httpx.AsyncClient(follow_redirects=True, timeout=30.0) as client:
            response = await client.get(podcast_feed, headers=headers)
            response.raise_for_status()  # Will raise an httpx.HTTPStatusError for 4XX/5XX responses
            return Response(content=response.content, media_type="application/xml")
    except httpx.HTTPStatusError as e:
        # Add more detailed error logging
        error_message = f"HTTP error fetching podcast feed: {str(e)}"
        logging.error(error_message)
        raise HTTPException(status_code=e.response.status_code,
                           detail=f"Failed to fetch podcast feed: {e.response.reason_phrase}")
    except httpx.RequestError as e:
        # Handle request errors (network issues, etc.)
        error_message = f"Request error fetching podcast feed: {str(e)}"
        logging.error(error_message)
        raise HTTPException(status_code=500, detail="Failed to fetch podcast feed due to network or connection issues")
    except Exception as e:
        # Catch-all for other unexpected errors
        error_message = f"Unexpected error fetching podcast feed: {str(e)}"
        logging.error(error_message)
        raise HTTPException(status_code=500, detail="Unexpected error occurred while fetching the podcast feed")

NAMESPACE = {'podcast': 'https://podcastindex.org/namespace/1.0'}

async def fetch_feed(feed_url: str) -> str:
    async with httpx.AsyncClient(follow_redirects=True) as client:
        response = await client.get(feed_url)
        response.raise_for_status()
        return response.text

async def fetch_json(url: str) -> Optional[dict]:
    async with httpx.AsyncClient(follow_redirects=True) as client:
        response = await client.get(url)
        response.raise_for_status()
        return response.json()

def parse_chapters(feed_content: str, audio_url: str) -> List[Dict[str, Optional[str]]]:
    chapters = []
    try:
        root = ET.fromstring(feed_content)
        episodes = root.findall('.//item')
        for episode in episodes:
            enclosure_element = episode.find('enclosure')
            enclosure_url = enclosure_element.attrib.get('url') if enclosure_element is not None else None
            if enclosure_element is not None and enclosure_url == audio_url:
                chapters_element = episode.find('podcast:chapters', NAMESPACE)
                if chapters_element is not None:
                    chapters_url = chapters_element.attrib.get('url')
                    if chapters_url:
                        return chapters_url  # Return the chapters URL to fetch the JSON
                    else:
                        print(f"Chapter element with missing URL: {ET.tostring(chapters_element, encoding='unicode')}")
                break  # Exit loop once the matching episode is found
    except ET.ParseError as e:
        print(f"XML parsing error: {e} - Content: {feed_content[:200]}")  # Log the error and first 200 characters of content
    return chapters

def parse_transcripts(feed_content: str, audio_url: str) -> List[Dict[str, Optional[str]]]:
    transcripts = []
    try:
        root = ET.fromstring(feed_content)
        episodes = root.findall('.//item')
        for episode in episodes:
            enclosure_element = episode.find('enclosure')
            enclosure_url = enclosure_element.attrib.get('url') if enclosure_element is not None else None
            if enclosure_element is not None and enclosure_url == audio_url:
                transcript_elements = episode.findall('podcast:transcript', NAMESPACE)
                for transcript_element in transcript_elements:
                    transcript_url = transcript_element.attrib.get('url')
                    transcript_type = transcript_element.attrib.get('type')
                    transcript_language = transcript_element.attrib.get('language')
                    transcript_rel = transcript_element.attrib.get('rel')
                    transcripts.append({
                        "url": transcript_url,
                        "mime_type": transcript_type,
                        "language": transcript_language,
                        "rel": transcript_rel
                    })
                break  # Exit loop once the matching episode is found
    except ET.ParseError as e:
        print(f"XML parsing error: {e} - Content: {feed_content[:200]}")  # Log the error and first 200 characters of content
    return transcripts


class TTLCache:
    def __init__(self, maxsize: int = 1000, ttl: int = 3600):
        self.maxsize = maxsize
        self.ttl = ttl
        self.cache: Dict[Tuple, Tuple[Any, float]] = {}

    async def get_or_set(self, key: Tuple, callback: Callable):
        current_time = time.time()

        # Check if key exists and hasn't expired
        if key in self.cache:
            result, timestamp = self.cache[key]
            if current_time - timestamp < self.ttl:
                return result

        # If we get here, either key doesn't exist or has expired
        try:
            # Await the callback here
            result = await callback()

            # Store new result
            self.cache[key] = (result, current_time)

            # Enforce maxsize by removing oldest entries
            if len(self.cache) > self.maxsize:
                oldest_key = min(self.cache.keys(), key=lambda k: self.cache[k][1])
                del self.cache[oldest_key]

            return result
        except Exception as e:
            logging.error(f"Error in cache callback: {e}")
            raise

def async_ttl_cache(maxsize: int = 1000, ttl: int = 3600):
    cache = TTLCache(maxsize=maxsize, ttl=ttl)

    def decorator(func):
        @wraps(func)
        async def wrapper(*args, **kwargs):
            # Create a cache key from the function arguments
            key = (func.__name__, args, frozenset(kwargs.items()))

            try:
                # Create an async callback
                async def callback():
                    return await func(*args, **kwargs)

                return await cache.get_or_set(key, callback)
            except Exception as e:
                logging.error(f"Error in cached function {func.__name__}: {e}")
                # Fall back to calling the function directly
                return await func(*args, **kwargs)

        return wrapper
    return decorator

@async_ttl_cache(maxsize=1000, ttl=3600)
async def get_podpeople_hosts(podcast_index_id: int) -> List[Dict[str, Optional[str]]]:
    try:
        async with httpx.AsyncClient(timeout=5.0) as client:
            url = f"{people_url}/api/hosts/{podcast_index_id}"
            response = await client.get(url)
            response.raise_for_status()
            hosts_data = response.json()

            if hosts_data:
                return [{
                    "name": host.get("name"),
                    "role": host.get("role", "Host"),
                    "group": None,
                    "img": host.get("img"),
                    "href": host.get("link"),
                    "description": host.get("description")
                } for host in hosts_data]
    except Exception as e:
        logging.error(f"Error fetching hosts: {e}")

    return []

async def parse_people(feed_content: str, audio_url: Optional[str] = None, podcast_index_id: Optional[int] = None) -> List[Dict[str, Optional[str]]]:
    people = []
    try:
        root = ET.fromstring(feed_content)
        if audio_url:
            # Look for episode-specific people
            episodes = root.findall('.//item')
            for episode in episodes:
                enclosure_element = episode.find('enclosure')
                enclosure_url = enclosure_element.attrib.get('url') if enclosure_element is not None else None
                if enclosure_element is not None and enclosure_url == audio_url:
                    person_elements = episode.findall('podcast:person', NAMESPACE)
                    if person_elements:
                        for person_element in person_elements:
                            people.append({
                                "name": person_element.text,
                                "role": person_element.attrib.get('role'),
                                "group": person_element.attrib.get('group'),
                                "img": person_element.attrib.get('img'),
                                "href": person_element.attrib.get('href'),
                            })
                    break

        if not people:
            # Fall back to channel-wide people
            person_elements = root.findall('.//channel/podcast:person', NAMESPACE)
            for person_element in person_elements:
                people.append({
                    "name": person_element.text,
                    "role": person_element.attrib.get('role'),
                    "group": person_element.attrib.get('group'),
                    "img": person_element.attrib.get('img'),
                    "href": person_element.attrib.get('href'),
                })
    except ET.ParseError as e:
        logging.error(f"XML parsing error: {e} - Content: {feed_content[:200]}")

    # If no people found in the feed, fall back to podpeople_db
    if not people and podcast_index_id:
        # Use the async version
        people = await get_podpeople_hosts(podcast_index_id)

    return people

@app.get("/api/data/fetch_podcasting_2_data")
async def fetch_podcasting_2_data(
    episode_id: int,
    user_id: int,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Invalid API key or insufficient permissions")

    try:
        # Get all the metadata
        episode_metadata = database_functions.functions.get_episode_metadata(database_type, cnx, episode_id, user_id)
        podcast_id = database_functions.functions.get_podcast_id_from_episode(cnx, database_type, episode_id, user_id)
        podcast_feed = database_functions.functions.get_podcast_details(database_type, cnx, user_id, podcast_id)

        episode_url = episode_metadata['episodeurl']
        podcast_feed_url = podcast_feed['feedurl']
        podcast_index_id = database_functions.functions.get_podcast_index_id(cnx, database_type, podcast_id)

        # Set up common request parameters
        headers = {
            'User-Agent': 'PinePods/1.0',
            'Accept': 'application/xml, application/rss+xml, text/xml, application/json'
        }

        # Check if podcast requires authentication
        auth = None
        if podcast_feed.get('username') and podcast_feed.get('password'):
            auth = httpx.BasicAuth(
                username=podcast_feed['username'],
                password=podcast_feed['password']
            )

        # Fetch feed content with authentication if needed
        async with httpx.AsyncClient(timeout=10.0, follow_redirects=True) as client:
            try:
                response = await client.get(
                    podcast_feed_url,
                    headers=headers,
                    auth=auth
                )
                response.raise_for_status()
                feed_content = response.text
            except httpx.HTTPStatusError as e:
                if e.response.status_code == 401:
                    logging.error(f"Authentication failed for podcast feed: {podcast_feed_url}")
                    raise HTTPException(
                        status_code=401,
                        detail="Authentication required or invalid credentials for podcast feed"
                    )
                raise

        # Parse feed content
        chapters_url = parse_chapters(feed_content, episode_url)
        transcripts = parse_transcripts(feed_content, episode_url)
        people = await parse_people(feed_content, episode_url, podcast_index_id)

        # Get chapters if available
        chapters_data = []
        if chapters_url:
            try:
                async with httpx.AsyncClient(timeout=5.0, follow_redirects=True) as client:
                    # Use same auth for chapters if it's from the same domain
                    chapters_auth = auth if chapters_url.startswith(podcast_feed_url) else None
                    chapters_headers = {
                        'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
                        'Accept': 'application/json, text/javascript, */*; q=0.01',
                        'Accept-Language': 'en-US,en;q=0.9',
                        'Referer': podcast_feed_url
                    }
                    response = await client.get(
                        chapters_url,
                        headers=chapters_headers,
                        auth=chapters_auth
                    )
                    response.raise_for_status()
                    chapters_data = response.json().get('chapters', [])
            except Exception as e:
                logging.error(f"Error fetching chapters: {e}")
                # Continue with empty chapters rather than failing completely

        return {
            "chapters": chapters_data,
            "transcripts": transcripts,
            "people": people
        }

    except httpx.HTTPStatusError as e:
        logging.error(f"HTTP error in fetch_podcasting_2_data: {e}")
        raise HTTPException(
            status_code=e.response.status_code,
            detail=f"Error fetching podcast data: {str(e)}"
        )
    except httpx.RequestError as e:
        logging.error(f"Request error in fetch_podcasting_2_data: {e}")
        raise HTTPException(
            status_code=500,
            detail=f"Failed to fetch podcast data: {str(e)}"
        )
    except Exception as e:
        logging.error(f"Error in fetch_podcasting_2_data: {e}")
        # Return partial data if we have it
        if any(var in locals() for var in ['chapters_data', 'transcripts', 'people']):
            return {
                "chapters": locals().get('chapters_data', []),
                "transcripts": locals().get('transcripts', []),
                "people": locals().get('people', [])
            }
        raise HTTPException(status_code=500, detail=str(e))

def is_valid_image_url(url: str) -> bool:
    """Validate image URL for security"""
    parsed = urlparse(url)
    # Check if URL is absolute and uses http(s)
    if not parsed.scheme or parsed.scheme not in ('http', 'https'):
        return False
    return True

@app.get("/api/proxy/image")
async def proxy_image(
    url: str = Query(..., description="URL of the image to proxy")
):
    logging.info(f"Image proxy request received for URL: {url}")

    if not is_valid_image_url(url):
        logging.error(f"Invalid image URL: {url}")
        raise HTTPException(status_code=400, detail="Invalid image URL")

    try:
        async with httpx.AsyncClient(follow_redirects=True) as client:
            logging.info(f"Fetching image from: {url}")
            response = await client.get(url, timeout=10.0)
            logging.info(f"Image fetch response status: {response.status_code}")
            logging.info(f"Response headers: {response.headers}")

            response.raise_for_status()

            content_type = response.headers.get("Content-Type", "")
            logging.info(f"Content type: {content_type}")

            if not content_type.startswith(("image/", "application/octet-stream")):
                logging.error(f"Invalid content type: {content_type}")
                raise HTTPException(status_code=400, detail="URL does not point to an image")

            headers = {
                "Content-Type": content_type,
                "Cache-Control": "public, max-age=86400",
                "Access-Control-Allow-Origin": "*",
                "X-Content-Type-Options": "nosniff"
            }
            logging.info("Returning image response")

            return StreamingResponse(
                response.aiter_bytes(),
                headers=headers,
                media_type=content_type
            )
    except Exception as e:
        logging.error(f"Error in image proxy: {str(e)}")
        raise HTTPException(status_code=500, detail=str(e))


def parse_podroll(feed_content: str) -> List[Dict[str, Optional[str]]]:
    podroll = []
    try:
        root = ET.fromstring(feed_content)
        podroll_element = root.find('.//channel/podcast:podroll', NAMESPACE)
        if podroll_element is not None:
            for remote_item in podroll_element.findall('podcast:remoteItem', NAMESPACE):
                podroll.append({
                    "feed_guid": remote_item.attrib.get('feedGuid')
                })
    except ET.ParseError as e:
        logging.error(f"XML parsing error: {e} - Content: {feed_content[:200]}")  # Log the error and first 200 characters of content
    return podroll

def parse_funding(feed_content: str) -> List[Dict[str, Optional[str]]]:
    funding = []
    try:
        root = ET.fromstring(feed_content)
        funding_elements = root.findall('.//channel/podcast:funding', NAMESPACE)
        for funding_element in funding_elements:
            funding.append({
                "url": funding_element.attrib.get('url'),
                "description": funding_element.text
            })
    except ET.ParseError as e:
        logging.error(f"XML parsing error: {e} - Content: {feed_content[:200]}")  # Log the error and first 200 characters of content
    return funding

def parse_value(feed_content: str) -> List[Dict[str, Optional[str]]]:
    value = []
    try:
        root = ET.fromstring(feed_content)
        value_elements = root.findall('.//channel/podcast:value', NAMESPACE)
        for value_element in value_elements:
            value_recipients = []
            for recipient in value_element.findall('podcast:valueRecipient', NAMESPACE):
                value_recipients.append({
                    "name": recipient.attrib.get('name'),
                    "type": recipient.attrib.get('type'),
                    "address": recipient.attrib.get('address'),
                    "split": recipient.attrib.get('split')
                })
            value.append({
                "type": value_element.attrib.get('type'),
                "method": value_element.attrib.get('method'),
                "suggested": value_element.attrib.get('suggested'),
                "recipients": value_recipients
            })
    except ET.ParseError as e:
        logging.error(f"XML parsing error: {e} - Content: {feed_content[:200]}")  # Log the error and first 200 characters of content
    return value

def parse_hosts(feed_content: str) -> List[Dict[str, Optional[str]]]:
    people = []
    try:
        root = ET.fromstring(feed_content)
        person_elements = root.findall('.//channel/podcast:person', NAMESPACE)
        for person_element in person_elements:
            role = person_element.attrib.get('role', 'host').lower()
            if role == 'host':
                people.append({
                    "name": person_element.text,
                    "role": role,
                    "group": person_element.attrib.get('group'),
                    "img": person_element.attrib.get('img'),
                    "href": person_element.attrib.get('href')
                })
    except ET.ParseError as e:
        logging.error(f"XML parsing error: {e} - Content: {feed_content[:200]}")  # Log the error and first 200 characters of content
    return people

async def get_podcast_hosts(cnx, database_type, podcast_id, feed_content, podcast_index_id):
    # First, try to parse hosts from the feed content
    hosts = parse_hosts(feed_content)

    # If no hosts found, try podpeople_db
    if not hosts:
        if podcast_index_id:
            hosts = await get_podpeople_hosts(podcast_index_id)

    # If still no hosts found, return a default host
    if not hosts:
        hosts = [{
            "name": "Unknown Host",
            "role": "Host",
            "description": "No host information available.",
            "img": None,
            "href": None
        }]

    return hosts

@app.get("/api/data/fetch_podcasting_2_pod_data")
async def fetch_podcasting_2_pod_data(podcast_id: int, user_id: int, cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Invalid API key or insufficient permissions")

    # Fetch the podcast details including auth credentials
    podcast_feed = database_functions.functions.get_podcast_details(database_type, cnx, user_id, podcast_id)
    podcast_feed_url = podcast_feed['feedurl']

    # Set up HTTP client with authentication if credentials exist
    async with httpx.AsyncClient(follow_redirects=True) as client:
        headers = {
            'User-Agent': 'PinePods/1.0',
            'Accept': 'application/xml, application/rss+xml, text/xml'
        }

        # Check if podcast requires authentication
        auth = None
        if podcast_feed.get('username') and podcast_feed.get('password'):
            auth = httpx.BasicAuth(
                username=podcast_feed['username'],
                password=podcast_feed['password']
            )

        try:
            response = await client.get(
                podcast_feed_url,
                headers=headers,
                auth=auth,
                timeout=30.0  # Add reasonable timeout
            )
            response.raise_for_status()
            feed_content = response.text

            logging.info(f"Successfully fetched feed content from {podcast_feed_url}")

            # Parse the feed content for various metadata
            people = await get_podcast_hosts(cnx, database_type, podcast_id, feed_content, podcast_feed['podcastindexid'])
            podroll = parse_podroll(feed_content)
            funding = parse_funding(feed_content)
            value = parse_value(feed_content)

            logging.debug(f"Parsed metadata - People: {len(people) if people else 0} entries")

            return {
                "people": people,
                "podroll": podroll,
                "funding": funding,
                "value": value
            }

        except httpx.HTTPStatusError as e:
            if e.response.status_code == 401:
                logging.error(f"Authentication failed for podcast feed: {podcast_feed_url}")
                raise HTTPException(
                    status_code=401,
                    detail="Authentication required or invalid credentials for podcast feed"
                )
            raise HTTPException(
                status_code=e.response.status_code,
                detail=f"Error fetching podcast feed: {str(e)}"
            )
        except httpx.RequestError as e:
            logging.error(f"Request error fetching podcast feed: {str(e)}")
            raise HTTPException(
                status_code=500,
                detail=f"Failed to fetch podcast feed: {str(e)}"
            )
        except Exception as e:
            logging.error(f"Unexpected error processing podcast feed: {str(e)}")
            raise HTTPException(
                status_code=500,
                detail=f"Error processing podcast feed: {str(e)}"
            )


class PodcastResponse(BaseModel):
    podcastid: int
    podcastname: str
    feedurl: str

class PodPeopleResponse(BaseModel):
    success: bool
    podcasts: List[PodcastResponse]

@app.get("/api/data/podpeople/host_podcasts")
async def get_host_podcasts(
    hostname: str,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    """
    Get podcasts associated with a host from the podpeople database.
    """
    # Verify API key
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Invalid API key or insufficient permissions")

    try:
        # Make request to podpeople database
        async with httpx.AsyncClient(follow_redirects=True) as client:
            logging.info(f"Making request to {people_url}/api/hostsearch?name={hostname}")
            response = await client.get(
                f"{people_url}/api/hostsearch",  # Changed this line to match working endpoint
                params={"name": hostname}
            )
            response.raise_for_status()
            podpeople_data = response.json()

            logging.info(f"Received response from podpeople: {podpeople_data}")

            # Transform the podpeople response into our expected format
            podcasts = []
            if podpeople_data.get("success") and podpeople_data.get("podcasts"):
                for podcast in podpeople_data["podcasts"]:
                    podcasts.append({
                        'podcastid': podcast['id'],
                        'podcastname': podcast['title'],
                        'feedurl': podcast['feed_url']
                    })

            logging.info(f"Transformed response: {podcasts}")

            return PodPeopleResponse(
                success=True,
                podcasts=podcasts
            )

    except httpx.HTTPStatusError as e:
        logging.error(f"HTTP error from podpeople: {str(e)}")
        raise HTTPException(
            status_code=e.response.status_code,
            detail=f"Error from podpeople service: {str(e)}"
        )
    except httpx.RequestError as e:
        logging.error(f"Error connecting to podpeople: {str(e)}")
        raise HTTPException(
            status_code=500,
            detail=f"Error connecting to podpeople service: {str(e)}"
        )
    except Exception as e:
        logging.error(f"Unexpected error: {str(e)}")
        raise HTTPException(
            status_code=500,
            detail=f"Unexpected error: {str(e)}"
        )

@app.post("/api/data/check_episode_playback")
async def api_check_episode_playback(
        user_id: int = Form(...),
        episode_title: Optional[str] = Form(None),
        episode_url: Optional[str] = Form(None),
        cnx=Depends(get_database_connection),
        api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == user_id or is_web_key:
        logging.info(f"Received: user_id={user_id}, episode_title={episode_title}, episode_url={episode_url}")

        has_playback, listen_duration = database_functions.functions.check_episode_playback(
            cnx, database_type, user_id, episode_title, episode_url
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
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)

    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    elevated_access = await has_elevated_access(api_key, cnx)

    if not elevated_access:
        # Get user ID from API key
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

        if user_id != user_id_from_api_key:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN,
                                detail="You are not authorized to access these user details")
    result = database_functions.functions.get_user_details_id(cnx, database_type, user_id)
    if result:
        return result
    else:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="User not found")


@app.get("/api/data/get_theme/{user_id}")
async def api_get_theme(user_id: int, cnx=Depends(get_database_connection),
                        api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    # Allow the action if the API key belongs to the user, or it's the web API key
    if key_id == user_id or is_web_key:
        theme = database_functions.functions.get_theme(cnx, database_type, user_id)
        return {"theme": theme}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only get themes for yourself!")


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

class AddPodcastRequest(BaseModel):
    podcast_values: PodcastValuesModel
    podcast_index_id: int = Field(default=0)

@app.post("/api/data/add_podcast")
async def api_add_podcast(
    request: AddPodcastRequest,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id == request.podcast_values.user_id or is_web_key:
        if database_functions.functions.check_gpodder_settings(database_type, cnx, request.podcast_values.user_id):
            gpodder_url, gpodder_token, gpodder_login = database_functions.functions.get_nextcloud_settings(database_type, cnx, request.podcast_values.user_id)
            gpod_type = database_functions.functions.get_gpodder_type(cnx, database_type, request.podcast_values.user_id)

            if gpod_type == "gpodder":
                default_device = database_functions.functions.get_default_gpodder_device(cnx, database_type, request.podcast_values.user_id)
                device_name = default_device["name"] if default_device else f"pinepods-internal-{request.podcast_values.user_id}"

            if gpod_type == "nextcloud":
                database_functions.functions.add_podcast_to_nextcloud(cnx, database_type, gpodder_url, gpodder_login, gpodder_token, request.podcast_values.pod_feed_url)
            else:
                database_functions.functions.add_podcast_to_opodsync(cnx, database_type, request.podcast_values.user_id, gpodder_url, gpodder_login, gpodder_token, request.podcast_values.pod_feed_url, device_name)

        result = database_functions.functions.add_podcast(
            cnx,
            database_type,
            request.podcast_values.dict(),
            request.podcast_values.user_id,
            30,
            podcast_index_id=request.podcast_index_id
        )

        if isinstance(result, tuple):
            podcast_id, first_episode_id = result
        else:
            podcast_id = result
            first_episode_id = None  # Or fetch it if needed

        if podcast_id:
            return {"success": True, "podcast_id": podcast_id, "first_episode_id": first_episode_id}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only add podcasts for yourself!")

@app.post("/api/data/enable_disable_guest")
async def api_enable_disable_guest(is_admin: bool = Depends(check_if_admin), cnx=Depends(get_database_connection)):
    database_functions.functions.enable_disable_guest(cnx, database_type)
    return {"success": True}


@app.post("/api/data/enable_disable_downloads")
async def api_enable_disable_downloads(is_admin: bool = Depends(check_if_admin), cnx=Depends(get_database_connection)):
    database_functions.functions.enable_disable_downloads(cnx, database_type)
    return {"success": True}


@app.post("/api/data/enable_disable_self_service")
async def api_enable_disable_self_service(is_admin: bool = Depends(check_if_admin),
                                          cnx=Depends(get_database_connection)):
    database_functions.functions.enable_disable_self_service(cnx, database_type)
    return {"success": True}


@app.get("/api/data/self_service_status")
async def api_self_service_status(cnx=Depends(get_database_connection)):
    status = database_functions.functions.self_service_status(cnx, database_type)
    # Return status directly without wrapping it in another dict
    return status  # Instead of {"status": status}

class FirstAdminRequest(BaseModel):
    username: str
    password: str
    email: str
    fullname: str



@app.post("/api/data/create_first")
async def create_first_admin(
    request: FirstAdminRequest,
    background_tasks: BackgroundTasks,
    cnx=Depends(get_database_connection)
):
    if database_functions.functions.check_admin_exists(cnx, database_type):
        raise HTTPException(
            status_code=403,
            detail="An admin user already exists"
        )
    try:
        user_id = database_functions.functions.add_admin_user(
            cnx,
            database_type,
            (request.fullname, request.username.lower(), request.email, request.password)
        )

        background_tasks.add_task(run_startup_tasks_background)
        return {"message": "Admin user created successfully", "user_id": user_id}
    except Exception as e:
        raise HTTPException(
            status_code=500,
            detail=str(e)
        )

def run_startup_tasks_background():
    cnx = create_database_connection()
    try:
        with open("/tmp/web_api_key.txt", "r") as f:
            web_key = f.read().strip()
        init_request = InitRequest(api_key=web_key)
        # Execute startup tasks directly instead of calling the endpoint
        is_valid = database_functions.functions.verify_api_key(cnx, database_type, web_key)
        is_web_key = web_key == base_webkey.web_key
        if not is_valid or not is_web_key:
            raise Exception("Invalid web key")
        database_functions.functions.add_news_feed_if_not_added(database_type, cnx)
    except Exception as e:
        logger.error(f"Background startup tasks failed: {e}")
    finally:
        close_database_connection(cnx)

@app.put("/api/data/increment_listen_time/{user_id}")
async def api_increment_listen_time(user_id: int, cnx=Depends(get_database_connection),
                                    api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    # Allow the action if the API key belongs to the user, or it's the web API key
    if key_id == user_id or is_web_key:
        database_functions.functions.increment_listen_time(cnx, database_type, user_id)
        return {"detail": "Listen time incremented."}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only increment your own listen time.")


@app.put("/api/data/increment_played/{user_id}")
async def api_increment_played(user_id: int, cnx=Depends(get_database_connection),
                               api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == user_id or is_web_key:
        database_functions.functions.increment_played(cnx, database_type, user_id)
        return {"detail": "Played count incremented."}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only increment your own play count.")


class RecordHistoryData(BaseModel):
    episode_id: int
    user_id: int
    episode_pos: float
    is_youtube: bool = False  # Default to False for backward compatibility

@app.post("/api/data/record_podcast_history")
async def api_record_podcast_history(data: RecordHistoryData, cnx=Depends(get_database_connection),
                                   api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                          detail="Your API key is either invalid or does not have correct permission")

    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id == data.user_id or is_web_key:
        database_functions.functions.record_podcast_history(
            cnx,
            database_type,
            data.episode_id,
            data.user_id,
            data.episode_pos,
            data.is_youtube
        )
        return {"detail": "History recorded successfully."}
    else:
        raise HTTPException(status_code=403,
                          detail="You can only record history for yourself!")


class GetEpisodeIdRequest(BaseModel):
    podcast_id: int
    user_id: int
    is_youtube: bool = False  # Add default False


@app.post("/api/data/get_episode_id")
async def api_get_episode_id(data: GetEpisodeIdRequest, cnx=Depends(get_database_connection),
                           api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Your API key is either invalid or does not have correct permission")

    episode_id = database_functions.functions.get_first_episode_id(
        cnx,
        database_type,
        data.podcast_id,
        data.user_id,
        data.is_youtube
    )

    if episode_id is None:
        raise HTTPException(status_code=404, detail="No episodes found for this podcast.")
    return {"episode_id": episode_id}



class DownloadPodcastData(BaseModel):
    episode_id: int
    user_id: int
    is_youtube: bool = False  # Default to False for backward compatibility

@app.post("/api/data/download_podcast")
async def api_download_podcast(
    data: DownloadPodcastData,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    """
    Queue a single episode or YouTube video for download.
    This uses the Celery task queue to handle the download asynchronously.
    """
    # Validate API key
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(
            status_code=403,
            detail="Your API key is either invalid or does not have correct permission"
        )
    # Check permissions
    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
    if key_id != data.user_id and not is_web_key:
        raise HTTPException(
            status_code=403,
            detail="You can only download content for yourself!"
        )
    try:
        # Check if already downloaded
        is_downloaded = database_functions.functions.check_downloaded(
            cnx,
            database_type,
            data.user_id,
            data.episode_id,
            data.is_youtube
        )
        if is_downloaded:
            return {"detail": "Content already downloaded."}
        # Queue the appropriate download task
        if data.is_youtube:
            task = database_functions.tasks.download_youtube_video_task.delay(data.episode_id, data.user_id, database_type)
            content_type = "YouTube video"
        else:
            task = database_functions.tasks.download_podcast_task.delay(data.episode_id, data.user_id, database_type)
            content_type = "Podcast episode"
        return {
            "detail": f"{content_type} download has been queued and will process in the background.",
            "task_id": task.id
        }
    except Exception as e:
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error queueing download: {str(e)}"
        )

def download_content_fun(episode_id: int, user_id: int, is_youtube: bool):
    cnx = create_database_connection()
    try:
        if is_youtube:
            database_functions.functions.download_youtube_video(cnx, database_type, episode_id, user_id)
        else:
            database_functions.functions.download_podcast(cnx, database_type, episode_id, user_id)
    finally:
        cnx.close()


class DownloadAllPodcastData(BaseModel):
    podcast_id: int
    user_id: int
    is_youtube: bool = False

# Updated API endpoint using Celery for mass downloads
@app.post("/api/data/download_all_podcast")
async def api_download_all_podcast(
    data: DownloadAllPodcastData,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    """
    Queue all episodes of a podcast or videos of a YouTube channel for download.
    Uses a Celery task queue to process downloads in the background without blocking the server.
    """
    # Validate API key
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(
            status_code=403,
            detail="Your API key is either invalid or does not have correct permission"
        )

    # Check permissions
    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id != data.user_id and not is_web_key:
        raise HTTPException(
            status_code=403,
            detail="You can only download content for yourself!"
        )

    try:
        # Verify the podcast/channel exists
        if data.is_youtube:
            # Check if channel exists
            videos = database_functions.functions.get_video_ids_for_podcast(
                cnx, database_type, data.podcast_id
            )
            if not videos:
                return {"detail": "No videos found for the given YouTube channel."}
        else:
            # Check if podcast exists
            episodes = database_functions.functions.get_episode_ids_for_podcast(
                cnx, database_type, data.podcast_id
            )
            if not episodes:
                return {"detail": "No episodes found for the given podcast."}

        # Queue the download task using Celery
        task = database_functions.tasks.queue_podcast_downloads.delay(
            data.podcast_id,
            data.user_id,
            database_type,
            data.is_youtube
        )

        return {
            "detail": f"{'YouTube channel' if data.is_youtube else 'Podcast'} download has been queued. "
                    "Episodes will be downloaded in the background.",
            "task_id": task.id
        }

    except Exception as e:
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error queueing downloads: {str(e)}"
        )

@app.get("/api/data/download_status/{user_id}")
async def api_download_status(
    user_id: int,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    """
    Get the status of all active downloads for a user.
    """
    # Validate API key
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(
            status_code=403,
            detail="Your API key is either invalid or does not have correct permission"
        )

    # Check permissions
    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id != user_id and not is_web_key:
        raise HTTPException(
            status_code=403,
            detail="You can only view your own downloads!"
        )

    try:
        # Get all active downloads for the user
        downloads = database_functions.tasks.download_manager.get_user_downloads(user_id)

        # Return the downloads
        return {
            "downloads": downloads,
            "count": len(downloads)
        }

    except Exception as e:
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error retrieving download status: {str(e)}"
        )

class DeletePodcastData(BaseModel):
    episode_id: int
    user_id: int
    is_youtube: bool = False  # Default to False for backward compatibility

@app.post("/api/data/delete_episode")
async def api_delete_podcast(data: DeletePodcastData, cnx=Depends(get_database_connection),
                           api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                          detail="Your API key is either invalid or does not have correct permission")

    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id == data.user_id or is_web_key:
        database_functions.functions.delete_episode(database_type, cnx, data.episode_id,
                                                 data.user_id, data.is_youtube)
        return {"detail": "Episode(s) Deleted"}
    else:
        raise HTTPException(status_code=403,
                          detail="You can only delete content for yourself!")

class MarkEpisodeCompletedData(BaseModel):
    episode_id: int
    user_id: int
    is_youtube: bool = False  # Added field with default False

@app.post("/api/data/mark_episode_completed")
async def api_mark_episode_completed(data: MarkEpisodeCompletedData, cnx=Depends(get_database_connection),
                                     api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == data.user_id or is_web_key:
        database_functions.functions.mark_episode_completed(
            cnx,
            database_type,
            data.episode_id,
            data.user_id,
            data.is_youtube
        )
        return {"detail": "Episode marked as completed."}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only mark episodes as completed for yourself.")

@app.post("/api/data/mark_episode_uncompleted")
async def api_mark_episode_uncompleted(data: MarkEpisodeCompletedData, cnx=Depends(get_database_connection),
                                     api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                          detail="Your API key is either invalid or does not have correct permission")

    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id == data.user_id or is_web_key:
        database_functions.functions.mark_episode_uncompleted(
            cnx,
            database_type,
            data.episode_id,
            data.user_id,
            data.is_youtube
        )
        return {"detail": "Episode marked as uncompleted."}
    else:
        raise HTTPException(status_code=403,
                          detail="You can only mark episodes as uncompleted for yourself.")

class AutoDownloadRequest(BaseModel):
    podcast_id: int
    auto_download: bool
    user_id: int

@app.post("/api/data/enable_auto_download")
async def api_enable_auto_download(data: AutoDownloadRequest, cnx=Depends(get_database_connection),
                                   api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id == data.user_id:
        database_functions.functions.enable_auto_download(cnx, database_type, data.podcast_id, data.user_id, data.auto_download)
        return {"detail": "Auto-download status updated."}
    else:
        raise HTTPException(status_code=403, detail="You can only modify your own podcasts.")

class AutoDownloadStatusRequest(BaseModel):
    podcast_id: int
    user_id: int

class AutoDownloadStatusResponse(BaseModel):
    auto_download: bool

@app.post("/api/data/get_auto_download_status")
async def api_get_auto_download_status(data: AutoDownloadStatusRequest, cnx=Depends(get_database_connection),
                                       api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Your API key is either invalid or does not have correct permission")

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
    if key_id != data.user_id:
        raise HTTPException(status_code=403, detail="You can only get the status for your own podcast.")

    status = database_functions.functions.call_get_auto_download_status(cnx, database_type, data.podcast_id, data.user_id)
    if status is None:
        raise HTTPException(status_code=404, detail="Podcast not found")

    return AutoDownloadStatusResponse(auto_download=status)

class SkipTimesRequest(BaseModel):
    podcast_id: int
    start_skip: Optional[int] = 0
    end_skip: Optional[int] = 0
    user_id: int

@app.post("/api/data/adjust_skip_times")
async def api_adjust_skip_times(data: SkipTimesRequest, cnx=Depends(get_database_connection),
                                api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id == data.user_id or is_web_key:
        database_functions.functions.adjust_skip_times(cnx, database_type, data.podcast_id, data.start_skip, data.end_skip)
        return {"detail": "Skip times updated."}
    else:
        raise HTTPException(status_code=403, detail="You can only modify your own podcasts.")

class AutoSkipTimesRequest(BaseModel):
    podcast_id: int
    user_id: int

class AutoSkipTimesResponse(BaseModel):
    start_skip: int
    end_skip: int

@app.post("/api/data/get_auto_skip_times")
async def api_get_auto_skip_times(data: AutoSkipTimesRequest, cnx=Depends(get_database_connection),
                                  api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Your API key is either invalid or does not have correct permission")

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
    if key_id != data.user_id:
        raise HTTPException(status_code=403, detail="You can only get the skip times for your own podcast.")

    start_skip, end_skip = database_functions.functions.get_auto_skip_times(cnx, database_type, data.podcast_id, data.user_id)
    if start_skip is None or end_skip is None:
        raise HTTPException(status_code=404, detail="Podcast not found")

    return AutoSkipTimesResponse(start_skip=start_skip, end_skip=end_skip)

class PlayEpisodeDetailsRequest(BaseModel):
    podcast_id: int
    user_id: int
    is_youtube: bool = False

class PlayEpisodeDetailsResponse(BaseModel):
    playback_speed: float
    start_skip: int
    end_skip: int

@app.post("/api/data/get_play_episode_details")
async def api_get_play_episode_details(data: PlayEpisodeDetailsRequest,
                                       cnx=Depends(get_database_connection),
                                       api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                          detail="Your API key is either invalid or does not have correct permission")

    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id == data.user_id or is_web_key:
        # Get all details in one function call
        playback_speed, start_skip, end_skip = database_functions.functions.get_play_episode_details(
            cnx,
            database_type,
            data.user_id,
            data.podcast_id,
            data.is_youtube
        )

        return PlayEpisodeDetailsResponse(
            playback_speed=playback_speed,
            start_skip=start_skip,
            end_skip=end_skip
        )
    else:
        raise HTTPException(status_code=403,
                          detail="You can only get metadata for yourself!")

class ClearPlaybackSpeedRequest(BaseModel):
    podcast_id: int
    user_id: int

@app.post("/api/data/clear_podcast_playback_speed")
async def api_clear_podcast_playback_speed(data: ClearPlaybackSpeedRequest,
                                          cnx=Depends(get_database_connection),
                                          api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                          detail="Your API key is either invalid or does not have correct permission")

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
    if key_id != data.user_id:
        raise HTTPException(status_code=403,
                          detail="You can only modify your own podcast settings!")

    success = database_functions.functions.clear_podcast_playback_speed(
        cnx,
        database_type,
        data.podcast_id,
        data.user_id
    )

    if success:
        return {"message": "Playback speed cleared successfully"}
    else:
        raise HTTPException(status_code=500, detail="Failed to clear playback speed")

class SetPlaybackSpeedPodcast(BaseModel):
    user_id: int
    podcast_id: int
    playback_speed: float

class SetPlaybackSpeedUser(BaseModel):
    user_id: int
    playback_speed: float

@app.post("/api/data/podcast/set_playback_speed")
async def api_set_playback_speed_podcast(data: SetPlaybackSpeedPodcast, cnx=Depends(get_database_connection),
                                api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Your API key is either invalid or does not have correct permission")
    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
    if key_id == data.user_id or is_web_key:
        database_functions.functions.set_playback_speed_podcast(cnx, database_type, data.podcast_id, data.playback_speed)
        return {"detail": "Default podcast playback speed updated."}
    else:
        raise HTTPException(status_code=403, detail="You can only modify your own podcasts.")

@app.post("/api/data/user/set_playback_speed")
async def api_set_playback_speed_user(data: SetPlaybackSpeedUser, cnx=Depends(get_database_connection),
                                api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Your API key is either invalid or does not have correct permission")
    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
    if key_id == data.user_id or is_web_key:
        database_functions.functions.set_playback_speed_user(cnx, database_type, data.user_id, data.playback_speed)
        return {"detail": "Default playback speed updated."}
    else:
        raise HTTPException(status_code=403, detail="You can only modify your own settings.")


class SaveEpisodeData(BaseModel):
    episode_id: int
    user_id: int
    is_youtube: bool = False

@app.post("/api/data/save_episode")
async def api_save_episode(data: SaveEpisodeData, cnx=Depends(get_database_connection),
                         api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                          detail="Your API key is either invalid or does not have correct permission")

    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id == data.user_id or is_web_key:
        ep_status = database_functions.functions.check_saved(
            cnx, database_type, data.user_id, data.episode_id, data.is_youtube
        )
        if ep_status:
            return {"detail": f"{'Video' if data.is_youtube else 'Episode'} already saved."}
        else:
            success = database_functions.functions.save_episode(
                cnx, database_type, data.episode_id, data.user_id, data.is_youtube
            )
        if success:
            return {"detail": f"{'Video' if data.is_youtube else 'Episode'} saved!"}
        else:
            raise HTTPException(status_code=400, detail=f"Error saving {'video' if data.is_youtube else 'episode'}.")
    else:
        raise HTTPException(status_code=403,
                          detail=f"You can only save {'videos' if data.is_youtube else 'episodes'} of your own!")

class RemoveSavedEpisodeData(BaseModel):
    episode_id: int
    user_id: int
    is_youtube: bool = False

@app.post("/api/data/remove_saved_episode")
async def api_remove_saved_episode(data: RemoveSavedEpisodeData, cnx=Depends(get_database_connection),
                                 api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if is_valid_key:
        key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
        if key_id == data.user_id:
            database_functions.functions.remove_saved_episode(
                cnx, database_type, data.episode_id, data.user_id, data.is_youtube
            )
            return {"detail": f"Saved {'video' if data.is_youtube else 'episode'} removed."}
        else:
            raise HTTPException(status_code=403,
                              detail=f"You can only remove {'videos' if data.is_youtube else 'episodes'} of your own!")
    else:
        raise HTTPException(status_code=403,
                          detail="Your API key is either invalid or does not have correct permission")


class AddCategoryData(BaseModel):
    podcast_id: int
    user_id: int
    category: str

@app.post("/api/data/add_category")
async def api_add_category(data: AddCategoryData, cnx=Depends(get_database_connection),
                           api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == data.user_id or is_web_key:
        existing_categories = database_functions.functions.get_categories(cnx, database_type, data.podcast_id, data.user_id)
        if data.category in existing_categories:
            return {"detail": "Category already exists."}
        else:
            success = database_functions.functions.add_category(cnx, database_type, data.podcast_id, data.user_id, data.category)
            if success:
                return {"detail": "Category added!"}
            else:
                raise HTTPException(status_code=400, detail="Error adding category.")
    else:
        raise HTTPException(status_code=403, detail="You can only modify categories of your own podcasts!")

class RemoveCategoryData(BaseModel):
    podcast_id: int
    user_id: int
    category: str

@app.post("/api/data/remove_category")
async def api_remove_category(data: RemoveCategoryData, cnx=Depends(get_database_connection),
                              api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if is_valid_key:
        key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
        if key_id == data.user_id:
            database_functions.functions.remove_category(cnx, database_type, data.podcast_id, data.user_id, data.category)
            return {"detail": "Category removed."}
        else:
            raise HTTPException(status_code=403,
                                detail="You can only modify categories of your own podcasts!")
    else:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

class UpdateFeedCutoffDaysData(BaseModel):
    podcast_id: int
    user_id: int
    feed_cutoff_days: int

@app.post("/api/data/update_feed_cutoff_days")
async def api_update_feed_cutoff_days(data: UpdateFeedCutoffDaysData, cnx=Depends(get_database_connection),
                                     api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == data.user_id or is_web_key:
        success = database_functions.functions.update_feed_cutoff_days(cnx, database_type, data.podcast_id, data.user_id, data.feed_cutoff_days)
        if success:
            return {"detail": "Feed cutoff days updated successfully!"}
        else:
            raise HTTPException(status_code=400, detail="Error updating feed cutoff days.")
    else:
        raise HTTPException(status_code=403, detail="You can only modify settings of your own podcasts!")

@app.get("/api/data/get_feed_cutoff_days")
async def api_get_feed_cutoff_days(podcast_id: int, user_id: int, cnx=Depends(get_database_connection),
                                  api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == user_id or is_web_key:
        feed_cutoff_days = database_functions.functions.get_feed_cutoff_days(cnx, database_type, podcast_id, user_id)
        if feed_cutoff_days is not None:
            return {"podcast_id": podcast_id, "user_id": user_id, "feed_cutoff_days": feed_cutoff_days}
        else:
            raise HTTPException(status_code=404, detail="Podcast not found or does not belong to the user.")
    else:
        raise HTTPException(status_code=403, detail="You can only access settings of your own podcasts!")

class TogglePodcastNotificationData(BaseModel):
    user_id: int
    podcast_id: int
    enabled: bool

@app.put("/api/data/podcast/toggle_notifications")
async def api_toggle_podcast_notifications(
    data: TogglePodcastNotificationData,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Invalid API key")

    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id == data.user_id or is_web_key:
        success = database_functions.functions.toggle_podcast_notifications(
            cnx,
            database_type,
            data.podcast_id,
            data.user_id,
            data.enabled
        )
        if success:
            return {"detail": "Podcast notification settings updated successfully"}
        else:
            raise HTTPException(status_code=400, detail="Error updating podcast notification settings")
    else:
        raise HTTPException(status_code=403, detail="You can only modify your own podcast settings")

class SetPodcastFeedCutoff(BaseModel):
    user_id: int
    podcast_id: int
    feed_cutoff: int

@app.put("/api/data/podcast/set_feed_cutoff")
async def api_toggle_podcast_notifications(
    data: SetPodcastFeedCutoff,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Invalid API key")

    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id == data.user_id or is_web_key:
        success = database_functions.functions.set_feed_cutoff(
            cnx,
            database_type,
            data.podcast_id,
            data.user_id,
            data.feed_cutoff
        )
        if success:
            return {"detail": "Podcast feed cutoff setting updated successfully"}
        else:
            raise HTTPException(status_code=400, detail="Error updating podcast nfeed cutoff")
    else:
        raise HTTPException(status_code=403, detail="You can only modify your own podcast settings")

class PodcastNotificationStatusData(BaseModel):
    user_id: int
    podcast_id: int

@app.post("/api/data/podcast/notification_status")
async def api_get_podcast_notification_status(
    data: PodcastNotificationStatusData,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Invalid API key")

    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id == data.user_id or is_web_key:
        enabled = database_functions.functions.get_podcast_notification_status(
            cnx,
            database_type,
            data.podcast_id,
            data.user_id
        )
        return {"enabled": enabled}
    else:
        raise HTTPException(status_code=403, detail="You can only check your own podcast settings")

class NotificationSettingsData(BaseModel):
    user_id: int
    platform: str
    enabled: bool
    ntfy_topic: Optional[str]
    ntfy_server_url: Optional[str]
    gotify_url: Optional[str]
    gotify_token: Optional[str]

@app.get("/api/data/user/notification_settings")
async def api_get_notification_settings(user_id: int, cnx=Depends(get_database_connection),
                                     api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Invalid API key")

    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id == user_id or is_web_key:
        settings = database_functions.functions.get_notification_settings(cnx, database_type, user_id)
        return {"settings": settings}
    else:
        raise HTTPException(status_code=403, detail="You can only access your own notification settings")

@app.put("/api/data/user/notification_settings")
async def api_update_notification_settings(data: NotificationSettingsData, cnx=Depends(get_database_connection),
                                        api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Invalid API key")

    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id == data.user_id or is_web_key:
        success = database_functions.functions.update_notification_settings(
            cnx,
            database_type,
            data.user_id,
            data.platform,
            data.enabled,
            data.ntfy_topic,
            data.ntfy_server_url,
            data.gotify_url,
            data.gotify_token
        )
        if success:
            return {"detail": "Notification settings updated successfully"}
        else:
            raise HTTPException(status_code=400, detail="Error updating notification settings")
    else:
        raise HTTPException(status_code=403, detail="You can only modify your own notification settings")

class NotificationTestRequest(BaseModel):
    user_id: int
    platform: str

@app.post("/api/data/user/test_notification")
async def api_test_notification(
    data: NotificationTestRequest,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Invalid API key")

    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id == data.user_id or is_web_key:
        success = database_functions.functions.send_test_notification(
            cnx,
            database_type,
            data.user_id,
            data.platform
        )
        if success:
            return {"detail": "Test notification sent successfully"}
        else:
            raise HTTPException(status_code=400, detail="Error sending test notification")
    else:
        raise HTTPException(status_code=403, detail="You can only send test notifications to your own account")

class RecordListenDurationData(BaseModel):
    episode_id: int
    user_id: int
    listen_duration: float
    is_youtube: Optional[bool] = False


@app.post("/api/data/record_listen_duration")
async def get(data: RecordListenDurationData, cnx=Depends(get_database_connection),
                                     api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Ignore listen duration for episodes with ID 0
    if data.episode_id == 0:
        return {"detail": "Listen duration for episode ID 0 is ignored."}

    # Continue as normal for all other episode IDs
    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id == data.user_id or is_web_key:
        if data.is_youtube:
            database_functions.functions.record_youtube_listen_duration(cnx, database_type, data.episode_id, data.user_id, data.listen_duration)
        else:
            database_functions.functions.record_listen_duration(cnx, database_type, data.episode_id, data.user_id, data.listen_duration)
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
        database_functions.functions.refresh_pods(cnx, database_type)
    finally:
        close_database_connection(cnx)

# Store locks per user to prevent concurrent refresh jobs
user_locks = {}

# Store active WebSocket connections
active_websockets = {}

@app.websocket("/ws/api/data/episodes/{user_id}")
async def websocket_endpoint(websocket: WebSocket, user_id: int, cnx=Depends(get_database_connection), nextcloud_refresh: bool = Query(False), api_key: str = Query(None)):
    await websocket.accept()
    try:
        print(f"User {user_id} connected to WebSocket")
        # Validate the API key
        is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
        if not is_valid_key:
            await websocket.send_json({"detail": "Invalid API key or insufficient permissions"})
            await websocket.close()
            return
        # Continue as normal for all other episode IDs
        is_web_key = api_key == base_webkey.web_key
        key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
        print(f"User ID: {user_id}, Key ID: {key_id}, Web Key: {is_web_key}")
        if key_id != user_id and not is_web_key:
            await websocket.send_json({"detail": "You can only refresh your own podcasts"})
            await websocket.close()
            return
        if user_id in user_locks:
            await websocket.send_json({"detail": "Refresh job already running for this user."})
            await websocket.close()
            return
        if user_id not in active_websockets:
            active_websockets[user_id] = []
        print(f"Active WebSockets: {active_websockets}")
        active_websockets[user_id].append(websocket)
        # Create a lock for the user and start the refresh task
        user_locks[user_id] = Lock()
        try:
            # Acquire the lock
            user_locks[user_id].acquire()
            print(f"Acquired lock for user {user_id}")
            # Run the refresh process asynchronously without blocking the WebSocket
            task = asyncio.create_task(run_refresh_process(user_id, nextcloud_refresh, websocket, cnx))
            print(f"Task created for user {user_id}")
            # Keep the WebSocket connection alive while the task is running
            while not task.done():
                try:
                    await asyncio.wait_for(websocket.receive_text(), timeout=1.0)
                except asyncio.TimeoutError:
                    # This is expected, we're just using it to keep the connection alive
                    pass
                except Exception as e:
                    print(f"WebSocket disconnected: {str(e)}. Cancelling task.")
                    task.cancel()
                    break
        except Exception as e:
            await websocket.send_json({"detail": f"Error: {str(e)}"})
        finally:
            # Always release the lock and clean up
            user_locks[user_id].release()
            del user_locks[user_id]
            if user_id in active_websockets:
                active_websockets[user_id].remove(websocket)
                if not active_websockets[user_id]:
                    del active_websockets[user_id]
            # For the WebSocket dependency, use the proper function
            close_database_connection(cnx)
            await websocket.close()
    except Exception as e:
        # Handle any unexpected errors
        await websocket.send_json({"detail": f"Unexpected error: {str(e)}"})
        await websocket.close()

async def run_refresh_process(user_id, nextcloud_refresh, websocket, cnx):
    print("Starting refresh process")
    print(f"Running refresh process for user in job {user_id}")
    try:
        # First get total count of podcasts
        cursor = cnx.cursor()
        if database_type == "postgresql":
            cursor.execute('''
                SELECT COUNT(*), array_agg("podcastname")
                FROM "Podcasts"
                WHERE "userid" = %s
            ''', (user_id,))
        else:
            cursor.execute('''
                SELECT COUNT(*), GROUP_CONCAT(PodcastName)
                FROM Podcasts
                WHERE UserID = %s
            ''', (user_id,))
        count_result = cursor.fetchone()
        # Handle both dictionary and tuple results
        if isinstance(count_result, dict):
            total_podcasts = count_result['count'] if count_result else 0
        else:
            total_podcasts = count_result[0] if count_result else 0
        await websocket.send_json({
            "progress": {
                "current": 0,
                "total": total_podcasts,
                "current_podcast": ""
            }
        })

        # Get default device information for sync
        default_device_id = database_functions.functions.get_or_create_default_device(cnx, database_type, user_id)
        default_device_name = None

        if default_device_id:
            # Get the device name
            device_cursor = cnx.cursor()
            if database_type == "postgresql":
                device_query = 'SELECT DeviceName FROM "GpodderDevices" WHERE DeviceID = %s'
            else:
                device_query = "SELECT DeviceName FROM GpodderDevices WHERE DeviceID = %s"

            device_cursor.execute(device_query, (default_device_id,))
            device_result = device_cursor.fetchone()
            device_cursor.close()

            if device_result:
                default_device_name = device_result[0] if isinstance(device_result, tuple) else device_result["devicename"]
                print(f"Using default device for sync: {default_device_name} (ID: {default_device_id})")
            else:
                print("Default device ID found but no name - will use automatic fallback")
        else:
            print("No default device found - will use automatic fallback")

        if nextcloud_refresh:
            await websocket.send_json({"detail": "Refreshing Nextcloud subscriptions..."})
            print(f"Refreshing Nextcloud subscriptions for user {user_id}")
            gpodder_url, gpodder_token, gpodder_login = database_functions.functions.get_nextcloud_settings(database_type, cnx, user_id)
            pod_sync_type = database_functions.functions.get_gpodder_type(cnx, database_type, user_id)
            if pod_sync_type == "nextcloud":
                await asyncio.to_thread(database_functions.functions.refresh_nextcloud_subscription,
                                      database_type, cnx, user_id, gpodder_url, gpodder_token, gpodder_login, pod_sync_type,
                                      default_device_id, default_device_name, False)
            else:
                await asyncio.to_thread(database_functions.functions.refresh_gpodder_subscription,
                                      database_type, cnx, user_id, gpodder_url, gpodder_token, gpodder_login, pod_sync_type,
                                      default_device_id, default_device_name, False)
            await websocket.send_json({"detail": "Pod Sync subscription refresh complete."})
        # Get list of podcast names for progress updates
        print('Getting list')
        if database_type == "postgresql":
            cursor.execute('''
                SELECT "podcastid", "podcastname", "feedurl", "artworkurl", "autodownload",
                       "username", "password", "isyoutubechannel", "feedcutoffdays"
                FROM "Podcasts"
                WHERE "userid" = %s
            ''', (user_id,))
        else:
            cursor.execute('''
                SELECT PodcastID, PodcastName, FeedURL, ArtworkURL, AutoDownload,
                       Username, Password, IsYouTubeChannel, FeedCutoffDays
                FROM Podcasts
                WHERE UserID = %s
            ''', (user_id,))
        podcasts = cursor.fetchall()
        print('got list')

        # Process each podcast
        current = 0
        for podcast in podcasts:
            current += 1
            if isinstance(podcast, dict):
                if database_type == "postgresql":
                    podcast_id = podcast['podcastid']
                    podcast_name = podcast['podcastname']
                    feed_url = podcast['feedurl']
                    artwork_url = podcast['artworkurl']
                    auto_download = podcast['autodownload']
                    username = podcast['username']
                    password = podcast['password']
                    is_youtube = podcast['isyoutubechannel']
                    feed_cutoff = podcast['feedcutoffdays']
                else:
                    podcast_id = podcast['PodcastID']
                    podcast_name = podcast['PodcastName']
                    feed_url = podcast['FeedURL']
                    artwork_url = podcast['ArtworkURL']
                    auto_download = podcast['AutoDownload']
                    username = podcast['Username']
                    password = podcast['Password']
                    is_youtube = podcast['IsYouTubeChannel']
                    feed_cutoff = podcast['FeedCutoffDays']
            else:
                podcast_id, podcast_name, feed_url, artwork_url, auto_download, username, password, is_youtube, feed_cutoff = podcast

            await websocket.send_json({
                "progress": {
                    "current": current,
                    "total": total_podcasts,
                    "current_podcast": podcast_name
                }
            })

            # Refresh this podcast
            # print(f'is it youtube?: {is_youtube}')
            try:
                if is_youtube is True:
                    # Extract channel ID from feed URL
                    channel_id = feed_url.split('channel/')[-1] if 'channel/' in feed_url else feed_url
                    channel_id = channel_id.split('/')[0].split('?')[0]
                    youtube_episodes = await asyncio.to_thread(
                        database_functions.youtube.process_youtube_videos,
                        database_type,
                        podcast_id,
                        channel_id,
                        cnx,
                        feed_cutoff
                    )
                    if youtube_episodes:
                        for episode in youtube_episodes:
                            if user_id in active_websockets:
                                for ws in active_websockets[user_id]:
                                    await ws.send_json({"new_episode": episode})
                else:
                    episodes = await asyncio.to_thread(
                        database_functions.functions.add_episodes,
                        cnx,
                        database_type,
                        podcast_id,
                        feed_url,
                        artwork_url,
                        auto_download,
                        username,
                        password,
                        True # websocket
                    )

                    if episodes:
                        for episode in episodes:
                            if user_id in active_websockets:
                                for ws in active_websockets[user_id]:
                                    await ws.send_json({"new_episode": episode})
            except Exception as e:
                print(f"Error refreshing podcast {podcast_id}: {str(e)}")
                continue

    except Exception as e:
        await websocket.send_json({"detail": f"Error during refresh: {e}"})
    finally:
        # Clear explicit reference
        if cnx:
            try:
                # Get connection type
                connection_type = type(cnx).__name__
                print(f"Closing connection of type: {connection_type}")

                # For PooledMySQLConnection
                if connection_type == "PooledMySQLConnection":
                    print("Detected PooledMySQLConnection - using special handling")
                    # DO NOTHING - don't try to close or modify it
                    # Just let it go out of scope and be garbage collected
                    pass
                # Regular MySQL connection
                elif "MySQL" in connection_type:
                    print("Detected MySQL connection - using basic close")
                    try:
                        cnx.close()
                    except Exception as e:
                        print(f"MySQL close error (ignored): {e}")
                # PostgreSQL connection
                elif hasattr(cnx, 'closed'):
                    print("Detected PostgreSQL connection")
                    if not cnx.closed:
                        cnx.close()
                # Generic
                elif hasattr(cnx, 'close'):
                    print("Using generic close method")
                    cnx.close()

                print("Connection handling complete")
            except Exception as e:
                print(f"Connection handling error: {e}")

            # Force drop reference regardless of what happened above
            cnx = None

        # Force garbage collection
        import gc
        gc.collect()
        print("Garbage collection complete")

@app.get("/api/data/get_stats")
async def api_get_stats(user_id: int, cnx=Depends(get_database_connection),
                        api_key: str = Depends(get_api_key_from_header)):
    logging.info('Fetching API key')
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key
    logging.info('Getting key ID')
    logger.info(f'id {user_id}')
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
    logging.info(f'Got key ID: {key_id}')

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == user_id or is_web_key:
        stats = database_functions.functions.get_stats(cnx, database_type, user_id)
        logging.info('Got stats')
        if stats is None:
            raise HTTPException(status_code=404, detail="Stats not found for the given user ID")
        return stats
    else:
        raise HTTPException(status_code=403, detail="You can only get stats for your own account.")



@app.get("/api/data/get_user_episode_count")
async def api_get_user_episode_count(user_id: int, cnx=Depends(get_database_connection),
                                     api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)

    if not is_valid_key:
        logging.error(f"not valid key")
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    elevated_access = await has_elevated_access(api_key, cnx)

    if not elevated_access:
        # Get user ID from API key
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

        if user_id != user_id_from_api_key:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN,
                                detail="You are not authorized to access these user details")
    episode_count = database_functions.functions.get_user_episode_count(cnx, database_type, user_id)
    if episode_count:
        return episode_count
    else:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="User not found")


@app.get("/api/data/get_user_info")
async def api_get_user_info(is_admin: bool = Depends(check_if_admin), cnx=Depends(get_database_connection)):
    user_info = database_functions.functions.get_user_info(database_type, cnx)
    return user_info

@app.get("/api/data/my_user_info/{user_id}")
async def api_get_my_user_info(
    user_id: int,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    try:
        is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
        if not is_valid_key:
            raise HTTPException(
                status_code=403,
                detail="Your API key is either invalid or does not have correct permission"
            )

        # Check if the API key belongs to the requested user_id
        key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
        is_web_key = api_key == base_webkey.web_key

        if key_id != user_id and not is_web_key:
            raise HTTPException(
                status_code=403,
                detail="You can only retrieve your own user information!"
            )

        user_info = database_functions.functions.get_my_user_info(database_type, cnx, user_id)
        if not user_info:
            raise HTTPException(status_code=404, detail="User not found")

        return user_info

    except HTTPException:
        raise
    except Exception as e:
        logging.error(f"Error in api_get_my_user_info: {str(e)}")
        raise HTTPException(status_code=500, detail="An error occurred while retrieving user information")

@app.get("/api/data/check_podcast", response_model=Dict[str, bool])
async def api_check_podcast(
    user_id: int,
    podcast_name: str,
    podcast_url: str,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if is_valid_key:
        exists = database_functions.functions.check_podcast(cnx, database_type, user_id, podcast_name, podcast_url)
        return {"exists": exists}
    else:
        raise HTTPException(status_code=403, detail="Your API key is either invalid or does not have correct permission")

@app.get("/api/data/check_youtube_channel", response_model=Dict[str, bool])
async def api_check_youtube_channel(
    user_id: int,
    channel_name: str,
    channel_url: str,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if is_valid_key:
        exists = database_functions.functions.check_youtube_channel(
            cnx, database_type, user_id, channel_name, channel_url
        )
        return {"exists": exists}
    else:
        raise HTTPException(
            status_code=403,
            detail="Your API key is either invalid or does not have correct permission"
        )


@app.get("/api/data/user_admin_check/{user_id}")
async def api_user_admin_check_route(user_id: int, api_key: str = Depends(get_api_key_from_header),
                                     cnx=Depends(get_database_connection)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")
    elevated_access = await has_elevated_access(api_key, cnx)
    if not elevated_access:
        # Get user ID from API key
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

        if user_id != user_id_from_api_key:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN,
                                detail="You are not authorized to check admin status for other users")
    is_admin = await run_in_threadpool(database_functions.functions.user_admin_check, cnx, database_type, user_id)
    return {"is_admin": is_admin}

class RemoveYouTubeChannelData(BaseModel):
    user_id: int
    channel_name: str
    channel_url: str

@app.post("/api/data/remove_youtube_channel")
async def api_remove_youtube_channel_route(
    data: RemoveYouTubeChannelData = Body(...),
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(
            status_code=403,
            detail="Your API key is either invalid or does not have correct permission"
        )

    elevated_access = await has_elevated_access(api_key, cnx)
    if not elevated_access:
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
        if data.user_id != user_id_from_api_key:
            raise HTTPException(
                status_code=status.HTTP_403_FORBIDDEN,
                detail="You are not authorized to remove channels for other users"
            )

    database_functions.functions.remove_youtube_channel_by_url(
        cnx, database_type, data.channel_name, data.channel_url, data.user_id
    )
    return {"success": True}

class RemovePodcastData(BaseModel):
    user_id: int
    podcast_name: str
    podcast_url: str


@app.post("/api/data/remove_podcast")
async def api_remove_podcast_route(data: RemovePodcastData = Body(...), cnx=Depends(get_database_connection),
                                   api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")
    elevated_access = await has_elevated_access(api_key, cnx)
    if not elevated_access:
        # Get user ID from API key
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
        if data.user_id != user_id_from_api_key:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN,
                                detail="You are not authorized to remove podcasts for other users")

    # First, get the podcast ID and check if it's a YouTube channel
    podcast_id = database_functions.functions.get_podcast_id(database_type, cnx, data.user_id, data.podcast_url, data.podcast_name)

    if podcast_id is None:
        raise HTTPException(status_code=404, detail="Podcast not found")

    # Check if this is a YouTube channel
    is_youtube = database_functions.functions.check_youtube_channel_id(cnx, database_type, podcast_id)

    # Track if episodes have been handled
    episodes_handled = False

    if database_functions.functions.check_gpodder_settings(database_type, cnx, data.user_id):
        logging.info('get cloud vals')
        gpodder_url, gpodder_token, gpodder_login = database_functions.functions.get_nextcloud_settings(database_type, cnx, data.user_id)

        # Get the full gpodder settings to check URL
        gpodder_settings = database_functions.functions.get_gpodder_settings(database_type, cnx, data.user_id)

        logging.info('em cloud')
        podcast_feed = database_functions.functions.get_podcast_feed_by_id(cnx, database_type, podcast_id)
        gpod_type = database_functions.functions.get_gpodder_type(cnx, database_type, data.user_id)

        # Get the correct device name, matching what we do in add_podcast
        device_name = f"pinepods-internal-{data.user_id}"  # Default device name
        if gpod_type == "gpodder":
            default_device = database_functions.functions.get_default_gpodder_device(cnx, database_type, data.user_id)
            if default_device:
                device_name = default_device["name"]

        if gpod_type == "nextcloud":
            database_functions.functions.remove_podcast_from_nextcloud(cnx, database_type, gpodder_url, gpodder_login, gpodder_token, podcast_feed)
        else:
            # Modified return value includes whether episodes were handled
            success, episodes_handled = database_functions.functions.remove_podcast_from_opodsync(
                cnx, database_type, data.user_id, gpodder_url, gpodder_login,
                gpodder_token, podcast_feed, device_name
            )

    # Only run the appropriate remove function if episodes weren't already handled by gpodder sync
    if not episodes_handled:
        if is_youtube:
            database_functions.functions.remove_youtube_channel(cnx, database_type, podcast_id, data.user_id)
        else:
            database_functions.functions.remove_podcast(cnx, database_type, data.podcast_name, data.podcast_url, data.user_id)
    else:
        logging.info('skipping remove - already handled by gpodder sync')

    return {"success": True}

class RemovePodcastIDData(BaseModel):
    user_id: int
    podcast_id: int
    is_youtube: bool = False

@app.post("/api/data/remove_podcast_id")
async def api_remove_podcast_route_id(data: RemovePodcastIDData = Body(...),
                                   cnx=Depends(get_database_connection),
                                   api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")
    elevated_access = await has_elevated_access(api_key, cnx)
    if not elevated_access:
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
        if data.user_id != user_id_from_api_key:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN,
                                detail="You are not authorized to remove content for other users")
    if data.is_youtube:
        database_functions.functions.remove_youtube_channel(cnx, database_type, data.podcast_id, data.user_id)
    else:
        # Existing podcast removal logic
        logging.info('check gpod')
        episodes_handled = False  # Track whether episodes were already handled by gpodder sync

        if database_functions.functions.check_gpodder_settings(database_type, cnx, data.user_id):
            logging.info('get cloud vals')
            gpodder_url, gpodder_token, gpodder_login = database_functions.functions.get_nextcloud_settings(database_type, cnx, data.user_id)

            # Get the full gpodder settings to check URL
            gpodder_settings = database_functions.functions.get_gpodder_settings(database_type, cnx, data.user_id)

            logging.info('em cloud')
            podcast_feed = database_functions.functions.get_podcast_feed_by_id(cnx, database_type, data.podcast_id)
            gpod_type = database_functions.functions.get_gpodder_type(cnx, database_type, data.user_id)

            # Get the correct device name, matching what we do in add_podcast
            device_name = f"pinepods-internal-{data.user_id}"  # Default device name
            if gpod_type == "gpodder":
                default_device = database_functions.functions.get_default_gpodder_device(cnx, database_type, data.user_id)
                if default_device:
                    device_name = default_device["name"]

            if gpod_type == "nextcloud":
                database_functions.functions.remove_podcast_from_nextcloud(cnx, database_type, gpodder_url, gpodder_login, gpodder_token, podcast_feed)
            else:
                # Modified return value includes whether episodes were handled
                success, episodes_handled = database_functions.functions.remove_podcast_from_opodsync(
                    cnx, database_type, data.user_id, gpodder_url, gpodder_login,
                    gpodder_token, podcast_feed, device_name
                )

        # Only run remove_podcast_id if episodes weren't already handled by gpodder sync
        if not episodes_handled:
            logging.info('rm pod id')
            database_functions.functions.remove_podcast_id(cnx, database_type, data.podcast_id, data.user_id)
        else:
            logging.info('skipping rm pod id - already handled by gpodder sync')

    return {"success": True}


@app.get("/api/data/return_pods/{user_id}")
async def api_return_pods(user_id: int, cnx=Depends(get_database_connection),
                         api_key: str = Depends(get_api_key_from_header)):
    try:
        is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
        if not is_valid_key:
            raise HTTPException(status_code=403,
                              detail="Your API key is either invalid or does not have correct permission")

        is_web_key = api_key == base_webkey.web_key
        key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

        if key_id == user_id or is_web_key:
            pods = database_functions.functions.return_pods(database_type, cnx, user_id)

            # Return empty list if no podcasts found
            if not pods:
                return {"pods": []}

            # Filter out any None values that might have slipped through
            cleaned_pods = []
            for pod in pods:
                if pod and isinstance(pod, dict):
                    cleaned_pod = {
                        k: v if v is not None else ""
                        for k, v in pod.items()
                    }
                    cleaned_pods.append(cleaned_pod)

            return {"pods": cleaned_pods}
        else:
            raise HTTPException(status_code=403,
                              detail="You can only return pods for yourself!")

    except Exception as e:
        logging.error(f"Error in api_return_pods: {str(e)}")
        return {"pods": [], "error": "An error occurred while retrieving podcasts"}

@app.get("/api/data/user_history/{user_id}")
async def api_user_history(user_id: int, cnx=Depends(get_database_connection),
                           api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == user_id or is_web_key:
        history = database_functions.functions.user_history(cnx, database_type, user_id)
        return {"data": history}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only return history for yourself!")



@app.get("/api/data/saved_episode_list/{user_id}")
async def api_saved_episode_list(user_id: int, cnx=Depends(get_database_connection),
                                 api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

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
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == user_id or is_web_key:
        downloaded_episodes = database_functions.functions.download_episode_list(database_type, cnx, user_id)
        return {"downloaded_episodes": downloaded_episodes}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only return downloaded episodes for yourself!")


class UserValues(BaseModel):
    fullname: str
    username: str
    email: str
    hash_pw: str

@app.post("/api/data/add_user")
async def api_add_user(is_admin: bool = Depends(check_if_admin),
                      cnx=Depends(get_database_connection),
                      api_key: str = Depends(get_api_key_from_header),
                      user_values: UserValues = Body(...)):
    try:
        user_id = database_functions.functions.add_user(cnx, database_type, (
            user_values.fullname, user_values.username.lower(), user_values.email, user_values.hash_pw))

        if not user_id:
            raise HTTPException(
                status_code=500,
                detail="Failed to create user - no user ID returned"
            )

        return {"detail": "Success", "user_id": user_id}

    except psycopg.errors.UniqueViolation as e:
        error_detail = str(e)
        if "Users_username_key" in error_detail:
            raise HTTPException(
                status_code=409,
                detail="This username is already taken. Please choose a different username."
            )
        elif "Users_email_key" in error_detail:
            raise HTTPException(
                status_code=409,
                detail="This email address is already registered. Please use a different email."
            )
        else:
            raise HTTPException(
                status_code=409,
                detail="A conflict occurred while creating the user. Please try again with different credentials."
            )

    except psycopg.errors.OperationalError as e:
        logging.error(f"Database operational error: {str(e)}")
        raise HTTPException(
            status_code=503,
            detail="Unable to connect to the database. Please try again later."
        )

    except mysql.connector.errors.IntegrityError as e:
        error_msg = str(e)
        if "Duplicate entry" in error_msg and "username" in error_msg.lower():
            raise HTTPException(
                status_code=409,
                detail="This username is already taken. Please choose a different username."
            )
        elif "Duplicate entry" in error_msg and "email" in error_msg.lower():
            raise HTTPException(
                status_code=409,
                detail="This email address is already registered. Please use a different email."
            )
        else:
            raise HTTPException(
                status_code=409,
                detail="A conflict occurred while creating the user. Please try again with different credentials."
            )

    except Exception as e:
        logging.error(f"Unexpected error adding user: {str(e)}")
        raise HTTPException(
            status_code=500,
            detail=f"An unexpected error occurred while creating the user: {str(e)}"
        )


@app.post("/api/data/add_login_user")
async def api_add_user(cnx=Depends(get_database_connection),
                      user_values: UserValues = Body(...)):
    try:
        self_service = database_functions.functions.check_self_service(cnx, database_type)
        if not self_service:
            raise HTTPException(
                status_code=403,
                detail="Your API key is either invalid or does not have correct permission"
            )

        user_id = database_functions.functions.add_user(cnx, database_type, (
            user_values.fullname, user_values.username.lower(), user_values.email, user_values.hash_pw))

        if not user_id:
            raise HTTPException(
                status_code=500,
                detail="Failed to create user account - no user ID returned"
            )

        return {"detail": "User added successfully", "user_id": user_id}

    except UniqueViolation as e:
        error_detail = str(e)
        if "Users_username_key" in error_detail:
            raise HTTPException(
                status_code=409,
                detail="This username is already taken. Please choose a different username."
            )
        elif "Users_email_key" in error_detail:
            raise HTTPException(
                status_code=409,
                detail="This email address is already registered. Please use a different email."
            )
        else:
            raise HTTPException(
                status_code=409,
                detail="A conflict occurred while creating the user. Please try again with different credentials."
            )

    except OperationalError as e:
        logging.error(f"Database operational error: {str(e)}")
        raise HTTPException(
            status_code=503,
            detail="Unable to connect to the database. Please try again later."
        )

    except Exception as e:
        logging.error(f"Unexpected error adding user: {str(e)}")
        raise HTTPException(
            status_code=500,
            detail=f"An unexpected error occurred while creating your account: {str(e)}"
        )

@app.put("/api/data/set_fullname/{user_id}")
async def api_set_fullname(user_id: int, new_name: str = Query(...), cnx=Depends(get_database_connection),
                           api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)

    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    elevated_access = await has_elevated_access(api_key, cnx)

    if not elevated_access:
        # Get user ID from API key
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

        if user_id != user_id_from_api_key:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN,
                                detail="You are not authorized to access these user details")
    try:
        database_functions.functions.set_fullname(cnx, database_type, user_id, new_name)
        return {"detail": "Fullname updated."}
    except:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="User not found")


class PasswordUpdateRequest(BaseModel):
    hash_pw: str

@app.put("/api/data/set_password/{user_id}")
async def api_set_password(
    user_id: int,
    request: PasswordUpdateRequest,  # Use the Pydantic model
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    hash_pw = request.hash_pw  # Extract the hash_pw from the request model

    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)

    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Your API key is either invalid or does not have correct permission")

    elevated_access = await has_elevated_access(api_key, cnx)

    if not elevated_access:
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, api_key)

        if user_id != user_id_from_api_key:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN, detail="You are not authorized to access these user details")

    try:
        database_functions.functions.set_password(cnx, database_type, user_id, hash_pw)
        return {"detail": "Password updated."}
    except Exception as e:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail=f"User not found. Error: {str(e)}")

@app.put("/api/data/user/set_email")
async def api_set_email(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header),
                        user_id: int = Body(...), new_email: str = Body(...)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)

    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    elevated_access = await has_elevated_access(api_key, cnx)

    if not elevated_access:
        # Get user ID from API key
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

        if user_id != user_id_from_api_key:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN,
                                detail="You are not authorized to access these user details")
    try:
        database_functions.functions.set_email(cnx, database_type, user_id, new_email)
        return {"detail": "Email updated."}
    except:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="User not found")


@app.put("/api/data/user/set_username")
async def api_set_username(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header),
                           user_id: int = Body(...), new_username: str = Body(...)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)

    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    elevated_access = await has_elevated_access(api_key, cnx)

    if not elevated_access:
        # Get user ID from API key
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

        if user_id != user_id_from_api_key:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN,
                                detail="You are not authorized to access these user details")
    try:
        database_functions.functions.set_username(cnx, database_type, user_id, new_username.lower())
        return {"detail": "Username updated."}
    except:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="User not found")


@app.put("/api/data/user/set_isadmin")
async def api_set_isadmin(is_admin: bool = Depends(check_if_admin), cnx=Depends(get_database_connection),
                          user_id: int = Body(...), isadmin: bool = Body(...)):
    database_functions.functions.set_isadmin(cnx, database_type, user_id, isadmin)
    return {"detail": "IsAdmin status updated."}


@app.get("/api/data/user/final_admin/{user_id}")
async def api_final_admin(is_admin: bool = Depends(check_if_admin), cnx=Depends(get_database_connection),
                          user_id: int = Path(...)):
    is_final_admin = database_functions.functions.final_admin(cnx, database_type, user_id)
    return {"final_admin": is_final_admin}


@app.delete("/api/data/user/delete/{user_id}")
async def api_delete_user(is_admin: bool = Depends(check_if_admin), cnx=Depends(get_database_connection),
                          user_id: int = Path(...)):
    database_functions.functions.delete_user(cnx, database_type, user_id)
    return {"status": "User deleted"}


class OIDCProviderValues(BaseModel):
    provider_name: str
    client_id: str
    client_secret: str
    authorization_url: str
    token_url: str
    user_info_url: str
    button_text: str
    scope: Optional[str] = "openid email profile"
    button_color: Optional[str] = "#000000"
    button_text_color: Optional[str] = "#000000"
    icon_svg: Optional[str] = None

@app.post("/api/data/add_oidc_provider")
async def api_add_oidc_provider(
    is_admin: bool = Depends(check_if_admin),
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header),
    provider_values: OIDCProviderValues = Body(...)):
    try:
        provider_id = database_functions.functions.add_oidc_provider(cnx, database_type, (
            provider_values.provider_name,
            provider_values.client_id,
            provider_values.client_secret,
            provider_values.authorization_url,
            provider_values.token_url,
            provider_values.user_info_url,
            provider_values.button_text,
            provider_values.scope,
            provider_values.button_color,
            provider_values.button_text_color,
            provider_values.icon_svg
        ))
        if not provider_id:
            raise HTTPException(
                status_code=500,
                detail="Failed to create provider - no provider ID returned"
            )
        return {"detail": "Success", "provider_id": provider_id}
    except psycopg.errors.UniqueViolation:
        raise HTTPException(
            status_code=409,
            detail="A provider with this name already exists"
        )
    except Exception as e:
        logging.error(f"Unexpected error adding provider: {str(e)}")
        raise HTTPException(
            status_code=500,
            detail=f"An unexpected error occurred while creating the provider: {str(e)}"
        )

@app.post("/api/data/remove_oidc_provider")
async def api_remove_oidc_provider(
    is_admin: bool = Depends(check_if_admin),
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header),
    provider_id: int = Body(...)):
    try:
        result = database_functions.functions.remove_oidc_provider(cnx, database_type, provider_id)
        if not result:
            raise HTTPException(
                status_code=404,
                detail="Provider not found"
            )
        return {"detail": "Success"}
    except Exception as e:
        logging.error(f"Unexpected error removing provider: {str(e)}")
        raise HTTPException(
            status_code=500,
            detail=f"An unexpected error occurred while removing the provider: {str(e)}"
        )

@app.get("/api/data/list_oidc_providers")
async def api_list_oidc_providers(
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)):
    try:
        providers = database_functions.functions.list_oidc_providers(cnx, database_type)
        return {"providers": providers}
    except Exception as e:
        logging.error(f"Unexpected error listing providers: {str(e)}")
        raise HTTPException(
            status_code=500,
            detail=f"An unexpected error occurred while listing providers: {str(e)}"
        )

# Public reqeust for login info
@app.get("/api/data/public_oidc_providers")
async def api_public_oidc_providers(cnx=Depends(get_database_connection)):
    """Get minimal OIDC provider info needed for login screen buttons."""
    try:
        providers = database_functions.functions.get_public_oidc_providers(cnx, database_type)
        return {"providers": providers}
    except Exception as e:
        logging.error(f"Unexpected error getting public provider info: {str(e)}")
        raise HTTPException(
            status_code=500,
            detail=f"An unexpected error occurred: {str(e)}"
        )


@app.put("/api/data/user/set_theme")
async def api_set_theme(user_id: int = Body(...), new_theme: str = Body(...), cnx=Depends(get_database_connection),
                        api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == user_id or is_web_key:
        database_functions.functions.set_theme(cnx, database_type, user_id, new_theme)
        return {"message": "Theme updated successfully"}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only set your own theme!")

@app.post("/api/data/create_api_key")
async def api_create_api_key(
        user_id: int = Body(..., embed=True),
        rssonly: bool = Body(..., embed=True),
        podcast_ids: Optional[List[int]] = Body(None, embed=True),
        cnx=Depends(get_database_connection),
        api_key: str = Depends(get_api_key_from_header)):
    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
    if user_id == key_id or is_web_key:
        if rssonly:
            new_key = database_functions.functions.create_rss_key(cnx, database_type, user_id, podcast_ids)
        else:
            new_key = database_functions.functions.create_api_key(cnx, database_type, user_id)
        return {"rss_key" if rssonly else "api_key": new_key}
    else:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

@app.post("/api/data/set_rss_key_podcasts")
async def api_set_rss_key_podcasts(
        user_id: int = Body(..., embed=True),
        rss_key_id: int = Body(..., embed=True),
        podcast_ids: Optional[List[int]] = Body(None, embed=True),
        cnx=Depends(get_database_connection),
        api_key: str = Depends(get_api_key_from_header)):
    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
    if user_id == key_id or is_web_key:
        database_functions.functions.set_rss_key_podcasts(cnx, database_type, rss_key_id, podcast_ids)
        return {"message": "Podcast IDs updated successfully"}
    else:
        raise HTTPException(status_code=403, detail="Your API key is either invalid or does not have correct permission")

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
        print(traceback.format_exc())  # Print full exception information
        raise HTTPException(status_code=500, detail=f"Failed to send email: {str(e)}")

class SendEmailValues(BaseModel):
    to_email: str
    subject : str
    message: str  # Add this line

def send_email_with_settings(email_values, database_type, payload: SendEmailValues):

    try:
        msg = MIMEMultipart()
        msg['From'] = email_values['FromEmail']
        msg['To'] = payload.to_email
        msg['Subject'] = payload.subject
        msg.attach(MIMEText(payload.message, 'plain'))

        try:
            port = int(email_values['ServerPort'])
            if email_values['Encryption'] == "SSL/TLS":
                server = smtplib.SMTP_SSL(email_values['ServerName'], port)
            elif email_values['Encryption'] == "StartTLS":
                server = smtplib.SMTP(email_values['ServerName'], port)
                server.starttls()
            else:
                server = smtplib.SMTP(email_values['ServerName'], port)

            if email_values['AuthRequired']:
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
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Invalid API key")

    email_values = database_functions.functions.get_email_settings(cnx, database_type)
    if not email_values:
        raise HTTPException(status_code=404, detail="Email settings not found")

    try:
        send_status = await run_in_threadpool(send_email_with_settings, email_values, database_type, payload)
        return {"email_status": send_status}
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Failed to send email: {str(e)}")


@app.post("/api/data/save_email_settings")
async def api_save_email_settings(email_settings: dict = Body(..., embed=True),
                                  is_admin: bool = Depends(check_if_admin), cnx=Depends(get_database_connection)):
    database_functions.functions.save_email_settings(cnx, database_type, email_settings)
    return {"message": "Email settings saved."}


@app.get("/api/data/get_encryption_key")
async def api_get_encryption_key(is_admin: bool = Depends(check_if_admin), cnx=Depends(get_database_connection)):
    encryption_key = database_functions.functions.get_encryption_key(cnx, database_type)
    return {"encryption_key": encryption_key}


@app.get("/api/data/get_email_settings")
async def api_get_email_settings(is_admin: bool = Depends(check_if_admin), cnx=Depends(get_database_connection)):
    email_settings = database_functions.functions.get_email_settings(cnx, database_type)
    return email_settings


class DeleteAPIKeyHeaders(BaseModel):
    api_id: str
    user_id: str


@app.delete("/api/data/delete_api_key")
async def api_delete_api_key(payload: DeleteAPIKeyHeaders, cnx=Depends(get_database_connection),
                             api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)

    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    elevated_access = await has_elevated_access(api_key, cnx)

    if not elevated_access:
        # Get user ID from API key
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

        if payload.user_id != user_id_from_api_key:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN,
                                detail="You are not authorized to access or remove other users api-keys.")
    # Check if the API key to be deleted is the same as the one used in the current request
    if database_functions.functions.is_same_api_key(cnx, database_type, payload.api_id, api_key):
        raise HTTPException(status_code=403,
                            detail="You cannot delete the API key that is currently in use.")
    # Check if the API key belongs to the guest user (user_id 1)
    if database_functions.functions.belongs_to_guest_user(cnx, database_type, payload.api_id):
        raise HTTPException(status_code=403,
                            detail="Cannot delete guest user api.")

    # Proceed with deletion if the checks pass
    database_functions.functions.delete_api(cnx, database_type, payload.api_id)
    return {"detail": "API key deleted."}


@app.get("/api/data/get_api_info/{user_id}")
async def api_get_api_info(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header),
                           user_id: int = Path(...)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)

    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")
    elevated_access = await has_elevated_access(api_key, cnx)

    if not elevated_access:
        # Get user ID from API key
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

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
    email_setup = database_functions.functions.get_email_settings(cnx, database_type)
    if email_setup['Server_Name'] == "default_server":
        raise HTTPException(status_code=403,
                            detail="Email settings not configured. Please contact your administrator.")
    else:
        check_user = database_functions.functions.check_reset_user(cnx, database_type, payload.username.lower(), payload.email)
        if check_user:
            create_code = database_functions.functions.reset_password_create_code(cnx, database_type, payload.email)

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
                database_functions.functions.reset_password_remove_code(cnx, database_type, payload.email)
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
    code_valid = database_functions.functions.verify_reset_code(cnx, database_type, payload.email, payload.reset_code)
    if code_valid is None:
        raise HTTPException(status_code=404, detail="User not found")
    elif not code_valid:
        raise HTTPException(status_code=400, detail="Code is invalid")
        # return {"code_valid": False}

    message = database_functions.functions.reset_password_prompt(cnx, database_type, payload.email, payload.new_password)
    if message is None:
        raise HTTPException(status_code=500, detail="Failed to reset password")
    return {"message": message}

class EpisodeMetadata(BaseModel):
    episode_id: int
    user_id: int
    person_episode: bool = False  # Default to False if not specified
    is_youtube: bool = False

@app.post("/api/data/get_episode_metadata")
async def api_get_episode_metadata(data: EpisodeMetadata, cnx=Depends(get_database_connection),
                                 api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                          detail="Your API key is either invalid or does not have correct permission")

    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id == data.user_id or is_web_key:
        episode = database_functions.functions.get_episode_metadata(
            database_type,
            cnx,
            data.episode_id,
            data.user_id,
            data.person_episode,
            data.is_youtube
        )
        return {"episode": episode}
    else:
        raise HTTPException(status_code=403,
                          detail="You can only get metadata for yourself!")

class GetPlaybackSpeed(BaseModel):
    podcast_id: Optional[int] = None
    user_id: int

@app.post("/api/data/get_playback_speed")
async def api_get_playback_speed(data: GetPlaybackSpeed, cnx=Depends(get_database_connection),
                                 api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                          detail="Your API key is either invalid or does not have correct permission")
    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
    if key_id == data.user_id or is_web_key:
        # Fix the parameter order to match the function definition
        is_youtube = False  # Add the is_youtube parameter
        playback_speed = database_functions.functions.get_playback_speed(
            cnx,                # Connection should be first
            database_type,      # Then database type
            data.user_id,       # Then user_id
            is_youtube,         # Then is_youtube parameter
            data.podcast_id     # Then optional podcast_id
        )
        return {"playback_speed": playback_speed}
    else:
        raise HTTPException(status_code=403,
                          detail="You can only get metadata for yourself!")

@app.get("/api/data/generate_mfa_secret/{user_id}")
async def generate_mfa_secret(user_id: int, cnx=Depends(get_database_connection),
                              api_key: str = Depends(get_api_key_from_header)):
    # Perform API key validation and user authorization checks as before
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        logging.warning(f"Invalid API key: {api_key}")
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key
    logging.info(f"Is web key: {is_web_key}")

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
    logging.info(f"Key ID from API key: {key_id}")

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == user_id or is_web_key:
        user_details = database_functions.functions.get_user_details_id(cnx, database_type, user_id)
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
    logging.info(f"Verifying MFA code for user_id: {body.user_id} with code: {body.mfa_code}")

    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        logging.warning(f"Invalid API key: {api_key}")
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key
    logging.info(f"Is web key: {is_web_key}")

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
    logging.info(f"Key ID from API key: {key_id}")

    if key_id == body.user_id or is_web_key:
        secret = temp_mfa_secrets.get(body.user_id)
        if secret is None:
            raise HTTPException(status_code=status.HTTP_404_NOT_FOUND,
                                detail="MFA setup not initiated or expired.")
        if secret:
            logging.info(f"Retrieved secret for user_id")
        else:
            logging.warning(f"No secret found for user_id")
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
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        logging.warning(f"Invalid API key: {api_key}")
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key
    logging.info(f"Is web key: {is_web_key}")

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
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
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)

    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    elevated_access = await has_elevated_access(api_key, cnx)

    if not elevated_access:
        # Get user ID from API key
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

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
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

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
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)

    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    elevated_access = await has_elevated_access(api_key, cnx)

    if not elevated_access:
        # Get user ID from API key
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

        if body.user_id != user_id_from_api_key:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN,
                                detail="You are not authorized to access these user details")

    result = database_functions.functions.delete_mfa_secret(database_type, cnx, body.user_id)
    if result:
        return {"deleted": result}
    else:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="User not found")

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
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)

    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    elevated_access = await has_elevated_access(api_key, cnx)

    if not elevated_access:
        # Get user ID from API key
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

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
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)

    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    elevated_access = await has_elevated_access(api_key, cnx)

    if not elevated_access:
        # Get user ID from API key
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

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
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == user_id or is_web_key:
        first_login_status = database_functions.functions.first_login_done(database_type, cnx, user_id)
        return {"FirstLogin": first_login_status}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only run first login for yourself!")

class SelectedEpisodesDelete(BaseModel):
    selected_episodes: List[int] = Field(..., title="List of Episode IDs")
    user_id: int = Field(..., title="User ID")


@app.post("/api/data/delete_selected_episodes")
async def delete_selected_episodes(data: SelectedEpisodesDelete, cnx=Depends(get_database_connection),
                                   api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == data.user_id or is_web_key:
        if is_valid_key:
            delete_status = database_functions.functions.delete_selected_episodes(cnx, database_type, data.selected_episodes,
                                                                                  data.user_id)
            return {"status": delete_status}
        else:
            raise HTTPException(status_code=403,
                                detail="Your API key is either invalid or does not have correct permission")
    else:
        raise HTTPException(status_code=403,
                            detail="You can only delete your own selected episodes!")

class SearchPodcastData(BaseModel):
    search_term: str
    user_id: int


@app.post("/api/data/search_data")
async def search_data(data: SearchPodcastData, cnx=Depends(get_database_connection),
                      api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if is_valid_key:
        result = database_functions.functions.search_data(database_type, cnx, data.search_term, data.user_id)
        return {"data": result}
    else:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")


class QueuePodData(BaseModel):
    episode_id: int
    user_id: int
    is_youtube: bool = False

@app.post("/api/data/queue_pod")
async def queue_pod(data: QueuePodData, cnx=Depends(get_database_connection),
                   api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                          detail="Your API key is either invalid or does not have correct permission")

    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id == data.user_id or is_web_key:
        ep_status = database_functions.functions.check_queued(
            database_type, cnx, data.episode_id, data.user_id, data.is_youtube
        )
        if ep_status:
            return {"data": f"{'Video' if data.is_youtube else 'Episode'} already in queue"}
        else:
            result = database_functions.functions.queue_pod(
                database_type, cnx, data.episode_id, data.user_id, data.is_youtube
            )
            return {"data": result}
    else:
        raise HTTPException(status_code=403,
                          detail=f"You can only add {'videos' if data.is_youtube else 'episodes'} to your own queue!")

class QueueRmData(BaseModel):
    episode_id: int
    user_id: int
    is_youtube: bool = False

@app.post("/api/data/remove_queued_pod")
async def remove_queued_pod(data: QueueRmData, cnx=Depends(get_database_connection),
                          api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                          detail="Your API key is either invalid or does not have correct permission")

    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id == data.user_id or is_web_key:
        result = database_functions.functions.remove_queued_pod(
            database_type, cnx, data.episode_id, data.user_id, data.is_youtube
        )
        return result
    else:
        raise HTTPException(status_code=403,
                          detail=f"You can only remove {'videos' if data.is_youtube else 'episodes'} for your own queue!")
# class QueuedEpisodesData(BaseModel):
#     user_id: int


@app.get("/api/data/get_queued_episodes")
async def get_queued_episodes(user_id: int = Query(...), cnx=Depends(get_database_connection),
                              api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == user_id or is_web_key:
        result = database_functions.functions.get_queued_episodes(database_type, cnx, user_id)
        return {"data": result}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only get episodes from your own queue!")

class ReorderRequest(BaseModel):
    episode_ids: List[int]

@app.post("/api/data/reorder_queue")
async def reorder_queue(request: ReorderRequest, user_id: int = Query(...), cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == user_id or is_web_key:
        success = database_functions.functions.reorder_queued_episodes(database_type, cnx, user_id, request.episode_ids)
        if success:
            return {"message": "Queue reordered successfully"}
        else:
            raise HTTPException(status_code=500, detail="Failed to reorder the queue")
    else:
        raise HTTPException(status_code=403, detail="You can only reorder your own queue!")

@app.get("/api/data/check_episode_in_db/{user_id}")
async def check_episode_in_db(user_id: int, episode_title: str = Query(...), episode_url: str = Query(...), cnx=Depends(get_database_connection),
                              api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Your API key is either invalid or does not have correct permission")

    if database_functions.functions.id_from_api_key(cnx, database_type, api_key) != user_id:
        raise HTTPException(status_code=403, detail="You can only check episodes for your own account")

    episode_exists = database_functions.functions.check_episode_exists(cnx, database_type, user_id, episode_title, episode_url)
    return {"episode_in_db": episode_exists}

@app.get("/api/data/get_pinepods_version")
async def get_pinepods_version(cnx=Depends(get_database_connection),
                               api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    result = database_functions.functions.get_pinepods_version()
    return {"data": result}

@app.post("/api/data/share_episode/{episode_id}")
async def share_episode(episode_id: int, cnx=Depends(get_database_connection),
                        api_key: str = Depends(get_api_key_from_header)):
    import uuid
    from datetime import datetime, timedelta
    # Verify API key validity
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Your API key is either invalid or does not have the correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    # Generate the URL key and expiration date
    url_key = str(uuid.uuid4())  # Generates a unique URL key
    expiration_date = datetime.utcnow() + timedelta(days=60)  # Expire in 60 days

    # Call database function to insert the shared episode entry
    result = database_functions.functions.add_shared_episode(database_type, cnx, episode_id, url_key, expiration_date)

    if result:
        return {"url_key": url_key}
    else:
        raise HTTPException(status_code=500, detail="Failed to share episode")


@app.get("/api/data/cleanup_tasks")
async def api_cleanup_tasks(
    background_tasks: BackgroundTasks,
    is_admin: bool = Depends(check_if_admin)
) -> Dict[str, str]:
    """
    Endpoint to trigger cleanup of old PeopleEpisodes and expired SharedEpisodes
    """
    background_tasks.add_task(cleanup_tasks)
    return {"detail": "Cleanup tasks initiated."}

def cleanup_tasks():
    """
    Background task to run database cleanup operations
    """
    cnx = create_database_connection()
    try:
        database_functions.functions.cleanup_old_episodes(cnx, database_type)
    except Exception as e:
        print(f"Error during cleanup tasks: {str(e)}")
    finally:
        close_database_connection(cnx)

@app.get("/api/data/update_playlists")
async def api_update_playlists(
    background_tasks: BackgroundTasks,
    is_admin: bool = Depends(check_if_admin)
) -> Dict[str, str]:
    """
    Endpoint to trigger playlist updates for all playlists (system and user-defined)
    """
    background_tasks.add_task(update_playlists_task)
    return {"detail": "Playlist update initiated."}

def update_playlists_task():
    """
    Background task to update all playlists
    """
    print("Starting background playlist update task")
    try:
        cnx = create_database_connection()
        try:
            database_functions.functions.update_all_playlists(cnx, database_type)
            print("Background playlist update task completed successfully")
        except Exception as e:
            print(f"Error in update_all_playlists: {str(e)}")
            if hasattr(e, '__traceback__'):
                import traceback
                print(traceback.format_exc())
        finally:
            close_database_connection(cnx)
    except Exception as e:
        print(f"Critical error in update_playlists_task: {str(e)}")
        if hasattr(e, '__traceback__'):
            import traceback
            print(traceback.format_exc())


class PlaylistCreate(BaseModel):
    name: str
    description: Optional[str]
    podcast_ids: Optional[List[int]]
    include_unplayed: bool = True
    include_partially_played: bool = True
    include_played: bool = False
    play_progress_min: Optional[float] = None  # Made optional
    play_progress_max: Optional[float] = None  # Made optional
    time_filter_hours: Optional[int] = None    # Made optional
    min_duration: Optional[int]
    max_duration: Optional[int]
    sort_order: str = "date_desc"
    group_by_podcast: bool = False
    max_episodes: Optional[int]
    user_id: int
    icon_name: str = "ph-playlist"

class PlaylistDelete(BaseModel):
    user_id: int
    playlist_id: int

class PlaylistsGet(BaseModel):
    user_id: int

@app.post("/api/data/create_playlist")
async def api_create_playlist(
    data: PlaylistCreate,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
) -> Dict[str, Any]:
    """
    Create a new custom playlist
    """
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(
            status_code=403,
            detail="Your API key is either invalid or does not have correct permission"
        )

    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id == data.user_id or is_web_key:
        try:
            playlist_id = database_functions.functions.create_playlist(
                cnx,
                database_type,
                data
            )
            return {"detail": "Playlist created successfully", "playlist_id": playlist_id}
        except Exception as e:
            raise HTTPException(status_code=400, detail=str(e))
    else:
        raise HTTPException(
            status_code=403,
            detail="You can only create playlists for yourself!"
        )

@app.delete("/api/data/delete_playlist")
async def api_delete_playlist(
    data: PlaylistDelete,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
) -> Dict[str, str]:
    """
    Delete a playlist
    """
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(
            status_code=403,
            detail="Your API key is either invalid or does not have correct permission"
        )

    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id == data.user_id or is_web_key:
        try:
            database_functions.functions.delete_playlist(
                cnx,
                database_type,
                data.user_id,
                data.playlist_id
            )
            return {"detail": "Playlist deleted successfully"}
        except Exception as e:
            raise HTTPException(status_code=400, detail=str(e))
    else:
        raise HTTPException(
            status_code=403,
            detail="You can only delete your own playlists!"
        )

@app.get("/api/data/get_playlists")
async def api_get_playlists(
    user_id: int,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
) -> Dict[str, List[Dict[str, Any]]]:
    """
    Get all playlists accessible to the user
    """
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(
            status_code=403,
            detail="Your API key is either invalid or does not have correct permission"
        )

    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id == user_id or is_web_key:
        try:
            playlists = database_functions.functions.get_playlists(
                cnx,
                database_type,
                user_id
            )
            return {"playlists": playlists}
        except Exception as e:
            raise HTTPException(status_code=400, detail=str(e))
    else:
        raise HTTPException(
            status_code=403,
            detail="You can only view your own playlists!"
        )


@app.get("/api/data/get_playlist_episodes")
async def api_get_playlist_episodes(
    user_id: int,
    playlist_id: int,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
) -> Dict[str, Any]:
    """
    Get all episodes in a playlist
    """
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(
            status_code=403,
            detail="Your API key is either invalid or does not have correct permission"
        )

    try:
        return database_functions.functions.get_playlist_episodes(
            cnx,
            database_type,
            user_id,
            playlist_id
        )
    except Exception as e:
        raise HTTPException(status_code=400, detail=str(e))


@app.get("/api/data/episode_by_url/{url_key}")
async def get_episode_by_url_key(url_key: str, cnx=Depends(get_database_connection)):
    # Find the episode ID associated with the URL key
    print('running inside ep by url')
    episode_id = database_functions.functions.get_episode_id_by_url_key(database_type, cnx, url_key)
    print(f'outside dunc {episode_id}')
    if episode_id is None:
        raise HTTPException(status_code=404, detail="Invalid or expired URL key")

    # Now retrieve the episode metadata using the episode_id
    try:
        episode_data = database_functions.functions.get_episode_metadata_id(database_type, cnx, episode_id)  # UserID is None because we are bypassing normal user auth for shared links
        return {"episode": episode_data}
    except ValueError as e:
        raise HTTPException(status_code=404, detail=str(e))


class LoginInitiateData(BaseModel):
    user_id: int
    nextcloud_url: str

@app.post("/api/data/initiate_nextcloud_login")
async def initiate_nextcloud_login(data: LoginInitiateData, cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header)):
    import requests

    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == data.user_id or is_web_key:
        login_url = f"{data.nextcloud_url}/index.php/login/v2"
        try:
            response = requests.post(login_url)
            response.raise_for_status()  # This will raise an HTTPError for bad responses
            return response.json()
        except requests.HTTPError as http_err:
            # Log the detailed error
            detail = f"Nextcloud login failed with status code {response.status_code}: {response.text}"
            raise HTTPException(status_code=response.status_code, detail=detail)
        except requests.RequestException as req_err:
            # General request exception handling (e.g., network issues)
            raise HTTPException(status_code=500, detail=f"Failed to reach Nextcloud server: {str(req_err)}")
    else:
        raise HTTPException(status_code=403, detail="You are not authorized to initiate this action.")

class GpodderAuthRequest(BaseModel):
    gpodder_url: str
    gpodder_username: str
    gpodder_password: str

@app.post("/api/data/verify_gpodder_auth")
async def verify_gpodder_auth(request: GpodderAuthRequest):
    from requests.auth import HTTPBasicAuth
    auth = HTTPBasicAuth(request.gpodder_username, request.gpodder_password)
    async with httpx.AsyncClient() as client:
        try:
            response = await client.post(f"{request.gpodder_url}/api/2/auth/{request.gpodder_username}/login.json", auth=auth)
            response.raise_for_status()  # Will raise an httpx.HTTPStatusError for 4XX/5XX responses
            if response.status_code == 200:
                return {"status": "success", "message": "Logged in!"}
            else:
                raise HTTPException(status_code=response.status_code, detail="Authentication failed")
        except httpx.HTTPStatusError as e:
            raise HTTPException(status_code=e.response.status_code, detail="Authentication failed")
        except Exception as e:
            raise HTTPException(status_code=500, detail="Internal Server Error")

class GpodderSettings(BaseModel):
    user_id: int
    gpodder_url: str
    gpodder_token: str

@app.post("/api/data/add_gpodder_settings")
async def add_gpodder_settings(data: GpodderSettings, cnx=Depends(get_database_connection),
                              api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == data.user_id or is_web_key:
        result = database_functions.functions.add_gpodder_settings(database_type, cnx, data.user_id, data.gpodder_url, data.gpodder_token)
        return {"data": result}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only add your own gpodder data!")

class GpodderSettings(BaseModel):
    user_id: int
    gpodder_url: str
    gpodder_username: str
    gpodder_password: str


@app.post("/api/data/add_gpodder_server")
async def add_gpodder_server(
    data: GpodderSettings,
    background_tasks: BackgroundTasks,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id == data.user_id or is_web_key:
        # First add the gpodder server
        result = database_functions.functions.add_gpodder_server(
            database_type,
            cnx,
            data.user_id,
            data.gpodder_url,
            data.gpodder_username,
            data.gpodder_password
        )

        # Get the user's gpodder settings - similar to what refresh_nextcloud_subscription does
        if database_type == "postgresql":
            cursor = cnx.cursor()
            cursor.execute('''
                SELECT "userid", "gpodderurl", "gpoddertoken", "gpodderloginname"
                FROM "Users"
                WHERE "userid" = %s AND "gpodderurl" IS NOT NULL
            ''', (data.user_id,))
            user = cursor.fetchone()
        else:
            cursor = cnx.cursor()
            cursor.execute('''
                SELECT UserID, GpodderUrl, GpodderToken, GpodderLoginName
                FROM Users
                WHERE UserID = %s AND GpodderUrl IS NOT NULL
            ''', (data.user_id,))
            user = cursor.fetchone()

        if user:
            if isinstance(user, dict):
                if database_type == "postgresql":
                    gpodder_url = user["gpodderurl"]
                    gpodder_token = user["gpoddertoken"]
                    gpodder_login = user["gpodderloginname"]
                else:
                    gpodder_url = user["GpodderUrl"]
                    gpodder_token = user["GpodderToken"]
                    gpodder_login = user["GpodderLoginName"]
            else:
                _, gpodder_url, gpodder_token, gpodder_login = user

            # Add the refresh task for just this user
            background_tasks.add_task(
                refresh_nextcloud_subscription_for_user,
                database_type,
                data.user_id,
                gpodder_url,
                gpodder_token,
                gpodder_login
            )

        return {"data": result}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only add your own gpodder data!")


class RemoveGpodderSettings(BaseModel):
    user_id: int

@app.post("/api/data/remove_gpodder_settings")
async def remove_gpodder_settings(data: RemoveGpodderSettings, cnx=Depends(get_database_connection),
                              api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == data.user_id or is_web_key:
        result = database_functions.functions.remove_gpodder_settings(database_type, cnx, data.user_id)
        return {"data": result}
    else:
        raise HTTPException(status_code=403,
                            detail="You can only remove your own gpodder data!")

@app.get("/api/data/check_gpodder_settings/{user_id}")
async def check_gpodder_settings(user_id: int, cnx=Depends(get_database_connection),
                               api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

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
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

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
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)

    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    elevated_access = await has_elevated_access(api_key, cnx)

    if not elevated_access:
        # Get user ID from API key
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

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
            result = database_functions.functions.add_gpodder_settings(database_type, cnx, data.user_id, str(data.nextcloud_url), credentials["appPassword"], credentials["loginName"], "nextcloud")
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
        close_database_connection(cnx)
    for user in users:
        # Handle both dictionary and tuple cases
        if isinstance(user, dict):
            if database_type == "postgresql":
                user_id = user["userid"]
                gpodder_url = user["gpodderurl"]
                gpodder_token = user["gpoddertoken"]
                gpodder_login = user["gpodderloginname"]
                sync_type = user.get("pod_sync_type", "None")
            else:
                user_id = user["UserID"]
                gpodder_url = user["GpodderUrl"]
                gpodder_token = user["GpodderToken"]
                gpodder_login = user["GpodderLoginName"]
                sync_type = user.get("Pod_Sync_Type", "None")
        else:  # assuming tuple
            # Now handle 5 values instead of 4
            if len(user) >= 5:
                user_id, gpodder_url, gpodder_token, gpodder_login, sync_type = user
            else:
                user_id, gpodder_url, gpodder_token, gpodder_login = user
                sync_type = "None"

        # Pass the sync_type to the refresh function
        background_tasks.add_task(
            refresh_nextcloud_subscription_for_user,
            database_type,
            user_id,
            gpodder_url,
            gpodder_token,
            gpodder_login,
            sync_type  # Add this parameter
        )
    return {"status": "success", "message": "Nextcloud subscriptions refresh initiated."}

def refresh_nextcloud_subscription_for_user(database_type, user_id, gpodder_url, gpodder_token, gpodder_login, sync_type=None):
    cnx = create_database_connection()
    try:
        # If sync_type wasn't passed, try to get it from the database
        if not sync_type:
            sync_type = database_functions.functions.get_gpodder_type(cnx, database_type, user_id)

        # Determine if this is internal based on URL
        is_internal = gpodder_url == "http://localhost:8042"
        print(f"Using {'internal' if is_internal else 'external'} gpodder API for user {user_id}")

        # Special handling for nextcloud sync
        if sync_type == "nextcloud":
            print(f"Using nextcloud gpodder API for user {user_id}")
            success = database_functions.functions.refresh_nextcloud_subscription(
                database_type,
                cnx,
                user_id,
                gpodder_url,
                gpodder_token,
                gpodder_login,
                sync_type
            )
            return success
        # For all other GPodder sync types, use the standard refresh function
        elif sync_type in ["gpodder", "both", "external"]:
            # Get default device ID
            device_id = database_functions.functions.get_or_create_default_device(cnx, database_type, user_id)

            # Get device name if we have a device ID
            device_name = None
            if device_id:
                cursor = cnx.cursor()
                if database_type == "postgresql":
                    query = 'SELECT DeviceName FROM "GpodderDevices" WHERE DeviceID = %s'
                else:
                    query = "SELECT DeviceName FROM GpodderDevices WHERE DeviceID = %s"

                cursor.execute(query, (device_id,))
                result = cursor.fetchone()
                cursor.close()

                if result:
                    device_name = result[0] if isinstance(result, tuple) else result["devicename"]

            # Determine if this is a remote sync
            is_remote = not is_internal and sync_type in ["external", "both"]

            success = database_functions.functions.refresh_gpodder_subscription(
                database_type,
                cnx,
                user_id,
                gpodder_url,
                gpodder_token,
                gpodder_login,
                sync_type,
                device_id,
                device_name,
                is_remote
            )
            return success
        else:
            print(f"GPodder sync not enabled for user {user_id} (sync_type: {sync_type})")
            return False
    finally:
        close_database_connection(cnx)

class RemoveSyncRequest(BaseModel):
    user_id: int

@app.delete("/api/data/remove_podcast_sync")
async def remove_podcast_sync(data: RemoveSyncRequest, cnx=Depends(get_database_connection),
                             api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the user has permission to modify this user's data
    elevated_access = await has_elevated_access(api_key, cnx)
    if not elevated_access:
        user_id_from_api_key = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
        if data.user_id != user_id_from_api_key:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN,
                                detail="You are not authorized to modify these user settings")

    # Remove the sync settings
    database_functions.functions.remove_gpodder_settings(database_type, cnx, data.user_id)

    return {"success": True, "message": "Podcast sync settings removed successfully"}

def check_valid_feed(feed_url: str, username: Optional[str] = None, password: Optional[str] = None):
    """
    Check if the provided URL points to a valid podcast feed.
    Uses both direct content-type checking and feedparser validation.

    Args:
        feed_url: URL of the podcast feed
        username: Optional username for authenticated feeds
        password: Optional password for authenticated feeds

    Returns:
        feedparser.FeedParserDict: The parsed feed if valid

    Raises:
        ValueError: If the feed is invalid or inaccessible
    """
    import feedparser
    import requests
    from requests.auth import HTTPBasicAuth
    from typing import Optional

    # Common podcast feed content types
    VALID_CONTENT_TYPES = [
        'application/xml',
        'text/xml',
        'application/rss+xml',
        'application/atom+xml',
        'application/rdf+xml',
    ]

    def is_valid_content_type(content_type: str) -> bool:
        """Check if the content type indicates XML content."""
        content_type = content_type.lower().split(';')[0].strip()
        return any(valid_type in content_type for valid_type in VALID_CONTENT_TYPES) or 'xml' in content_type

    # Use requests to fetch the feed content
    try:
        # Set multiple user agents and accept headers to improve compatibility
        headers = {
            'User-Agent': 'Mozilla/5.0 (compatible; PodcastApp/1.0; +https://example.com)',
            'Accept': 'application/rss+xml, application/atom+xml, application/xml, text/xml, */*'
        }

        # Handle authentication if provided
        auth = HTTPBasicAuth(username, password) if username and password else None

        # Make the request with a timeout
        response = requests.get(
            feed_url,
            headers=headers,
            auth=auth,
            timeout=10,
            allow_redirects=True
        )
        response.raise_for_status()

        # Get content type, handling cases where it might not be present
        content_type = response.headers.get('Content-Type', '').lower()

        # Special handling for feeds that don't properly set content type
        if not is_valid_content_type(content_type):
            # Try to parse it anyway - some feeds might be valid despite wrong content type
            feed_content = response.content
            parsed_feed = feedparser.parse(feed_content)

            # If we can parse it and it has required elements, accept it despite content type
            if (parsed_feed.get('version') and
                'title' in parsed_feed.feed and
                'link' in parsed_feed.feed):
                return parsed_feed

            # If we can't parse it, then it's probably actually invalid
            raise ValueError(
                f"Unexpected Content-Type: {content_type}. "
                "The feed URL must point to an XML feed file."
            )

        feed_content = response.content

    except requests.RequestException as e:
        raise ValueError(f"Error fetching the feed: {str(e)}")

    # Parse the feed content using feedparser
    parsed_feed = feedparser.parse(feed_content)

    # Check for feedparser errors
    if parsed_feed.get('bozo') == 1:
        exception = parsed_feed.get('bozo_exception')
        if exception:
            raise ValueError(f"Feed parsing error: {str(exception)}")

    # Validate the parsed feed has required elements
    if not parsed_feed.get('version'):
        raise ValueError("Invalid podcast feed URL or content: Could not determine feed version.")

    required_attributes = ['title', 'link']
    missing_attributes = [attr for attr in required_attributes if attr not in parsed_feed.feed]

    if missing_attributes:
        raise ValueError(
            f"Feed missing required attributes: {', '.join(missing_attributes)}. "
            "The URL must point to a valid podcast feed."
        )

    # Check for podcast-specific elements
    has_items = len(parsed_feed.entries) > 0
    if not has_items:
        raise ValueError("Feed contains no episodes.")

    return parsed_feed



class CustomPodcast(BaseModel):
    feed_url: str
    user_id: int
    username: Optional[str] = None
    password: Optional[str] = None

@app.post("/api/data/add_custom_podcast")
async def add_custom_pod(data: CustomPodcast, cnx=Depends(get_database_connection),
                     api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    # Allow the action if the API key belongs to the user or it's the web API key
    if key_id == data.user_id or is_web_key:
        try:
            parsed_feed = check_valid_feed(data.feed_url, data.username, data.password)
        except ValueError as e:
            logger.error(f"Failed to parse: {str(e)}")
            raise HTTPException(status_code=400, detail=str(e))

        # Assuming the rest of the code processes the podcast correctly
        try:
            podcast_id = database_functions.functions.add_custom_podcast(database_type, cnx, data.feed_url, data.user_id, data.username, data.password)
            print('custom done')
            podcast_details = database_functions.functions.get_podcast_details(database_type, cnx, data.user_id, podcast_id)
            return {"data": podcast_details}
        except Exception as e:
            logger.error(f"Failed to process the podcast: {str(e)}")
            raise HTTPException(status_code=500, detail=f"Failed to process the podcast: {str(e)}")
    else:
        raise HTTPException(status_code=403,
                            detail="You can only add podcasts for yourself!")

class PersonEpisodesRequest(BaseModel):
    user_id: int
    person_id: int

@app.get("/api/data/person/episodes/{user_id}/{person_id}")
async def api_return_person_episodes(
    user_id: int,
    person_id: int,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(
            status_code=403,
            detail="Your API key is either invalid or does not have correct permission"
        )

    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id == user_id or is_web_key:
        episodes = database_functions.functions.return_person_episodes(database_type, cnx, user_id, person_id)
        if episodes is None:
            episodes = []
        return {"episodes": episodes}
    else:
        raise HTTPException(
            status_code=403,
            detail="You can only view episodes for your own subscriptions!"
        )

@app.get("/api/data/refresh_hosts")
async def refresh_all_hosts(
    background_tasks: BackgroundTasks,
    cnx=Depends(get_database_connection), is_admin: bool = Depends(check_if_admin),
    api_key: str = Depends(get_api_key_from_header),
):
    """Refresh episodes for all subscribed hosts"""
    # Verify it's the system/web API key
    if api_key != base_webkey.web_key:
        raise HTTPException(status_code=403, detail="This endpoint requires system API key")
    try:
        cursor = cnx.cursor()
        # Get all unique people that users are subscribed to
        if database_type == "postgresql":
            cursor.execute("""
                SELECT DISTINCT p.PersonID, p.Name, p.UserID
                FROM "People" p
            """)
        else:  # MySQL
            cursor.execute("""
                SELECT DISTINCT p.PersonID, p.Name, p.UserID
                FROM People p
            """)

        subscribed_hosts = cursor.fetchall()
        if not subscribed_hosts:
            return {"message": "No subscribed hosts found"}

        # Process each host in the background
        hosts_to_process = []
        for host in subscribed_hosts:
            # Handle both tuple and dict result formats
            if isinstance(host, dict):
                person_id = host.get('PersonID', host.get('personid'))
                person_name = host.get('Name', host.get('name'))
                user_id = host.get('UserID', host.get('userid'))
            else:  # tuple
                person_id, person_name, user_id = host

            hosts_to_process.append(person_name)
            background_tasks.add_task(
                process_person_subscription_task,
                user_id,
                person_id,
                person_name
            )

        return {
            "message": f"Refresh initiated for {len(subscribed_hosts)} hosts",
            "hosts": hosts_to_process
        }
    except Exception as e:
        logging.error(f"Error refreshing hosts: {str(e)}")
        raise HTTPException(status_code=500, detail=str(e))

class PersonSubscribeRequest(BaseModel):
    person_name: str
    person_img: str
    podcast_id: int

@app.post("/api/data/person/subscribe/{user_id}/{person_id}")
async def api_subscribe_to_person(
    user_id: int,
    person_id: int,
    request: PersonSubscribeRequest,
    background_tasks: BackgroundTasks,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Invalid or unauthorized API key")

    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id == user_id or is_web_key:
        success, db_person_id = database_functions.functions.subscribe_to_person(
            cnx,
            database_type,
            user_id,
            person_id,
            request.person_name,
            request.person_img,
            request.podcast_id
        )

        if success:
            # Add background task to process the subscription using the actual PersonID
            background_tasks.add_task(
                process_person_subscription_task,
                user_id,
                db_person_id,  # Use the actual PersonID from the database
                request.person_name
            )
            return {
                "message": "Successfully subscribed to person",
                "person_id": db_person_id  # Return the actual person ID
            }
        else:
            raise HTTPException(status_code=400, detail="Failed to subscribe to person")
    else:
        raise HTTPException(status_code=403, detail="You can only subscribe for yourself!")

class UniqueShow(TypedDict):
    title: str
    feed_url: str
    feed_id: int

def process_person_subscription_task(
    user_id: int,
    person_id: int,
    person_name: str
) -> None:
    """Regular synchronous task for processing person subscription"""
    cnx = create_database_connection()
    try:
        # Run the async function in a new event loop
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        loop.run_until_complete(
            process_person_subscription(user_id, person_id, person_name, cnx)
        )
        loop.close()
        # After successful person subscription processing, trigger a server refresh
        print("Person subscription processed, initiating server refresh...")
        try:
            refresh_pods_task()
            print("Server refresh completed successfully")
        except Exception as refresh_error:
            print(f"Error during server refresh: {refresh_error}")
            # Don't raise the error here - we don't want to fail the whole operation
            # if just the refresh fails
            pass
    except Exception as e:
        print(f"Error in process_person_subscription_task: {e}")
        raise
    finally:
        close_database_connection(cnx)

async def process_person_subscription(
    user_id: int,
    person_id: int,
    person_name: str,
    cnx
) -> None:
    """Async function to process person subscription and gather their shows"""
    print(f"Starting refresh for host: {person_name} (ID: {person_id})")
    try:
        # Set of unique shows (title, feed_url, feed_id)
        processed_shows: Set[Tuple[str, str, int]] = set()

        # 1. Get podcasts from podpeople
        async with httpx.AsyncClient(timeout=30.0) as client:
            try:
                podpeople_response = await client.get(
                    f"{people_url}/api/hostsearch",
                    params={"name": person_name}
                )
                podpeople_response.raise_for_status()
                podpeople_data = podpeople_response.json()

                # Check if we got valid data
                if podpeople_data and podpeople_data.get("success"):
                    for podcast in podpeople_data.get("podcasts", []):
                        processed_shows.add((
                            podcast['title'],
                            podcast['feed_url'],
                            podcast['id']
                        ))
            except Exception as e:
                print(f"Error getting data from podpeople: {str(e)}")
                # Continue execution even if podpeople lookup fails
                pass

        # 2. Get podcasts from podcast index
        print(f"API URL configured as: {api_url}")
        async with httpx.AsyncClient(timeout=30.0) as client:
            try:
                index_response = await client.get(
                    f"{api_url}",
                    params={
                        "query": person_name,
                        "index": "person",
                        "search_type": "person"
                    }
                )
                index_response.raise_for_status()
                index_data = index_response.json()

                if index_data and "items" in index_data:
                    for episode in index_data["items"]:
                        if all(field is not None for field in [episode.get("feedTitle"), episode.get("feedUrl"), episode.get("feedId")]):
                            processed_shows.add((
                                episode["feedTitle"],
                                episode["feedUrl"],
                                episode["feedId"]
                            ))
            except Exception as e:
                print(f"Error getting data from podcast index: {str(e)}")
                # Continue execution even if podcast index lookup fails
                pass

        # Only continue if we found any shows
        if not processed_shows:
            print(f"No shows found for person: {person_name}")
            return

        # 3. Process each unique show
        for title, feed_url, feed_id in processed_shows:
            try:
                # First check if podcast exists for user
                user_podcast_id = database_functions.functions.get_podcast_id(
                    database_type,
                    cnx,
                    user_id,
                    feed_url,
                    title
                )

                # Get podcast details and add as system podcast
                podcast_values = database_functions.app_functions.get_podcast_values(
                    feed_url,
                    1,  # System UserID
                    None,
                    None,
                    False
                )

                if not user_podcast_id:
                    # Check if system podcast exists (UserID = 0)
                    system_podcast_id = database_functions.functions.get_podcast_id(
                        database_type,
                        cnx,
                        1,  # System UserID
                        feed_url,
                        title
                    )

                    if system_podcast_id is None:
                        # If not found for system, add as a new system podcast
                        podcast_values = database_functions.app_functions.get_podcast_values(
                            feed_url,
                            1,  # System UserID
                            None,
                            None,
                            False
                        )
                        success = database_functions.functions.add_person_podcast(
                            cnx,
                            database_type,
                            podcast_values,
                            1  # System UserID
                        )
                        if success:
                            # Get the newly created podcast ID
                            system_podcast_id = database_functions.functions.get_podcast_id(
                                database_type,
                                cnx,
                                1,  # System UserID
                                feed_url,
                                title
                            )
                    podcast_id = system_podcast_id
                else:
                    podcast_id = user_podcast_id

                print(f"Using podcast: ID={podcast_id}, Title={title}")
                # 4. Add episodes to PeopleEpisodes
                database_functions.functions.add_people_episodes(
                    cnx,
                    database_type,
                    person_id=person_id,
                    podcast_id=podcast_id,
                    feed_url=feed_url,
                )

            except Exception as e:
                logging.error(f"Error processing show {title}: {str(e)}")
                continue

    except Exception as e:
        logging.error(f"Error processing person subscription: {str(e)}")
        raise

class UnsubscribeRequest(BaseModel):
    person_name: str

@app.delete("/api/data/person/unsubscribe/{user_id}/{person_id}")
async def api_unsubscribe_from_person(
    user_id: int,
    person_id: int,
    request: UnsubscribeRequest,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Invalid or unauthorized API key")
    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
    if key_id == user_id or is_web_key:
        success = database_functions.functions.unsubscribe_from_person(cnx, database_type, user_id, person_id, request.person_name)
        if success:
            return {"message": "Successfully unsubscribed from person"}
        else:
            raise HTTPException(status_code=400, detail="Failed to unsubscribe from person")
    else:
        raise HTTPException(status_code=403, detail="You can only unsubscribe for yourself!")

@app.get("/api/data/person/subscriptions/{user_id}")
async def api_get_person_subscriptions(
    user_id: int,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403, detail="Invalid or unauthorized API key")

    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

    if key_id == user_id or is_web_key:
        subscriptions = database_functions.functions.get_person_subscriptions(cnx, database_type, user_id)
        return {"subscriptions": subscriptions}
    else:
        raise HTTPException(status_code=403, detail="You can only view your own subscriptions!")


@app.get("/api/data/stream/{episode_id}")
async def stream_episode(
    episode_id: int,
    cnx=Depends(get_database_connection),
    api_key: str = Query(..., alias='api_key'),
    user_id: int = Query(..., alias='user_id'),
    source_type: str = Query(None, alias='type')
):
    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
    if not key_id and not is_web_key:
        rss_key = database_functions.functions.get_rss_key_if_valid(cnx, database_type, api_key)
        if not rss_key:
            raise HTTPException(status_code=403, detail="Invalid API key")
        key_id = rss_key.get('user_id')
        universal_key = (not rss_key.get('podcast_ids') or len(rss_key.get('podcast_ids')) == 0 or -1 in rss_key.get('podcast_ids'))
        if not universal_key and not database_functions.functions.validate_episode_access(cnx, database_type, episode_id, rss_key.get('podcast_ids')):
            raise HTTPException(status_code=403, detail="You do not have permission to access this episode")

    if key_id == user_id or is_web_key:
        # Choose which lookup to use based on source_type
        if source_type == "youtube":
            file_path = database_functions.functions.get_youtube_video_location(cnx, database_type, episode_id, user_id)
            print(f'file path in if source youtube {file_path}')
        else:
            file_path = database_functions.functions.get_download_location(cnx, database_type, episode_id, user_id)
            print(f'file path in if source else {file_path}')

        if file_path:
            return FileResponse(path=file_path, media_type='audio/mpeg', filename=os.path.basename(file_path))
        else:
            raise HTTPException(status_code=404, detail="Episode not found or not downloaded")
    else:
        raise HTTPException(status_code=403, detail="You do not have permission to access this episode")

class UpdateGpodderSyncRequest(BaseModel):
    enabled: bool

@app.post("/api/data/gpodder/toggle")
async def toggle_gpodder_sync(
    request: UpdateGpodderSyncRequest,
    background_tasks: BackgroundTasks,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    """Enable or disable gpodder sync for the current user"""
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(
            status_code=403,
            detail="Your API key is either invalid or does not have correct permission"
        )
    # Get the user ID from the API key
    user_id_result = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
    print(f"User ID result: {user_id_result}")
    if isinstance(user_id_result, dict):
        user_id = user_id_result.get('userid')
    else:
        user_id = user_id_result[0] if isinstance(user_id_result, tuple) else user_id_result
    if not user_id:
        raise HTTPException(status_code=403, detail="Invalid API key")
    try:
        print(f"Request to toggle gpodder sync: {request.enabled}")
        user_data = database_functions.functions.get_user_gpodder_status(cnx, database_type, user_id)
        if not user_data:
            raise HTTPException(status_code=404, detail="User not found")
        # Get initial state
        current_sync_type = user_data["sync_type"]
        print(f"Current sync type: {current_sync_type}")
        device_info = None
        if request.enabled:
            # Enable gpodder sync
            result = database_functions.functions.set_gpodder_internal_sync(cnx, database_type, user_id)
            if not result:
                raise HTTPException(status_code=500, detail="Failed to enable gpodder sync")
            device_info = result

            # Get required parameters for refresh_gpodder_subscription
            gpodder_settings = database_functions.functions.get_gpodder_settings(database_type, cnx, user_id)
            gpodder_token = gpodder_settings.get("gpoddertoken", "")
            gpodder_login = gpodder_settings.get("gpodderloginname", "")
            print(gpodder_settings)

            # Get the updated sync type after enabling
            updated_user_data = database_functions.functions.get_user_gpodder_status(cnx, database_type, user_id)
            updated_sync_type = updated_user_data["sync_type"]
            # gpodder_login = gpodder_settings.get("gpodderloginname", "")
            # gpodder_token = gpodder_settings.get("gpoddertoken", "")
            device_id = device_info.get("device_id") if device_info else None
            device_name = device_info.get("device_name") if device_info else None

            background_tasks.add_task(
                refresh_gpodder_subscription_for_background,  # Use the wrapper function
                database_type,
                user_id,
                'http://localhost:8042',
                gpodder_token,
                gpodder_login,
                updated_sync_type,
                device_id,
                device_name,
                False  # is_remote
            )
            print(f"Added background task to sync gpodder for user: {user_id}")
        else:
            # Disable gpodder sync
            success = database_functions.functions.disable_gpodder_internal_sync(cnx, database_type, user_id)
            if not success:
                raise HTTPException(status_code=500, detail="Failed to disable gpodder sync")
        # Get updated state after changes
        updated_data = database_functions.functions.get_user_gpodder_status(cnx, database_type, user_id)
        new_sync_type = updated_data["sync_type"]
        print(f"Updated sync type: {new_sync_type}")
        response = {
            "sync_type": new_sync_type,
            "gpodder_enabled": new_sync_type in ["gpodder", "both"],
            "external_enabled": new_sync_type in ["external", "both"],
            "external_url": updated_data.get("gpodder_url") if new_sync_type in ["external", "both"] else None,
            "api_url": "http://localhost:8042" if new_sync_type in ["gpodder", "both"] else None
        }
        # Add device information if available
        if device_info and request.enabled:
            response["device_name"] = device_info["device_name"]
            response["device_id"] = device_info["device_id"]
        print(f"Returning response: {response}")
        return response
    except Exception as e:
        print(f"Error in toggle_gpodder_sync: {e}")
        raise HTTPException(status_code=500, detail=f"Internal server error: {str(e)}")


def refresh_gpodder_subscription_for_background(database_type, user_id, gpodder_url, gpodder_token,
                                               gpodder_login, sync_type, device_id=None, device_name=None, is_remote=False):
    """Wrapper function for background tasks to ensure proper database connection handling"""
    from database_functions.db_client import create_database_connection, close_database_connection
    import logging

    logger = logging.getLogger(__name__)

    # Create a new connection explicitly for this background task
    cnx = create_database_connection()

    try:
        print(f"Starting background refresh for user {user_id} with sync_type {sync_type}")
        # Call the original function with our managed connection
        success = database_functions.functions.refresh_gpodder_subscription(
            database_type,
            cnx,
            user_id,
            gpodder_url,
            gpodder_token,
            gpodder_login,
            sync_type,
            device_id,
            device_name,
            is_remote
        )
        return success
    except Exception as e:
        logger.error(f"Error in background gpodder refresh: {str(e)}")
        return False
    finally:
        # Always close the connection we created
        close_database_connection(cnx)
        print(f"Closed database connection for background task for user {user_id}")
# Helper function to generate a token for internal gpodder API
def generate_gpodder_token(user_id):
    import secrets
    token = secrets.token_hex(16)
    return f"internal_gpodder_{user_id}_{token}"

@app.get("/api/data/gpodder/status")
async def get_gpodder_status(
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    """Get the current gpodder sync status for the user"""
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(
            status_code=403,
            detail="Your API key is either invalid or does not have correct permission"
        )

    # Get the user ID from the API key
    user_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
    if not user_id:
        raise HTTPException(status_code=403, detail="Invalid API key")

    try:
        user_data = database_functions.functions.get_user_gpodder_status(cnx, database_type, user_id)

        if not user_data:
            raise HTTPException(status_code=404, detail="User not found")

        sync_type = user_data["sync_type"]

        return {
            "sync_type": sync_type,
            "gpodder_enabled": sync_type in ["gpodder", "both"],
            "external_enabled": sync_type in ["external", "both"],
            "external_url": user_data["gpodder_url"],
                "api_url": "http://localhost:8042"  # Replace with actual API URL if needed
        }
    except Exception as e:
        print(f"Error in get_gpodder_status: {e}")
        raise HTTPException(status_code=500, detail=f"Internal server error: {str(e)}")

class BackupUser(BaseModel):
    user_id: int


@app.post("/api/data/backup_user", response_class=PlainTextResponse)
async def backup_user(data: BackupUser, cnx=Depends(get_database_connection),
                      api_key: str = Depends(get_api_key_from_header)):
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(status_code=403,
                            detail="Your API key is either invalid or does not have correct permission")

    # Check if the provided API key is the web key
    is_web_key = api_key == base_webkey.web_key

    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)

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
        dump_data = database_functions.functions.backup_server(database_type, cnx, request.database_pass)
    except Exception as e:
        raise HTTPException(status_code=status.HTTP_400_BAD_REQUEST, detail=str(e))
    return Response(content=dump_data, media_type="text/plain")

@app.post("/api/data/restore_server")
async def api_restore_server(
    background_tasks: BackgroundTasks,
    backup_file: UploadFile,
    database_pass: str = Form(...),
    is_admin: bool = Depends(check_if_admin),
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    if not is_admin:
        raise HTTPException(status_code=403, detail="Not authorized")

    if not backup_file.filename.endswith('.sql'):
        raise HTTPException(status_code=400, detail="Invalid file type. Only .sql files are allowed")

    file_content = await backup_file.read()
    if len(file_content) > 100 * 1024 * 1024:  # 100MB limit
        raise HTTPException(status_code=413, detail="File too large")

    logging.info(f"Restoring server with uploaded backup file")
    background_tasks.add_task(restore_server_fun, database_pass, file_content)
    return JSONResponse(content={"detail": "Server restoration started."})

def restore_server_fun(database_pass: str, server_restore_data: str):
    # Assuming create_database_connection and restore_server are defined in database_functions.functions
    cnx = create_database_connection()  # Replace with your method to create a new DB connection
    try:
        # Restore server using the provided password and data
        database_functions.functions.restore_server(cnx, database_pass, server_restore_data)
    finally:
        cnx.close()

@app.get("/api/data/rss_feed_status")
async def get_rss_feed_status(
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    """Get RSS feed enabled status for current user"""
    try:
        key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
        print(f'user_id for rss: {key_id}')
        if not key_id:
            raise HTTPException(status_code=403, detail="Invalid API key")

        status = database_functions.functions.get_rss_feed_status(cnx, database_type, key_id)
        print(status)
        return status

    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))

@app.post("/api/data/toggle_rss_feeds")
async def toggle_rss_feeds_endpoint(
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    """Toggle RSS feed status for current user"""
    try:
        key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
        if not key_id:
            raise HTTPException(status_code=403, detail="Invalid API key")
        new_status = database_functions.functions.toggle_rss_feeds(cnx, database_type, key_id)
        return {"success": True, "enabled": new_status}
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))

@app.get("/api/feed/{user_id}")
async def get_user_feed(
    request: Request,
    user_id: int,
    api_key: str,  # Now a query parameter
    limit: int = 100,
    podcast_id: Optional[List[int]] = None,
    source_type: str = Query(None, alias='type'),
    cnx=Depends(get_database_connection)
):
    """Get RSS feed for all podcasts or a specific podcast"""
    print(f'user: {user_id}, api: {api_key}')
    try:
        if reverse_proxy == "True":
            domain = f'{proxy_protocol}://{proxy_host}'
        else:
            domain = f'{request.url.scheme}://{request.url.hostname}'


        rss_key = database_functions.functions.get_rss_key_if_valid(cnx, database_type, api_key, podcast_id)

        # TODO: remove this once backwards compatibility is no longer needed
        if not rss_key:
            key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
            if not key_id:
                raise HTTPException(status_code=403, detail="Invalid API key")
            rss_key = {
                "podcast_ids": [ -1 ],
                "user_id": key_id,
                "key": api_key
            }

        feed_content = database_functions.functions.generate_podcast_rss(
            database_type,
            cnx,
            rss_key,
            limit,
            source_type,
            domain,
            podcast_id
        )
        return Response(
            content=feed_content,
            media_type="application/rss+xml"
        )
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))

@app.post("/api/data/rss_feed_status/{user_id}")
async def toggle_rss_feeds(
    user_id: int,
    enable: bool,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    """Enable or disable RSS feeds for a user"""
    try:
        key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
        if not key_id:
            raise HTTPException(status_code=403, detail="Invalid API key")

        new_status = database_functions.functions.set_rss_feed_status(cnx, database_type, user_id, enable)
        return {"status": "success", "enabled": new_status}

    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


class YouTubeChannel(BaseModel):
    channel_id: str
    name: str
    description: str
    subscriber_count: Optional[int]
    url: str
    video_count: Optional[int]
    thumbnail_url: Optional[str]
    recent_videos: List[dict] = []

    class Config:
        json_encoders = {
            list: lambda v: v  # Preserve lists during JSON encoding
        }

@app.get("/api/data/search_youtube_channels")
async def search_youtube_channels(
    query: str,
    max_results: int = 5,
    user_id: int = None,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    # Validate API key
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(
            status_code=403,
            detail="Your API key is either invalid or does not have correct permission"
        )

    # Check if web key and verify user permission
    is_web_key = api_key == base_webkey.web_key
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
    if not (key_id == user_id or is_web_key):
        raise HTTPException(
            status_code=403,
            detail="You can only search with your own account."
        )

    try:
        # First get channel ID using a search
        search_url = f"ytsearch{max_results*4}:{query}"

        ydl_opts = {
            'quiet': True,
            'extract_flat': True,
            'no_warnings': True,
            'skip_download': True,
            'extract_info': True,
        }

        with YoutubeDL(ydl_opts) as ydl:
            logging.info(f"Searching YouTube with query: {query}")
            results = ydl.extract_info(search_url, download=False)

            if not results or 'entries' not in results:
                return {"results": []}

            processed_results = []
            seen_channels = set()  # Track unique channels
            channel_videos = {}

            for entry in results.get('entries', []):
                try:
                    channel_id = entry.get('channel_id') or entry.get('uploader_id')
                    if not channel_id:
                        continue

                    # First collect the video regardless of whether we've seen the channel
                    if channel_id not in channel_videos:
                        channel_videos[channel_id] = []
                    if len(channel_videos[channel_id]) < 3:  # Limit to 3 videos
                        channel_videos[channel_id].append({
                            'id': entry.get('id', ''),
                            'title': entry.get('title', ''),
                            'duration': entry.get('duration'),
                            'url': f"https://www.youtube.com/watch?v={entry.get('id')}"
                        })
                        print(f"Added video to channel {channel_id}, now has {len(channel_videos[channel_id])} videos")


                    # Now check if we've already processed this channel
                    if channel_id in seen_channels:
                        continue

                    seen_channels.add(channel_id)

                    # Get minimal channel info
                    channel_opts = ydl_opts.copy()
                    channel_opts['extract_flat'] = True
                    channel_opts['process'] = False

                    channel_url = f"https://www.youtube.com/channel/{channel_id}"
                    channel_info = ydl.extract_info(
                        channel_url,
                        download=False,
                        process=False  # Don't process more than necessary
                    )

                    # Get avatar URL from channel info
                    thumbnail_url = None
                    if channel_info and channel_info.get('thumbnails'):
                        # Try to find avatar-specific thumbnails first
                        avatar_thumbnails = [t for t in channel_info['thumbnails']
                                            if t.get('id', '').startswith('avatar')]

                        if avatar_thumbnails:
                            # Get the largest avatar thumbnail
                            thumbnail_url = avatar_thumbnails[-1]['url']
                        else:
                            # Fallback: try to find any thumbnail with "avatar" in the URL
                            avatar_thumbnails = [t for t in channel_info['thumbnails']
                                               if 'avatar' in t.get('url', '').lower()]
                            if avatar_thumbnails:
                                thumbnail_url = avatar_thumbnails[-1]['url']
                            else:
                                # Last resort: use the first thumbnail
                                thumbnail_url = channel_info['thumbnails'][0]['url']
                    print(f"Creating channel {channel_id} with {len(channel_videos[channel_id])} videos")
                    channel = YouTubeChannel(
                        channel_id=channel_id,
                        name=entry.get('channel', '') or entry.get('uploader', ''),
                        description=entry.get('description', '')[:500] if entry.get('description') else '',
                        subscriber_count=None,
                        url=f"https://www.youtube.com/channel/{channel_id}",
                        video_count=None,
                        thumbnail_url=thumbnail_url or entry.get('channel_thumbnail', ''),
                        recent_videos=channel_videos[channel_id]  # <-- Use our collected videos here
                    )

                    if len(processed_results) < max_results:
                        channel_dict = channel.dict()
                        channel_dict['recent_videos'] = channel_videos[channel_id]  # Explicitly set after dict conversion
                        processed_results.append(channel_dict)
                    else:
                        break

                except Exception as entry_error:
                    logging.error(f"Error processing channel entry: {entry_error}")
                    continue

            logging.info(f"Found {len(processed_results)} channels")
            return {"results": processed_results}

    except Exception as e:
        logging.error(f"YouTube channel search error: {str(e)}")
        raise HTTPException(
            status_code=500,
            detail=f"Error searching YouTube channels: {str(e)}"
        )

def process_youtube_channel(podcast_id: int, channel_id: str, feed_cutoff: int):
    cnx = create_database_connection()
    try:
        database_functions.youtube.process_youtube_videos(database_type, podcast_id, channel_id, cnx, feed_cutoff)
    finally:
        close_database_connection(cnx)

@app.post("/api/data/youtube/subscribe")
async def subscribe_to_youtube_channel(
    channel_id: str,
    user_id: int,
    background_tasks: BackgroundTasks,
    feed_cutoff: int = 30,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    """Subscribe to a YouTube channel"""
    import logging
    logger = logging.getLogger(__name__)

    try:
        logger.info(f"Starting subscription for channel {channel_id}")

        existing_id = database_functions.functions.check_existing_channel_subscription(cnx, database_type, channel_id, user_id)
        if existing_id:
            logger.info(f"Channel {channel_id} already subscribed")
            return {
                "success": True,
                "podcast_id": existing_id,
                "message": "Already subscribed to this channel"
            }

        logger.info("Getting channel info")
        channel_info = await database_functions.youtube.get_channel_info(channel_id)

        logger.info("Adding channel to database")
        podcast_id = database_functions.functions.add_youtube_channel(cnx, database_type, channel_info, user_id, feed_cutoff)

        logger.info(f"Starting background task for podcast_id {podcast_id}")
        background_tasks.add_task(process_youtube_channel, podcast_id, channel_id, feed_cutoff)

        logger.info("Subscription completed successfully")
        return {
            "success": True,
            "podcast_id": podcast_id,
            "message": "Channel subscription initiated. Videos will be processed in background."
        }
    except Exception as e:
        logger.error(f"Error subscribing to channel: {str(e)}", exc_info=True)
        raise HTTPException(
            status_code=500,
            detail=f"Error subscribing to channel: {str(e)}"
        )

@app.post("/api/auth/store_state")
async def store_oidc_state(
    request: Request,
):
    try:
        data = await request.json()
        state = data.get('state')
        client_id = data.get('client_id')

        if not state or not client_id:
            raise HTTPException(status_code=400, detail="Missing state or client_id")

        success = database_functions.oidc_state_manager.oidc_state_manager.store_state(state, client_id)
        if not success:
            raise HTTPException(status_code=500, detail="Failed to store state")

        return {"status": "success"}
    except Exception as e:
        logging.error(f"Error storing OIDC state: {str(e)}")
        raise HTTPException(status_code=500, detail="Failed to store state")

@app.get("/api/auth/callback")
async def oidc_callback(
    request: Request,
    code: str,
    state: str = None,
    cnx=Depends(get_database_connection)
):
    try:
        base_url = str(request.base_url)[:-1]
        # Force HTTPS if running in production
        if not base_url.startswith('http://localhost'):
            if base_url.startswith('http:'):
                base_url = 'https:' + base_url[5:]

        print(f"Base URL: {base_url}")
        frontend_base = base_url.replace('/api', '')

        # Get client_id from query parameters
        client_id = database_functions.oidc_state_manager.oidc_state_manager.get_client_id(state)
        if not client_id:
            return RedirectResponse(
                url=f"{frontend_base}/oauth/callback?error=invalid_state"
            )

        registered_redirect_uri = f"{base_url}/api/auth/callback"
        print(f"Using redirect_uri: {registered_redirect_uri}")

        # Get OIDC provider details
        provider = database_functions.functions.get_oidc_provider(cnx, database_type, client_id)
        if not provider:
            return RedirectResponse(
                url=f"{frontend_base}/oauth/callback?error=invalid_provider"
            )

        # Unpack provider details
        provider_id, client_id, client_secret, token_url, userinfo_url = provider

        # Exchange authorization code for access token
        async with httpx.AsyncClient() as client:
            try:
                token_response = await client.post(
                    token_url,
                    data={
                        "grant_type": "authorization_code",
                        "code": code,
                        "redirect_uri": registered_redirect_uri,
                        "client_id": client_id,
                        "client_secret": client_secret,
                    },
                    headers={
                        "Accept": "application/json"
                    }
                )

                if token_response.status_code != 200:
                    return RedirectResponse(
                        url=f"{frontend_base}/oauth/callback?error=token_exchange_failed"
                    )

                token_data = token_response.json()
                print(f"Token response: {token_data}")
                access_token = token_data.get("access_token")

                # Get user info from OIDC provider
                headers = {
                    "Authorization": f"Bearer {access_token}",
                    "User-Agent": "PinePods/1.0",  # Add a meaningful user agent
                    "Accept": "application/json"
                }
                userinfo_response = await client.get(userinfo_url, headers=headers)

                if userinfo_response.status_code != 200:
                    error_content = userinfo_response.text
                    print(f"GitHub API error: {error_content}")
                    return RedirectResponse(
                        url=f"{frontend_base}/oauth/callback?error=userinfo_failed"
                    )

                user_info = userinfo_response.json()
                print(f"User info response: {user_info}")
                email = user_info.get("email")

                parsed_url = urlparse(userinfo_url)
                if not email and parsed_url.hostname == 'api.github.com':
                    # For GitHub, we may need to make a separate request for emails
                    # because GitHub doesn't include email in user info if it's private
                    emails_response = await client.get(
                        'https://api.github.com/user/emails',
                        headers=headers
                    )

                    if emails_response.status_code == 200:
                        emails = emails_response.json()
                        # Find the primary email
                        for email_obj in emails:
                            if email_obj.get('primary') and email_obj.get('verified'):
                                email = email_obj.get('email')
                                break

                        # If no primary found, take the first verified one
                        if not email:
                            for email_obj in emails:
                                if email_obj.get('verified'):
                                    email = email_obj.get('email')
                                    break

                if not email:
                    return RedirectResponse(
                        url=f"{frontend_base}/oauth/callback?error=email_required"
                    )

            except httpx.RequestError:
                return RedirectResponse(
                    url=f"{frontend_base}/oauth/callback?error=network_error"
                )

            # Check if user exists
            user = database_functions.functions.get_user_by_email(cnx, database_type, email)

            # In your OIDC callback function, replace the user creation section with:

            if not user:
                # Create new user
                print(f"User with email {email} not found, creating new user")
                fullname = user_info.get("name", "")
                username = email.split("@")[0].lower()
                base_username = username
                counter = 1
                max_attempts = 10

                while counter <= max_attempts:
                    try:
                        print(f"Attempt {counter} to create user with base username: {base_username}")
                        user_id = database_functions.functions.create_oidc_user(
                            cnx, database_type, email, fullname, username
                        )
                        print(f"User created successfully with ID: {user_id}")

                        if not user_id:
                            print(f"ERROR: Invalid user_id returned: {user_id}")
                            return RedirectResponse(
                                url=f"{frontend_base}/oauth/callback?error=invalid_user_id"
                            )

                        print(f"Creating API key for user_id: {user_id}")
                        api_key = database_functions.functions.create_api_key(cnx, database_type, user_id)
                        print(f"API key created: {api_key[:5]}... (truncated for security)")
                        break
                    except UniqueViolation:
                        print(f"Username conflict with {username}, trying next variation")
                        username = f"{base_username}{counter}"
                        counter += 1
                        if counter > max_attempts:
                            print(f"Failed to create user after {max_attempts} attempts due to username conflicts")
                            return RedirectResponse(
                                url=f"{frontend_base}/oauth/callback?error=username_conflict"
                            )
                    except Exception as e:
                        print(f"Error during user creation: {str(e)}")
                        import traceback
                        print(f"Traceback: {traceback.format_exc()}")
                        return RedirectResponse(
                            url=f"{frontend_base}/oauth/callback?error=user_creation_failed&details={str(e)[:50]}"
                        )
                else:
                    print("Failed to create user after maximum attempts")
                    return RedirectResponse(
                        url=f"{frontend_base}/oauth/callback?error=user_creation_failed"
                    )

            else:
                # Existing user - retrieve their API key
                print(f"User with email {email} found, retrieving API key")
                user_id = user[0] if isinstance(user, tuple) else user['userid']  # Adjust based on your DB return format

                api_key = database_functions.functions.get_user_api_key(cnx, database_type, user_id)
                if not api_key:
                    print(f"No API key found for user_id: {user_id}, creating a new one")
                    api_key = database_functions.functions.create_api_key(cnx, database_type, user_id)

                print(f"API key retrieved: {api_key[:5]}... (truncated for security)")

            # Success case - redirect with API key
            return RedirectResponse(url=f"{frontend_base}/oauth/callback?api_key={api_key}")

    except Exception as e:
        logging.error(f"OIDC callback error: {str(e)}")
        return RedirectResponse(
            url=f"{frontend_base}/oauth/callback?error=authentication_failed"
        )

# Store active connections
class ConnectionManager:
    def __init__(self):
        # Map of user_id to list of websocket connections
        self.active_connections: Dict[int, List[WebSocket]] = {}

    async def connect(self, websocket: WebSocket, user_id: int):
        await websocket.accept()
        if user_id not in self.active_connections:
            self.active_connections[user_id] = []
        self.active_connections[user_id].append(websocket)

    def disconnect(self, websocket: WebSocket, user_id: int):
        if user_id in self.active_connections:
            if websocket in self.active_connections[user_id]:
                self.active_connections[user_id].remove(websocket)
            if not self.active_connections[user_id]:
                del self.active_connections[user_id]

    async def broadcast_to_user(self, user_id: int, message: Dict[str, Any]):
        if user_id in self.active_connections:
            # Convert to JSON once for efficiency
            json_message = json.dumps(message)
            disconnected = []

            # Send to all connections for this user
            for websocket in self.active_connections[user_id]:
                try:
                    await websocket.send_text(json_message)
                except Exception:
                    disconnected.append(websocket)

            # Clean up any failed connections
            for websocket in disconnected:
                self.disconnect(websocket, user_id)

# Initialize connection manager
manager = ConnectionManager()

# Define the broadcast message model
class BroadcastMessage(BaseModel):
    user_id: int
    message: Dict[str, Any]

@app.post("/api/tasks/broadcast")
async def broadcast_task_update(
    data: BroadcastMessage,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    """Endpoint to broadcast a task update to a user via WebSocket"""

    # Verify API key
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(
            status_code=403,
            detail="Your API key is either invalid or does not have correct permission"
        )

    # Check if manager has the user in active connections
    user_id = data.user_id
    has_connections = user_id in manager.active_connections
    print(f"Broadcasting to user {user_id}, has connections: {has_connections}")

    if has_connections:
        # Broadcast the message
        await manager.broadcast_to_user(user_id, data.message)
        return {"success": True, "message": f"Broadcast sent to user {user_id}"}
    else:
        print(f"No active connections for user {user_id}")
        return {"success": False, "message": f"No active connections for user {user_id}"}

# Model for task query parameters
class TaskQueryParams(BaseModel):
    user_id: int

# Extract API key from WebSocket query parameters
async def get_api_key_from_websocket(websocket: WebSocket) -> str:
    query_params = websocket.query_params
    api_key = query_params.get("api_key")

    if not api_key:
        raise ValueError("API key is required")

    return api_key

@app.get("/api/tasks/active")
async def get_active_tasks(
    user_id: int,
    cnx=Depends(get_database_connection),
    api_key: str = Depends(get_api_key_from_header)
):
    # Verify API key
    is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
    if not is_valid_key:
        raise HTTPException(
            status_code=403,
            detail="Your API key is either invalid or does not have correct permission"
        )

    # Check if user has permission to access these tasks
    key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
    is_web_key = api_key == base_webkey.web_key

    if key_id != user_id and not is_web_key:
        raise HTTPException(
            status_code=403,
            detail="You can only view your own tasks"
        )

    # Get all active tasks for the user - this needs to be expanded
    # to include all types of tasks, not just downloads
    active_tasks = database_functions.tasks.get_all_active_tasks(user_id)

    return {"tasks": active_tasks}

# Add this DEBUG logging to your FastAPI WebSocket endpoint in clientapi.py
@app.websocket("/ws/api/tasks/{user_id}")
async def websocket_endpoint(
    websocket: WebSocket,
    user_id: int,
    cnx=Depends(get_database_connection)
):
    print(f"WebSocket connection request received for user {user_id}")
    # Get API key from websocket query params
    try:
        api_key = await get_api_key_from_websocket(websocket)
        print(f"WebSocket API key validated for user {user_id}")

        # Verify API key
        is_valid_key = database_functions.functions.verify_api_key(cnx, database_type, api_key)
        if not is_valid_key:
            print(f"Invalid API key for WebSocket connection, user {user_id}")
            await websocket.close(code=status.WS_1008_POLICY_VIOLATION)
            return

        # Check if user has permission
        key_id = database_functions.functions.id_from_api_key(cnx, database_type, api_key)
        is_web_key = api_key == base_webkey.web_key

        if key_id != user_id and not is_web_key:
            print(f"Permission denied for WebSocket connection, user {user_id}")
            await websocket.close(code=status.WS_1008_POLICY_VIOLATION)
            return

        # Accept the connection
        await manager.connect(websocket, user_id)
        print(f"WebSocket connection accepted for user {user_id}")

        # Send initial task list with all types of tasks
        active_tasks = database_functions.tasks.get_all_active_tasks(user_id)
        print(f"Found {len(active_tasks)} active tasks for user {user_id}")
        await websocket.send_text(json.dumps({
            "event": "initial",
            "tasks": active_tasks
        }))

        # Keep connection alive and handle messages
        try:
            while True:
                # Handle any incoming messages (client might request refresh)
                message = await websocket.receive_text()
                data = json.loads(message)
                print(f"Received WebSocket message from user {user_id}: {data}")

                if data.get("action") == "refresh":
                    # Send updated task list with all tasks
                    active_tasks = database_functions.tasks.get_all_active_tasks(user_id)
                    await websocket.send_text(json.dumps({
                        "event": "refresh",
                        "tasks": active_tasks
                    }))

                # Wait a short while before next iteration
                await asyncio.sleep(0.1)

        except WebSocketDisconnect:
            print(f"WebSocket disconnected for user {user_id}")
            manager.disconnect(websocket, user_id)

    except Exception as e:
        print(f"WebSocket error for user {user_id}: {str(e)}")
        try:
            await websocket.close(code=status.WS_1011_INTERNAL_ERROR)
        except:
            pass


class InitRequest(BaseModel):
    api_key: str

@app.post("/api/init/startup_tasks")
async def run_startup_tasks(request: InitRequest, cnx=Depends(get_database_connection)):
    try:
        print('start of startup')
        # Verify if the API key is valid
        is_valid = database_functions.functions.verify_api_key(cnx, database_type, request.api_key)
        web_key = database_functions.functions.get_web_key(cnx, database_type)
        # Check if the provided API key is the web key
        is_web_key = request.api_key == web_key

        if not is_valid or not is_web_key:
            raise HTTPException(status_code=status.HTTP_403_FORBIDDEN, detail="Invalid or unauthorized API key")

        # Execute the startup tasks
        database_functions.functions.add_news_feed_if_not_added(database_type, cnx)
        return {"status": "Startup tasks completed successfully."}

        database_functions.valkey_client.connect()
    except Exception as e:
        logger.error(f"Error in startup tasks: {e}")
        raise HTTPException(status_code=status.HTTP_500_INTERNAL_SERVER_ERROR, detail="Failed to complete startup tasks")
    finally:
        # The connection will automatically be closed by FastAPI's dependency system
        pass

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
        log_config=config_file,
        limit_concurrency=1000,
    )
