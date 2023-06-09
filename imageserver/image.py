from flask import Flask, request, Response, send_file
from flask_caching import Cache
from flask_cors import CORS
import requests
from requests import Timeout
import os
import sys
from werkzeug.datastructures import Headers
from PIL import Image
import io
from concurrent.futures import ThreadPoolExecutor, TimeoutError
from PIL import UnidentifiedImageError

sys.path.append('/pinepods')

def open_image(file):
    try:
        with Image.open(file) as image:
            if image.mode == 'RGBA' or image.mode == 'P':
                image = image.convert('RGB')
            output = io.BytesIO()
            image.save(output, format='JPEG', optimize=True, quality=50) # Compress and save the image
            return output.getvalue()
    except UnidentifiedImageError:
        print("Unidentified image, using default.")
        with Image.open('/pinepods/images/pinepods-logo.png') as image:
            output = io.BytesIO()
            image.save(output, format='JPEG')
            return output.getvalue()

def optimize_image(content):
    with io.BytesIO(content) as f:
        with ThreadPoolExecutor(max_workers=1) as executor:
            future = executor.submit(open_image, f)
            try:
                return future.result(timeout=1)  # set timeout to 1 second
            except TimeoutError:
                print("Image processing took too long, using default.")
                with Image.open('/pinepods/images/pinepods-logo.png') as image:
                    output = io.BytesIO()
                    image.save(output, format='JPEG')
                    return output.getvalue()



app = Flask(__name__)
CORS(app)
cache = Cache(app, config={'CACHE_TYPE': 'filesystem', 'CACHE_DIR': '/pinepods/cache'})

@app.route('/proxy')
def proxy():
    url = request.args.get('url')
    if url.startswith('http'):
        headers = {}
        if 'Range' in request.headers:
            headers['Range'] = request.headers['Range']
        
        # Check if the URL is an audio or image file
        if url.endswith(('.mp3', '.wav', '.ogg', '.flac')):
            # Try to get the response from cache
            response = cache.get(url)
            if response is None:
                response = requests.get(url, headers=headers)
                # Cache the entire audio file content
                cache.set(url, response.content)
            content = response
        elif url.endswith(('.png', '.jpg', '.jpeg', '.gif')):
            response = cache.get(url)
            if response is None:
                try:
                    response = requests.get(url, headers=headers, timeout=10)  # set a timeout
                    response = optimize_image(response.content)
                    cache.set(url, response)
                except Timeout:
                    print(f'The request for {url} timed out')
                    return send_file('/pinepods/images/pinepods-logo.jpeg', mimetype='image/jpeg')
            content = response
        else:
            try:
                response = requests.get(url, headers=headers, timeout=1)  # set a timeout
                content = response.content
            except Timeout:
                print(f'The request for {url} timed out')
                return Response('Request timeout', status=408)  # return a 408 Timeout response
    else:
        if not os.path.isfile(url):
            return Response('File not found', status=404)
        with open(url, 'rb') as f:
            content = f.read()
            
    headers = Headers({
        'Access-Control-Allow-Origin': '*',
        'Access-Control-Allow-Methods': 'GET, POST, PUT, DELETE, OPTIONS',
        'Access-Control-Allow-Headers': 'Origin, Content-Type, Accept, Authorization',
        'Access-Control-Expose-Headers': 'Content-Length, X-Requested-With, Content-Type, Authorization',
        'Access-Control-Allow-Credentials': 'true'
    })

    return Response(content, status=206 if 'Range' in request.headers else 200, headers=headers)


if __name__ == '__main__':
    app.run()
