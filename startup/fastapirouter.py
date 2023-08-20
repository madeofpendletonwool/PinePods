from concurrent.futures import ThreadPoolExecutor
from fastapi import FastAPI, Request, HTTPException, Response, WebSocket
import httpx
import logging
import websockets
from fastapi.middleware.cors import CORSMiddleware
from starlette.responses import StreamingResponse, FileResponse
from PIL import Image, UnidentifiedImageError
import io


app = FastAPI()
app.add_middleware(
    CORSMiddleware,
    allow_origins=["http://localhost:8040", "http://localhost:8032", "http://localhost:8034", "http://localhost:8000"],  # replace <FRONTEND_PORT> with the port of your frontend app
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)
logging.basicConfig(level=logging.INFO)


@app.api_route("/api/{api_path:path}", methods=["GET", "POST", "PUT", "DELETE"])
async def proxy_api_requests(request: Request, api_path: str):
    async with httpx.AsyncClient() as client:
        # Filter out headers that might conflict with the internal service
        headers = {k: v for k, v in request.headers.items() if k not in ["Host", "Connection"]}

        try:
            print(request.method, f"http://localhost:8032/api/{api_path}", headers, request.cookies, request.body)
            response = await client.request(
                request.method,
                f"http://localhost:8032/api/{api_path}",
                headers=headers,
                cookies=request.cookies,
                data=await request.body(),
            )
            print(response.status_code, response.text)
        except httpx.HTTPError as exc:
            print(f"An error occurred while making the request: {exc}")
            return Response(content=f"Proxy Error: {exc}", status_code=502)

        return Response(content=response.content, status_code=response.status_code, headers=dict(response.headers))


@app.api_route("/proxy/", methods=["GET", "POST", "PUT", "DELETE"])
async def proxy_image_requests(request: Request):
    url = request.query_params.get("url")

    if not url:
        return Response(content="URL parameter missing.", status_code=400)

    print(url)
    headers = {k: v for k, v in request.headers.items() if k not in ["Host", "Connection"]}
    async with httpx.AsyncClient() as client:
        try:
            response = await client.request(
                request.method,
                f"http://localhost:8000/proxy?url={url}",
                headers=headers,
                cookies=request.cookies,
                data=await request.body(),
            )
            print(response.status_code, response.text)
        except httpx.HTTPError as exc:
            print(f"An error occurred while making the request: {exc}")
            return Response(content=f"Proxy Error: {exc}", status_code=502)

        return Response(content=response.content, status_code=response.status_code, headers=dict(response.headers))


@app.api_route("/{path:path}", methods=["GET", "POST", "PUT", "DELETE"])
async def proxy_requests(request: Request, path: str):
    print("Entered /main route")
    headers = {k: v for k, v in request.headers.items() if k not in ["Host", "Connection"]}
    async with httpx.AsyncClient() as client:
        try:
            print(request.method, f"http://localhost:8034/{path}", headers, request.cookies, request.body)
            response = await client.request(
                request.method,
                f"http://localhost:8034/{path}",
                headers=headers,
                cookies=request.cookies,
                data=await request.body(),
            )
            print(response.status_code, response.text)
        except httpx.HTTPError as exc:
            print(f"An error occurred while making the request: {exc}")
            return Response(content=f"Proxy Error: {exc}", status_code=502)

        return Response(content=response.content, status_code=response.status_code, headers=dict(response.headers))


import asyncio

@app.websocket("/ws")
async def websocket_endpoint(websocket: WebSocket):
    await websocket.accept()
    # You can also add authentication here if needed.
    remote_websocket = await websockets.connect("ws://localhost:8034/ws")
    try:
        async def forward(local_ws, remote_ws):
            while True:
                data = await local_ws.receive_text()
                await remote_ws.send(data)

        async def backward(local_ws, remote_ws):
            while True:
                remote_data = await remote_ws.recv()
                await local_ws.send_text(remote_data)

        await asyncio.gather(forward(websocket, remote_websocket), backward(websocket, remote_websocket))

    except websockets.ConnectionClosed as e:
        logging.info(f"Connection closed: {e}")
    except Exception as e:
        logging.error(f"Error in websocket: {e}")
    finally:
        await remote_websocket.close()


if __name__ == '__main__':
    import uvicorn
    uvicorn.run("fastapirouter:app", host="0.0.0.0", port=80)
