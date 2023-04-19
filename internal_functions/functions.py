import requests

def searchpod(podcast_value, api_url):
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

if __name__ == '__main__':
    api_url = 'https://api.pinepods.online/api/search'
    podcast_value = 'my brother my brother and me'
    results = searchpod(podcast_value)
    print(results)
    return_results = results['feeds']
    for d in return_results:
        for k, v in d.items():
            if k == 'title':
                print(d['title'])