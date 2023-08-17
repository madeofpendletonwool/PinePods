from fastapi import FastAPI, Request, HTTPException, Response
import httpx
import logging

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
                response = await client.request(
                    request.method,
                    f"http://localhost:8032/{path}",
                    headers=headers,
                    cookies=request.cookies,
                    data=request.body,
                )
            # Forward to Image Server (Proxy)
            elif "/proxy" in path:
                response = await client.request(
                    request.method,
                    f"http://localhost:8000/{path}",
                    headers=headers,
                    cookies=request.cookies,
                    data=request.body,
                )
            # Forward to the Main App
            else:
                response = await client.request(
                    request.method,
                    f"http://localhost:8034/{path}",
                    headers=headers,
                    cookies=request.cookies,
                    data=request.body,
                )

            return Response(content=response.content, status_code=response.status_code, headers=dict(response.headers))
        except httpx.RequestError as exc:
            logging.error(f"An error occurred while requesting {exc.request.url!r}.")
            raise HTTPException(status_code=500, detail="Internal Server Error")

if __name__ == '__main__':
    import uvicorn
    uvicorn.run("fastapirouter:app", host="0.0.0.0", port=80)
