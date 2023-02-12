import requests

def searchpod(podcast_value):
    # Set the API endpoint URL
    api_url = 'http://10.0.0.15:5000/api/search'

    # Set the query parameter
    params = {'query': f'{podcast_value}'}

    # Make the GET request to the API
    response = requests.get(api_url, params=params)

    try:
        search_results = response.json()
        return search_results
    except:
        search_results = response.status_code
        return search_results