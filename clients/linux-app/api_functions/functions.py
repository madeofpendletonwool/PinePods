import requests
import secrets

def generate_session_token():
    return secrets.token_hex(32)

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

def call_api_config(url, headers):
    response = requests.get(url + "/config", headers=headers)
    if response.status_code == 200:
        config_data = response.json()
        return (
            config_data["api_url"],
            config_data["proxy_url"],
            config_data["proxy_host"],
            config_data["proxy_port"],
            config_data["proxy_protocol"],
            config_data["reverse_proxy"],
        )
    else:
        print("Error getting API configuration:", response.status_code)
        return None

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
    session_token = generate_session_token()
    response = requests.post(url + f"/create_session/{user_id}", headers=headers, json={"session_token": session_token})
    if response.status_code == 200:
        print("Session created successfully")
        return session_token
    else:
        print("Error creating session:", response.status_code)
        return None

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

def call_add_podcast(url, headers, podcast_values, user_id):
    response = requests.post(url + "/add_podcast", headers=headers, json={"podcast_values": podcast_values, "user_id": user_id})
    if response.status_code == 200:
        success = response.json()["success"]
        if success:
            print("Podcast added successfully")
            return True
        else:
            print("Podcast already exists for the user")
            return False
    else:
        print("Error adding podcast:", response.status_code)
        return None

def call_enable_disable_guest(url, headers):
    response = requests.post(url + "/enable_disable_guest", headers=headers)
    if response.status_code == 200:
        success = response.json()["success"]
        if success:
            print("Guest account status changed successfully")
            return True
        else:
            print("Error changing guest account status")
            return False
    else:
        print("Error changing guest account status:", response.status_code)
        return None

def call_enable_disable_self_service(url, headers):
    response = requests.post(url + "/enable_disable_self_service", headers=headers)
    if response.status_code == 200:
        success = response.json()["success"]
        if success:
            print("Self-service status changed successfully")
            return True
        else:
            print("Error changing self-service status")
            return False
    else:
        print("Error changing self-service status:", response.status_code)
        return None

def call_self_service_status(url, headers):
    response = requests.get(url + "/self_service_status", headers=headers)
    if response.status_code == 200:
        status = response.json()["status"]
        print(f'status should be 0 1 or true false: {status}')
        return status
    else:
        print("Error fetching self-service status:", response.status_code)
        return None

def call_increment_listen_time(url, headers, user_id):
    response = requests.put(url + f"/increment_listen_time/{user_id}", headers=headers)
    if response.status_code == 200:
        print("Listen time incremented.")
    else:
        print("Error incrementing listen time:", response.status_code)

def call_increment_played(url, headers, user_id):
    response = requests.put(url + f"/increment_played/{user_id}", headers=headers)
    if response.status_code == 200:
        print("Played count incremented.")
    else:
        print("Error incrementing played count:", response.status_code)

def call_record_podcast_history(url, headers, episode_title, user_id, episode_pos):
    data = {
        "episode_title": episode_title,
        "user_id": user_id,
        "episode_pos": episode_pos,
    }
    response = requests.post(url + f"/record_podcast_history", headers=headers, json=data)
    if response.status_code == 200:
        print("Podcast history recorded.")
    else:
        print("Error recording podcast history:", response.status_code)

def call_download_podcast(url, headers, episode_url, title, user_id):
    data = {
        "episode_url": episode_url,
        "title": title,
        "user_id": user_id,
    }
    response = requests.post(url + f"/download_podcast", headers=headers, json=data)
    if response.status_code == 200:
        print("Podcast downloaded.")
        return True
    else:
        print("Error downloading podcast:", response.status_code)
        return False

def call_delete_podcast(url, headers, episode_url, title, user_id):
    data = {
        "episode_url": episode_url,
        "title": title,
        "user_id": user_id,
    }
    response = requests.post(url + f"/delete_podcast", headers=headers, json=data)
    if response.status_code == 200:
        print("Podcast deleted.")
    else:
        print("Error deleting podcast:", response.status_code)

