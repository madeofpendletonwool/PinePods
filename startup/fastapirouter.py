from fastapi import FastAPI, Request, HTTPException, Response, WebSocket
import httpx
import logging
import websockets
from fastapi.middleware.cors import CORSMiddleware
import gzip
from io import BytesIO
import os
import uvicorn

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

        # Build the URL for the proxy request, including the original query parameters
        proxy_url = f"http://localhost:8032/api/{api_path}"
        if request.query_params:
            proxy_url += f"?{request.query_params}"

        try:
            response = await client.request(
                request.method,
                proxy_url,
                headers=headers,
                cookies=request.cookies,
                data=await request.body(),
            )

            # Check if the response is gzipped
            if response.headers.get("Content-Encoding") == "gzip":
                buffer = BytesIO(response.content)
                try:
                    with gzip.GzipFile(fileobj=buffer, mode='rb') as f:
                        decompressed_content = f.read()
                except gzip.BadGzipFile:
                    decompressed_content = response.content
            else:
                decompressed_content = response.content

            # Exclude the 'Content-Length' and 'Content-Encoding' from the forwarded headers
            forward_headers = {
                k: v for k, v in response.headers.items()
                if k.lower() not in ["content-length", "content-encoding"]
            }

            return Response(
                content=decompressed_content,
                status_code=response.status_code,
                headers=forward_headers
            )
        except httpx.HTTPError as exc:
            print(f"An error occurred while making the request: {exc}")
            return Response(content=f"Proxy Error: {exc}", status_code=502)


@app.api_route("/mover/", methods=["GET", "POST", "PUT", "DELETE"])
async def proxy_image_requests(request: Request):
    print("Entered /proxy route")
    url = request.query_params.get("url")

    if not url:
        return Response(content="URL parameter missing.", status_code=400)

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
        except httpx.HTTPError as exc:
            print(f"An error occurred while making the request: {exc}")
            return Response(content=f"Proxy Error: {exc}", status_code=502)

        return Response(content=response.content, status_code=response.status_code, headers=dict(response.headers))


@app.api_route("/{path:path}", methods=["GET", "POST", "PUT", "DELETE"])
async def proxy_requests(request: Request, path: str):
    headers = {k: v for k, v in request.headers.items() if k not in ["Host", "Connection"]}
    async with httpx.AsyncClient() as client:
        try:
            response = await client.request(
                request.method,
                f"http://localhost:8034/{path}",
                headers=headers,
                cookies=request.cookies,
                data=await request.body(),
            )
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
    # Fetch the PROXY_PORT environment variable. If not set, default to 8040
    proxy_port = int(os.getenv('PINEPODS_PORT', 8040))

    uvicorn.run("fastapirouter:app", host="0.0.0.0", port=proxy_port)
