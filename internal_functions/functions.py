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


def searchpod(podcast_value, api_url):
    print(f"API URL: {api_url}")
    print(f"Podcast Value: {podcast_value}")

    # Set the query parameter
    params = {'query': f'{podcast_value}'}
    print(f"Params: {params}")

    # Make the GET request to the API
    response = requests.get(api_url, params=params)
    print(f"Response: {response}")
    print(f"Response content: {response.content}")

    try:
        search_results = response.json()
        return search_results
    except:
        search_results = response.status_code
        return search_results


if __name__ == '__main__':
    api_url = 'https://search.pinepods.online/api/search'
    podcast_value = 'Me and My Brothers Podcast'
    results = searchpod(podcast_value, api_url)
    print(results)
    if isinstance(results, str):
        print(f"Error occurred: {results}")
    else:
        return_results = results.get('feeds', [])
        for d in return_results:
            print(d.get('title', ''))
