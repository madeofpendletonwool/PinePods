# Fast API
from fastapi import FastAPI, Depends, HTTPException, status, Request, Header, Body, Path, Form, File, UploadFile, Query, \
    BackgroundTasks, WebSocket
from fastapi.security import APIKeyHeader, HTTPBasic, HTTPBasicCredentials
from fastapi.responses import PlainTextResponse

# Needed Modules
from contextlib import contextmanager
from passlib.context import CryptContext
# Add PostgreSQL imports
import psycopg2
from psycopg2 import pool
import os
from datetime import datetime
from fastapi.middleware.gzip import GZipMiddleware
from starlette.middleware.sessions import SessionMiddleware
from starlette.requests import Request
import secrets
import requests
from pydantic import BaseModel, Field
from typing import Dict
from typing import List
from typing import Optional
from typing import Generator
import json
import logging
from typing import Any
import argparse
import sys
from pyotp import TOTP
import base64
import threading
import time
import asyncio

# Internal Modules
sys.path.append('/pinepods')

import database_functions.functions
import Auth.Passfunctions

secret_key_middle = secrets.token_hex(32)

logging.basicConfig(level=logging.INFO)

from database_functions import functions

print('Client API Server is Starting!')

app = FastAPI()
app.add_middleware(GZipMiddleware, minimum_size=1000)
app.add_middleware(SessionMiddleware, secret_key=secret_key_middle)

API_KEY_NAME = "pinepods_api"
api_key_header = APIKeyHeader(name=API_KEY_NAME, auto_error=False)

pwd_context = CryptContext(schemes=["bcrypt"], deprecated="auto")

# Proxy variables
proxy_host = os.environ.get("PROXY_HOST", "localhost")
proxy_port = os.environ.get("PROXY_PORT", "8000")
proxy_protocol = os.environ.get("PROXY_PROTOCOL", "http")
reverse_proxy = os.environ.get("REVERSE_PROXY", "False")

# Podcast Index API url
api_url = os.environ.get("API_URL", "https://api.pinepods.online/api/search")

# Initial Vars needed to start and used throughout
if reverse_proxy == "True":
    proxy_url = f'{proxy_protocol}://{proxy_host}/proxy?url='
else:
    proxy_url = f'{proxy_protocol}://{proxy_host}:{proxy_port}/proxy?url='
print(f'Proxy url is configured to {proxy_url}')


# Update the connection pool setup function
def setup_connection_pool():
    db_host = os.environ.get("DB_HOST", "127.0.0.1")
    db_port = os.environ.get("DB_PORT", "5432")  # Default PostgreSQL port
    db_user = os.environ.get("DB_USER", "postgres")  # Default PostgreSQL user
    db_password = os.environ.get("DB_PASSWORD", "password")
    db_name = os.environ.get("DB_NAME", "pypods_database")

    return pool.SimpleConnectionPool(
        1,  # minconn
        32,  # maxconn
        host=db_host,
        port=db_port,
        user=db_user,
        password=db_password,
        dbname=db_name
    )

# Update the database connection function
def get_database_connection() -> pool.SimpleConnectionPool:
    try:
        db = connection_pool.getconn()
        yield db
    except Exception as e:
        raise HTTPException(500, "Unable to connect to the database")
    finally:
        connection_pool.putconn(db)


connection_pool = setup_connection_pool()


def get_api_keys(cnx):
    cursor = cnx.cursor(dictionary=True)
    query = "SELECT * FROM APIKeys"
    cursor.execute(query)
    rows = cursor.fetchall()
    cursor.close()
    return rows


def get_api_key(request: Request, api_key: str = Depends(api_key_header),
                cnx: Generator = Depends(get_database_connection)):
    if api_key is None:
        raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED, detail="API key is missing")

    api_keys = get_api_keys(cnx)

    for api_key_entry in api_keys:
        stored_key = api_key_entry["APIKey"]
        client_id = api_key_entry["APIKeyID"]

        if api_key == stored_key:  # Direct comparison instead of using Passlib
            request.session["api_key"] = api_key  # Store the API key in the session
            return client_id

    raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED, detail="Invalid API key")


def get_api_key_from_header(api_key: str = Header(None, name="Api-Key")):
    if not api_key:
        raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED, detail="Not authenticated")
    return api_key