def call_save_episode(url, headers, episode_url, title, user_id):
    data = {
        "episode_url": episode_url,
        "title": title,
        "user_id": user_id,
    }
    response = requests.post(url + f"/save_episode", headers=headers, json=data)
    if response.status_code == 200:
        print("Episode saved.")
    else:
        print("Error saving episode:", response.status_code)

def call_remove_saved_episode(url, headers, episode_url, title, user_id):
    data = {
        "episode_url": episode_url,
        "title": title,
        "user_id": user_id,
    }
    response = requests.post(url + f"/remove_saved_episode", headers=headers, json=data)
    if response.status_code == 200:
        print("Saved episode removed.")
    else:
        print("Error removing saved episode:", response.status_code)

def call_record_listen_duration(url, headers, episode_url, title, user_id, listen_duration):
    data = {
        "episode_url": episode_url,
        "title": title,
        "user_id": user_id,
        "listen_duration": listen_duration
    }
    response = requests.post(url + f"/record_listen_duration", headers=headers, json=data)
    if response.status_code == 200:
        print("Listen duration recorded.")
    else:
        print("Error recording listen duration:", response.status_code)

def call_refresh_pods(url, headers):
    response = requests.get(url + f"/refresh_pods", headers=headers)
    if response.status_code == 200:
        print("Podcasts refreshed.")
    else:
        print("Error refreshing podcasts:", response.status_code)

def call_get_stats(url, headers, user_id):
    response = requests.get(url + f"/get_stats?user_id={user_id}", headers=headers)
    if response.status_code == 200:
        stats = response.json()
        return stats
    else:
        print("Error getting stats:", response.status_code)
        return None

def call_get_user_episode_count(url, headers, user_id):
    response = requests.get(url + f"/get_user_episode_count?user_id={user_id}", headers=headers)
    if response.status_code == 200:
        episode_count = response.json()
        return episode_count
    else:
        print("Error getting user episode count:", response.status_code)
        return None

def call_get_user_info(url, headers):
    response = requests.get(url + "/get_user_info", headers=headers)
    if response.status_code == 200:
        user_info = response.json()
        return user_info
    else:
        print("Error getting user information:", response.status_code)
        return None

def call_check_podcast(url, headers, user_id, podcast_name):
    data = {"user_id": user_id, "podcast_name": podcast_name}
    response = requests.post(url + "/check_podcast", headers=headers, json=data)
    if response.status_code == 200:
        return response.json()["exists"]
    else:
        print("Error checking podcast:", response.status_code)
        return False

def call_user_admin_check(url, headers, user_id):
    response = requests.get(url + f"/api/user_admin_check/{user_id}", headers=headers)
    if response.status_code == 200:
        return response.json()["is_admin"]
    else:
        print("Error fetching user admin status:", response.status_code)
        return False

def call_remove_podcast(url, headers, podcast_name, user_id):
    data = {"podcast_name": podcast_name, "user_id": user_id}
    response = requests.post(url + "/api/remove_podcast", headers=headers, json=data)
    if response.status_code == 200:
        return True
    else:
        print("Error removing podcast:", response.status_code)
        return False

def call_return_pods(url, headers, user_id):
    response = requests.get(url + f"/api/return_pods/{user_id}", headers=headers)
    if response.status_code == 200:
        return response.json()["pods"]
    else:
        print("Error fetching podcasts:", response.status_code)
        return None

def call_user_history(url, headers, user_id):
    response = requests.get(url + f"/api/user_history/{user_id}", headers=headers)
    if response.status_code == 200:
        return response.json()["history"]
    else:
        print("Error fetching user history:", response.status_code)
        return None

def call_saved_episode_list(url, headers, user_id):
    response = requests.get(url + f"/api/saved_episode_list/{user_id}", headers=headers)
    if response.status_code == 200:
        return response.json()["saved_episodes"]
    else:
        print("Error fetching saved episode list:", response.status_code)
        return None

def call_download_episode_list(url, headers, user_id):
    data = {"user_id": user_id}
    response = requests.post(url + "/api/download_episode_list", headers=headers, json=data)
    if response.status_code == 200:
        return response.json()["downloaded_episodes"]
    else:
        print("Error fetching downloaded episodes:", response.status_code)
        return None

