import requests
import pandas as pd
from requests_html import HTML
from requests_html import HTMLSession
from pyPodcastParser.Podcast import Podcast
import requests
import warnings

warnings.simplefilter(action='ignore', category=FutureWarning)

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

def get_feed(url):
    """Return a Pandas dataframe containing the RSS feed contents.

    Args: 
        url (string): URL of the RSS feed to read.

    Returns:
        df (dataframe): Pandas dataframe containing the RSS feed contents.
    """
    
    response = get_source(url)
    
    df = pd.DataFrame(columns = ['title', 'pubDate', 'description'])

    with response as r:
        items = r.html.find("item", first=False)


        for item in items:        

            title = item.find('title', first=True).text
            pubDate = item.find('pubDate', first=True).text
            description = item.find('description', first=True).text

            row = {'title': title, 'pubDate': pubDate, 'description': description}
            df = df.append(row, ignore_index=True)

    return df

df = get_feed('https://feeds.feedburner.com/ThisFilipinoAmericanLifePodcast')
print(df.head())