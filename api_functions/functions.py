import requests
import secrets
import json
from pydantic import BaseModel


def generate_session_token():
    return secrets.token_hex(32)


def call_clean_expired_sessions(url, headers):
    # print(f'in clean expired call {headers}')
    response = requests.post(url + "/clean_expired_sessions/", headers=headers)
    if response.status_code == 200:
        print('Response good!')
        # print(response.json())
    else:
        print("Error calling clean_expired_sessions:", response.status_code)


def call_verify_key(url, headers):
    response = requests.get(url + "/verify_key", headers=headers)
    if response.status_code == 200:
        print('Response good!')
        return {"status": "success"}
    else:
        print("Error calling verify_key:", response.status_code)
        return {"status": "error", "code": response.status_code}


def call_check_saved_session(url, headers, session_value):
    response = requests.get(url + f"/check_saved_session/{session_value}", headers=headers)
    if response.status_code == 200:
        user_id = response.json()
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
        return is_active
    else:
        print("Error fetching guest status:", response.status_code)
        return None


def call_download_status(url, headers):
    response = requests.get(url + "/download_status", headers=headers)
    if response.status_code == 200:
        is_active = response.json()
        print("Download status:", is_active)
        return is_active
    else:
        print("Error fetching guest status:", response.status_code)
        return None


def call_user_admin_check(url, headers, user_id):
    response = requests.get(url + f"/user_admin_check/{user_id}", headers=headers)
    if response.status_code == 200:
        return response.json()["is_admin"]
    else:
        print("Error fetching user admin status:", response.status_code)
        return False


def call_get_user_details(url, headers, username):
    response = requests.get(url + f"/user_details/{username}", headers=headers)
    if response.status_code == 200:
        user_details = response.json()
        return user_details
    else:
        print("Error fetching user details:", response.status_code)
        return None


def call_get_user_details_id(url, headers, user_id):
    response = requests.get(url + f"/user_details_id/{user_id}", headers=headers)
    if response.status_code == 200:
        user_details = response.json()
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
        print("Error details:", response.text)
        return None


def call_verify_password(url, headers, username, password):
    response = requests.post(url + "/verify_password/", json={"username": username, "password": password},
                             headers=headers)
    if response.status_code == 200:
        is_password_valid = response.json()["is_password_valid"]
        return is_password_valid
    else:
        print("Error verifying password:", response.status_code)
        print("Error details:", response.text)
        return None


def call_return_episodes(url, headers, user_id):
    response = requests.get(url + f"/return_episodes/{user_id}", headers=headers)
    if response.status_code == 200:
        episodes = response.json()["episodes"]
        if episodes:
            return episodes
        else:
            return None
    else:
        print("Error fetching episodes:", response.status_code)
        print("Error details:", response.text)
        return None


def call_check_episode_playback(url, headers, user_id, episode_title, episode_url):
    payload = {
        "user_id": user_id,
        "episode_title": episode_title,
        "episode_url": episode_url
    }
    response = requests.post(url + "/check_episode_playback", data=payload, headers=headers)
    if response.status_code == 200:
        playback_data = response.json()
        return playback_data['has_playback'], playback_data['listen_duration']
    else:
        return None, None


def call_get_user_details_id(url, headers, user_id):
    response = requests.get(url + f"/user_details_id/{user_id}", headers=headers)
    if response.status_code == 200:
        user_details = response.json()
        return user_details
    else:
        print("Error fetching user details:", response.status_code)
        return None


def call_get_theme(url, headers, user_id):
    response = requests.get(url + f"/get_theme/{user_id}", headers=headers)
    if response.status_code == 200:
        theme = response.json()["theme"]
        return theme
    else:
        print("Error fetching theme:", response.status_code)
        return None


def call_add_podcast(url, headers, podcast_values, user_id):
    data = {
        "podcast_values": json.dumps(podcast_values),
        "user_id": str(user_id)
    }
    response = requests.post(url + "/add_podcast", headers=headers, data=data)
    if response.status_code == 200:
        success = response.json()["success"]
        if success:
            return True
        else:
            return False
    else:
        print("Error adding podcast:", response.status_code)
        return None


