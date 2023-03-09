from flask import Flask, request, Response
import requests
import os

app = Flask(__name__)

@app.route('/proxy')
def proxy():
    url = request.args.get('url')
    if url.startswith('http'):
        # handle remote URL
        response = requests.get(url)
        content = response.content
    else:
        # handle local file path
        if not os.path.isfile(url):
            return Response('File not found', status=404)
        with open(url, 'rb') as f:
            content = f.read()
    headers = {
        'Access-Control-Allow-Origin': '*',
        'Access-Control-Allow-Methods': 'GET, POST, PUT, DELETE, OPTIONS',
        'Access-Control-Allow-Headers': 'Origin, Content-Type, Accept, Authorization',
        'Access-Control-Expose-Headers': 'Content-Length, X-Requested-With, Content-Type, Authorization',
        'Access-Control-Allow-Credentials': 'true'
    }
    return Response(content, status=200, headers=headers)

if __name__ == '__main__':
    app.run()