@app.get('/api/data')
async def get_data(client_id: str = Depends(get_api_key)):
    # You can use client_id to fetch specific data for the client
    # ...

    return {"status": "success", "data": "Your data"}


@app.get('/api/pinepods_check')
async def pinepods_check():
    return {"status_code": 200, "pinepods_instance": True}


@app.post("/api/data/clean_expired_sessions/")
async def api_clean_expired_sessions(cnx=Depends(get_database_connection),
                                     api_key: str = Depends(get_api_key_from_header)):
    database_functions.functions.clean_expired_sessions(cnx)
    return {"status": "success"}


@app.get("/api/data/check_saved_session/{session_value}", response_model=int)
async def api_check_saved_session(session_value: str, cnx=Depends(get_database_connection),
                                  api_key: str = Depends(get_api_key_from_header)):
    result = database_functions.functions.check_saved_session(cnx, session_value)
    if result:
        return result
    else:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="No saved session found")


@app.get("/api/data/config")
async def api_config(api_key: str = Depends(get_api_key_from_header)):
    global api_url, proxy_url, proxy_host, proxy_port, proxy_protocol, reverse_proxy
    return {
        "api_url": api_url,
        "proxy_url": proxy_url,
        "proxy_host": proxy_host,
        "proxy_port": proxy_port,
        "proxy_protocol": proxy_protocol,
        "reverse_proxy": reverse_proxy,
    }


@app.get("/api/data/guest_status", response_model=bool)
async def api_guest_status(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header)):
    result = database_functions.functions.guest_status(cnx)
    return result


@app.get("/api/data/download_status", response_model=bool)
async def api_download_status(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header)):
    result = database_functions.functions.download_status(cnx)
    return result


@app.get("/api/data/user_details/{username}")
async def api_get_user_details(username: str, cnx=Depends(get_database_connection),
                               api_key: str = Depends(get_api_key_from_header)):
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
    database_functions.functions.create_session(cnx, user_id, session_data.session_token)
    return {"status": "success"}


class VerifyPasswordInput(BaseModel):
    username: str
    password: str


@app.post("/api/data/verify_password/")
async def api_verify_password(data: VerifyPasswordInput, cnx=Depends(get_database_connection),
                              api_key: str = Depends(get_api_key_from_header)):
    is_password_valid = Auth.Passfunctions.verify_password(cnx, data.username, data.password)
    return {"is_password_valid": is_password_valid}


@app.get("/api/data/return_episodes/{user_id}")
async def api_return_episodes(user_id: int, cnx=Depends(get_database_connection),
                              api_key: str = Depends(get_api_key_from_header)):
    episodes = database_functions.functions.return_episodes(cnx, user_id)
    if episodes is None:
        episodes = []  # Return an empty list instead of raising an exception
    return {"episodes": episodes}


@app.post("/api/data/check_episode_playback")
async def api_check_episode_playback(
        user_id: int = Form(...),
        episode_title: Optional[str] = Form(None),
        episode_url: Optional[str] = Form(None),
        cnx=Depends(get_database_connection),
        api_key: str = Depends(get_api_key_from_header)):
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


@app.get("/api/data/user_details_id/{user_id}")
async def api_get_user_details_id(user_id: int, cnx=Depends(get_database_connection),
                                  api_key: str = Depends(get_api_key_from_header)):
    result = database_functions.functions.get_user_details_id(cnx, user_id)
    if result:
        return result
    else:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="User not found")


@app.get("/api/data/get_theme/{user_id}")
async def api_get_theme(user_id: int, cnx=Depends(get_database_connection),
                        api_key: str = Depends(get_api_key_from_header)):
    theme = database_functions.functions.get_theme(cnx, user_id)
    return {"theme": theme}


@app.post("/api/data/add_podcast")
async def api_add_podcast(podcast_values: str = Form(...), user_id: int = Form(...),
                          cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header)):
    podcast_values = json.loads(podcast_values)
    result = database_functions.functions.add_podcast(cnx, podcast_values, user_id)
    if result:
        return {"success": True}
    else:
        return {"success": False}