def call_enable_disable_guest(url, headers):
    response = requests.post(url + "/enable_disable_guest", headers=headers)
    if response.status_code == 200:
        success = response.json()["success"]
        if success:
            return True
        else:
            return False
    else:
        print("Error changing guest account status:", response.status_code)
        return None


def call_enable_disable_downloads(url, headers):
    response = requests.post(url + "/enable_disable_downloads", headers=headers)
    if response.status_code == 200:
        success = response.json()["success"]
        if success:
            return True
        else:
            return False
    else:
        print("Error changing Download Status:", response.status_code)
        return None


def call_enable_disable_self_service(url, headers):
    response = requests.post(url + "/enable_disable_self_service", headers=headers)
    if response.status_code == 200:
        success = response.json()["success"]
        if success:
            return True
        else:
            return False
    else:
        print("Error changing self-service status:", response.status_code)
        return None


def call_self_service_status(url, headers):
    response = requests.get(url + "/self_service_status", headers=headers)
    if response.status_code == 200:
        status = response.json()["status"]
        return status
    else:
        print("Error fetching self-service status:", response.status_code)
        return None


def call_increment_listen_time(url, headers, user_id):
    response = requests.put(url + f"/increment_listen_time/{user_id}", headers=headers)
    if response.status_code == 200:
        return True
    else:
        print("Error incrementing listen time:", response.status_code)


def call_increment_played(url, headers, user_id):
    response = requests.put(url + f"/increment_played/{user_id}", headers=headers)
    if response.status_code == 200:
        return True
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
        return True
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
        return True
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
        return True
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
        return True
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
        return True
    else:
        print("Error recording listen duration:", response.status_code)


def call_refresh_pods(url, headers):
    response = requests.get(url + f"/refresh_pods", headers=headers)
    if response.status_code == 200:
        return True
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
        print(f"Error checking podcast: {response.status_code}, response: {response.text}")
        return False


def call_remove_podcast(url, headers, podcast_name, user_id):
    data = {"podcast_name": podcast_name, "user_id": user_id}
    response = requests.post(url + "/remove_podcast", headers=headers, json=data)
    if response.status_code == 200:
        return True
    else:
        print("Error removing podcast:", response.status_code)
        return False


def call_return_pods(url, headers, user_id):
    response = requests.get(url + f"/return_pods/{user_id}", headers=headers)
    if response.status_code == 200:
        return response.json()["pods"]
    else:
        print("Error fetching podcasts:", response.status_code)
        return None


def call_user_history(url, headers, user_id):
    response = requests.get(url + f"/user_history/{user_id}", headers=headers)
    if response.status_code == 200:
        return response.json()["history"]
    else:
        print("Error fetching user history:", response.status_code)
        return None


def call_saved_episode_list(url, headers, user_id):
    response = requests.get(url + f"/saved_episode_list/{user_id}", headers=headers)
    if response.status_code == 200:
        return response.json()["saved_episodes"]
    else:
        print("Error fetching saved episode list:", response.status_code)
        return None


def call_download_episode_list(url, headers, user_id):
    data = {"user_id": user_id}
    response = requests.post(url + "/download_episode_list", headers=headers, data=data)
    if response.status_code == 200:
        return response.json()["downloaded_episodes"]
    else:
        print("Error fetching downloaded episodes:", response.status_code)
        return None


def call_get_encryption_key(url, headers):
    response = requests.get(url + "/get_encryption_key", headers=headers)
    if response.status_code == 200:
        encryption_key = response.json()['encryption_key']
        return encryption_key
    else:
        print("Error getting encryption key:", response.status_code)
        return None


def call_save_email_settings(url, headers, server_name, server_port, from_email, send_mode, encryption, auth_required,
                             email_username, email_password, encryption_key):
    from cryptography.fernet import Fernet

    if encryption_key is None:
        print("Cannot save settings without encryption key.")
        return

    cipher_suite = Fernet(encryption_key)

    # Only encrypt password if it's not None
    if email_password is not None:
        encrypted_password = cipher_suite.encrypt(email_password.encode())
        # Decode encrypted password back to string
        decoded_password = encrypted_password.decode()
    else:
        decoded_password = None

    data = {
        "email_settings": {
            "server_name": server_name,
            "server_port": server_port,
            "from_email": from_email,
            "send_mode": send_mode,
            "encryption": encryption,
            "auth_required": auth_required,
            "email_username": email_username,
            "email_password": decoded_password,
        }
    }

    response = requests.post(url + "/save_email_settings", headers=headers, json=data)
    if response.status_code == 200:
        return True
    else:
        print("Error saving email settings:", response.status_code)
        print("Response body:", response.json())


