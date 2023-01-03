from flask import Flask, request
import pandas as pd
import requests
from requests_html import HTML
from requests_html import HTMLSession
import warnings
from lxml import etree

warnings.simplefilter(action='ignore', category=FutureWarning)

app = Flask(__name__)

def get_source(url):
    """Return the source code for the provided URL. 

    Args: 
        url (string): URL of the page to scrape.

    Returns:
        response (object): HTTP response object from requests_html. 
    """

    try:
        session = HTMLSession()
        response = session.get(url)
        return response

    except requests.exceptions.RequestException as e:
        print(e)

import requests
import xmltodict

def get_feed(url):
    """Return a Pandas dataframe containing the RSS feed contents.

    Args: 
        url (string): URL of the RSS feed to read.

    Returns:
        df (dataframe): Pandas dataframe containing the RSS feed contents.
    """

    # Send a request to the URL and get the response
    response = requests.get(url)
    
    # Parse the XML content of the response
    feed_dict = xmltodict.parse(response.content)
    
    # Extract the 'item' elements from the feed
    items = feed_dict['rss']['channel']['item']
    
    # Create a list of dictionaries for each item, with keys 'title', 'pubDate', and 'description'
    data = []
    for item in items:
        row = {
            'title': item['title'],
            'pubDate': item['pubDate'],
            'description': item['description']
        }
        data.append(row)
    
    # Create a Pandas dataframe from the list of dictionaries
    df = pd.DataFrame(data, columns=['title', 'pubDate', 'description'])
    
    return df


@app.route('/parse', methods=['POST'])
def parse():
    # Get the URL to parse from the request body
    data = request.get_json()
    url = data['url']
    
    # Parse the XML data and return it as a dataframe
    df = get_feed(url)
    return df.to_json()

if __name__ == '__main__':
    app.run(host='0.0.0.0', port=5001)