@app.post("/api/data/enable_disable_guest")
async def api_enable_disable_guest(cnx=Depends(get_database_connection),
                                   api_key: str = Depends(get_api_key_from_header)):
    database_functions.functions.enable_disable_guest(cnx)
    return {"success": True}


@app.post("/api/data/enable_disable_downloads")
async def api_enable_disable_downloads(cnx=Depends(get_database_connection),
                                       api_key: str = Depends(get_api_key_from_header)):
    database_functions.functions.enable_disable_downloads(cnx)
    return {"success": True}


@app.post("/api/data/enable_disable_self_service")
async def api_enable_disable_self_service(cnx=Depends(get_database_connection),
                                          api_key: str = Depends(get_api_key_from_header)):
    database_functions.functions.enable_disable_self_service(cnx)
    return {"success": True}


@app.get("/api/data/self_service_status")
async def api_self_service_status(cnx=Depends(get_database_connection),
                                  api_key: str = Depends(get_api_key_from_header)):
    status = database_functions.functions.self_service_status(cnx)
    return {"status": status}


@app.put("/api/data/increment_listen_time/{user_id}")
async def api_increment_listen_time(user_id: int, cnx=Depends(get_database_connection),
                                    api_key: str = Depends(get_api_key_from_header)):
    database_functions.functions.increment_listen_time(cnx, user_id)
    return {"detail": "Listen time incremented."}


@app.put("/api/data/increment_played/{user_id}")
async def api_increment_played(user_id: int, cnx=Depends(get_database_connection),
                               api_key: str = Depends(get_api_key_from_header)):
    database_functions.functions.increment_played(cnx, user_id)
    return {"detail": "Played count incremented."}


class RecordHistoryData(BaseModel):
    episode_title: str
    user_id: int
    episode_pos: float


@app.post("/api/data/record_podcast_history")
async def api_record_podcast_history(data: RecordHistoryData, cnx=Depends(get_database_connection),
                                     api_key: str = Depends(get_api_key_from_header)):
    database_functions.functions.record_podcast_history(cnx, data.episode_title, data.user_id, data.episode_pos)
    return {"detail": "Podcast history recorded."}


class DownloadPodcastData(BaseModel):
    episode_url: str
    title: str
    user_id: int


@app.post("/api/data/download_podcast")
async def api_download_podcast(data: DownloadPodcastData, cnx=Depends(get_database_connection),
                               api_key: str = Depends(get_api_key_from_header)):
    result = database_functions.functions.download_podcast(cnx, data.episode_url, data.title, data.user_id)
    if result:
        return {"detail": "Podcast downloaded."}
    else:
        raise HTTPException(status_code=400, detail="Error downloading podcast.")


class DeletePodcastData(BaseModel):
    episode_url: str
    title: str
    user_id: int


@app.post("/api/data/delete_podcast")
async def api_delete_podcast(data: DeletePodcastData, cnx=Depends(get_database_connection),
                             api_key: str = Depends(get_api_key_from_header)):
    database_functions.functions.delete_podcast(cnx, data.episode_url, data.title, data.user_id)
    return {"detail": "Podcast deleted."}


class SaveEpisodeData(BaseModel):
    episode_url: str
    title: str
    user_id: int


@app.post("/api/data/save_episode")
async def api_save_episode(data: SaveEpisodeData, cnx=Depends(get_database_connection),
                           api_key: str = Depends(get_api_key_from_header)):
    success = database_functions.functions.save_episode(cnx, data.episode_url, data.title, data.user_id)
    if success:
        return {"detail": "Episode saved."}
    else:
        raise HTTPException(status_code=400, detail="Error saving episode.")


class RemoveSavedEpisodeData(BaseModel):
    episode_url: str
    title: str
    user_id: int


@app.post("/api/data/remove_saved_episode")
async def api_remove_saved_episode(data: RemoveSavedEpisodeData, cnx=Depends(get_database_connection),
                                   api_key: str = Depends(get_api_key_from_header)):
    database_functions.functions.remove_saved_episode(cnx, data.episode_url, data.title, data.user_id)
    return {"detail": "Saved episode removed."}


class RecordListenDurationData(BaseModel):
    episode_url: str
    title: str
    user_id: int
    listen_duration: float