def call_get_email_info(url, headers):
    response = requests.get(url + "/get_email_settings", headers=headers)

    if response.status_code == 200:
        return response.json()
    else:
        print("Error retrieving email settings:", response.status_code)
        print("Response body:", response.json())
        return None


def call_return_selected_episode(api_url, headers, user_id, title, episode_url):
    data = {"user_id": user_id, "title": title, "url": episode_url}
    response = requests.post(api_url + "/return_selected_episode", headers=headers, json=data)
    if response.status_code == 200:
        return response.json()["episode_info"]
    else:
        print("Error fetching selected episode:", response.status_code)
        return None


def call_check_usernames(url, headers, username):
    data = {"username": username}
    response = requests.post(url + "/check_usernames", headers=headers,
                             json=username)  # Send the username directly as a string
    if response.status_code == 200:
        return response.json()["username_exists"]
    else:
        print("Error checking usernames:", response.status_code)
        print("Error message:", response.text)
        return False


def call_add_user(url, headers, fullname, username, email, hash_pw, salt):
    user_values = {"fullname": fullname, "username": username, "email": email, "hash_pw": hash_pw, "salt": salt}
    response = requests.post(url + "/add_user", headers=headers, json=user_values)
    if response.status_code == 200:
        return True
    else:
        print("Error adding user:", response.status_code)


def call_set_fullname(url, headers, user_id, new_name):
    params = {"new_name": new_name}
    response = requests.put(url + f"/set_fullname/{user_id}", headers=headers, params=params)
    if response.status_code == 200:
        return True
    else:
        print("Error updating fullname:", response.status_code)


def call_set_password(url, headers, user_id, salt, hash_pw):
    data = {"salt": salt, "hash_pw": hash_pw}
    response = requests.put(url + f"/set_password/{user_id}", headers=headers, json=data)
    if response.status_code == 200:
        return True
    else:
        print("Error updating password:", response.status_code)


def call_set_email(url, headers, user_id, email):
    data = {"user_id": self.user_id, "new_email": self.email}
    response = requests.put(app_api.url + "/user/set_email", headers=app_api.headers, json=data)
    if response.status_code != 200:
        print("Error updating email:", response.status_code)


def call_set_username(url, headers, user_id, new_username):
    data = {"user_id": user_id, "new_username": new_username}
    response = requests.put(url + "/user/set_username", headers=headers, json=data)
    if response.status_code != 200:
        print("Error updating username:", response.status_code)


def call_set_isadmin(url, headers, user_id, isadmin):
    data = {"user_id": user_id, "isadmin": isadmin}
    response = requests.put(url + "/user/set_isadmin", headers=headers, json=data)
    if response.status_code != 200:
        print("Error updating IsAdmin status:", response.status_code)


def call_final_admin(url, headers, user_id):
    response = requests.get(url + f"/user/final_admin/{user_id}", headers=headers)
    if response.status_code == 200:
        final_admin_data = response.json()
        return final_admin_data["final_admin"]
    else:
        print("Error checking final admin:", response.status_code)
        return False


def call_delete_user(url, headers, user_id):
    response = requests.delete(url + f"/user/delete/{user_id}", headers=headers)
    if response.status_code == 200:
        return True
    else:
        print("Error deleting user:", response.status_code)


def call_set_theme(url, headers, user_id, theme):
    data = {"user_id": user_id, "new_theme": theme}
    response = requests.put(url + "/user/set_theme", headers=headers, json=data)
    if response.status_code != 200:
        print("Error updating theme:", response.status_code)


def call_check_downloaded(url, headers, user_id, title, ep_url):
    params = {"user_id": user_id, "title": title, "url": ep_url}
    response = requests.get(url + "/user/check_downloaded", headers=headers, params=params)
    if response.status_code == 200:
        return response.json()["is_downloaded"]
    else:
        print("Error checking downloaded status:", response.status_code)
        return False


