import requests

def call_clean_expired_sessions(url):
    response = requests.post(url + "/clean_expired_sessions/")
    if response.status_code == 200:
        print(response.json())
    else:
        print("Error calling clean_expired_sessions:", response.status_code)

def call_check_saved_session(url):
    response = requests.get(url + "/check_saved_session/")
    if response.status_code == 200:
        user_id = response.json()
        print("User ID:", user_id)
    else:
        print("No saved session found")