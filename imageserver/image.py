from flask import Flask, request, Response, make_response
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


@app.route('/proxy')
def proxy():
    url = request.args.get('url')
    if url.startswith('http'):
        # handle remote URL
        headers = {}
        if 'Range' in request.headers:
            headers['Range'] = request.headers['Range']

        # Check if the URL is an image file
        if url.endswith(('.png', '.jpg', '.jpeg', '.gif')):
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

    flask_response = make_response(content)

    # Preserve important headers from the original response
    for header in ['Content-Type', 'Content-Length', 'Last-Modified']:
        if header in headers:
            flask_response.headers[header] = headers[header]

    # Add your custom headers
    flask_response.headers.extend({
        'Access-Control-Allow-Origin': '*',
        'Access-Control-Allow-Methods': 'GET, POST, PUT, DELETE, OPTIONS',
        'Access-Control-Allow-Headers': 'Origin, Content-Type, Accept, Authorization',
        'Access-Control-Expose-Headers': 'Content-Length, X-Requested-With, Content-Type, Authorization',
        'Access-Control-Allow-Credentials': 'true'
    })

    return flask_response


if __name__ == '__main__':
    app.run()
