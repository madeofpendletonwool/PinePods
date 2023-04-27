import requests

def call_clean_expired_sessions(url, headers):
    print(f'in clean expired call {headers}')
    response = requests.post(url + "/clean_expired_sessions/", headers=headers)
    if response.status_code == 200:
        print(response.json())
    else:
        print("Error calling clean_expired_sessions:", response.status_code)

def call_check_saved_session(url, headers):
    response = requests.get(url + "/check_saved_session/", headers=headers)
    if response.status_code == 200:
        user_id = response.json()
        print("User ID:", user_id)
        return user_id
    else:
        print("No saved session found")

def call_guest_status(url, headers):
    response = requests.get(url + "/guest_status", headers=headers)
    if response.status_code == 200:
        is_active = response.json()
        print("Guest status:", is_active)
        return is_active
    else:
        print("Error fetching guest status:", response.status_code)
        return None

def call_get_user_details(url, headers, username):
    response = requests.get(url + f"/user_details/{username}", headers=headers)
    if response.status_code == 200:
        user_details = response.json()
        print("User details:", user_details)
        return user_details
    else:
        print("Error fetching user details:", response.status_code)
        return None

def call_get_user_details_id(url, headers, user_id):
    response = requests.get(url + f"/user_details_id/{user_id}", headers=headers)
    if response.status_code == 200:
        user_details = response.json()
        print("User details:", user_details)
        return user_details
    else:
        print("Error fetching user details:", response.status_code)
        return None


def call_create_session(url, headers, user_id):
    response = requests.post(url + f"/create_session/{user_id}", headers=headers)
    if response.status_code == 200:
        print("Session created successfully")
    else:
        print("Error creating session:", response.status_code)

def call_verify_password(url, headers, username, password):
    response = requests.post(url + "/verify_password/", json={"username": username, "password": password}, headers=headers)
    if response.status_code == 200:
        is_password_valid = response.json()["is_password_valid"]
        print("Is password valid:", is_password_valid)
        return is_password_valid
    else:
        print("Error verifying password:", response.status_code)
        return None


def call_return_episodes(url, headers, user_id):
    response = requests.get(url + f"/return_episodes/{user_id}", headers=headers)
    if response.status_code == 200:
        episodes = response.json()["episodes"]
        if episodes:
            print("Episodes:", episodes)
        else:
            print("No episodes found.")
            return None
        return episodes
    else:
        print("Error fetching episodes:", response.status_code)
        return None


def call_check_episode_playback(url, headers, user_id, episode_title, episode_url):
    payload = {
        "user_id": user_id,
        "episode_title": episode_title,
        "episode_url": episode_url
    }
    response = requests.post(url + "/check_episode_playback", json=payload, headers=headers)
    if response.status_code == 200:
        playback_data = response.json()
        print("Playback data:", playback_data)
        return playback_data
    else:
        print("Error checking episode playback:", response.status_code)
        return None

def call_get_user_details_id(url, headers, user_id):
    response = requests.get(url + f"/user_details_id/{user_id}", headers=headers)
    if response.status_code == 200:
        user_details = response.json()
        print("User details:", user_details)
        return user_details
    else:
        print("Error fetching user details:", response.status_code)
        return None

def call_get_theme(url, headers, user_id):
    response = requests.get(url + f"/get_theme/{user_id}", headers=headers)
    if response.status_code == 200:
        theme = response.json()["theme"]
        print("Theme:", theme)
        return theme
    else:
        print("Error fetching theme:", response.status_code)
        return None
