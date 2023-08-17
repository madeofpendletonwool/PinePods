from fastapi import FastAPI, Request, HTTPException, Response
import httpx
import logging
print('starting fastapi')

app = FastAPI()

@app.get("/")
def read_root():
    return {"Hello": "World"}

if __name__ == '__main__':
    import uvicorn
    uvicorn.run("fastapirouter:app", host="0.0.0.0", port=80)
