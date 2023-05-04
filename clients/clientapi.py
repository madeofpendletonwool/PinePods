from fastapi import FastAPI, Depends, HTTPException, status, Request, Header, Body, Path
from fastapi.security import APIKeyHeader, HTTPBasic, HTTPBasicCredentials
from passlib.context import CryptContext
import mysql.connector
from mysql.connector import pooling
import os
from datetime import datetime
from fastapi.middleware.gzip import GZipMiddleware
from starlette.middleware.sessions import SessionMiddleware
import secrets
import requests
import database_functions.functions
import Auth.Passfunctions
from pydantic import BaseModel
from typing import Dict
from typing import List

secret_key_middle = secrets.token_hex(32)



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

#Initial Vars needed to start and used throughout
if reverse_proxy == "True":
    proxy_url = f'{proxy_protocol}://{proxy_host}/proxy?url='
else:
    proxy_url = f'{proxy_protocol}://{proxy_host}:{proxy_port}/proxy?url='
print(f'Proxy url is configured to {proxy_url}')

def get_database_connection():
    return connection_pool.get_connection()


def setup_connection_pool():
    db_host = os.environ.get("DB_HOST", "127.0.0.1")
    db_port = os.environ.get("DB_PORT", "3306")
    db_user = os.environ.get("DB_USER", "root")
    db_password = os.environ.get("DB_PASSWORD", "password")
    db_name = os.environ.get("DB_NAME", "pypods_database")

    return pooling.MySQLConnectionPool(
        pool_name="pinepods_api_pool",
        pool_size=25,  # Adjust the pool size according to your needs
        pool_reset_session=True,
        host=db_host,
        port=db_port,
        user=db_user,
        password=db_password,
        database=db_name,
    )

connection_pool = setup_connection_pool()

def get_api_keys(cnx):
    cursor = cnx.cursor(dictionary=True)
    query = "SELECT * FROM APIKeys"
    cursor.execute(query)
    rows = cursor.fetchall()
    cursor.close()
    return rows

def get_api_key(request: Request, api_key: str = Depends(api_key_header)):
    if api_key is None:
        raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED, detail="API key is missing")

    cnx = get_database_connection()
    api_keys = get_api_keys(cnx)
    cnx.close()

    for api_key_entry in api_keys:
        stored_key = api_key_entry["APIKey"]
        client_id = api_key_entry["APIKeyID"]

        if api_key == stored_key:  # Direct comparison instead of using Passlib
            request.session["api_key"] = api_key  # Store the API key in the session
            return client_id

    raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED, detail="Invalid API key")

def get_api_key_from_header(api_key: str = Header(None, name="Api-Key")):
    print("Received API Key:", api_key)  # Debugging 
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
async def api_clean_expired_sessions(api_key: str = Depends(get_api_key_from_header)):
    print(f'in clean expired post {api_key}')
    cnx = get_database_connection()
    database_functions.functions.clean_expired_sessions(cnx)
    return {"status": "success"}

