# Various flet imports
import flet as ft
from flet import Text, colors, icons, ButtonStyle, Row, alignment, border_radius, animation, \
    MainAxisAlignment, padding

import internal_functions.functions
import Auth.Passfunctions
import api_functions.functions
from api_functions.functions import call_api_config
import app_functions.functions
from helpers import user
from helpers import navigation

# Other Imports
import os
import appdirs
import requests
from requests.exceptions import RequestException, MissingSchema
from cryptography.fernet import Fernet
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.kdf.pbkdf2 import PBKDF2HMAC
import base64
from flask import Flask, Response
from flask_caching import Cache

# from main import login_screen


class PR:
    def __init__(self, page):
        self.pr = ft.ProgressRing()
        self.progress_stack = ft.Stack([self.pr], bottom=25, right=30, left=13, expand=True)
        self.page = page
        self.active_pr = False

    def touch_stack(self):
        self.page.overlay.append(self.progress_stack)
        self.active_pr = True

    def rm_stack(self):
        if self.active_pr:
            self.page.overlay.remove(self.progress_stack)
            self.active_pr = False


class API:
    def __init__(self, page, key, salt, login_screen, active_user):
        self.proxy_protocol = None
        self.proxy_port = None
        self.proxy_host = None
        self.proxy_url = None
        self.search_api_url = None
        self.encrypted_data = None
        self.data = None
        self.api_key = None
        self.cache = None
        self.data_dir = None
        self.app_name = None
        self.pr_instance = PR(page)
        self.login_screen = login_screen
        self.page = page
        self.active_user = active_user
        # self.user_data_dir = user_data_dir
        # self.pr_instance = pr_instance
        # self.modify_user = modify_user
        # self.login_username = login_username
        # self.login_password = login_password
        # self.new_nav = new_nav
        self.url = None
        self.api_value = None
        self.username = None
        self.password = None
        self.headers = None
        self.cred_headers = None
        self.page = page
        self.server_name = None
        self.salt = salt
        self.key = None

        # --- Create Flask app for caching ------------------------------------------------
        self.app = Flask(__name__)

    def preload_audio_file(self, url):
        response = requests.get(self.proxy_url, params={'url': url})
        if response.status_code == 200:
            # Cache the file content
            self.cache.set(url, response.content)

    def initialize_audio_routes(self):
        self.cache = Cache(self.app, config={'CACHE_TYPE': 'simple'})

        @self.app.route('/preload/<path:url>')
        def route_preload_audio_file(url):
            self.preload_audio_file(url)
            return ""

        @self.app.route('/cached_audio/<path:url>')
        def serve_cached_audio(url):
            content = cache.get(url)

            if content is not None:
                response = Response(content, content_type='audio/mpeg')
                return response
            else:
                return "", 404

        return cache

    def get_api_file_path(self):
        self.app_name = 'pinepods'
        self.data_dir = appdirs.user_data_dir(self.app_name)
        os.makedirs(self.data_dir, exist_ok=True)
        session_file_path = os.path.join(self.data_dir, "api_config.txt")
        return session_file_path

    def save_server_vals(self):
        session_file_path = self.get_api_file_path()
        self.data = f"{self.api_key}\n{self.server_name}\n"
        self.encrypt_data()
        with open(session_file_path, "wb") as file:
            file.write(self.encrypted_data)

    def encrypt_data(self):
        f = Fernet(self.key)
        self.encrypted_data = f.encrypt(self.data.encode())

    def decrypt_data(self):
        f = Fernet(self.key)
        data = f.decrypt(self.encrypted_data).decode()
        return data

    def get_saved_session_id_from_file(self):
        session_file_path = self.get_session_file_path()
        try:
            with open(session_file_path, "rb") as file:
                self.encrypted_data = file.read()
                session_id = self.decrypt_data()
                return session_id
        except FileNotFoundError:
            return None

    def get_session_file_path(self):
        app_name = 'pinepods'
        data_dir = appdirs.user_data_dir(app_name)
        os.makedirs(data_dir, exist_ok=True)
        session_file_path = os.path.join(data_dir, "session.txt")
        return session_file_path

    def get_key(self, password):
        kdf = PBKDF2HMAC(
            algorithm=hashes.SHA256(),
            length=32,
            salt=self.salt,
            iterations=100000,
        )
        self.key = base64.urlsafe_b64encode(kdf.derive(password))
        return self.key

    def api_verify_username(self, server_name, username, password, retain_session=False):
        # pr_instance.touch_stack()
        self.page.update()
        check_url = server_name + "/api/pinepods_check"
        self.url = server_name + "/api/data"  # keep this for later use
        self.server_name = server_name

        if not username and password:
            self.show_error_snackbar("Username and Password required")
            self.pr_instance.rm_stack()
            self.page.update()
            return

        self.username = username
        self.password = password
        self.cred_headers = {"username": self.username, "password": self.password}

        try:
            check_response = requests.get(check_url, timeout=10)
            if check_response.status_code != 200:
                self.show_error_snackbar("Unable to find a Pinepods instance at this URL.")
                self.pr_instance.rm_stack()
                self.page.update()
                return

            check_data = check_response.json()

            if "pinepods_instance" not in check_data or not check_data["pinepods_instance"]:
                self.show_error_snackbar("Unable to find a Pinepods instance at this URL.")
                self.pr_instance.rm_stack()
                self.page.update()
                return

        except MissingSchema:
            self.show_error_snackbar("This doesn't appear to be a proper URL.")
        except requests.exceptions.Timeout:
            self.show_error_snackbar("Request timed out. Please check your URL.")
        except RequestException as e:
            def start_config(page):
                page.go("/server_config")

            self.show_error_snackbar(f"Request failed: {e}")
            start_config(self.page)

        else:
            # If we reach here, it means the pinepods_check was successful.
            # Do the rest of your logic here.
            api_key = api_functions.functions.call_get_key(self.url, self.username.value, self.password.value)

            if not api_key or api_key.get('status') != 'success':
                self.page.go("/server_config")
                self.show_error_snackbar(f"Invalid User Credentials: {api_key.get('status')}")
                self.pr_instance.rm_stack()
                self.page.update()
                return

            else:
                self.headers = {"Api-Key": api_key['retrieved_key']}
                self.api_value = api_key['retrieved_key']
                api_functions.functions.call_clean_expired_sessions(self.url, self.headers)
                saved_session_value = self.get_saved_session_id_from_file()
                check_session = api_functions.functions.call_check_saved_session(self.url, self.headers,
                                                                                 saved_session_value)
                # global search_api_url
                # global proxy_url
                # global proxy_host
                # global proxy_port
                # global proxy_protocol
                # global reverse_proxy
                # global cache
                self.search_api_url, self.proxy_url, self.proxy_host, self.proxy_port, self.proxy_protocol, self.reverse_proxy = call_api_config(
                    self.url, self.headers)
                # self.show_error_snackbar(f"Connected to {proxy_host}!")
                # Initialize the audio routes
                self.cache = self.initialize_audio_routes()

                if retain_session:
                    self.save_server_vals()

                if self.login_screen:
                    login_details = api_functions.functions.call_get_user_details(self.url, self.headers,
                                                                                  username.value)
                    self.active_user.user_id = login_details['UserID']
                    self.active_user.fullname = login_details['Fullname']
                    self.active_user.username = login_details['Username']
                    self.active_user.email = login_details['Email']
                    if self.page.web:
                        navigation.start_login(self.page)
                    else:
                        if check_session:
                            self.active_user.saved_login(check_session)
                        else:
                            navigation.go_homelogin(self.page, self.active_user, self)

                else:
                    self.active_user.user_id = 1
                    self.active_user.fullname = 'Guest User'
                    navigation.go_homelogin(self.page, self.active_user, self)

        # pr_instance.rm_stack()
        self.page.update()

    def api_verify(self, server_name, api_value, retain_session=False):
        # pr_instance.touch_stack()
        self.page.update()
        check_url = server_name + "/api/pinepods_check"
        self.url = server_name + "/api/data"  # keep this for later use

        if not api_value:
            self.show_error_snackbar("API key is required.")
            self.pr_instance.rm_stack()
            self.page.update()
            return

        self.api_value = api_value
        self.headers = {"Api-Key": self.api_value}

        try:
            check_response = requests.get(check_url, timeout=10)
            if check_response.status_code != 200:
                self.show_error_snackbar("Unable to find a Pinepods instance at this URL.")
                self.pr_instance.rm_stack()
                self.page.update()
                return

            check_data = check_response.json()

            if "pinepods_instance" not in check_data or not check_data["pinepods_instance"]:
                self.show_error_snackbar("Unable to find a Pinepods instance at this URL.")
                self.pr_instance.rm_stack()
                self.page.update()
                return

        except MissingSchema:
            self.show_error_snackbar("This doesn't appear to be a proper URL.")
        except requests.exceptions.Timeout:
            self.show_error_snackbar("Request timed out. Please check your URL.")
        except RequestException as e:
            self.show_error_snackbar(f"Request failed: {e}")
            navigation.start_config(self.page)

        else:
            # If we reach here, it means the pinepods_check was successful.
            # Do the rest of your logic here.
            key_check = api_functions.functions.call_verify_key(self.url, self.headers)

            if not key_check or key_check.get('status') != 'success':
                self.page.go("/server_config")
                self.show_error_snackbar(f"Invalid API key: {key_check.get('status')}")
                self.pr_instance.rm_stack()
                self.page.update()
                return

            else:
                api_functions.functions.call_clean_expired_sessions(self.url, self.headers)
                saved_session_value = self.get_saved_session_id_from_file()
                check_session = api_functions.functions.call_check_saved_session(self.url, self.headers,
                                                                                 saved_session_value)
                global search_api_url
                global proxy_url
                global proxy_host
                global proxy_port
                global proxy_protocol
                global reverse_proxy
                global cache
                search_api_url, proxy_url, proxy_host, proxy_port, proxy_protocol, reverse_proxy = call_api_config(
                    self.url, self.headers)
                # self.show_error_snackbar(f"Connected to {proxy_host}!")
                # Initialize the audio routes
                cache = self.initialize_audio_routes()

                if retain_session:
                    self.save_server_vals()

                if login_screen:
                    user_id = api_functions.functions.call_get_user(self.url, self.headers)
                    print(user_id)
                    login_details = api_functions.functions.call_get_user_details_id(self.url,
                                                                                     self.headers,
                                                                                     user_id['retrieved_id'])
                    print(login_details)
                    self.active_user.user_id = login_details['UserID']
                    self.active_user.fullname = login_details['Fullname']
                    self.active_user.username = login_details['Username']
                    self.active_user.email = login_details['Email']

                    if self.page.web:
                        navigation.start_login(self.page)
                    else:
                        if check_session:
                            self.active_user.saved_login(check_session)
                        else:
                            navigation.go_homelogin(self.page, self.active_user, self)

                else:
                    self.active_user.user_id = 1
                    self.active_user.fullname = 'Guest User'
                    navigation.go_homelogin(self.page, self.active_user, self)

        # pr_instance.rm_stack()
        self.page.update()

    def show_error_snackbar(self, message):
        self.page.snack_bar = ft.SnackBar(ft.Text(message))
        self.page.snack_bar.open = True
        self.page.update()