@app.post("/api/data/record_listen_duration")
async def api_record_listen_duration(data: RecordListenDurationData, cnx=Depends(get_database_connection),
                                     api_key: str = Depends(get_api_key_from_header)):
    database_functions.functions.record_listen_duration(cnx, data.episode_url, data.title, data.user_id,
                                                        data.listen_duration)
    return {"detail": "Listen duration recorded."}


@app.get("/api/data/refresh_pods")
async def api_refresh_pods(background_tasks: BackgroundTasks, cnx=Depends(get_database_connection),
                           api_key: str = Depends(get_api_key_from_header)):
    background_tasks.add_task(database_functions.functions.refresh_pods, cnx)
    return {"detail": "Refresh initiated."}


@app.get("/api/data/get_stats")
async def api_get_stats(user_id: int, cnx=Depends(get_database_connection),
                        api_key: str = Depends(get_api_key_from_header)):
    stats = database_functions.functions.get_stats(cnx, user_id)
    return stats


@app.get("/api/data/get_user_episode_count")
async def api_get_user_episode_count(user_id: int, cnx=Depends(get_database_connection),
                                     api_key: str = Depends(get_api_key_from_header)):
    episode_count = database_functions.functions.get_user_episode_count(cnx, user_id)
    return episode_count


@app.get("/api/data/get_user_info")
async def api_get_user_info(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header)):
    user_info = database_functions.functions.get_user_info(cnx)
    return user_info


class CheckPodcastData(BaseModel):
    user_id: int
    podcast_name: str


@app.post("/api/data/check_podcast", response_model=Dict[str, bool])
async def api_check_podcast(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header),
                            data: CheckPodcastData = Body(...)):
    print(f"Received data: {data}")
    exists = database_functions.functions.check_podcast(cnx, data.user_id, data.podcast_name)
    return {"exists": exists}


@app.get("/api/data/user_admin_check/{user_id}")
async def api_user_admin_check_route(user_id: int, cnx=Depends(get_database_connection),
                                     api_key: str = Depends(get_api_key_from_header)):
    is_admin = database_functions.functions.user_admin_check(cnx, user_id)
    return {"is_admin": is_admin}


class RemovePodcastData(BaseModel):
    user_id: int
    podcast_name: str


@app.post("/api/data/remove_podcast")
async def api_remove_podcast_route(data: RemovePodcastData = Body(...), cnx=Depends(get_database_connection),
                                   api_key: str = Depends(get_api_key_from_header)):
    database_functions.functions.remove_podcast(cnx, data.podcast_name, data.user_id)
    return {"status": "Podcast removed"}


@app.get("/api/data/return_pods/{user_id}")
async def api_return_pods(user_id: int, cnx=Depends(get_database_connection),
                          api_key: str = Depends(get_api_key_from_header)):
    pods = database_functions.functions.return_pods(cnx, user_id)
    return {"pods": pods}


@app.get("/api/data/user_history/{user_id}")
async def api_user_history(user_id: int, cnx=Depends(get_database_connection),
                           api_key: str = Depends(get_api_key_from_header)):
    history = database_functions.functions.user_history(cnx, user_id)
    return {"history": history}


@app.get("/api/data/saved_episode_list/{user_id}")
async def api_saved_episode_list(user_id: int, cnx=Depends(get_database_connection),
                                 api_key: str = Depends(get_api_key_from_header)):
    saved_episodes = database_functions.functions.saved_episode_list(cnx, user_id)
    return {"saved_episodes": saved_episodes}


@app.post("/api/data/download_episode_list")
async def api_download_episode_list(cnx=Depends(get_database_connection),
                                    api_key: str = Depends(get_api_key_from_header), user_id: int = Form(...)):
    downloaded_episodes = database_functions.functions.download_episode_list(cnx, user_id)
    return {"downloaded_episodes": downloaded_episodes}


@app.post("/api/data/return_selected_episode")
async def api_return_selected_episode(cnx=Depends(get_database_connection),
                                      api_key: str = Depends(get_api_key_from_header), user_id: int = Body(...),
                                      title: str = Body(...), url: str = Body(...)):
    episode_info = database_functions.functions.return_selected_episode(cnx, user_id, title, url)
    return {"episode_info": episode_info}


@app.post("/api/data/check_usernames")
async def api_check_usernames(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header),
                              username: str = Body(...)):
    result = database_functions.functions.check_usernames(cnx, username)
    return {"username_exists": result}


