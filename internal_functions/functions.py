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
    # Set the query parameter
    params = {'query': podcast_value}

    try:
        # Make the GET request to the API
        response = requests.get(api_url, params=params)
        response.raise_for_status()  # raise exception if invalid HTTP status code received
    except requests.exceptions.HTTPError as http_err:
        return f"HTTP error occurred: {http_err}"
    except requests.exceptions.ConnectionError as conn_err:
        return f"Error connecting to the server: {conn_err}"
    except requests.exceptions.RequestException as req_err:
        return f"Error occurred: {req_err}"

    try:
        search_results = response.json()
        return search_results
    except json.JSONDecodeError as json_err:
        return f"Error decoding JSON: {json_err}"

if __name__ == '__main__':
    api_url = 'https://api.pinepods.online/api/search'
    podcast_value = 'my brother my brother and me'
    results = searchpod(podcast_value, api_url)
    if isinstance(results, str):
        print(f"Error occurred: {results}")
    else:
        return_results = results.get('feeds', [])
        for d in return_results:
            print(d.get('title', ''))