def call_get_queue_list(url, headers, queue_urls):
    data = {"queue_urls": queue_urls}
    response = requests.post(url + "/api/get_queue_list", headers=headers, json=data)
    if response.status_code == 200:
        return response.json()["queue_list"]
    else:
        print("Error fetching queue list:", response.status_code)
        return None

def call_return_selected_episode(api_url, headers, user_id, title, episode_url):
    data = {"user_id": user_id, "title": title, "url": episode_url}
    response = requests.post(api_url + "/api/return_selected_episode", headers=headers, json=data)
    if response.status_code == 200:
        return response.json()["episode_info"]
    else:
        print("Error fetching selected episode:", response.status_code)
        return None

def call_check_usernames(url, headers, username):
    data = {"username": username}
    response = requests.post(url + "/api/check_usernames", headers=headers, json=data)
    if response.status_code == 200:
        return response.json()["username_exists"]
    else:
        print("Error checking usernames:", response.status_code)
        return False

def call_add_user(url, headers, user_values):
    data = {"user_values": user_values}
    response = requests.post(url + "/api/add_user", headers=headers, json=data)
    if response.status_code == 200:
        print("User added successfully.")
    else:
        print("Error adding user:", response.status_code)

def call_set_fullname(url, headers, user_id, new_name):
    data = {"new_name": new_name}
    response = requests.put(url + f"/api/set_fullname/{user_id}", headers=headers, json=data)
    if response.status_code == 200:
        print("Fullname updated successfully.")
    else:
        print("Error updating fullname:", response.status_code)

def call_set_password(url, headers, user_id, salt, hash_pw):
    data = {"salt": salt, "hash_pw": hash_pw}
    response = requests.put(url + f"/api/set_password/{user_id}", headers=headers, json=data)
    if response.status_code == 200:
        print("Password updated successfully.")
    else:
        print("Error updating password:", response.status_code)

def call_set_email(url, headers, user_id, email):
    data = {"user_id": self.user_id, "new_email": self.email}
    response = requests.put(app_api.url + "/api/user/set_email", headers=app_api.headers, json=data)
    if response.status_code != 200:
        print("Error updating email:", response.status_code)

def call_set_username(url, headers, user_id, new_username):
    data = {"user_id": user_id, "new_username": new_username}
    response = requests.put(url + "/api/user/set_username", headers=headers, json=data)
    if response.status_code != 200:
        print("Error updating username:", response.status_code)

def call_set_isadmin(url, headers, user_id, isadmin):
    data = {"user_id": user_id, "isadmin": isadmin}
    response = requests.put(url + "/api/user/set_isadmin", headers=headers, json=data)
    if response.status_code != 200:
        print("Error updating IsAdmin status:", response.status_code)

def call_final_admin(url, headers, user_id):
    response = requests.get(url + f"/api/user/final_admin/{user_id}", headers=headers)
    if response.status_code == 200:
        final_admin_data = response.json()
        return final_admin_data["final_admin"]
    else:
        print("Error checking final admin:", response.status_code)
        return False

def call_delete_user(url, headers, user_id):
    response = requests.delete(url + f"/api/user/delete/{user_id}", headers=headers)
    if response.status_code == 200:
        print("User deleted")
    else:
        print("Error deleting user:", response.status_code)

def call_set_theme(url, headers, user_id, theme):
    data = {"user_id": user_id, "new_theme": theme}
    response = requests.put(url + "/api/user/set_theme", headers=headers, json=data)
    if response.status_code != 200:
        print("Error updating theme:", response.status_code)

def call_check_downloaded(url, headers, user_id, title, ep_url):
    params = {"user_id": user_id, "title": title, "url": ep_url}
    response = requests.get(url + "/api/user/check_downloaded", headers=headers, params=params)
    if response.status_code == 200:
        return response.json()["is_downloaded"]
    else:
        print("Error checking downloaded status:", response.status_code)
        return False

def call_check_saved(url, headers, user_id, title, ep_url):
    params = {"user_id": user_id, "title": title, "url": ep_url}
    response = requests.get(url + "/api/user/check_saved", headers=headers, params=params)
    if response.status_code == 200:
        return response.json()["is_saved"]
    else:
        print("Error checking saved status:", response.status_code)
        return False
