#!/bin/python

import podcastindex
import requests
from flask import Flask, jsonify, request

app = Flask(__name__)

@app.route('/api/search', methods=['GET'])
def search_podcasts():
    query = request.args.get('query')
    config = {
        "api_key": "api_key",
        "api_secret": "api_secret"
    }
    index = podcastindex.init(config)

    # result_unparse = index.search(query, clean=True)
    # return result_unparse
    try:
        result_unparse = index.search(query, clean=True)
        return result_unparse
    except podcastindex.APIError as e:
        return jsonify({'error': str(e)})

if __name__ == '__main__':
    app.run(host='0.0.0.0', port=5000)
    