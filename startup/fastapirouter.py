from fastapi import FastAPI, Request, HTTPException, Response, WebSocket
import httpx
import logging
import websockets

app = FastAPI()
logging.basicConfig(level=logging.INFO)

@app.api_route("/{path:path}", methods=["GET", "POST", "PUT", "DELETE"])
async def proxy_requests(request: Request, path: str):
    async with httpx.AsyncClient() as client:
        try:
            # Filter out headers that might conflict with the internal service
            headers = {k: v for k, v in request.headers.items() if k not in ["Host", "Connection"]}

            # Forward to API
            if "/api" in path:
                try:
                    print(request.method, f"http://localhost:8032/{path}", headers, request.cookies, request.body)
                    response = await client.request(
                        request.method,
                        f"http://localhost:8032/{path}",
                        headers=headers,
                        cookies=request.cookies,
                        data=await request.body(),
                    )
                    print(response.status_code, response.text)
                except httpx.HTTPError as exc:
                    print(f"An error occurred while making the request: {exc}")
            # Forward to Image Server (Proxy)
            elif "/proxy" in path:
                response = await client.request(
                    request.method,
                    f"http://localhost:8000/{path}",
                    headers=headers,
                    cookies=request.cookies,
                    data=await request.body(),
                )
            # Forward to the Main App
            else:
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

            return Response(content=response.content, status_code=response.status_code, headers=dict(response.headers))
        except httpx.RequestError as exc:
            logging.error(f"An error occurred while requesting {exc.request.url!r}.")
            raise HTTPException(status_code=500, detail="Internal Server Error")

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
        await websocket.close()
        await remote_websocket.close()


if __name__ == '__main__':
    import uvicorn
    uvicorn.run("fastapirouter:app", host="0.0.0.0", port=80)