class UserValues(BaseModel):
    fullname: str
    username: str
    email: str
    hash_pw: bytes
    salt: bytes


@app.post("/api/data/add_user")
async def api_add_user(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header),
                       user_values: UserValues = Body(...)):
    # Convert base64 strings back to bytes
    hash_pw_bytes = base64.b64decode(user_values.hash_pw)
    salt_bytes = base64.b64decode(user_values.salt)
    database_functions.functions.add_user(cnx, (
    user_values.fullname, user_values.username, user_values.email, hash_pw_bytes, salt_bytes))
    return {"detail": "User added."}


@app.put("/api/data/set_fullname/{user_id}")
async def api_set_fullname(user_id: int, new_name: str = Query(...), cnx=Depends(get_database_connection),
                           api_key: str = Depends(get_api_key_from_header)):
    database_functions.functions.set_fullname(cnx, user_id, new_name)
    return {"detail": "Fullname updated."}


@app.put("/api/data/set_password/{user_id}")
async def api_set_password(user_id: int, salt: str = Body(...), hash_pw: str = Body(...),
                           cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header)):
    database_functions.functions.set_password(cnx, user_id, salt, hash_pw)
    return {"detail": "Password updated."}


@app.put("/api/data/user/set_email")
async def api_set_email(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header),
                        user_id: int = Body(...), new_email: str = Body(...)):
    database_functions.functions.set_email(cnx, user_id, new_email)
    return {"detail": "Email updated."}


@app.put("/api/data/user/set_username")
async def api_set_username(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header),
                           user_id: int = Body(...), new_username: str = Body(...)):
    database_functions.functions.set_username(cnx, user_id, new_username)
    return {"detail": "Username updated."}


@app.put("/api/data/user/set_isadmin")
async def api_set_isadmin(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header),
                          user_id: int = Body(...), isadmin: bool = Body(...)):
    database_functions.functions.set_isadmin(cnx, user_id, isadmin)
    return {"detail": "IsAdmin status updated."}


@app.get("/api/data/user/final_admin/{user_id}")
async def api_final_admin(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header),
                          user_id: int = Path(...)):
    is_final_admin = database_functions.functions.final_admin(cnx, user_id)
    return {"final_admin": is_final_admin}


@app.delete("/api/data/user/delete/{user_id}")
async def api_delete_user(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header),
                          user_id: int = Path(...)):
    database_functions.functions.delete_user(cnx, user_id)
    return {"status": "User deleted"}


@app.put("/api/data/user/set_theme")
async def api_set_theme(user_id: int = Body(...), new_theme: str = Body(...), cnx=Depends(get_database_connection),
                        api_key: str = Depends(get_api_key_from_header)):
    database_functions.functions.set_theme(cnx, user_id, new_theme)
    return {"message": "Theme updated successfully"}


@app.get("/api/data/user/check_downloaded")
async def api_check_downloaded(user_id: int, title: str, url: str, cnx=Depends(get_database_connection),
                               api_key: str = Depends(get_api_key_from_header)):
    is_downloaded = database_functions.functions.check_downloaded(cnx, user_id, title, url)
    return {"is_downloaded": is_downloaded}


@app.get("/api/data/user/check_saved")
async def api_check_saved(user_id: int, title: str, url: str, cnx=Depends(get_database_connection),
                          api_key: str = Depends(get_api_key_from_header)):
    is_saved = database_functions.functions.check_saved(cnx, user_id, title, url)
    return {"is_saved": is_saved}


@app.post("/api/data/create_api_key")
async def api_create_api_key(user_id: int = Body(..., embed=True), cnx=Depends(get_database_connection),
                             api_key: str = Depends(get_api_key_from_header)):
    new_api_key = database_functions.functions.create_api_key(cnx, user_id)
    return {"api_key": new_api_key}


@app.post("/api/data/save_email_settings")
async def api_save_email_settings(email_settings: dict = Body(..., embed=True), cnx=Depends(get_database_connection),
                                  api_key: str = Depends(get_api_key_from_header)):
    database_functions.functions.save_email_settings(cnx, email_settings)
    return {"message": "Email settings saved."}


