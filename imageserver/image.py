from flask import Flask, request, Response
from flask_caching import Cache
from flask_cors import CORS
import requests
import os
from werkzeug.datastructures import Headers
from PIL import Image
import io

def optimize_image(content):
    with io.BytesIO(content) as f:
        with Image.open(f) as image:
            if image.mode == 'RGBA' or image.mode == 'P':
                image = image.convert('RGB')
            output = io.BytesIO()
            image.save(output, format='JPEG', optimize=True, quality=50) # Compress and save the image
            return output.getvalue()


app = Flask(__name__)
CORS(app)
cache = Cache(app, config={'CACHE_TYPE': 'simple'})

@app.route('/proxy')
def proxy():
    url = request.args.get('url')
    if url.startswith('http'):
        # handle remote URL
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
        # Check if the URL is an image file
        elif url.endswith(('.png', '.jpg', '.jpeg', '.gif')):
            # Try to get the response from cache
            response = cache.get(url)
            if response is None:
                response = requests.get(url, headers=headers)
                # Optimize the image content
                content = optimize_image(response.content)
                # Cache the response for 300 seconds
                cache.set(url, content, timeout=300)
        else:
            # For non-image files, make the request normally
            response = requests.get(url, headers=headers)

        content = response.content
        headers = response.headers
    else:
        # handle local file path
        if not os.path.isfile(url):
            return Response('File not found', status=404)
        with open(url, 'rb') as f:
            content = f.read()
        headers = {}

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
