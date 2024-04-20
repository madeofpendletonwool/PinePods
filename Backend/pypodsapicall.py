from datetime import date
import hashlib
import json
import requests
import time
from flask import Flask, request
from flask_cors import CORS
import os

api_key = os.environ.get('API_KEY')
api_secret = os.environ.get('API_SECRET')

app = Flask(__name__)
CORS(app)

# setup some basic vars for the search api. 
# for more information, see https://api.podcastindex.org/developer_docs
url = "https://api.podcastindex.org/api/1.0/search/byterm?q="

# the api follows the Amazon style authentication
# see https://docs.aws.amazon.com/AmazonS3/latest/dev/S3_Authentication2.html

# we'll need the unix time
epoch_time = int(time.time())

# our hash here is the api key + secret + time 
data_to_hash = api_key + api_secret + str(epoch_time)
# which is then sha-1'd
sha_1 = hashlib.sha1(data_to_hash.encode()).hexdigest()

# now we build our request headers
headers = {
    'X-Auth-Date': str(epoch_time),
    'X-Auth-Key': api_key,
    'Authorization': sha_1,
    'User-Agent': 'postcasting-index-python-cli'
}

@app.route('/api/search', methods=['GET'])
def search():
    query = request.args.get('query', '')
    index = request.args.get('index', '')
    search_url = url + query

    if index.lower() == 'itunes':
        itunes_search_url = f"https://itunes.apple.com/search?term={query}&media=podcast"
        r = requests.get(itunes_search_url)
    else:  # default to podcast index
        # update headers with new date and hash
        epoch_time = int(time.time())
        data_to_hash = api_key + api_secret + str(epoch_time)
        sha_1 = hashlib.sha1(data_to_hash.encode()).hexdigest()
        headers['X-Auth-Date'] = str(epoch_time)
        headers['Authorization'] = sha_1

        # perform the actual post request
        r = requests.post(search_url, headers=headers)

    # if it's successful, return the contents (in a prettified json-format)
    # else, return the error code we received
    if r.status_code == 200:
        pretty_json = json.loads(r.text)
        return json.dumps(pretty_json, indent=2)
    else:
        return '<< Received ' + str(r.status_code) + '>>'


if __name__ == '__main__':
    app.run(host='0.0.0.0', port=5000)