@app.get("/api/data/get_encryption_key")
async def api_get_encryption_key(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header)):
    encryption_key = database_functions.functions.get_encryption_key(cnx)
    return {"encryption_key": encryption_key}


@app.get("/api/data/get_email_settings")
async def api_get_email_settings(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header)):
    email_settings = database_functions.functions.get_email_settings(cnx)
    return email_settings


@app.delete("/api/data/delete_api_key/{api_id}")
async def api_delete_api_key(api_id: int, cnx=Depends(get_database_connection),
                             api_key: str = Depends(get_api_key_from_header)):
    database_functions.functions.delete_api(cnx, api_id)
    return {"detail": "API key deleted."}


@app.get("/api/data/get_api_info")
async def api_get_api_info(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header)):
    api_information = database_functions.functions.get_api_info(cnx)
    return {"api_info": api_information}


class ResetPasswordPayload(BaseModel):
    email: str
    reset_code: str


@app.post("/api/data/reset_password_create_code")
async def api_reset_password_route(payload: ResetPasswordPayload, cnx=Depends(get_database_connection),
                                   api_key: str = Depends(get_api_key_from_header)):
    user_exists = database_functions.functions.reset_password_create_code(cnx, payload.email, payload.reset_code)
    return {"user_exists": user_exists}


@app.post("/api/data/verify_reset_code")
async def api_verify_reset_code_route(payload: ResetPasswordPayload, cnx=Depends(get_database_connection),
                                      api_key: str = Depends(get_api_key_from_header)):
    code_valid = database_functions.functions.verify_reset_code(cnx, payload.email, payload.reset_code)
    if code_valid is None:
        raise HTTPException(status_code=404, detail="User not found")
    return {"code_valid": code_valid}


class ResetPasswordPayloadVerify(BaseModel):
    email: str
    salt: str
    hashed_pw: str


@app.post("/api/data/reset_password_prompt")
async def api_reset_password_verify_route(payload: ResetPasswordPayloadVerify, cnx=Depends(get_database_connection),
                                          api_key: str = Depends(get_api_key_from_header)):
    message = database_functions.functions.reset_password_prompt(cnx, payload.email, payload.salt, payload.hashed_pw)
    if message is None:
        raise HTTPException(status_code=404, detail="User not found")
    return {"message": message}


@app.post("/api/data/clear_guest_data")
async def api_clear_guest_data(cnx=Depends(get_database_connection), api_key: str = Depends(get_api_key_from_header)):
    message = database_functions.functions.clear_guest_data(cnx)
    if message is None:
        raise HTTPException(status_code=404, detail="User not found")
    return {"message": message}


class EpisodeMetadata(BaseModel):
    episode_url: str
    episode_title: str
    user_id: int


@app.post("/api/data/get_episode_metadata")
async def api_get_episode_metadata(data: EpisodeMetadata, cnx=Depends(get_database_connection),
                                   api_key: str = Depends(get_api_key_from_header)):
    episode = database_functions.functions.get_episode_metadata(cnx, data.episode_url, data.episode_title, data.user_id)
    return {"episode": episode}


class MfaSecretData(BaseModel):
    user_id: int
    mfa_secret: str


@app.post("/api/data/save_mfa_secret")
async def api_save_mfa_secret(data: MfaSecretData, cnx=Depends(get_database_connection),
                              api_key: str = Depends(get_api_key_from_header)):
    success = database_functions.functions.save_mfa_secret(cnx, data.user_id, data.mfa_secret)
    if success:
        return {"status": "success"}
    else:
        return {"status": "error"}


@app.get("/api/data/check_mfa_enabled/{user_id}")
async def api_check_mfa_enabled(user_id: int, cnx=Depends(get_database_connection),
                                api_key: str = Depends(get_api_key_from_header)):
    is_enabled = database_functions.functions.check_mfa_enabled(cnx, user_id)
    return {"mfa_enabled": is_enabled}


class VerifyMFABody(BaseModel):
    user_id: int
    mfa_code: str


@app.post("/api/data/verify_mfa")
async def api_verify_mfa(body: VerifyMFABody, cnx=Depends(get_database_connection),
                         api_key: str = Depends(get_api_key_from_header)):
    secret = database_functions.functions.get_mfa_secret(cnx, body.user_id)

    if secret is None:
        return {"verified": False}
    else:
        totp = TOTP(secret)
        verification_result = totp.verify(body.mfa_code)
        return {"verified": verification_result}

    if response.status_code == 200:
        return response.json().get('deleted', False)

    return False