@app.get("/api/data/check_saved_session/{session_value}", response_model=int)
async def api_check_saved_session(session_value: str, api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
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
async def api_guest_status(api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    result = database_functions.functions.guest_status(cnx)
    return result

@app.get("/api/data/user_details/{username}")
async def api_get_user_details(username: str, api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    result = database_functions.functions.get_user_details(cnx, username)
    if result:
        return result
    else:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="User not found")

class SessionData(BaseModel):
    session_token: str

@app.post("/api/data/create_session/{user_id}")
async def api_create_session(user_id: int, session_data: SessionData, api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    database_functions.functions.create_session(cnx, user_id, session_data.session_token)
    return {"status": "success"}

class VerifyPasswordInput(BaseModel):
    username: str
    password: str

@app.post("/api/data/verify_password/")
async def api_verify_password(data: VerifyPasswordInput, api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    is_password_valid = Auth.Passfunctions.verify_password(cnx, data.username, data.password)
    return {"is_password_valid": is_password_valid}

@app.get("/api/data/return_episodes/{user_id}")
async def api_return_episodes(user_id: int, api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    episodes = database_functions.functions.return_episodes(cnx, user_id)
    if episodes is None:
        episodes = []  # Return an empty list instead of raising an exception
    return {"episodes": episodes}


@app.post("/api/data/check_episode_playback")
async def api_check_episode_playback(
    user_id: int,
    episode_title: str,
    episode_url: str,
    api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    has_playback, listen_duration = database_functions.functions.check_episode_playback(
        cnx, user_id, episode_title, episode_url
    )
    if has_playback:
        return {"has_playback": True, "listen_duration": listen_duration}
    else:
        return {"has_playback": False, "listen_duration": 0}

@app.get("/api/data/user_details_id/{user_id}")
async def api_get_user_details_id(user_id: int, api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    result = database_functions.functions.get_user_details_id(cnx, user_id)
    if result:
        return result
    else:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="User not found")

@app.get("/api/data/get_theme/{user_id}")
async def api_get_theme(user_id: int, api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    theme = database_functions.functions.get_theme(cnx, user_id)
    return {"theme": theme}

@app.post("/api/data/add_podcast")
async def api_add_podcast(podcast_values: List[str], user_id: int, api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    result = database_functions.functions.add_podcast(cnx, podcast_values, user_id)
    if result:
        return {"success": True}
    else:
        raise HTTPException(status_code=status.HTTP_400_BAD_REQUEST, detail="Podcast already exists for the user")

@app.post("/api/data/enable_disable_guest")
async def api_enable_disable_guest(api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    database_functions.functions.enable_disable_guest(cnx)
    return {"success": True}

@app.post("/api/data/enable_disable_self_service")
async def api_enable_disable_self_service(api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    database_functions.functions.enable_disable_self_service(cnx)
    return {"success": True}

@app.get("/api/data/self_service_status")
async def api_self_service_status(api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    status = database_functions.functions.self_service_status(cnx)
    return {"status": status}

@app.put("/api/data/increment_listen_time/{user_id}")
async def api_increment_listen_time(user_id: int, api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    database_functions.functions.increment_listen_time(cnx, user_id)
    return {"detail": "Listen time incremented."}

@app.put("/api/data/increment_played/{user_id}")
async def api_increment_played(user_id: int, api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    database_functions.functions.increment_played(cnx, user_id)
    return {"detail": "Played count incremented."}


class RecordHistoryData(BaseModel):
    episode_title: str
    user_id: int
    episode_pos: float

@app.post("/api/data/record_podcast_history")
async def api_record_podcast_history(data: RecordHistoryData, api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    database_functions.functions.record_podcast_history(cnx, data.episode_title, data.user_id, data.episode_pos)
    return {"detail": "Podcast history recorded."}

class DownloadPodcastData(BaseModel):
    episode_url: str
    title: str
    user_id: int

@app.post("/api/data/download_podcast")
async def api_download_podcast(data: DownloadPodcastData, api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
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
async def api_delete_podcast(data: DeletePodcastData, api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    database_functions.functions.delete_podcast(cnx, data.episode_url, data.title, data.user_id)
    return {"detail": "Podcast deleted."}

class SaveEpisodeData(BaseModel):
    episode_url: str
    title: str
    user_id: int

@app.post("/api/data/save_episode")
async def api_save_episode(data: SaveEpisodeData, api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
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
async def api_remove_saved_episode(data: RemoveSavedEpisodeData, api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    database_functions.functions.remove_saved_episode(cnx, data.episode_url, data.title, data.user_id)
    return {"detail": "Saved episode removed."}

class RecordListenDurationData(BaseModel):
    episode_url: str
    title: str
    user_id: int
    listen_duration: float

@app.post("/api/data/record_listen_duration")
async def api_record_listen_duration(data: RecordListenDurationData, api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    database_functions.functions.record_listen_duration(cnx, data.episode_url, data.title, data.user_id, data.listen_duration)
    return {"detail": "Listen duration recorded."}

@app.get("/api/data/refresh_pods")
async def api_refresh_pods(api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    database_functions.functions.refresh_pods(cnx)
    return {"detail": "Podcasts refreshed."}

@app.get("/api/data/get_stats")
async def api_get_stats(user_id: int, api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    stats = database_functions.functions.get_stats(cnx, user_id)
    return stats

@app.get("/api/data/get_user_episode_count")
async def api_get_user_episode_count(user_id: int, api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    episode_count = database_functions.functions.get_user_episode_count(cnx, user_id)
    return episode_count

@app.get("/api/data/get_user_info")
async def api_get_user_info(api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    user_info = database_functions.functions.get_user_info(cnx)
    return user_info

class CheckPodcastData(BaseModel):
    user_id: int
    podcast_name: str

@app.post("/api/data/check_podcast", response_model=Dict[str, bool])
async def api_check_podcast(api_key: str = Depends(get_api_key_from_header), data: CheckPodcastData = Depends()):
    cnx = get_database_connection()
    exists = database_functions.functions.check_podcast(cnx, data.user_id, data.podcast_name)
    return {"exists": exists}

@app.get("/api/user_admin_check/{user_id}")
async def api_user_admin_check_route(user_id: int, api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    is_admin = database_functions.functions.user_admin_check(cnx, user_id)
    return {"is_admin": is_admin}

@app.post("/api/remove_podcast")
async def api_remove_podcast_route(podcast_name: str, user_id: int, api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    database_functions.functions.remove_podcast(cnx, podcast_name, user_id)
    return {"status": "Podcast removed"}

@app.get("/api/return_pods/{user_id}")
async def api_return_pods(user_id: int, api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    pods = database_functions.functions.return_pods(cnx, user_id)
    return {"pods": pods}

@app.get("/api/user_history/{user_id}")
async def api_user_history(user_id: int, api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    history = database_functions.functions.user_history(cnx, user_id)
    return {"history": history}

@app.get("/api/saved_episode_list/{user_id}")
async def api_saved_episode_list(user_id: int, api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    saved_episodes = database_functions.functions.saved_episode_list(cnx, user_id)
    return {"saved_episodes": saved_episodes}

@app.post("/api/download_episode_list")
async def api_download_episode_list(api_key: str = Depends(get_api_key_from_header), user_id: int = Body(...)):
    cnx = get_database_connection()
    downloaded_episodes = database_functions.functions.download_episode_list(cnx, user_id)
    return {"downloaded_episodes": downloaded_episodes}

@app.post("/api/get_queue_list")
async def api_get_queue_list(api_key: str = Depends(get_api_key_from_header), queue_urls: List[str] = Body(...)):
    cnx = get_database_connection()
    queue_list = database_functions.functions.get_queue_list(cnx, queue_urls)
    return {"queue_list": queue_list}

@app.post("/api/return_selected_episode")
async def api_return_selected_episode(api_key: str = Depends(get_api_key_from_header), user_id: int = Body(...), title: str = Body(...), url: str = Body(...)):
    cnx = get_database_connection()
    episode_info = database_functions.functions.return_selected_episode(cnx, user_id, title, url)
    return {"episode_info": episode_info}

@app.post("/api/check_usernames")
async def api_check_usernames(api_key: str = Depends(get_api_key_from_header), username: str = Body(...)):
    cnx = get_database_connection()
    result = database_functions.functions.check_usernames(cnx, username)
    return {"username_exists": result}

@app.post("/api/add_user")
async def api_add_user(api_key: str = Depends(get_api_key_from_header), user_values: List[str] = Body(...)):
    cnx = get_database_connection()
    database_functions.functions.add_user(cnx, tuple(user_values))
    return {"detail": "User added."}

@app.put("/api/set_fullname/{user_id}")
async def api_set_fullname(user_id: int, new_name: str = Body(...), api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    database_functions.functions.set_fullname(cnx, user_id, new_name)
    return {"detail": "Fullname updated."}

@app.put("/api/set_password/{user_id}")
async def api_set_password(user_id: int, salt: str = Body(...), hash_pw: str = Body(...), api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    database_functions.functions.set_password(cnx, user_id, salt, hash_pw)
    return {"detail": "Password updated."}

@app.put("/api/user/set_email")
async def api_set_email(api_key: str = Depends(get_api_key_from_header), user_id: int = Body(...), new_email: str = Body(...)):
    cnx = get_database_connection()
    database_functions.functions.set_email(cnx, user_id, new_email)
    return {"detail": "Email updated."}

@app.put("/api/user/set_username")
async def api_set_username(api_key: str = Depends(get_api_key_from_header), user_id: int = Body(...), new_username: str = Body(...)):
    cnx = get_database_connection()
    database_functions.functions.set_username(cnx, user_id, new_username)
    return {"detail": "Username updated."}

@app.put("/api/user/set_isadmin")
async def api_set_isadmin(api_key: str = Depends(get_api_key_from_header), user_id: int = Body(...), isadmin: bool = Body(...)):
    cnx = get_database_connection()
    database_functions.functions.set_isadmin(cnx, user_id, isadmin)
    return {"detail": "IsAdmin status updated."}

@app.get("/api/user/final_admin/{user_id}")
async def api_final_admin(api_key: str = Depends(get_api_key_from_header), user_id: int = Path(...)):
    cnx = get_database_connection()
    is_final_admin = database_functions.functions.final_admin(cnx, user_id)
    return {"final_admin": is_final_admin}

@app.delete("/api/user/delete/{user_id}")
async def api_delete_user(api_key: str = Depends(get_api_key_from_header), user_id: int = Path(...)):
    cnx = get_database_connection()
    database_functions.functions.delete_user(cnx, user_id)
    return {"status": "User deleted"}

@app.put("/api/user/set_theme")
async def api_set_theme(user_id: int, new_theme: str, cnx=Depends(get_database_connection)):
    database_functions.functions.set_theme(cnx, user_id, new_theme)
    return {"message": "Theme updated successfully"}

@app.get("/api/user/check_downloaded")
async def api_check_downloaded(user_id: int, title: str, url: str, cnx=Depends(get_database_connection)):
    is_downloaded = database_functions.functions.check_downloaded(cnx, user_id, title, url)
    return {"is_downloaded": is_downloaded}

@app.get("/api/user/check_saved")
async def api_check_saved(user_id: int, title: str, url: str, cnx=Depends(get_database_connection)):
    is_saved = database_functions.functions.check_saved(cnx, user_id, title, url)
    return {"is_saved": is_saved}




if __name__ == '__main__':
    import uvicorn
    uvicorn.run("clientapi:app", host="0.0.0.0", port=8032)
