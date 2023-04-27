from fastapi import FastAPI, Depends, HTTPException, status, Request, Header
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

secret_key_middle = secrets.token_hex(32)



from database_functions import functions

print('Client API Server is Starting!')

app = FastAPI()
app.add_middleware(GZipMiddleware, minimum_size=1000)
app.add_middleware(SessionMiddleware, secret_key=secret_key_middle)

API_KEY_NAME = "pinepods_api"
api_key_header = APIKeyHeader(name=API_KEY_NAME, auto_error=False)

pwd_context = CryptContext(schemes=["bcrypt"], deprecated="auto")



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

@app.get("/api/data/check_saved_session/", response_model=int)
async def api_check_saved_session(api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    result = database_functions.functions.check_saved_session(cnx)
    if result:
        return result
    else:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="No saved session found")

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

@app.post("/api/data/create_session/{user_id}")
async def api_create_session(user_id: int, api_key: str = Depends(get_api_key_from_header)):
    cnx = get_database_connection()
    database_functions.functions.create_session(cnx, user_id)
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
    if episodes:
        return {"episodes": episodes}
    else:
        raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="No episodes found")

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




if __name__ == '__main__':
    import uvicorn
    uvicorn.run("clientapi:app", host="0.0.0.0", port=8032)