class UserIDBody(BaseModel):
    user_id: int


@app.delete("/api/data/delete_mfa")
async def api_delete_mfa(body: UserIDBody, cnx=Depends(get_database_connection),
                         api_key: str = Depends(get_api_key_from_header)):
    result = database_functions.functions.delete_mfa_secret(cnx, body.user_id)
    return {"deleted": result}


class AllEpisodes(BaseModel):
    pod_feed: str


@app.post("/api/data/get_all_episodes")
async def api_get_episodes(data: AllEpisodes, cnx=Depends(get_database_connection),
                           api_key: str = Depends(get_api_key_from_header)):
    episodes = database_functions.functions.get_all_episodes(cnx, data.pod_feed)
    return {"episodes": episodes}


class EpisodeToRemove(BaseModel):
    url: str
    title: str
    user_id: int


@app.post("/api/data/remove_episode_history")
async def api_remove_episode_from_history(data: EpisodeToRemove, cnx=Depends(get_database_connection),
                                          api_key: str = Depends(get_api_key_from_header)):
    success = database_functions.functions.remove_episode_history(cnx, data.url, data.title, data.user_id)
    return {"success": success}


# Model for request data
class TimeZoneInfo(BaseModel):
    user_id: int
    timezone: str
    hour_pref: int


# FastAPI endpoint
@app.post("/api/data/setup_time_info")
async def setup_timezone_info(data: TimeZoneInfo, cnx=Depends(get_database_connection),
                              api_key: str = Depends(get_api_key_from_header)):
    success = database_functions.functions.setup_timezone_info(cnx, data.user_id, data.timezone, data.hour_pref)
    return {"success": success}


@app.get("/api/data/get_time_info")
async def get_time_info(user_id: int, cnx=Depends(get_database_connection),
                        api_key: str = Depends(get_api_key_from_header)):
    timezone, hour_pref = database_functions.functions.get_time_info(cnx, user_id)
    return {"timezone": timezone, "hour_pref": hour_pref}


class UserLoginUpdate(BaseModel):
    user_id: int


@app.post("/api/data/first_login_done")
async def first_login_done(data: UserLoginUpdate, cnx=Depends(get_database_connection),
                           api_key: str = Depends(get_api_key_from_header)):
    first_login_status = database_functions.functions.first_login_done(cnx, data.user_id)
    return {"FirstLogin": first_login_status}


class SelectedEpisodesDelete(BaseModel):
    selected_episodes: List[int] = Field(..., title="List of Episode IDs")
    user_id: int = Field(..., title="User ID")


@app.post("/api/data/delete_selected_episodes")
async def delete_selected_episodes(data: SelectedEpisodesDelete, cnx=Depends(get_database_connection),
                                   api_key: str = Depends(get_api_key_from_header)):
    status = database_functions.functions.delete_selected_episodes(cnx, data.selected_episodes, data.user_id)
    return {"status": status}


class SelectedPodcastsDelete(BaseModel):
    delete_list: List[int] = Field(..., title="List of Podcast IDs")
    user_id: int = Field(..., title="User ID")


@app.post("/api/data/delete_selected_podcasts")
async def delete_selected_podcasts(data: SelectedPodcastsDelete, cnx=Depends(get_database_connection),
                                   api_key: str = Depends(get_api_key_from_header)):
    status = database_functions.functions.delete_selected_podcasts(cnx, data.delete_list, data.user_id)
    return {"status": status}


class SearchPodcastData(BaseModel):
    search_term: str
    user_id: int


@app.post("/api/data/search_data")
async def search_data(data: SearchPodcastData, cnx=Depends(get_database_connection),
                      api_key: str = Depends(get_api_key_from_header)):
    result = database_functions.functions.search_data(cnx, data.search_term, data.user_id)
    return {"data": result}


class QueuePodData(BaseModel):
    episode_title: str
    ep_url: str
    user_id: int