def call_check_saved(url, headers, user_id, title, ep_url):
    params = {"user_id": user_id, "title": title, "url": ep_url}
    response = requests.get(url + "/user/check_saved", headers=headers, params=params)
    if response.status_code == 200:
        return response.json()["is_saved"]
    else:
        print("Error checking saved status:", response.status_code)
        return False


def call_create_api_key(url, headers, user_id):
    data = {"user_id": user_id}
    response = requests.post(url + "/create_api_key", headers=headers, json=data)

    if response.status_code == 200:
        return response.json()["api_key"]
    else:
        print("Error creating API key:", response.status_code)
        print("Error message:", response.text)
        return None


def call_delete_api_key(url, headers, api_id, user_id):
    payload = {"api_id": api_id, "user_id": user_id}
    response = requests.delete(url + f"/delete_api_key", headers=headers, json=payload)

    if response.status_code == 200:
        return True
    else:
        print("Error deleting API key:", response.status_code)
        print("Error message:", response.text)


def call_get_api_info(url, headers, user_id):
    response = requests.get(url + f"/get_api_info/{user_id}", headers=headers)

    if response.status_code == 200:
        return response.json()["api_info"]
    else:
        print("Error getting API info:", response.status_code)
        print("Error message:", response.text)
        return []


def call_reset_password_create_code(url, headers, email, reset_code, user_id):
    payload = {"email": email, "reset_code": reset_code, "user_id": user_id}
    response = requests.post(url + "/reset_password_create_code", headers=headers, json=payload)
    if response.status_code == 200:
        return response.json()["user_exists"]
    else:
        print("Error resetting password:", response.status_code)
        return None


def call_verify_reset_code(url, headers, email, reset_code, user_id):
    payload = {"email": email, "reset_code": reset_code, "user_id": user_id}
    response = requests.post(url + "/verify_reset_code", headers=headers, json=payload)
    if response.status_code == 200:
        return response.json()["code_valid"]
    else:
        print("Error verifying reset code:", response.status_code)
        return False


def call_reset_password_prompt(url, headers, user_email, salt, hashed_pw, user_id):
    payload = {"email": user_email, "salt": salt.decode(), "hashed_pw": hashed_pw.decode(), "user_id": user_id}
    response = requests.post(url + "/reset_password_prompt", headers=headers, json=payload)
    if response.status_code == 200:
        return response.json()["message"]
    else:
        print("Error resetting password:", response.status_code)
        return None


def call_clear_guest_data(url, headers):
    response = requests.post(url + "/clear_guest_data", headers=headers)
    if response.status_code == 200:
        return response.json()["message"]
    else:
        print("Error clearing guest data:", response.status_code)
        return None


def call_get_episode_metadata(url, headers, episode_url, episode_title, user_id):
    print(episode_url, episode_title, user_id)
    data = {
        "episode_url": episode_url,
        "episode_title": episode_title,
        "user_id": user_id,
    }
    response = requests.post(url + f"/get_episode_metadata", headers=headers, json=data)
    if response.status_code == 200:
        return response.json()["episode"]
    else:
        print("Error fetching episode metadata:", response.status_code)
        return None


def call_save_mfa_secret(url, headers, user_id, mfa_secret):
    data = {
        "user_id": user_id,
        "mfa_secret": mfa_secret
    }
    response = requests.post(url + "/save_mfa_secret", headers=headers, json=data)

    if response.status_code == 200:
        return True
    else:
        print("Error saving MFA secret:", response.status_code)
        print("Error message:", response.text)
        return False


def call_check_mfa_enabled(url, headers, user_id):
    response = requests.get(url + f"/check_mfa_enabled/{user_id}", headers=headers)

    if response.status_code == 200:
        data = response.json()
        return data.get('mfa_enabled', False)
    else:
        print("Error checking MFA status:", response.status_code)
        print("Error message:", response.text)
        return False


def call_verify_mfa(url, headers, user_id, mfa_code):
    data = {
        "user_id": user_id,
        "mfa_code": mfa_code
    }
    response = requests.post(url + "/verify_mfa", headers=headers, json=data)

    if response.status_code == 200:
        data = response.json()
        return data.get('verified', False)
    else:
        print("Error verifying MFA code:", response.status_code)
        print("Error message:", response.text)
        return False


