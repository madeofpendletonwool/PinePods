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

async def open_image(file):
    try:
        with Image.open(file) as image:
            if image.mode == 'RGBA' or image.mode == 'P':
                image = image.convert('RGB')
            output = io.BytesIO()
            image.save(output, format='JPEG', optimize=True, quality=50) # Compress and save the image
            return output.getvalue()
    except UnidentifiedImageError:
        print("Unidentified image, using default.")
        with Image.open('/pinepods/images/pinepods-logo.jpeg') as image:
            output = io.BytesIO()
            image.save(output, format='JPEG')
            return output.getvalue()

async def optimize_image(content):
    with io.BytesIO(content) as f:
        with ThreadPoolExecutor(max_workers=1) as executor:
            future = executor.submit(open_image, f)
            try:
                return future.result(timeout=1)  # set timeout to 1 second
            except TimeoutError:
                print("Image processing took too long, using default.")
                with Image.open('/pinepods/images/pinepods-logo.jpeg') as image:
                    output = io.BytesIO()
                    image.save(output, format='JPEG')
                    return output.getvalue()


@app.api_route("/proxy/", methods=["GET", "POST", "PUT", "DELETE"])
async def proxy_image_requests(request: Request):
    url = request.query_params.get("url")
    print("Entered /proxy route")

    # Assuming this is a direct filesystem path
    if url.startswith('/pinepods'):
        try:
            # Directly serve the file if it exists
            return FileResponse(url, media_type="image/png")  # you may need to adjust the media type based on the actual image format
        except Exception as e:
            print(f"Error reading file: {e}")
            return Response(content="File not found", status_code=404)

    # For other URLs, use the proxy logic
    headers = {k: v for k, v in request.headers.items() if k not in ["Host", "Connection"]}

    async with httpx.AsyncClient() as client:
        try:
            response = await client.get(url, headers=headers)

            # Check if the URL is an audio or image file
            if url.endswith(('.mp3', '.wav', '.ogg', '.flac')):
                content = response.content  # Directly use audio content

            elif url.endswith(('.png', '.jpg', '.jpeg', '.gif')):
                content = await optimize_image(response.content)
            else:
                content = response.content  # Directly use content if not recognized

            content_type = response.headers.get("Content-Type")

            if 'Range' in headers:
                return StreamingResponse(io.BytesIO(content), media_type=content_type, status_code=206)
            return StreamingResponse(io.BytesIO(content), media_type=content_type)

        except (httpx.ReadTimeout, httpx.RequestError) as exc:
            print(f"An error occurred while making the request: {exc}")
            return Response(content=f"Proxy Error: {exc}", status_code=502)

        except Exception as e:
            print(f"Unexpected error occurred: {e}")
            return Response(content=f"Error: {e}", status_code=500)

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
