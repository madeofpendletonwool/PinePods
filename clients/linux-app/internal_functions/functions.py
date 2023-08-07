import requests
import json

def test_connection(api_url):
    try:
        response = requests.get(api_url)
        response.raise_for_status()
        return True
    except requests.exceptions.HTTPError as http_err:
        return f"HTTP error occurred: {http_err}"
    except requests.exceptions.ConnectionError as conn_err:
        return f"Your API_URL Variable is probably wrong. Error connecting: {conn_err}"
    except Exception as err:
        return f"An error occurred: {err}"
    # If there's no exception, the connection is established successfully


def searchpod(podcast_value, api_url, search_index='podcastindex'):
    # Set the query parameter
    params = {'query': f'{podcast_value}', 'index': search_index}

    # Make the GET request to the API
    response = requests.get(api_url, params=params)

    try:
        search_results = response.json()
        return search_results
    except:
        search_results = response.status_code
        return search_results

def get_podcast_values(feed_url, user_id):
    import feedparser
    # Parse the feed
    d = feedparser.parse(feed_url)

    # Extract needed values
    pod_title = d.feed.title if hasattr(d.feed, 'title') else None

    # For artwork, checking both generic and iTunes-specific
    pod_artwork = d.feed.image.href if hasattr(d.feed, 'image') and hasattr(d.feed.image, 'href') else None
    if not pod_artwork and hasattr(d.feed, 'itunes_image'):
        pod_artwork = d.feed.itunes_image['href']

    # For author, checking both generic and iTunes-specific
    pod_author = d.feed.author if hasattr(d.feed, 'author') else None
    if not pod_author and hasattr(d.feed, 'itunes_author'):
        pod_author = d.feed.itunes_author

    # Extracting categories, primarily from iTunes
    pod_categories = []
    if hasattr(d.feed, 'itunes_category'):
        for cat in d.feed.itunes_category:
            pod_categories.append(cat['text'])
            if 'itunes_category' in cat:
                for subcat in cat['itunes_category']:
                    pod_categories.append(subcat['text'])
    categories = json.dumps(pod_categories)

    # Description can be either generic or from iTunes
    pod_description = d.feed.description if hasattr(d.feed, 'description') else None
    if not pod_description and hasattr(d.feed, 'itunes_summary'):
        pod_description = d.feed.itunes_summary

    pod_episode_count = len(d.entries)
    pod_feed_url = feed_url  # since you passed it as an argument
    pod_website = d.feed.link if hasattr(d.feed, 'link') else None

    podcast_values = (
        pod_title, pod_artwork, pod_author, categories, pod_description, pod_episode_count, pod_feed_url,
        pod_website, user_id  # using the passed user_id directly
    )

    return podcast_values




if __name__ == '__main__':
    api_url = 'https://search.pinepods.online/api/search'
    podcast_value = 'my brother my brother and me'
    results = searchpod(podcast_value, api_url, 'itunes')
    # results = searchpod(podcast_value, api_url, 'itunes')
    print(results)
    if isinstance(results, str):
        print(f"Error occurred: {results}")
    else:
        if 'results' in results:  # if iTunes API was used
            for d in results['results']:
                print(d.get('trackName', ''))
        else:  # if PodcastIndex API was used
            return_results = results.get('feeds', [])
            for d in return_results:
                print(d.get('title', ''))