def call_delete_mfa_secret(url, headers, user_id):
    response = requests.delete(
        f"{url}/delete_mfa",
        headers=headers,
        json={"user_id": user_id}
    )

    if response.status_code == 200:
        return response.json().get('deleted', False)

    return False


def call_get_all_episodes(url, headers, pod_feed):
    data = {"pod_feed": pod_feed}
    response = requests.post(url + "/get_all_episodes", headers=headers, json=data)

    if response.status_code == 200:
        return response.json()["episodes"]
    else:
        print("Error getting Podcast Episodes:", response.status_code)
        print("Error message:", response.text)
        return None


def call_remove_episode_history(url, headers, ep_url, title, user_id):
    data = {"url": ep_url, "title": title, "user_id": user_id}
    response = requests.post(url + "/remove_episode_history", headers=headers, json=data)

    if response.status_code == 200:
        return response.json()["success"]
    else:
        print("Error removing episode from history:", response.status_code)
        print("Error message:", response.text)
        return None


def call_setup_time_info(url, headers, user_id, timezone, hour_pref):
    data = {"user_id": user_id, "timezone": timezone, "hour_pref": hour_pref}
    response = requests.post(url + "/setup_time_info", headers=headers, json=data)

    if response.status_code == 200:
        return response.json()["success"]
    else:
        print("Error setting up time info:", response.status_code)
        print("Error message:", response.text)
        return None


def call_get_time_info(url, headers, user_id):
    response = requests.get(url + "/get_time_info", headers=headers, params={"user_id": user_id})

    if response.status_code == 200:
        return response.json()["timezone"], response.json()["hour_pref"]
    else:
        print("Error getting time info:", response.status_code)
        print("Error message:", response.text)
        return None


def call_first_login_done(url, headers, user_id):
    data = {"user_id": user_id}
    response = requests.post(url + "/first_login_done", headers=headers, json=data)

    if response.status_code == 200:
        return response.json()["FirstLogin"]
    else:
        print("Error fetching first login status:", response.status_code)
        print("Error message:", response.text)
        return None


def call_delete_selected_episodes(url, headers, selected_episodes, user_id):
    data = {"selected_episodes": selected_episodes, "user_id": user_id}
    response = requests.post(url + "/delete_selected_episodes", headers=headers, json=data)

    if response.status_code == 200:
        return response.json()["status"]
    else:
        print("Error deleting selected episodes:", response.status_code)
        print("Error message:", response.text)
        return None


def call_delete_selected_podcasts(url, headers, delete_list, user_id):
    data = {"delete_list": delete_list, "user_id": user_id}
    response = requests.post(url + "/delete_selected_podcasts", headers=headers, json=data)

    if response.status_code == 200:
        return response.json()["status"]
    else:
        print("Error deleting selected podcasts:", response.status_code)
        print("Error message:", response.text)
        return None


def call_user_search(url, headers, user_id, search_term):
    data = {"search_term": search_term, "user_id": user_id}
    try:
        response = requests.post(url + "/search_data", headers=headers, json=data, timeout=30)
        response.raise_for_status()  # Raise an exception for HTTP errors
    except requests.exceptions.Timeout:
        print(f"Request timed out.")
        return None
    except requests.exceptions.HTTPError as http_err:
        print(f"HTTP error occurred: {http_err}")
        return None
    except Exception as err:
        print(f"Other error occurred: {err}")
        return None
    else:
        return response.json()["data"]


def call_queue_pod(url, headers, ep_url, episode_title, user_id):
    data = {"episode_title": episode_title, "ep_url": ep_url, "user_id": user_id}
    try:
        response = requests.post(url + "/queue_pod", headers=headers, json=data, timeout=30)
        response.raise_for_status()  # Raise an exception for HTTP errors
    except requests.exceptions.Timeout:
        print(f"Request timed out.")
        return None
    except requests.exceptions.HTTPError as http_err:
        print(f"HTTP error occurred: {http_err}")
        return None
    except Exception as err:
        print(f"Other error occurred: {err}")
        return None
    else:
        return response.json()["data"]


