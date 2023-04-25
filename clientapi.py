from fastapi import FastAPI, Depends, HTTPException, status
from fastapi.security import APIKeyHeader
from passlib.context import CryptContext
import mysql.connector
from mysql.connector import pooling
import os
from fastapi.middleware.gzip import GZipMiddleware


from database_functions import functions

print('Client API Server is Starting!')

app = FastAPI()
app.add_middleware(GZipMiddleware, minimum_size=1000)

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
        pool_size=5,  # Adjust the pool size according to your needs
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

def get_api_key(api_key: str = Depends(api_key_header)):
    if api_key is None:
        raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED, detail="API key is missing")

    cnx = get_database_connection()
    api_keys = get_api_keys(cnx)
    cnx.close()

    for api_key_entry in api_keys:
        hashed_key = api_key_entry["APIKey"]
        client_id = api_key_entry["APIKeyID"]

        if pwd_context.verify(api_key, hashed_key):
            return client_id

    raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED, detail="Invalid API key")

@app.get('/api/data')
async def get_data(client_id: str = Depends(get_api_key)):
    # You can use client_id to fetch specific data for the client
    # ...

    return {"status": "success", "data": "Your data"}


if __name__ == '__main__':
    import uvicorn
    uvicorn.run("clientapi:app", host="0.0.0.0", port=8032, reload=True)
