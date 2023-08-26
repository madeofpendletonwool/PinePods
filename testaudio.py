import requests


def test_proxy(proxy_url, url):
    try:
        # Make a request through the proxy
        response = requests.get(proxy_url, params={'url': url})

        # Check the response status code
        if response.status_code == 200:
            print("Success! Response code is 200.")
        else:
            print(f"Error! Response code is {response.status_code}.")

        # Print the response content
        print("Response content:")
        # print(response.content)

    except requests.exceptions.RequestException as e:
        print(f"An error occurred: {e}")


# Example usage:
proxy_url = 'https://pinepods.collinpendleton.com/mover/'
url = 'https://cdn.changelog.com/uploads/practicalai/236/practical-ai-236.mp3'
test_proxy(proxy_url, url)