def call_remove_queue_pod(url, headers, ep_url, episode_title, user_id):
    data = {"episode_title": episode_title, "ep_url": ep_url, "user_id": user_id}
    try:
        response = requests.post(url + "/remove_queued_pod", headers=headers, json=data, timeout=30)
        response.raise_for_status()  # Raise an exception for HTTP errors
    except requests.exceptions.Timeout:
        print(f"Request timed out.")
        return None
    except requests.exceptions.HTTPError as http_err:
        print(f"HTTP error occurred: {http_err}")
        return None
    except Exception as err:
        print(f"Other error occurred: {err}")
        return None
    else:
        return response.json()["data"]


def call_queued_episodes(url, headers, user_id):
    data = {"user_id": user_id}
    try:
        response = requests.get(url + "/get_queued_episodes", headers=headers, json=data, timeout=30)
        response.raise_for_status()  # Raise an exception for HTTP errors
    except requests.exceptions.Timeout:
        print(f"Request timed out.")
        return None
    except requests.exceptions.HTTPError as http_err:
        print(f"HTTP error occurred: {http_err}")
        return None
    except Exception as err:
        print(f"Other error occurred: {err}")
        return None
    else:
        return response.json()["data"]


# client_api.py

def call_queue_bump(url, headers, ep_url, title, user_id):
    data = {"ep_url": ep_url, "title": title, "user_id": user_id}
    try:
        response = requests.post(url + "/queue_bump", headers=headers, json=data, timeout=30)
        response.raise_for_status()  # Raise an exception for HTTP errors
    except requests.exceptions.Timeout:
        print(f"Request timed out.")
        return None
    except requests.exceptions.HTTPError as http_err:
        print(f"HTTP error occurred: {http_err}")
        return None
    except Exception as err:
        print(f"Other error occurred: {err}")
        return None
    else:
        return response.json()["data"]


def call_backup_user(url, headers, user_id, backup_dir):
    import os
    data = {"user_id": user_id}
    try:
        response = requests.post(url + "/backup_user", headers=headers, json=data, timeout=30)
        response.raise_for_status()

        # Check if the backup_dir exists; if not, create it
        if not os.path.exists(backup_dir):
            os.makedirs(backup_dir)

        with open(os.path.join(backup_dir, f"user_{user_id}_backup.opml"), 'w') as file:
            file.write(response.text)

    except requests.exceptions.Timeout:
        print(f"Request timed out.")
        return None
    except requests.exceptions.HTTPError as http_err:
        print(f"HTTP error occurred: {http_err}")
        return None
    except Exception as err:
        print(f"Other error occurred: {err}")
        return None
    else:
        return True


def call_backup_server(url, headers, backup_dir, database_pass):
    import os

    data = {"backup_dir": backup_dir, "database_pass": database_pass}

    try:
        response = requests.get(url + "/backup_server", headers=headers, json=data, timeout=60)
        response.raise_for_status()

        # Check if the backup_dir exists; if not, create it
        if not os.path.exists(backup_dir):
            os.makedirs(backup_dir)

        with open(os.path.join(backup_dir, "server_backup.sql"), 'wb') as file:
            file.write(response.content)
        return {"success": True, "error_message": None}

    except requests.exceptions.Timeout:
        return {"success": False, "error_message": "Request timed out."}
    except requests.exceptions.HTTPError as http_err:
        return {"success": False,
                "error_message": f"HTTP error occurred: {http_err} - Is your database password correct?"}
    except Exception as err:
        return {"success": False, "error_message": f"Other error occurred: {err}"}


def call_restore_server(url, headers, database_pass, server_restore_data):
    data = {"database_pass": database_pass, "server_restore_data": server_restore_data}

    try:
        response = requests.post(url + "/restore_server", headers=headers, json=data, timeout=60)
        response.raise_for_status()
        return {"success": True, "error_message": None}

    except requests.exceptions.Timeout:
        return {"success": False, "error_message": "Request timed out."}
    except requests.exceptions.HTTPError:
        return {"success": False, "error_message": f"HTTP error occurred: {response.text} - Is your password correct?"}
    except Exception as err:
        return {"success": False, "error_message": f"Other error occurred: {err}"}


def call_import_podcasts(url, headers, user_id, podcasts):
    data = {
        "user_id": user_id,
        "podcasts": podcasts
    }
    response = requests.post(url + "/import_podcasts", headers=headers, json=data)
    return response.json()