@app.post("/api/data/queue_pod")
async def queue_pod(data: QueuePodData, cnx=Depends(get_database_connection),
                    api_key: str = Depends(get_api_key_from_header)):
    result = database_functions.functions.queue_pod(cnx, data.episode_title, data.ep_url, data.user_id)
    return {"data": result}


class QueueRmData(BaseModel):
    episode_title: str
    ep_url: str
    user_id: int


@app.post("/api/data/remove_queued_pod")
async def remove_queued_pod(data: QueueRmData, cnx=Depends(get_database_connection),
                            api_key: str = Depends(get_api_key_from_header)):
    result = database_functions.functions.remove_queued_pod(cnx, data.episode_title, data.ep_url, data.user_id)
    return {"data": result}


class QueuedEpisodesData(BaseModel):
    user_id: int


@app.get("/api/data/get_queued_episodes")
async def get_queued_episodes(data: QueuedEpisodesData, cnx=Depends(get_database_connection),
                              api_key: str = Depends(get_api_key_from_header)):
    result = database_functions.functions.get_queued_episodes(cnx, data.user_id)
    return {"data": result}


class QueueBump(BaseModel):
    ep_url: str
    title: str
    user_id: int


@app.post("/api/data/queue_bump")
async def queue_bump(data: QueueBump, cnx=Depends(get_database_connection),
                     api_key: str = Depends(get_api_key_from_header)):
    try:
        print(data)
        result = database_functions.functions.queue_bump(cnx, data.ep_url, data.title, data.user_id)
    except Exception as e:
        raise HTTPException(status_code=400, detail=str(e))
    return {"data": result}


class BackupUser(BaseModel):
    user_id: int


@app.post("/api/data/backup_user", response_class=PlainTextResponse)
async def backup_user(data: BackupUser, cnx=Depends(get_database_connection)):
    try:
        opml_data = database_functions.functions.backup_user(cnx, data.user_id)
    except Exception as e:
        raise HTTPException(status_code=400, detail=str(e))
    return opml_data


class BackupServer(BaseModel):
    backup_dir: str
    database_pass: str


@app.get("/api/data/backup_server", response_class=PlainTextResponse)
async def backup_server(data: BackupServer, cnx=Depends(get_database_connection)):
    try:
        dump_data = database_functions.functions.backup_server(cnx, data.database_pass)
    except Exception as e:
        raise HTTPException(status_code=400, detail=str(e))
    return dump_data


# connected_clients = []
#
# @app.websocket("/api/data/ws")
# async def websocket_endpoint(websocket: WebSocket):
#     await websocket.accept()
#     connected_clients.append(websocket)
#     try:
#         while True:
#             data = await websocket.receive_text()
#             if data == "ping":
#                 await websocket.send_text("pong")
#     except:
#         connected_clients.remove(websocket)
#
# async def run_refresh_pods():
#     with get_database_connection() as db:
#         try:
#             database_functions.functions.refresh_pods(db)
#             for client in connected_clients:
#                 await client.send_text("refreshed")
#         except Exception as e:
#             logging.error(f"Error during refresh: {e}")
#
#
#
#
# @app.post("/api/data/start-refresh")
# async def start_refresh(background_tasks: BackgroundTasks):
#     background_tasks.add_task(run_refresh_pods)
#     return {"message": "Refresh started in the background"}
#
#
# def periodic_refresh():
#     print('starting scheduled refresh')
#
#     # First, create a new event loop and execute the refresh immediately on boot
#     loop = asyncio.new_event_loop()
#     asyncio.set_event_loop(loop)
#     loop.run_until_complete(run_refresh_pods())
#     loop.close()
#
#     # Now start the periodic (once per hour) refresh loop
#     while True:
#         time.sleep(3600)  # Sleep for an hour
#
#         loop = asyncio.new_event_loop()
#         asyncio.set_event_loop(loop)
#         loop.run_until_complete(run_refresh_pods())
#         loop.close()
#
#
# def start_periodic_refresh():
#     refresh_thread = threading.Thread(target=periodic_refresh)
#     refresh_thread.start()
#
# # Then, somewhere at the start of your program or app initialization:
# start_periodic_refresh()


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--port', type=int, default=8032, help='Port to run the server on')
    args = parser.parse_args()

    import uvicorn

    uvicorn.run("clientapi:app", host="0.0.0.0", port=args.port)
