# Various flet imports
import flet as ft
from flet import Text, colors, icons, ButtonStyle, Row, alignment, border_radius, animation, MainAxisAlignment, padding
# Internal Functions
import internal_functions.functions
import Auth.Passfunctions
import api_functions.functions
import app_functions.functions

# Others
import socket
from concurrent.futures import ThreadPoolExecutor, as_completed
import json
import re
import urllib.request
from requests.exceptions import RequestException, MissingSchema
import os
import requests
import time
import random
import string
import datetime
import html2text
import threading
from html.parser import HTMLParser
from flask import Flask
from flask_caching import Cache
import secrets
import appdirs
import logging
import hashlib
from dateutil import parser
import base64
import pyotp
import qrcode
import feedparser
from collections import defaultdict
from math import pi
import pytz
import shutil
from base64 import urlsafe_b64decode

logging.basicConfig(level=logging.WARNING, format='%(asctime)s - %(levelname)s - %(message)s')

# Wait for Client API Server to start
time.sleep(3)

# Proxy variables
proxy_host = os.environ.get("PROXY_HOST", "localhost")
proxy_port = os.environ.get("PROXY_PORT", "8000")
proxy_protocol = os.environ.get("PROXY_PROTOCOL", "http")
reverse_proxy = os.environ.get("REVERSE_PROXY", "False")

# Podcast Index API url
api_url = os.environ.get("API_URL", "https://api.pinepods.online/api/search")

# API Setup for FastAPI interactions with the database
with open("/tmp/web_api_key.txt", "r") as f:
    web_api_key = f.read().strip()

session_id = secrets.token_hex(32)  # Generate a 64-character hexadecimal string

# Initial Vars needed to start and used throughout
if reverse_proxy == "True":
    proxy_url = f'{proxy_protocol}://{proxy_host}/proxy?url='
else:
    proxy_url = f'{proxy_protocol}://{proxy_host}:{proxy_port}/proxy?url='


# --- Create Flask app for caching ------------------------------------------------
app = Flask(__name__)

def preload_audio_file(url, proxy_url, cache):
    response = requests.get(proxy_url, params={'url': url})
    if response.status_code == 200:
        # Cache the file content
        cache.set(url, response.content)

def initialize_audio_routes(app, proxy_url):
    cache = Cache(app, config={'CACHE_TYPE': 'simple'})

    @app.route('/preload/<path:url>')
    def route_preload_audio_file(url):
        preload_audio_file(url, proxy_url, cache)
        return ""

    @app.route('/cached_audio/<path:url>')
    def serve_cached_audio(url):
        content = cache.get(url)

        if content is not None:
            response = Response(content, content_type='audio/mpeg')
            return response
        else:
            return "", 404

    return cache

cache = initialize_audio_routes(app, proxy_url)
# Make login Screen start on boot
login_screen = True
user_home_dir = os.path.expanduser("~")
audio_playing = False
active_pod = 'Set at start'
script_dir = os.path.dirname(os.path.abspath(__file__))

appname = "pinepods"
appauthor = "Gooseberry Development"

# user_data_dir would be the equivalent to the home directory you were using
user_data_dir = appdirs.user_data_dir(appname, appauthor)
metadata_dir = os.path.join(user_data_dir, 'metadata')
backup_dir = os.path.join(user_data_dir, 'backups')

def main(page: ft.Page, session_value=None):
    # ---Flet Various Functions---------------------------------------------------------------

    class AnimatedButton:
        def __init__(self, rotate_button, download_ep_row_content, entry_seemore=None):
            self.rotate_button = rotate_button
            self.download_ep_row_content = download_ep_row_content
            self.rotate_pos = False
            self.entry_seemore = entry_seemore

        def animate_poddisplay(self, e):
            if not self.rotate_pos:
                self.rotate_pos = True
                self.download_ep_row_content.visible = True
                self.rotate_button.rotate.angle += pi / 2
                page.update()
            else:
                self.download_ep_row_content.visible = False
                self.rotate_button.rotate.angle -= pi / 2
                self.rotate_pos = False
                page.update()

        def animate(self, e):
            if not self.rotate_pos:
                self.rotate_pos = True
                self.download_ep_row_content.visible = True
                self.entry_seemore.visible = True
                self.rotate_button.rotate.angle += pi / 2
                page.update()
            else:
                self.download_ep_row_content.visible = False
                self.entry_seemore.visible = False
                self.rotate_button.rotate.angle -= pi / 2
                self.rotate_pos = False
                page.update()

    class API:
        def __init__(self, page):
            self.server_name = 'http://localhost:8032'
            self.api_value = web_api_key
            self.headers = None
            self.page = page
            self.headers = {"Api-Key": self.api_value}

        def api_verify(self, retain_session=False):
            self.url = self.server_name + "/api/data"
            check_url = self.server_name + "/api/pinepods_check"
            self.headers = {"Api-Key": self.api_value}

            initial_headers = {
                "pinepods_api": self.api_value,
            }

            try:
                check_response = requests.get(check_url, timeout=10)
                if check_response.status_code != 200:
                    self.show_error_snackbar("Unable to find a Pinepods instance at this URL.")
                    pr_instance.rm_stack()
                    self.page.update()
                    return

                check_data = check_response.json()

                if "pinepods_instance" not in check_data or not check_data["pinepods_instance"]:
                    self.show_error_snackbar("Unable to find a Pinepods instance at this URL.")
                    pr_instance.rm_stack()
                    self.page.update()
                    return

                response = requests.get(self.url, headers=initial_headers, timeout=10)
                response.raise_for_status()

            except MissingSchema:
                self.show_error_snackbar("This doesn't appear to be a proper URL.")
            except requests.exceptions.Timeout:
                self.show_error_snackbar("Request timed out. Please check your URL.")
            except RequestException as e:
                self.show_error_snackbar(f"Request failed: {e}")

            else:
                if response.status_code == 200:
                    data = response.json()

                    self.show_error_snackbar(f"Connected to {proxy_host}!")

                    if login_screen:
                        if page.web:
                            start_login(page)
                        else:
                            start_login(page)

                    else:
                        active_user.user_id = 1
                        active_user.fullname = 'Guest User'
                        go_homelogin(page)
                elif response.status_code == 401:
                    start_config(self.page)
                else:
                    self.show_error_snackbar(f"Request failed with status code: {response.status_code}")
            # pr_instance.rm_stack()
            self.page.update()

        def show_error_snackbar(self, message):
            self.page.snack_bar = ft.SnackBar(ft.Text(message))
            self.page.snack_bar.open = True
            self.page.update()

        def on_click_snacks(self):
            self.page.snack_bar = ft.SnackBar(ft.Text(f"Here's a snack"))
            self.page.snack_bar.open = True
            self.page.update()

    app_api = API(page)

    def send_podcast(pod_title, pod_artwork, pod_author, pod_categories, pod_description, pod_episode_count,
                     pod_feed_url, pod_website, page):
        pr_instance.touch_stack()
        page.update()
        categories = json.dumps(pod_categories)
        podcast_values = (
        pod_title, pod_artwork, pod_author, categories, pod_description, pod_episode_count, pod_feed_url, pod_website,
        active_user.user_id)
        return_value = api_functions.functions.call_add_podcast(app_api.url, app_api.headers, podcast_values,
                                                                active_user.user_id)
        pr_instance.rm_stack()
        if return_value == True:
            page.snack_bar = ft.SnackBar(ft.Text(f"Podcast Added Successfully!"))
            page.snack_bar.open = True
            page.update()
        else:
            page.snack_bar = ft.SnackBar(ft.Text(f"Podcast Already Added!"))
            page.snack_bar.open = True
            page.update()

    def pod_url_add(page):
        def close_pod_url_dlg(page):
            pod_url_dlg.open = False
            page.update()

        def close_pod_url_auto_dlg(e):
            pod_url_dlg.open = False
            page.update()

        def add_feed(e):
            active_user.feed_url = pod_url_box.value
            pr_instance.touch_stack()
            page.update()
            podcast_values = internal_functions.functions.get_podcast_values(active_user.feed_url, active_user.user_id)
            return_value = api_functions.functions.call_add_podcast(app_api.url, app_api.headers, podcast_values,
                                                                    active_user.user_id)
            pr_instance.rm_stack()
            close_pod_url_dlg(page)
            if return_value == True:
                page.snack_bar = ft.SnackBar(ft.Text(f"Podcast Added Successfully!"))
                page.snack_bar.open = True
                page.update()
            else:
                page.snack_bar = ft.SnackBar(ft.Text(f"Podcast Already Added!"))
                page.snack_bar.open = True
                page.update()

        pod_url_box = ft.TextField(label="Podcast Feed URL", icon=ft.icons.ADD_LINK, hint_text='https://mycoolpodcast/episodes/rss')
        pod_url_select_row = ft.Row(
            controls=[
                ft.TextButton("Confirm", on_click=add_feed),
                ft.TextButton("Cancel", on_click=close_pod_url_auto_dlg)
            ],
            alignment=ft.MainAxisAlignment.END
        )

        pod_url_dlg = ft.AlertDialog(
            modal=True,
            title=ft.Text(f"Confirm MFA:"),
            content=ft.Column(controls=[
                ft.Text(f'Input Podcast Feed URL below to add to database.', selectable=True),
                # ], tight=True),
                pod_url_box,
                # actions=[
                pod_url_select_row
            ],
                tight=True),
            actions_alignment=ft.MainAxisAlignment.END,
        )

        page.dialog = pod_url_dlg
        pod_url_dlg.open = True


    def invalid_username():
        page.dialog = username_invalid_dlg
        username_invalid_dlg.open = True
        page.update()

    def close_dlg(e):
        user_dlg.open = False
        page.update()
        go_home

    def save_session_to_file(session_id):
        with open("session.txt", "w") as file:
            file.write(session_id)

    def get_saved_session_from_file():
        try:
            with open("session.txt", "r") as file:
                session_id = file.read()
                return session_id
        except FileNotFoundError:
            return None

    def character_limit(screen_width):
        if screen_width < 400:
            return 40
        elif screen_width < 768:
            return 50
        elif screen_width < 768:
            return 60
        elif screen_width < 800:
            return 25
        elif screen_width < 900:
            return 35
        elif screen_width < 1000:
            return 40
        elif screen_width < 1100:
            return 45
        elif screen_width < 1200:
            return 55
        elif screen_width < 1300:
            return 70
        else:
            return None

    def truncate_text(text, max_chars):
        if max_chars and len(text) > max_chars:
            return text[:max_chars] + '...'
        else:
            return text

    def on_click_wronguser(page):
        page.snack_bar = ft.SnackBar(ft.Text(f"Wrong username or password. Please try again!"))
        page.snack_bar.open = True
        page.update()

    def on_click_novalues(page):
        page.snack_bar = ft.SnackBar(ft.Text(f"Please enter a username and a password before selecting Login"))
        page.snack_bar.open = True
        page.update()

    def launch_clicked_url(e):
        page.launch_url(e.data)

    def launch_pod_site(e):
        page.launch_url(clicked_podcast.website)

    def setup_user_for_otp():
        # generate a new secret for the user
        secret = pyotp.random_base32()

        # create a provisioning URL that the user can scan with their OTP app
        provisioning_url = pyotp.totp.TOTP(secret).provisioning_uri(name=active_user.email, issuer_name='PinePods')

        # convert this provisioning URL into a QR code and display it to the user
        # generate the QR code
        img = qrcode.make(provisioning_url)

        # Get current timestamp
        active_user.mfa_timestamp = datetime.datetime.now().strftime("%Y%m%d%H%M%S")

        # Save it to a file with a unique name
        filename = f"{user_data_dir}/{active_user.user_id}_qrcode_{active_user.mfa_timestamp}.png"  # for example
        img.save(filename)
        active_user.mfa_secret = secret

        return filename

    def seconds_to_time(seconds):
        minutes, seconds = divmod(seconds, 60)
        hours, minutes = divmod(minutes, 60)
        return '{:02d}:{:02d}:{:02d}'.format(hours, minutes, seconds)

    def get_progress(listen_time, duration):
        if duration == 0:
            progress = 0
            return progress
        else:
            progress = listen_time / duration
            return progress

    def check_image(artwork_path):
        if artwork_path.startswith('http'):
            # It's a URL, so return the path with the proxy URL appended
            return f"{proxy_url}{artwork_path}"
        else:
            # It's a local file path, so return the path as is
            return artwork_path

    def evaluate_podcast(pod_title, pod_artwork, pod_author, pod_categories, pod_description, pod_episode_count,
                         pod_feed_url, pod_website):
        global clicked_podcast
        clicked_podcast = Podcast(name=pod_title, artwork=pod_artwork, author=pod_author, description=pod_description,
                                  feedurl=pod_feed_url, website=pod_website, categories=pod_categories,
                                  episode_count=pod_episode_count)
        return clicked_podcast

    def download_episode_file(episode_url, podcast_name):
        download_dir = os.path.join(metadata_dir, 'downloads', podcast_name)
        os.makedirs(download_dir, exist_ok=True)

        response = requests.get(episode_url, stream=True)

        # The filename will be the last part of the URL
        filename = episode_url.split('/')[-1]
        file_path = os.path.join(download_dir, filename)

        with open(file_path, 'wb') as f:
            for chunk in response.iter_content(chunk_size=8192):
                if chunk:  # filter out keep-alive new chunks
                    f.write(chunk)

        return file_path





    def download_full_podcast(podcast_name, pod_feed, page):
        # First, get the list of all episodes in the podcast from the feed
        episode_list = api_functions.functions.call_get_all_episodes(app_api.url, app_api.headers, pod_feed)

        # If there are no episodes, return early
        if not episode_list:
            page.snack_bar = ft.SnackBar(content=ft.Text(f"No episodes found for podcast: {podcast_name}"))
            page.snack_bar.open = True
            page.update()
            return

        # Check if downloads are enabled
        download_status = api_functions.functions.call_download_status(app_api.url, app_api.headers)
        if not download_status:
            page.snack_bar = ft.SnackBar(content=ft.Text(
                f"Downloads are currently disabled! If you'd like to download episodes ask your administrator to enable the option."))
            page.snack_bar.open = True
            page.update()
            return

        # Add all episode URLs to the downloading list
        for episode in episode_list:
            active_user.downloading.append(episode['EpisodeURL'])
            active_user.downloading_name.append(episode['EpisodeTitle'])

        # Create a progress ring and add it to the page
        pr_instance.touch_stack()
        page.update()

        # For each episode in the podcast, try to download it
        for episode in episode_list:
            url = episode['EpisodeURL']
            title = episode['EpisodeTitle']

            # Check if the episode is already downloaded
            check_downloads = api_functions.functions.call_check_downloaded(app_api.url, app_api.headers,
                                                                            active_user.user_id, title, url)
            if check_downloads:
                page.snack_bar = ft.SnackBar(content=ft.Text(f"Episode: {title} is already downloaded!"))
                page.snack_bar.open = True
                page.update()
                continue

            # If it's not already downloaded, download the episode
            current_episode.url = url
            current_episode.title = title
            current_episode.download_pod()

            # Remove the downloaded episode URL from the downloading list
            active_user.downloading.remove(url)
            active_user.downloading_name.remove(title)

            page.snack_bar = ft.SnackBar(content=ft.Text(f"Episode: {title} has been downloaded!"))
            page.snack_bar.open = True
            page.update()

        # When all episodes are downloaded, remove the progress ring
        if pr_instance.active_pr == True:
            pr_instance.rm_stack()
        page.update()

    class Podcast:
        def __init__(self, name=None, artwork=None, author=None, description=None, feedurl=None, website=None,
                     categories=None, episode_count=None):
            self.name = name
            self.artwork = artwork
            self.author = author
            self.description = description
            self.feedurl = feedurl
            self.website = website
            self.categories = categories
            self.episode_count = episode_count

    class MyHTMLParser(HTMLParser):
        def __init__(self):
            self.is_html = False
            super().__init__()

        def handle_starttag(self, tag, attrs):
            self.is_html = True

    def is_html(text):
        parser = MyHTMLParser()
        parser.feed(text)
        return parser.is_html

    def self_service_user(self):
        def close_self_serv_dlg(page):
            self_service_dlg.open = False
            self.page.update()

        self_service_status = api_functions.functions.call_self_service_status(app_api.url, app_api.headers)

        if not self_service_status:
            self_service_dlg = ft.AlertDialog(
                modal=True,
                title=ft.Text(f"User Creation"),
                content=ft.Column(controls=[
                    ft.Text(
                        "Self Service User Creation is disabled. If you'd like an account please contact the admin or have them enable self service.")
                ], tight=True),
                actions=[
                    ft.TextButton("Close", on_click=close_self_serv_dlg)
                ],
                actions_alignment=ft.MainAxisAlignment.SPACE_EVENLY
            )
            self.page.dialog = self_service_dlg
            self_service_dlg.open = True
            self.page.update()

        elif self_service_status:
            new_user = User(page)

            self_service_name = ft.TextField(label="Full Name", icon=ft.icons.CARD_MEMBERSHIP,
                                             hint_text='John PinePods')
            self_service_email = ft.TextField(label="Email", icon=ft.icons.EMAIL,
                                              hint_text='ilovepinepods@pinepods.com')
            self_service_username = ft.TextField(label="Username", icon=ft.icons.PERSON, hint_text='pinepods_user1999')
            self_service_password = ft.TextField(label="Password", icon=ft.icons.PASSWORD, password=True,
                                                 can_reveal_password=True, hint_text='mY_SuPeR_S3CrEt!')
            self_service_dlg = ft.AlertDialog(
                modal=True,
                title=ft.Text(f"Create User:"),
                content=ft.Column(controls=[
                    self_service_name,
                    self_service_email,
                    self_service_username,
                    self_service_password
                ],
                    tight=True),
                actions=[
                    ft.TextButton("Create User", on_click=lambda x: (
                        new_user.set_username(self_service_username.value),
                        new_user.set_password(self_service_password.value),
                        new_user.set_email(self_service_email.value),
                        new_user.set_name(self_service_name.value),
                        new_user.verify_user_values_snack(),
                        new_user.create_user(),
                        new_user.user_created_snack(),
                        close_self_serv_dlg(page)
                    )),

                    ft.TextButton("Cancel", on_click=lambda x: close_self_serv_dlg(page))
                ],
                actions_alignment=ft.MainAxisAlignment.SPACE_EVENLY
            )
            self.page.dialog = self_service_dlg
            self_service_dlg.open = True
            self.page.update()

    class Toggle_Pod:
        initialized = False

        def __init__(self, page, go_home, url=None, name=None, length=None):
            if not Toggle_Pod.initialized:
                self.page = page
                self.go_home = go_home
                self.url = url
                self.name = name or ""
                self.title = ""
                self.artwork = ""
                self.audio_playing = False
                self.episode_file = url
                self.episode_name = name
                self.audio_element = None  # HTML5 audio element
                self.thread = None
                self.length = length or ""
                self.length_min = 0
                self.length_max = 3000
                self.seconds = 1
                self.pod_loaded = False
                self.last_listen_duration_update = datetime.datetime.now()
                self.volume = 1
                self.volume_timer = None
                self.volume_changed = False
                self.audio_con_art_url_parsed = None
                self.loading_audio = False
                self.local = False
                self.name_truncated = 'placeholder'
                # self.episode_name = self.name
                if url is None or name is None:
                    self.active_pod = 'Initial Value'
                else:
                    self.active_pod = self.name
                self.queue = []
                self.state = 'stopped'
                self.fs_play_button = ft.IconButton(
                    icon=ft.icons.PLAY_ARROW,
                    tooltip="Play Podcast",
                    icon_color="white",
                    on_click=lambda e: current_episode.fs_resume_podcast()
                )
                self.fs_pause_button = ft.IconButton(
                    icon=ft.icons.PAUSE,
                    tooltip="Pause Playback",
                    icon_color="white",
                    on_click=lambda e: current_episode.fs_pause_episode()
                )
                Toggle_Pod.initialized = True
            else:
                self.page = page
                self.go_home = go_home
                self.url = url
                self.name = name or ""
                self.title = ""
                self.artwork = ""
                self.audio_playing = False
                self.active_pod = self.name
                self.episode_file = url
                self.episode_name = name
                self.audio_element = None  # HTML5 audio element
                self.thread = None
                self.length = length or ""
                self.length_min = 0
                self.length_max = 3000
                self.seconds = 1
                self.pod_loaded = False
                self.last_listen_duration_update = datetime.datetime.now()
                self.volume = 1
                self.volume_timer = None
                self.volume_changed = False
                self.loading_audio = False
                self.local = False
                self.name_truncated = 'placeholder'
                # self.episode_name = self.name
                self.queue = []
                self.state = 'stopped'
                self.fs_play_button = ft.IconButton(
                    icon=ft.icons.PLAY_ARROW,
                    tooltip="Play Podcast",
                    icon_color="white",
                    on_click=lambda e: current_episode.fs_resume_podcast()
                )
                self.fs_pause_button = ft.IconButton(
                    icon=ft.icons.PAUSE,
                    tooltip="Pause Playback",
                    icon_color="white",
                    on_click=lambda e: current_episode.fs_pause_episode()
                )

        def run_function_every_60_seconds(self):
            while True:
                time.sleep(60)
                if self.audio_playing:
                    api_functions.functions.call_increment_listen_time(app_api.url, app_api.headers,
                                                                       active_user.user_id)

        def play_episode(self, e=None, listen_duration=None):
            api_functions.functions.call_queue_bump(app_api.url, app_api.headers, self.url, self.title,
                                                   active_user.user_id)
            if self.loading_audio == True:
                page.snack_bar = ft.SnackBar(content=ft.Text(
                    f"Please wait until current podcast has finished loading before selecting a new one."))
                page.snack_bar.open = True
                self.page.update()
            else:
                self.loading_audio = True
                pr_instance.touch_stack()
                page.update()
                # release audio_element if it exists
                if self.audio_element:
                    self.audio_element.release()
                # Preload the audio file and cache it
                global cache
                preload_audio_file(self.url, proxy_url, cache)

                self.audio_element = ft.Audio(src=f'{proxy_url}{urllib.parse.quote(self.url)}', autoplay=True, volume=1, on_state_changed=lambda e: self.on_state_changed(e.data))
                page.overlay.append(self.audio_element)
                # self.audio_element.play()

                self.audio_playing = True
                page.update()

                max_retries = 50
                sleep_time = 0.25
                tries = 0

                while tries < max_retries:
                    try:
                        duration = self.audio_element.get_duration()
                        if duration > 0:
                            media_length = duration
                            self.media_length = media_length
                            break
                    except Exception as e:
                        pass

                    tries += 1
                    time.sleep(sleep_time)

                if tries == max_retries:
                    page.snack_bar = ft.SnackBar(
                        content=ft.Text(f"Unable to load episode. Perhaps it no longer exists?"))
                    page.snack_bar.open = True
                    pr_instance.rm_stack()
                    self.audio_element.release()
                    self.page.update()
                    return

                if listen_duration:
                    listen_math = listen_duration * 1000
                    self.audio_element.seek(listen_math)

                self.record_history()
                api_functions.functions.call_increment_played(app_api.url, app_api.headers, active_user.user_id)

                # convert milliseconds to a timedelta object
                delta = datetime.timedelta(milliseconds=media_length)

                # convert timedelta object to datetime object
                datetime_obj = datetime.datetime(1, 1, 1) + delta

                # format datetime object to hh:mm:ss format with two decimal places
                total_length = datetime_obj.strftime('%H:%M:%S')
                time.sleep(1)
                self.length = total_length
                self.toggle_current_status()

                pr_instance.rm_stack()
                page.update()
                self.loading_audio = False
                self.local = False

                # convert milliseconds to seconds
                total_seconds = media_length // 1000
                self.seconds = total_seconds
                pod_controls.audio_scrubber.max = self.seconds

                threading.Thread(target=self.run_function_every_60_seconds, daemon=True).start()

                for i in range(total_seconds):
                    current_time = self.get_current_time()
                    if current_time is None:
                        continue
                    self.current_progress = current_time
                    self.toggle_second_status(self.audio_element.data)
                    time.sleep(1)

                    if (datetime.datetime.now() - self.last_listen_duration_update).total_seconds() > 15:
                        if self.audio_playing == True:
                            self.record_listen_duration()
                            self.last_listen_duration_update = datetime.datetime.now()

        def skip_episode(self):
            next_episode_url = self.queue.pop(0)
            self.play_episode(next_episode_url)

        def on_state_changed(self, status):
            self.state = status
            if status == 'completed':
                api_functions.functions.call_remove_queue_pod(app_api.url, app_api.headers, self.url, self.title,
                                                              active_user.user_id)
                self.queue = api_functions.functions.call_queued_episodes(app_api.url, app_api.headers,
                                                                          active_user.user_id)
                if len(self.queue) > 0:
                    next_episode = self.queue[0]  # First episode in the queue after sorting by QueuePosition
                    current_episode.url = next_episode['EpisodeURL']
                    current_episode.name = next_episode['EpisodeTitle']
                    current_episode.artwork = next_episode['EpisodeArtwork']
                    self.play_episode()
                else:
                    self.audio_element.release()
                    self.audio_playing = False
                    self.toggle_current_status()

        def pause_episode(self, e=None):
            self.audio_element.pause()
            self.audio_playing = False
            self.toggle_current_status()
            self.page.update()

        def resume_podcast(self, e=None):
            self.audio_element.resume()
            self.audio_playing = True
            self.toggle_current_status()
            self.page.update()

        def fs_pause_episode(self, e=None):
            self.fs_play_button.visible = True
            self.fs_pause_button.visible = False
            self.audio_element.pause()
            self.audio_playing = False
            self.fs_toggle_current_status()
            self.page.update()

        def fs_resume_podcast(self, e=None):
            self.fs_pause_button.visible = True
            self.fs_play_button.visible = False
            self.audio_element.resume()
            self.audio_playing = True
            self.fs_toggle_current_status()
            self.page.update()

        def fs_toggle_current_status(self):
            if self.audio_playing:
                pod_controls.play_button.visible = False
                pod_controls.pause_button.visible = True
                pod_controls.audio_container.bgcolor = active_user.main_color
                pod_controls.audio_container.visible = False
                max_chars = character_limit(int(page.width))
                self.name_truncated = truncate_text(self.name, max_chars)
                pod_controls.currently_playing.content = ft.Text(self.name_truncated, size=16)
                pod_controls.current_time.content = ft.Text(self.length, color=active_user.font_color)
                pod_controls.podcast_length.content = ft.Text(self.length)
                audio_con_artwork_no = random.randint(1, 12)
                audio_con_art_fallback = os.path.join(script_dir, "images", "logo_random",
                                                      f"{audio_con_artwork_no}.jpeg")
                audio_con_art_url = self.artwork if self.artwork else audio_con_art_fallback
                audio_con_art_url_parsed = check_image(audio_con_art_url)
                self.audio_con_art_url_parsed = audio_con_art_url_parsed
                pod_controls.audio_container_image_landing.src = audio_con_art_url_parsed
                pod_controls.audio_container_image_landing.width = 40
                pod_controls.audio_container_image_landing.height = 40
                pod_controls.audio_container_image_landing.border_radius = ft.border_radius.all(100)
                pod_controls.audio_container_image.border_radius = ft.border_radius.all(75)
                pod_controls.audio_container_image_landing.update()
                pod_controls.audio_scrubber.active_color = active_user.nav_color2
                pod_controls.audio_scrubber.inactive_color = active_user.nav_color2
                pod_controls.audio_scrubber.thumb_color = active_user.accent_color
                pod_controls.volume_container.bgcolor = active_user.main_color
                pod_controls.volume_down_icon.icon_color = active_user.accent_color
                pod_controls.volume_up_icon.icon_color = active_user.accent_color
                pod_controls.volume_button.icon_color = active_user.accent_color
                pod_controls.volume_slider.active_color = active_user.nav_color2
                pod_controls.volume_slider.inactive_color = active_user.nav_color2
                pod_controls.volume_slider.thumb_color = active_user.accent_color
                pod_controls.play_button.icon_color = active_user.accent_color
                pod_controls.pause_button.icon_color = active_user.accent_color
                pod_controls.seek_button.icon_color = active_user.accent_color
                pod_controls.currently_playing.color = active_user.font_color
                # current_time_text.color = active_user.font_color
                pod_controls.podcast_length.color = active_user.font_color
                self.page.update()
            else:
                pod_controls.pause_button.visible = False
                pod_controls.play_button.visible = True
                pod_controls.currently_playing.content = ft.Text(self.name_truncated, color=active_user.font_color, size=16)
                self.page.update()

        def toggle_current_status(self):
            if self.audio_playing:
                pod_controls.play_button.visible = False
                pod_controls.pause_button.visible = True
                pod_controls.audio_container.bgcolor = active_user.main_color
                pod_controls.audio_container.visible = True
                max_chars = character_limit(int(page.width))
                self.name_truncated = truncate_text(self.name, max_chars)
                pod_controls.currently_playing.content = ft.Text(self.name_truncated, size=16)
                pod_controls.current_time.content = ft.Text(self.length, color=active_user.font_color)
                pod_controls.podcast_length.content = ft.Text(self.length)
                audio_con_artwork_no = random.randint(1, 12)
                audio_con_art_fallback = os.path.join(script_dir, "images", "logo_random",
                                                      f"{audio_con_artwork_no}.jpeg")
                audio_con_art_url = self.artwork if self.artwork else audio_con_art_fallback
                audio_con_art_url_parsed = check_image(audio_con_art_url)
                self.audio_con_art_url_parsed = audio_con_art_url_parsed
                pod_controls.audio_container_image_landing.src = audio_con_art_url_parsed
                pod_controls.audio_container_image_landing.width = 40
                pod_controls.audio_container_image_landing.height = 40
                pod_controls.audio_container_image_landing.border_radius = ft.border_radius.all(100)
                pod_controls.audio_container_image.border_radius = ft.border_radius.all(75)
                pod_controls.audio_container_image_landing.update()
                pod_controls.audio_scrubber.active_color = active_user.nav_color2
                pod_controls.audio_scrubber.inactive_color = active_user.nav_color2
                pod_controls.audio_scrubber.thumb_color = active_user.accent_color
                pod_controls.volume_container.bgcolor = active_user.main_color
                pod_controls.volume_down_icon.icon_color = active_user.accent_color
                pod_controls.volume_up_icon.icon_color = active_user.accent_color
                pod_controls.volume_button.icon_color = active_user.accent_color
                pod_controls.volume_slider.active_color = active_user.nav_color2
                pod_controls.volume_slider.inactive_color = active_user.nav_color2
                pod_controls.volume_slider.thumb_color = active_user.accent_color
                pod_controls.play_button.icon_color = active_user.accent_color
                pod_controls.pause_button.icon_color = active_user.accent_color
                pod_controls.seek_button.icon_color = active_user.accent_color
                pod_controls.currently_playing.color = active_user.font_color
                pod_controls.podcast_length.color = active_user.font_color
                self.page.update()
            else:
                pod_controls.pause_button.visible = False
                pod_controls.play_button.visible = True
                pod_controls.currently_playing.content = ft.Text(self.name_truncated, color=active_user.font_color, size=16)
                self.page.update()

        def volume_view(self):
            if pod_controls.volume_container.visible:
                pod_controls.volume_container.visible = False
                pod_controls.volume_container.update()
            else:
                pod_controls.volume_container.visible = True
                pod_controls.volume_container.update()
                self.volume_timer = threading.Timer(10, self.hide_volume_container)
                self.volume_timer.start()

        def volume_adjust(self):
            self.audio_element.volume = pod_controls.volume_slider.value
            self.audio_element.update()
            self.volume_changed = True
            if self.volume_timer:
                self.volume_timer.cancel()
            self.volume_timer = threading.Timer(5, self.hide_volume_container)
            self.volume_timer.start()

        def hide_volume_container(self):
            if not self.volume_changed:
                pod_controls.volume_container.visible = False
                pod_controls.volume_container.update()
                self.volume_timer = None
            else:
                self.volume_changed = False

        def toggle_second_status(self, status):
            if self.state == 'playing':
                pod_controls.audio_scrubber.value = self.get_current_seconds()
                pod_controls.audio_scrubber.update()
                pod_controls.current_time.content = ft.Text(self.current_progress, color=active_user.font_color)
                pod_controls.current_time.update()

            # self.page.update()

        def seek_episode(self):
            time = self.audio_element.get_current_position()
            seek_position = time + 10000
            self.audio_element.seek(seek_position)

        def seek_back_episode(self):
            time = self.audio_element.get_current_position()
            seek_position = time - 10000
            self.audio_element.seek(seek_position)

        def time_scrub(self, time):
            """
            Seeks to a specific time within the podcast.

            Args:
                time (int): The time in seconds to seek to.
            """
            time_ms = int(time * 1000)  # convert seconds to milliseconds
            if time_ms < 0:
                time_ms = 0
            elif time > self.seconds:
                time = self.seconds
            self.audio_element.seek(time_ms)

        def record_history(self):
            api_functions.functions.call_record_podcast_history(app_api.url, app_api.headers, self.title,
                                                                active_user.user_id, 0)

        def download_pod(self):
            api_functions.functions.call_download_podcast(app_api.url, app_api.headers, self.url, self.title,
                                                          active_user.user_id)

        def queue_pod(self, url, title):
            if not self.audio_playing:

                self.play_episode()
            else:
                api_functions.functions.call_queue_pod(app_api.url, app_api.headers, url, title,
                                                          active_user.user_id)

        def remove_queued_pod(self):
            try:
                api_functions.functions.call_remove_queue_pod(app_api.url, app_api.headers, self.url, self.title,
                                                       active_user.user_id)

            except ValueError:
                page.snack_bar = ft.SnackBar(content=ft.Text(f"Error: Episode not found in queue"))
                page.snack_bar.open = True
                self.page.update()

        def save_pod(self):
            api_functions.functions.call_save_episode(app_api.url, app_api.headers, self.url, self.title,
                                                      active_user.user_id)

        def remove_saved_pod(self):
            api_functions.functions.call_remove_saved_episode(app_api.url, app_api.headers, self.url, self.title,
                                                              active_user.user_id)

        def get_current_time(self):
            try:
                time = self.audio_element.get_current_position() // 1000  # convert milliseconds to seconds
            except Exception as e:
                if "Timeout" in str(e):  # Check if the exception is related to a timeout
                    return None
                time = self.audio_element.get_current_position() // 1000
            hours, remainder = divmod(time, 3600)
            minutes, seconds = divmod(remainder, 60)
            return f"{hours:02d}:{minutes:02d}:{seconds:02d}"

        def get_current_seconds(self):
            try:
                time_ms = self.audio_element.get_current_position()  # get current time in milliseconds
                if time_ms is not None:
                    time_sec = int(time_ms // 1000)  # convert milliseconds to seconds
                    return time_sec
                else:
                    return 0
            except Exception as e:
                time_ms = self.audio_element.get_current_position()  # get current time in milliseconds
                if time_ms is not None:
                    time_sec = int(time_ms // 1000)  # convert milliseconds to seconds
                    return time_sec
                else:
                    return 0

        def record_listen_duration(self):
            listen_duration = self.get_current_seconds()
            api_functions.functions.call_record_listen_duration(app_api.url, app_api.headers, self.url, self.name,
                                                                active_user.user_id, listen_duration)

    # ---Flet Various Elements----------------------------------------------------------------
    def close_invalid_dlg(e):
        username_invalid_dlg.open = False
        password_invalid_dlg.open = False
        email_invalid_dlg.open = False
        username_exists_dlg.open = False
        page.update()
        # Define User Creation Dialog

    user_dlg = ft.AlertDialog(
        modal=True,
        title=ft.Text("New User Created!"),
        content=ft.Text("You can now log in as this user"),
        actions=[
            ft.TextButton("Okay", on_click=close_dlg),
        ],
        actions_alignment=ft.MainAxisAlignment.END,
        on_dismiss=lambda e: go_home
    )
    username_invalid_dlg = ft.AlertDialog(
        modal=True,
        title=ft.Text("Username Invalid!"),
        content=ft.Text("Usernames must be unique and require at least 6 characters!"),
        actions=[
            ft.TextButton("Okay", on_click=close_invalid_dlg),
        ],
        actions_alignment=ft.MainAxisAlignment.END
    )
    password_invalid_dlg = ft.AlertDialog(
        modal=True,
        title=ft.Text("Password Invalid!"),
        content=ft.Text("Passwords require at least 8 characters, a number, a capital letter and a special character!"),
        actions=[
            ft.TextButton("Okay", on_click=close_invalid_dlg),
        ],
        actions_alignment=ft.MainAxisAlignment.END
    )
    email_invalid_dlg = ft.AlertDialog(
        modal=True,
        title=ft.Text("Invalid Email!"),
        content=ft.Text("Email appears to be non-standard email layout!"),
        actions=[
            ft.TextButton("Okay", on_click=close_invalid_dlg),
        ],
        actions_alignment=ft.MainAxisAlignment.END
    )
    username_exists_dlg = ft.AlertDialog(
        modal=True,
        title=ft.Text("Username already exists"),
        content=ft.Text("This username is already in use. Please try another."),
        actions=[
            ft.TextButton("Okay", on_click=close_invalid_dlg),
        ],
        actions_alignment=ft.MainAxisAlignment.END
    )


    # --Defining Routes---------------------------------------------------

    def start_config(page):
        page.go("/server_config")

    def first_time_config(page):
        page.go("/first_time_config")

    def start_login(page):
        page.go("/login")

    def open_mfa_login(e):
        page.go("/mfalogin")

    def view_pop(e):
        page.views.pop()
        top_view = page.views[-1]
        page.go(top_view.route)

    def open_poddisplay(e):
        pr_instance.touch_stack()
        page.update()
        page.go("/poddisplay")

    def open_settings(e):
        page.go("/settings")

    def open_queue(e):
        page.go("/queue")

    def open_downloads(e):
        page.go("/downloads")

    def open_saved_pods(e):
        page.go("/saved")

    def open_history(e):
        page.go("/history")

    def open_user_stats(e):
        page.go("/userstats")

    def open_currently_playing(e):
        page.go("/playing")

    def open_episode_select(page, url, title):
        current_episode.url = url
        current_episode.title = title
        page.go("/episode_display")

    def open_pod_list(e):
        page.update()
        page.go("/pod_list")

    def open_search(e):
        page.go("/user_search")

    def go_homelogin_guest(page):
        active_user.user_id = 1
        active_user.fullname = 'Guest User'
        active_user.theme_select()
        # Theme user elements
        page.banner.bgcolor = active_user.accent_color
        page.banner.leading = ft.Icon(ft.icons.WAVING_HAND, color=active_user.main_color, size=40)
        page.banner.content = ft.Text("""
    Welcome to PinePods! PinePods is an app built to save, listen, download, organize, and manage a selection of podcasts. Using the search function you can search for your favorite podcast, from there, click the add button to save your podcast to the database. PinePods will begin displaying new episodes of that podcast from then on to the homescreen when released. In addition, from search you can click on a podcast to view and listen to specific episodes. From the sidebar you can select your saved podcasts and manage them, view and manage your downloaded podcasts, edit app settings, check your listening history, and listen through episodes from your saved 'queue'. For more information on PinePods and the features it has please check out the documentation website listed below. For comments, feature requests, pull requests, and bug reports please open an issue, or fork PinePods from the repository and create a PR.
    """, color=active_user.main_color
                                      )
        page.banner.actions = [
            ft.ElevatedButton('Open PinePods Github Repo', on_click=open_repo, bgcolor=active_user.main_color,
                              color=active_user.accent_color),
            ft.ElevatedButton('Open PinePods Documentation Site', on_click=open_doc_site,
                              bgcolor=active_user.main_color, color=active_user.accent_color),
            ft.IconButton(icon=ft.icons.EXIT_TO_APP, on_click=close_banner, bgcolor=active_user.main_color)
        ]
        page.go("/first_time_config")

    def go_homelogin(page):
        active_user.theme_select()
        # Theme user elements
        page.banner.bgcolor = active_user.accent_color
        page.banner.leading = ft.Icon(ft.icons.WAVING_HAND, color=active_user.main_color, size=40)
        page.banner.content = ft.Text("""
    Welcome to PinePods! PinePods is an app built to save, listen, download, organize, and manage a selection of podcasts. Using the search function you can search for your favorite podcast, from there, click the add button to save your podcast to the database. PinePods will begin displaying new episodes of that podcast from then on to the homescreen when released. In addition, from search you can click on a podcast to view and listen to specific episodes. From the sidebar you can select your saved podcasts and manage them, view and manage your downloaded podcasts, edit app settings, check your listening history, and listen through episodes from your saved 'queue'. For more information on PinePods and the features it has please check out the documentation website listed below. For comments, feature requests, pull requests, and bug reports please open an issue, or fork PinePods from the repository and create a PR.
    """, color=active_user.main_color
                                      )
        page.banner.actions = [
            ft.ElevatedButton('Open PinePods Repo', on_click=open_repo, bgcolor=active_user.main_color,
                              color=active_user.accent_color),
            ft.ElevatedButton('Open PinePods Documentation Site', on_click=open_doc_site,
                              bgcolor=active_user.main_color, color=active_user.accent_color),
            ft.IconButton(icon=ft.icons.EXIT_TO_APP, on_click=close_banner, bgcolor=active_user.main_color)
        ]
        global new_nav
        new_nav = NavBar(page)
        new_nav.navbar.border = ft.border.only(right=ft.border.BorderSide(2, active_user.tertiary_color))
        new_nav.navbar_stack = ft.Stack([new_nav.navbar], expand=True)
        page.overlay.append(new_nav.navbar_stack)
        page.update()
        page.go("/")

    def reset_credentials(page):

        def close_self_service_pw_dlg(e):
            create_self_service_pw_dlg.open = False
            page.update()

        def create_reset_code(page, user_email):
            import random
            from cryptography.fernet import Fernet

            def close_code_pw_dlg(e):
                code_pw_dlg.open = False
                page.update()
            # Generate a random reset code
            reset_code = ''.join(random.choices(string.ascii_uppercase + string.digits, k=8))

            user_exist = api_functions.functions.call_reset_password_create_code(app_api.url, app_api.headers,
                                                                                 user_email, reset_code)
            if user_exist:
                def pw_reset(page, user_email, reset_code):
                    code_valid = api_functions.functions.call_verify_reset_code(app_api.url, app_api.headers,
                                                                                user_email, reset_code)
                    if code_valid == True:
                        def close_code_pw_reset_dlg(e):
                            code_pw_reset_dlg.open = False
                            page.update()

                        def verify_pw_reset(page, user_email, pw_reset_prompt, pw_verify_prompt):
                            if pw_reset_prompt == pw_verify_prompt:
                                salt, hash_pw = Auth.Passfunctions.hash_password(pw_reset_prompt)
                                api_functions.functions.call_reset_password_prompt(app_api.url, app_api.headers,
                                                                                   user_email, salt, hash_pw)
                                page.snack_bar = ft.SnackBar(content=ft.Text('Password Reset! You can now log in!'))
                                page.snack_bar.open = True
                                code_pw_reset_dlg.open = False
                                page.update()
                            else:
                                code_pw_reset_dlg.open = False
                                page.snack_bar = ft.SnackBar(
                                    content=ft.Text('Your Passwords do not match. Please try again.'))
                                page.snack_bar.open = True
                                page.update()
                        code_pw_dlg.open = False
                        page.update()
                        time.sleep(1)
                        pw_reset_prompt = ft.TextField(label="New Password", icon=ft.icons.PASSWORD, password=True,
                                                       can_reveal_password=True)
                        pw_verify_prompt = ft.TextField(label="Verify New Password", icon=ft.icons.PASSWORD,
                                                        password=True, can_reveal_password=True)
                        code_pw_reset_dlg = ft.AlertDialog(
                            modal=True,
                            title=ft.Text(f"Enter PW Reset Code:"),
                            content=ft.Column(controls=[
                                ft.Text("Reset Password:"),
                                ft.Text(f'Please enter your new password and then verify it below.', selectable=True),
                                pw_reset_prompt,
                                pw_verify_prompt
                            ], tight=True),
                            actions=[
                                ft.TextButton("Submit", on_click=lambda e: verify_pw_reset(page, user_email,
                                                                                           pw_reset_prompt.value,
                                                                                           pw_verify_prompt.value)),
                                ft.TextButton("Cancel", on_click=close_code_pw_reset_dlg)
                            ],
                            actions_alignment=ft.MainAxisAlignment.END
                        )
                        page.dialog = code_pw_reset_dlg
                        code_pw_reset_dlg.open = True
                        page.update()

                    else:
                        code_pw_dlg.open = False
                        page.snack_bar = ft.SnackBar(content=ft.Text('Code not valid. Please check your email.'))
                        page.snack_bar.open = True
                        page.update()
                # Create a progress ring while email sends
                pr_instance.touch_stack()
                create_self_service_pw_dlg.open = False
                page.update()
                # Send the reset code via email
                subject = "Your Password Reset Code"
                body = f"Your password reset code is: {reset_code}. This code will expire in 1 hour."
                email_information = api_functions.functions.call_get_email_info(app_api.url, app_api.headers)
                encrypt_key = api_functions.functions.call_get_encryption_key(app_api.url, app_api.headers)

                decoded_key = urlsafe_b64decode(encrypt_key)

                cipher_suite = Fernet(decoded_key)
                decrypted_text = cipher_suite.decrypt(email_information['Password'])
                decrypt_email_pw = decrypted_text.decode('utf-8')

                email_result = app_functions.functions.send_email(email_information['Server_Name'],
                                                                  email_information['Server_Port'],
                                                                  email_information['From_Email'], user_email,
                                                                  email_information['Send_Mode'],
                                                                  email_information['Encryption'],
                                                                  email_information['Auth_Required'],
                                                                  email_information['Username'], decrypt_email_pw,
                                                                  subject, body)
                page.snack_bar = ft.SnackBar(content=ft.Text(email_result))
                page.snack_bar.open = True
                page.update()
                create_self_service_pw_dlg.open = False

                code_reset_prompt = ft.TextField(label="Code", icon=ft.icons.PASSWORD)
                code_pw_dlg = ft.AlertDialog(
                    modal=True,
                    title=ft.Text(f"Enter PW Reset Code:"),
                    content=ft.Column(controls=[
                        ft.Text("Reset Password:"),
                        ft.Text(f'Please Enter the code that was sent to your email to reset your password.',
                                selectable=True),
                        code_reset_prompt
                    ], tight=True),
                    actions=[
                        ft.TextButton("Submit", on_click=lambda e: pw_reset(page, user_email, code_reset_prompt.value)),
                        ft.TextButton("Cancel", on_click=close_self_service_pw_dlg)
                    ],
                    actions_alignment=ft.MainAxisAlignment.END
                )
                page.dialog = code_pw_dlg
                code_pw_dlg.open = True
                pr_instance.rm_stack()
                page.update()

            else:
                page.snack_bar = ft.SnackBar(content=ft.Text('User not found with this email'))
                page.snack_bar.open = True
                page.update()

        pw_reset_email = ft.TextField(label="Email", icon=ft.icons.EMAIL, hint_text='ilovepinepods@pinepods.com')
        create_self_service_pw_dlg = ft.AlertDialog(
            modal=True,
            title=ft.Text(f"Reset Password:"),
            content=ft.Column(controls=[
                ft.Text(
                    f'To reset your password, please enter your email below and hit enter. An email will be sent to you with a code needed to reset if a user exists with that email.',
                    selectable=True),
                pw_reset_email
            ], tight=True),
            actions=[
                ft.TextButton("Submit", on_click=lambda e: create_reset_code(page, pw_reset_email.value)),
                ft.TextButton("Cancel", on_click=close_self_service_pw_dlg)
            ],
            actions_alignment=ft.MainAxisAlignment.END
        )
        page.dialog = create_self_service_pw_dlg
        create_self_service_pw_dlg.open = True
        page.update()

    def go_theme_rebuild(page):
        active_user.theme_select()
        # Theme user elements
        page.banner.bgcolor = active_user.accent_color
        page.banner.leading = ft.Icon(ft.icons.WAVING_HAND, color=active_user.main_color, size=40)
        page.banner.content = ft.Text("""
    Welcome to PinePods! PinePods is an app built to save, listen, download, organize, and manage a selection of podcasts. Using the search function you can search for your favorite podcast, from there, click the add button to save your podcast to the database. PinePods will begin displaying new episodes of that podcast from then on to the homescreen when released. In addition, from search you can click on a podcast to view and listen to specific episodes. From the sidebar you can select your saved podcasts and manage them, view and manage your downloaded podcasts, edit app settings, check your listening history, and listen through episodes from your saved 'queue.' For comments, feature requests, pull requests, and bug reports please open an issue, for fork PinePods from the repository:
    """, color=active_user.main_color
                                      )
        page.banner.actions = [
            ft.ElevatedButton('Open PinePods Repo', on_click=open_repo, bgcolor=active_user.main_color,
                              color=active_user.accent_color),
            ft.IconButton(icon=ft.icons.EXIT_TO_APP, on_click=close_banner, bgcolor=active_user.main_color)
        ]
        pod_controls.audio_container.bgcolor = active_user.main_color
        pod_controls.audio_scrubber.active_color = active_user.nav_color2
        pod_controls.audio_scrubber.inactive_color = active_user.nav_color2
        pod_controls.audio_scrubber.thumb_color = active_user.accent_color
        pod_controls.play_button.icon_color = active_user.accent_color
        pod_controls.pause_button.icon_color = active_user.accent_color
        pod_controls.seek_button.icon_color = active_user.accent_color
        pod_controls.currently_playing.color = active_user.font_color
        pod_controls.current_time.color = active_user.font_color
        pod_controls.podcast_length.color = active_user.font_color

        new_nav.navbar.border = ft.border.only(right=ft.border.BorderSide(2, active_user.tertiary_color))
        new_nav.navbar_stack = ft.Stack([new_nav.navbar], expand=True)
        page.overlay.append(new_nav.navbar_stack)
        page.update()
        page.go("/")

    def go_home(e):
        page.update()
        page.go("/")

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

    pr_instance = PR(page)

    def route_change(e):
        if pr_instance.active_pr == True:
            pr_instance.rm_stack()

        class Pod_View:
            def __init__(self, page):
                # self.view_list = ft.ListView(divider_thickness=3, auto_scroll=True)
                self.page = page
                self.ep_number = 1
                self.page_type = "None"
                self.row_list = ft.ListView(divider_thickness=3, auto_scroll=True)
                self.refresh_btn = ft.IconButton(icon=ft.icons.REFRESH, icon_color=active_user.font_color,
                                                 tooltip="Refresh Podcast List", on_click=self.refresh_podcasts)
                self.refresh_btn.icon_color = active_user.font_color
                self.refresh_ctn = ft.Container(
                    content=self.refresh_btn,
                    alignment=ft.alignment.top_left
                )
                self.banner_button = ft.ElevatedButton("Help!", on_click=show_banner_click)
                self.banner_button.bgcolor = active_user.accent_color
                self.banner_button.color = active_user.main_color
                self.settings_row = ft.Row(vertical_alignment=ft.CrossAxisAlignment.START,
                                           controls=[self.refresh_ctn, self.banner_button])
                self.search_row = ft.Row(spacing=20,
                                         controls=[page_items.search_pods, page_items.search_location, search_btn])
                self.top_row = ft.Row(alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
                                      vertical_alignment=ft.CrossAxisAlignment.START,
                                      controls=[self.settings_row, self.search_row])
                self.top_row_container = ft.Container(content=self.top_row, expand=True)
                self.top_row_container.padding = ft.padding.only(left=60)
                self.top_bar = ft.Row(vertical_alignment=ft.CrossAxisAlignment.START, controls=[self.top_row_container])
                if current_episode.audio_playing == True:
                    pod_controls.audio_container.visible = True

            def refresh_episodes(self):
                # Fetch new podcast episodes from the server.
                if self.page_type == "saved":
                    current_page_eps = api_functions.functions.call_saved_episode_list(app_api.url, app_api.headers,
                                                                                       active_user.user_id)
                elif self.page_type == "history":
                    current_page_eps = api_functions.functions.call_user_history(app_api.url, app_api.headers,
                                                                                 active_user.user_id)
                elif self.page_type == "queue":
                    current_page_eps = api_functions.functions.call_queued_episodes(app_api.url, app_api.headers,
                                                                             active_user.user_id)
                # Update the list with the new episodes.
                self.define_values(current_page_eps)

            def remove_saved_episode(self, url, title):
                current_episode.url = url
                current_episode.title = title
                current_episode.remove_saved_pod()
                self.page.snack_bar = ft.SnackBar(content=ft.Text(f"Episode: {title} has been removed from saved podcasts!"))
                self.page.snack_bar.open = True
                self.refresh_episodes()
                self.page.update()

            def episode_remove_queue(self, url, title):
                current_episode.url = url
                current_episode.title = title
                current_episode.remove_queued_pod()
                self.page.snack_bar = ft.SnackBar(content=ft.Text(f"Episode: {title} has been removed from the queue!"))
                self.page.snack_bar.open = True
                self.refresh_episodes()
                self.page.update()

            def episode_remove_history(self, url, title):
                api_functions.functions.call_remove_episode_history(app_api.url, app_api.headers, url, title,
                                                                    active_user.user_id)
                self.page.snack_bar = ft.SnackBar(content=ft.Text(f"Episode: {title} has been removed from history!"))
                self.page.snack_bar.open = True
                self.refresh_episodes()
                self.page.update()

            def refresh_podcasts(self, e):
                self.page.update()
                api_functions.functions.call_refresh_pods(app_api.url, app_api.headers)
                self.page.snack_bar = ft.SnackBar(content=ft.Text(f"Refresh Initiated!"))
                self.page.snack_bar.open = True
                self.page.update()

            def define_values(self, episodes):
                self.row_list.controls.clear()
                for values in episodes:
                    ep_title = values['EpisodeTitle']
                    pod_name = values['PodcastName']
                    unfilt_pub_date = values['EpisodePubDate']
                    # Parse the string into a datetime object
                    dt = datetime.datetime.strptime(unfilt_pub_date, "%Y-%m-%d")
                    # Format it in your desired format
                    pub_date = dt.strftime("%b %d, %Y")
                    ep_desc = values['EpisodeDescription']
                    ep_artwork = values['EpisodeArtwork']
                    ep_url = values['EpisodeURL']
                    # Now fetch the ListenDuration from the returned data
                    listen_duration = values.get('ListenDuration')
                    if self.page_type == "history":
                        ep_listen_date = values['ListenDate']
                    if self.page_type == "queue":
                        ep_queue_date = values['QueueDate']
                        ep_queue_position = values['QueuePosition']
                    ep_duration = values['EpisodeDuration']
                    # do something with the episode information
                    entry_title_button = ft.Text(f'{pod_name} - {ep_title}',
                                                 style=ft.TextThemeStyle.TITLE_MEDIUM,
                                                 color=active_user.font_color)
                    entry_title = ft.TextButton(content=entry_title_button,
                                                on_click=lambda x, url=ep_url,
                                                                title=ep_title: open_episode_select(page, url,
                                                                                                    title))
                    entry_seemore = ft.TextButton(text="See More...")
                    num_lines = ep_desc.count('\n')
                    if num_lines > 15:
                        if is_html(ep_desc):
                            # convert HTML to Markdown
                            markdown_desc = html2text.html2text(ep_desc)
                            if num_lines > 15:
                                # Split into lines, truncate to 15 lines, and join back into a string
                                lines = markdown_desc.splitlines()[:15]
                                markdown_desc = '\n'.join(lines)
                            # add inline style to change font color
                            entry_description = ft.Markdown(markdown_desc, on_tap_link=launch_clicked_url)
                            entry_seemore = ft.TextButton(text="See More...", on_click=lambda x,
                                                                                              url=ep_url,
                                                                                              title=ep_title: open_episode_select(
                                page, url, title))
                            entry_seemore.visible = False
                        else:
                            if num_lines > 15:
                                # Split into lines, truncate to 15 lines, and join back into a string
                                lines = ep_desc.splitlines()[:15]
                                ep_desc = '\n'.join(lines)
                            # display plain text
                            entry_description = ft.Text(ep_desc)

                    else:
                        if is_html(ep_desc):
                            # convert HTML to Markdown
                            markdown_desc = html2text.html2text(ep_desc)
                            # add inline style to change font color
                            entry_description = ft.Markdown(markdown_desc, on_tap_link=launch_clicked_url,
                                                            selectable=True)
                        else:
                            # display plain text
                            entry_description = ft.Text(ep_desc, selectable=True)
                    entry_released = ft.Text(f'Released on: {pub_date}', color=active_user.font_color)
                    art_no = random.randint(1, 12)
                    art_fallback = os.path.join(script_dir, "images", "logo_random", f"{art_no}.jpeg")
                    art_url = ep_artwork if ep_artwork else art_fallback
                    art_url_parsed = check_image(art_url)
                    entry_artwork_url = ft.Image(src=art_url_parsed, width=150, height=150)
                    ep_play_button = ft.IconButton(
                        icon=ft.icons.NOT_STARTED,
                        icon_color=active_user.accent_color,
                        icon_size=40,
                        tooltip="Start Episode From Beginning",
                        on_click=lambda x, url=ep_url, title=ep_title,
                                        artwork=ep_artwork: play_selected_episode(url, title, artwork)
                    )
                    ep_resume_button = ft.IconButton(
                        icon=ft.icons.PLAY_CIRCLE,
                        icon_color=active_user.accent_color,
                        icon_size=40,
                        tooltip="Resume Episode",
                        on_click=lambda x, url=ep_url, title=ep_title, artwork=ep_artwork,
                                        listen_duration=listen_duration: resume_selected_episode(url, title, artwork,
                                                                                                 listen_duration)
                    )
                    if self.page_type == "saved":
                        popup_button = ft.PopupMenuButton(
                            content=ft.Icon(ft.icons.ARROW_DROP_DOWN_CIRCLE_ROUNDED, color=active_user.accent_color,
                                            size=40, tooltip="Play Episode"),
                            items=[
                                ft.PopupMenuItem(icon=ft.icons.QUEUE, text="Queue",
                                                 on_click=lambda x, url=ep_url, title=ep_title,
                                                                 artwork=ep_artwork: queue_selected_episode(url,
                                                                                                            title,
                                                                                                            artwork,
                                                                                                            page)),
                                ft.PopupMenuItem(icon=ft.icons.DOWNLOAD, text="Server Download",
                                                 on_click=lambda x, url=ep_url,
                                                                 title=ep_title: download_selected_episode(url, title,
                                                                                                           page)),
                                ft.PopupMenuItem(icon=ft.icons.SAVE, text="Remove Saved Episode",
                                                 on_click=lambda x, url=ep_url,
                                                                 title=ep_title: self.remove_saved_episode(url, title))
                            ]
                        )
                    elif self.page_type == "history":
                        popup_button = ft.PopupMenuButton(
                            content=ft.Icon(ft.icons.ARROW_DROP_DOWN_CIRCLE_ROUNDED, color=active_user.accent_color,
                                            size=40, tooltip="Play Episode"),
                            items=[
                                ft.PopupMenuItem(icon=ft.icons.QUEUE, text="Remove From History",
                                                 on_click=lambda x, url=ep_url,
                                                                 title=ep_title: self.episode_remove_history(url,
                                                                                                             title)),
                                ft.PopupMenuItem(icon=ft.icons.QUEUE, text="Queue",
                                                 on_click=lambda x, url=ep_url, title=ep_title,
                                                                 artwork=ep_artwork: queue_selected_episode(url,
                                                                                                            title,
                                                                                                            artwork,
                                                                                                            page)),
                                ft.PopupMenuItem(icon=ft.icons.DOWNLOAD, text="Server Download",
                                                 on_click=lambda x, url=ep_url,
                                                                 title=ep_title: download_selected_episode(url, title,
                                                                                                           page)),
                                ft.PopupMenuItem(icon=ft.icons.SAVE, text="Save Episode",
                                                 on_click=lambda x, url=ep_url,
                                                                 title=ep_title: save_selected_episode(url, title,
                                                                                                       page))
                            ]
                        )
                    elif self.page_type == "queue":
                        popup_button = ft.PopupMenuButton(
                            content=ft.Icon(ft.icons.ARROW_DROP_DOWN_CIRCLE_ROUNDED, color=active_user.accent_color,
                                            size=40, tooltip="Play Episode"),
                            items=[
                                ft.PopupMenuItem(icon=ft.icons.QUEUE, text="Remove From Queue",
                                                 on_click=lambda x, url=ep_url,
                                                                 title=ep_title: self.episode_remove_queue(url, title)),
                                ft.PopupMenuItem(icon=ft.icons.DOWNLOAD, text="Server Download",
                                                 on_click=lambda x, url=ep_url,
                                                                 title=ep_title: download_selected_episode(url, title,
                                                                                                           page)),
                                ft.PopupMenuItem(icon=ft.icons.SAVE, text="Save Episode",
                                                 on_click=lambda x, url=ep_url,
                                                                 title=ep_title: save_selected_episode(url, title,
                                                                                                       page))
                            ]
                        )
                    else:
                        popup_button = ft.PopupMenuButton(
                            content=ft.Icon(ft.icons.ARROW_DROP_DOWN_CIRCLE_ROUNDED, color=active_user.accent_color,
                                            size=40, tooltip="Play Episode"),
                            items=[
                                ft.PopupMenuItem(icon=ft.icons.QUEUE, text="Queue",
                                                 on_click=lambda x, url=ep_url, title=ep_title,
                                                                 artwork=ep_artwork: queue_selected_episode(url, title,
                                                                                                            artwork,
                                                                                                            page)),
                                ft.PopupMenuItem(icon=ft.icons.DOWNLOAD, text="Server Download",
                                                 on_click=lambda x, url=ep_url,
                                                                 title=ep_title: download_selected_episode(url, title,
                                                                                                           page)),
                                ft.PopupMenuItem(icon=ft.icons.SAVE, text="Save Episode",
                                                 on_click=lambda x, url=ep_url,
                                                                 title=ep_title: save_selected_episode(url, title,
                                                                                                       page))
                            ]
                        )
                    rotate_button = ft.IconButton(
                        icon=ft.icons.ARROW_FORWARD_IOS,
                        icon_color=active_user.accent_color,
                        tooltip="Show Description",
                        rotate=ft.transform.Rotate(0, alignment=ft.alignment.center),
                        animate_rotation=ft.animation.Animation(300, ft.AnimationCurve.BOUNCE_OUT),
                    )

                    if listen_duration is not None:
                        listen_prog = seconds_to_time(listen_duration)
                        ep_prog = seconds_to_time(ep_duration)
                        progress_value = get_progress(listen_duration, ep_duration)
                        if self.page_type == "history":
                            entry_released = ft.Text(f'Listened on: {ep_listen_date}',
                                                     color=active_user.font_color)
                        entry_progress = ft.Row(controls=[ft.Text(listen_prog, color=active_user.font_color),
                                                          ft.ProgressBar(expand=True, value=progress_value,
                                                                         color=active_user.main_color),
                                                          ft.Text(ep_prog, color=active_user.font_color)])
                        if num_lines > 15:
                            ep_row_content = ft.ResponsiveRow([
                                ft.Column(col={"md": 2}, controls=[entry_artwork_url]),
                                ft.Column(col={"md": 9},
                                          controls=[entry_title, entry_description, entry_seemore,
                                                    entry_released, entry_progress, ft.Row(
                                                  controls=[ep_play_button, ep_resume_button,
                                                            popup_button])]),
                                ft.Column(col={"md": 1}, controls=[rotate_button]),
                            ])
                        else:
                            ep_row_content = ft.ResponsiveRow([
                                ft.Column(col={"md": 2}, controls=[entry_artwork_url]),
                                ft.Column(col={"md": 9},
                                          controls=[entry_title, entry_description, entry_released,
                                                    entry_progress, ft.Row(
                                                  controls=[ep_play_button, ep_resume_button,
                                                            popup_button])]),
                                ft.Column(col={"md": 1}, controls=[rotate_button]),
                            ])
                    else:
                        ep_dur = seconds_to_time(ep_duration)
                        dur_display = ft.Text(f'Episode Duration: {ep_dur}', color=active_user.font_color)
                        entry_controls = [entry_title, entry_description, entry_released, dur_display,
                                          ft.Row(controls=[ep_play_button, popup_button])]

                        if num_lines > 15:
                            entry_controls.insert(2, entry_seemore)  # Inserting the 'See More' button after 'entry_description'

                        ep_row_content = ft.ResponsiveRow([
                            ft.Column(col={"md": 2}, controls=[entry_artwork_url]),
                            ft.Column(col={"md": 9}, controls=entry_controls),
                            ft.Column(col={"md": 1}, controls=[rotate_button]),
                        ])
                    entry_description.visible = False
                    rotate_iteration = AnimatedButton(rotate_button, entry_description, entry_seemore)
                    rotate_button.on_click = rotate_iteration.animate

                    div_row = ft.Divider(color=active_user.accent_color)
                    ep_column = ft.Column(controls=[ep_row_content, div_row])
                    ep_row = ft.Container(content=ep_column)
                    ep_row.padding = padding.only(left=70, right=50)
                    self.row_list.controls.append(ep_row)
                    self.ep_number += 1
                return self.row_list

            def define_empty_values(self, name_text, title_text, desc_text):
                row_list = ft.ListView(divider_thickness=3, auto_scroll=True)

                pod_name = name_text
                ep_title = title_text
                pub_date = ""
                ep_desc = desc_text
                ep_url = ""
                entry_title = ft.Text(f'{pod_name} - {ep_title}', width=600, style=ft.TextThemeStyle.TITLE_MEDIUM)
                entry_description = ft.Text(ep_desc, width=800)
                entry_released = ft.Text(pub_date)
                artwork_no = random.randint(1, 12)
                artwork_url = os.path.join(script_dir, "images", "logo_random", f"{artwork_no}.jpeg")
                art_url_parsed = check_image(artwork_url)
                entry_artwork_url = ft.Image(src=art_url_parsed, width=150, height=150)
                ep_play_button = ft.IconButton(
                    icon=ft.icons.PLAY_DISABLED,
                    icon_color=active_user.accent_color,
                    icon_size=40,
                    tooltip="No Episodes Listened to yet"
                )
                # Creating column and row for home layout
                ep_column = ft.Column(
                    controls=[entry_title, entry_description, entry_released]
                )

                ep_row_content = ft.ResponsiveRow([
                    ft.Column(col={"md": 2}, controls=[entry_artwork_url]),
                    ft.Column(col={"md": 10}, controls=[ep_column, ep_play_button]),
                ])
                div_row = ft.Divider(color=active_user.accent_color)
                ep_column = ft.Column(controls=[ep_row_content, div_row])
                ep_row = ft.Container(content=ep_column)
                ep_row.padding = padding.only(left=70, right=50)
                row_list.controls.append(ep_row)
                return row_list

        if current_episode.audio_playing == True:
            pod_controls.audio_container.visible == True
        else:
            pod_controls.audio_container.visible == False

        def open_search(e):
            if page.width > 768:
                if page_items.search_pods.value:
                    new_search.searchvalue = page_items.search_pods.value
                    new_search.searchlocation = page_items.search_location.value
                    pr_instance.touch_stack()
                    page.update()
                    # Run the test_connection function
                    connection_test_result = internal_functions.functions.test_connection(api_url)
                    if connection_test_result is not True:
                        page.snack_bar = ft.SnackBar(content=ft.Text(connection_test_result))
                        page.snack_bar.open = True
                        pr_instance.rm_stack()
                        page.update()
                        return  # Do not proceed further if the connection test failed

                    page.go("/searchpod")
                else:
                    page.snack_bar = ft.SnackBar(content=ft.Text("Please enter a podcast to search for"))
                    page.snack_bar.open = True
                    page.update()


            else:
                def close_search_dlg(page):
                    search_dlg.open = False
                    page.update()

                def close_search_dlg_auto(e):
                    search_dlg.open = False
                    page.update()

                def search_podcast_small(e):
                    close_search_dlg(page)
                    pr_instance.touch_stack()
                    page.update()
                    connection_test_result = internal_functions.functions.test_connection(api_url)
                    if connection_test_result is not True:
                        page.snack_bar = ft.SnackBar(content=ft.Text(connection_test_result))
                        page.snack_bar.open = True
                        pr_instance.rm_stack()
                        page.update()
                        return  # Do not proceed further if the connection test failed

                    page.go("/searchpod")

                search_value_small = ft.TextField(label="Podcast", hint_text='Darknet Diaries')
                search_location_small = ft.Dropdown(color=active_user.font_color,
                                                    focused_bgcolor=active_user.main_color,
                                                    focused_border_color=active_user.accent_color,
                                                    focused_color=active_user.accent_color,
                                                    prefix_icon=ft.icons.MANAGE_SEARCH,
                                                    options=[
                                                        ft.dropdown.Option("podcastindex"),
                                                        ft.dropdown.Option("itunes"),
                                                    ]
                                                    )

                search_dlg = ft.AlertDialog(
                    modal=True,
                    title=ft.Text(f"Search Podcast:"),
                    content=ft.Column(controls=[
                        ft.Text(f"Enter a podcast to search for:", selectable=True),
                        search_value_small,
                        search_location_small
                    ], tight=True),
                    actions=[
                        ft.TextButton("Search!", on_click=search_podcast_small),
                        ft.TextButton("Close", on_click=close_search_dlg_auto)
                    ],
                    actions_alignment=ft.MainAxisAlignment.END
                )
                page.dialog = search_dlg
                search_dlg.open = True
                page.update()
        class Page_Vars:
            def __init__(self, page):
                self.search_pods = ft.TextField(label="Search for new podcast", content_padding=5, width=200)
                self.search_location = ft.Dropdown(color=active_user.font_color, focused_bgcolor=active_user.main_color,
                                                   focused_border_color=active_user.accent_color,
                                                   focused_color=active_user.accent_color,
                                                   prefix_icon=ft.icons.MANAGE_SEARCH,
                                                   options=[
                                                       ft.dropdown.Option("podcastindex"),
                                                       ft.dropdown.Option("itunes"),
                                                   ]
                                                   )

        page_items = Page_Vars(page)

        def adjust_audio_container(e):
            max_chars = character_limit(int(page.width))
            current_episode.name_truncated = truncate_text(current_episode.name, max_chars)
            pod_controls.currently_playing.content = ft.Text(current_episode.name_truncated, size=16)

            if page.width <= 768 and page.width != 0:
                print('using toggle pod currently')
                page_items.search_pods.visible = False
                page_items.search_location.visible = False

                ep_height = 100
                ep_width = 4000
                pod_controls.audio_container.height = ep_height
                pod_controls.audio_container.content = ft.Column(
                    horizontal_alignment=ft.CrossAxisAlignment.CENTER,
                    controls=[pod_controls.audio_container_pod_details, pod_controls.audio_controls_row])
            else:
                ep_height = 50
                ep_width = 4000
                page_items.search_pods.visible = True
                page_items.search_location.visible = True

                pod_controls.audio_container.height = ep_height
                pod_controls.audio_container.content = pod_controls.audio_container_row

            pod_controls.audio_container.update()
            page.update()

        max_chars = character_limit(int(page.width))
        current_episode.name_truncated = truncate_text(current_episode.name, max_chars)
        pod_controls.currently_playing.content = ft.Text(current_episode.name_truncated, size=16)

        if page.width <= 768 and page.width != 0:
            print('using toggle pod currently')
            page_items.search_pods.visible = False
            page_items.search_location.visible = False

            ep_height = 100
            ep_width = 4000
            pod_controls.audio_container.height = ep_height
            pod_controls.audio_container.content = ft.Column(
                horizontal_alignment=ft.CrossAxisAlignment.CENTER,
                controls=[pod_controls.audio_container_pod_details, pod_controls.audio_controls_row])
        else:
            ep_height = 50
            ep_width = 4000
            page_items.search_pods.visible = True
            page_items.search_location.visible = True

            pod_controls.audio_container.height = ep_height
            pod_controls.audio_container.content = pod_controls.audio_container_row

        pod_controls.audio_container.update()
        page.update()

        # This function gets called when the page resizes
        page.on_resize = adjust_audio_container

        # # Call it directly to set up the initial state based on screen size
        # adjust_audio_container(e)

        page_items.search_location.width = 130
        page_items.search_location.height = 50
        search_btn = ft.ElevatedButton("Search!", on_click=open_search)
        page_items.search_pods.color = active_user.accent_color
        page_items.search_pods.focused_bgcolor = active_user.accent_color
        page_items.search_pods.focused_border_color = active_user.accent_color
        page_items.search_pods.focused_color = active_user.accent_color
        page_items.search_pods.focused_color = active_user.accent_color
        page_items.search_pods.cursor_color = active_user.accent_color
        search_btn.bgcolor = active_user.accent_color
        search_btn.color = active_user.main_color

        if page.route == "/" or page.route == "/":
            page.bgcolor = colors.BLUE_GREY

            # Home Screen Podcast Layout (Episodes in Newest order)

            home_episodes = api_functions.functions.call_return_episodes(app_api.url, app_api.headers,
                                                                         active_user.user_id)
            home_layout = Pod_View(page)
            active_user.current_pod_view = home_layout

            home_layout.page_type = "home"

            if home_episodes is None:
                home_row_list = home_layout.define_empty_values(
                    "No Podcasts added yet",
                    "Podcasts you add will display new episodes here.",
                    "You can search podcasts in the upper right. Then click the plus button to add podcasts to the add. Click around on the navbar to manage podcasts you've added. Enjoy the listening!"
                )
            else:
                home_row_list = home_layout.define_values(home_episodes)

            home_row_contain = ft.Container(content=home_row_list)

            home_view = ft.View("/", [
                home_layout.top_bar,
                home_row_contain
            ]
                                )
            home_view.bgcolor = active_user.bgcolor
            home_view.scroll = ft.ScrollMode.AUTO
            page.views.append(
                home_view
            )
            if active_user.first_start == 0:
                active_user.first_start += 1

        if page.route == "/saved" or page.route == "/saved":

            # Get Pod info
            saved_episode_list = api_functions.functions.call_saved_episode_list(app_api.url, app_api.headers,
                                                                                 active_user.user_id)
            saved_layout = Pod_View(page)
            saved_layout.page_type = "saved"

            if saved_episode_list is None:
                saved_row_list = saved_layout.define_empty_values(
                    "No podcasts saved yet",
                    "Podcasts you save will display here.",
                    "Click the dropdown on podcasts and select save. This will save the podcast in order to easily find them for later listening. Think of this like a permanent queue."
                )
            else:
                saved_episode_list.reverse()
                saved_row_list = saved_layout.define_values(saved_episode_list)

            saved_row_contain = ft.Container(content=saved_row_list)
            saved_title = ft.Text(
                "Saved Episodes:",
                size=30,
                font_family="RobotoSlab",
                color=active_user.font_color,
                weight=ft.FontWeight.W_300,
            )
            saved_title_row = ft.Row(controls=[saved_title], alignment=ft.MainAxisAlignment.CENTER)

            # Create search view object
            ep_saved_view = ft.View("/saved",
                                    [
                                        saved_layout.top_bar,
                                        saved_title_row,
                                        saved_row_contain

                                    ]

                                    )
            ep_saved_view.bgcolor = active_user.bgcolor
            ep_saved_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                ep_saved_view

            )

        if page.route == "/history" or page.route == "/history":

            # Get Pod info
            hist_episodes = api_functions.functions.call_user_history(app_api.url, app_api.headers, active_user.user_id)
            hist_layout = Pod_View(page)
            hist_layout.page_type = "history"

            if hist_episodes is None:
                hist_row_list = hist_layout.define_empty_values(
                    "No Podcasts history yet",
                    "Podcasts you add will display here after you listen to them.",
                    "You can search podcasts in the upper right. Then click the plus button to add podcasts. Once you listen to episodes they will appear here."
                )

            else:
                hist_episodes.reverse()
                hist_row_list = hist_layout.define_values(hist_episodes)

            hist_row_contain = ft.Container(content=hist_row_list)
            history_title = ft.Text(
                "Listen History:",
                size=30,
                font_family="RobotoSlab",
                color=active_user.font_color,
                weight=ft.FontWeight.W_300,
            )
            history_title_row = ft.Row(controls=[history_title], alignment=ft.MainAxisAlignment.CENTER)

            # Create search view object
            ep_hist_view = ft.View("/history",
                                   [
                                       hist_layout.top_bar,
                                       history_title_row,
                                       hist_row_contain

                                   ]

                                   )
            ep_hist_view.bgcolor = active_user.bgcolor
            ep_hist_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                ep_hist_view

            )

        if page.route == "/queue" or page.route == "/queue":
            episode_queue_list = api_functions.functions.call_queued_episodes(app_api.url, app_api.headers,
                                                                             active_user.user_id)
            queue_layout = Pod_View(page)
            queue_layout.page_type = "queue"

            if episode_queue_list is None:
                queue_row_list = queue_layout.define_empty_values(
                    "No Podcasts added yet",
                    "Podcasts you queue will display here.",
                    "Click the dropdown on podcasts and select queue. This will queue the podcast to play next. If you queue a podcast while nothing is playing it will just play the podcast."
                )

            else:
                queue_row_list = queue_layout.define_values(episode_queue_list)

            queue_row_contain = ft.Container(content=queue_row_list)

            queue_title = ft.Text(
                "Current Listen Queue:",
                size=30,
                font_family="RobotoSlab",
                weight=ft.FontWeight.W_300,
            )
            queue_title_row = ft.Row(controls=[queue_title], alignment=ft.MainAxisAlignment.CENTER)

            # Create search view object
            ep_queue_view = ft.View("/queue",
                                    [
                                        queue_layout.top_bar,
                                        queue_title_row,
                                        queue_row_contain

                                    ]

                                    )
            ep_queue_view.bgcolor = active_user.bgcolor
            ep_queue_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                ep_queue_view

            )

        if page.route == "/user_search" or page.route == "/user_search":

            class Search:
                def __init__(self, page):
                    self.page = page
                    self.search_term = ''
                    self.search_lists = ft.ListView()
                    self.search_textbox = ft.TextField(label='Search Database', icon=ft.icons.SEARCH, hint_text='This American Life')
                    self.search_text_row = ft.Row(controls=[self.search_textbox])
                    self.search_text_row.alignment = ft.MainAxisAlignment.CENTER
                    self.search_container = ft.Container(content=self.search_text_row, alignment=ft.alignment.center)
                    self.search_container.horizontal_alignment = ft.CrossAxisAlignment.CENTER
                    self.searching_results = ft.ListView(divider_thickness=3, auto_scroll=True)



                def validate_search(self, e):
                    pr_instance.touch_stack()
                    self.page.update()
                    if self.search_textbox.value == '':
                        self.page.snack_bar = ft.SnackBar(content=ft.Text(f"Please enter a search term!"))
                        self.page.snack_bar.open = True
                        pr_instance.rm_stack()
                        self.page.update()
                        return
                    elif len(self.search_textbox.value) <= 3:
                        self.page.snack_bar = ft.SnackBar(content=ft.Text(f"Please enter at least 4 characters to search for"))
                        self.page.snack_bar.open = True
                        pr_instance.rm_stack()
                        self.page.update()
                        return
                    else:
                        self.search_term = self.search_textbox.value
                        self.execute_search()

                def execute_search(self):
                    self.search_data_list = api_functions.functions.call_user_search(app_api.url, app_api.headers, active_user.user_id, self.search_term)
                    self.searching_results.controls.clear()
                    page.update()
                    if self.search_data_list:
                        self.search_lists = search_layout.define_values(self.search_data_list)
                        pr_instance.rm_stack()
                        page.update()
                        self.searching_results.controls.append(self.search_lists)
                    else:
                        none_text = ft.Text("No results found! Try again!", size=16)
                        none_row = ft.Row(controls=[none_text], alignment=ft.MainAxisAlignment.CENTER)
                        pr_instance.rm_stack()
                        self.searching_results.controls.append(none_row)
                        page.update()
                    ep_search_view.controls.append(self.searching_results)
                    page.update()


            search_query = Search(page)
            page.update()
            search_layout = Pod_View(page)
            search_layout.page_type = "search"
            search_go = ft.ElevatedButton("Search!", on_click=search_query.validate_search)
            search_page_row = ft.Row(controls=[search_query.search_container, search_go])
            search_page_row.alignment = ft.MainAxisAlignment.CENTER

            search_title = ft.Text(
                "Search Database:",
                size=30,
                font_family="RobotoSlab",
                weight=ft.FontWeight.W_300,
            )
            search_title_row = ft.Row(controls=[search_title], alignment=ft.MainAxisAlignment.CENTER)

            # Create search view object
            ep_search_view = ft.View("/user_search",
                                    [
                                        search_layout.top_bar,
                                        search_title_row,
                                        search_page_row,
                                        # search_query.search_lists

                                    ]

                                    )
            ep_search_view.bgcolor = active_user.bgcolor
            ep_search_view.scroll = ft.ScrollMode.AUTO
            page.update()
            # Create final page
            page.views.append(
                ep_search_view

            )

        if page.route == "/downloads" or page.route == "/downloads":

            # Get Pod info
            download_episode_list = api_functions.functions.call_download_episode_list(app_api.url, app_api.headers,
                                                                                       active_user.user_id)

            class DownloadLayout:
                def __init__(self, page, download_type):
                    self.page = page
                    self.download_type = download_type
                    self.local_download_row_list = ft.ListView(divider_thickness=3, auto_scroll=True)
                    self.refresh_btn = ft.IconButton(icon=ft.icons.REFRESH, icon_color=active_user.font_color,
                                                     tooltip="Refresh Podcast List", on_click=self.refresh_podcasts)
                    self.refresh_btn.icon_color = active_user.font_color
                    self.refresh_ctn = ft.Container(
                        content=self.refresh_btn,
                        alignment=ft.alignment.top_left
                    )
                    self.banner_button = ft.ElevatedButton("Help!", on_click=show_banner_click)
                    self.banner_button.bgcolor = active_user.accent_color
                    self.banner_button.color = active_user.main_color
                    self.settings_row = ft.Row(vertical_alignment=ft.CrossAxisAlignment.START,
                                               controls=[self.refresh_ctn, self.banner_button])
                    self.search_row = ft.Row(spacing=25,
                                             controls=[page_items.search_pods, page_items.search_location, search_btn])
                    self.delete_list = []
                    self.checkboxes = []
                    self.selected_episodes = []
                    self.top_row = ft.Row(alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
                                          vertical_alignment=ft.CrossAxisAlignment.START,
                                          controls=[self.settings_row, self.search_row])
                    self.top_row_container = ft.Container(content=self.top_row, expand=True)
                    self.top_row_container.padding = ft.padding.only(left=60)
                    self.top_bar = ft.Row(vertical_alignment=ft.CrossAxisAlignment.START,
                                          controls=[self.top_row_container])
                    if current_episode.audio_playing == True:
                        pod_controls.audio_container.visible = True

                def refresh_podcasts(self, e):
                    pr_instance.touch_stack()
                    self.page.update()
                    download_episode_list = api_functions.functions.call_download_episode_list(app_api.url,
                                                                                               app_api.headers,
                                                                                               active_user.user_id)
                    if download_episode_list:

                        download_list.generate_layout(download_episode_list)
                    else:
                        download_list.define_empty_values(
                            "No Podcasts downloaded yet",
                            "Podcasts you download will display here.",
                            "Click the dropdown on podcasts and select server download. This will download the podcast to the server for local storage. Good for when you'd like to archive episodes. You can even mount the storage location to a nas or other network storage option. See the wiki for more details."
                        )
                    self.page.snack_bar = ft.SnackBar(content=ft.Text(f"Refresh Complete!"))
                    self.page.snack_bar.open = True
                    self.page.update()

                def refresh_downloaded_episodes(self):

                    # Fetch new podcast episodes from the server.
                    download_episode_list = api_functions.functions.call_download_episode_list(app_api.url,
                                                                                                   app_api.headers,
                                                                                                   active_user.user_id)
                    self.generate_layout(download_episode_list)

                def delete_selected_episode(self, url, title):
                    api_functions.functions.call_delete_podcast(app_api.url, app_api.headers, url, title,
                                                                active_user.user_id)
                    self.page.snack_bar = ft.SnackBar(content=ft.Text(f"Episode: {title} has deleted!"))
                    self.page.snack_bar.open = True
                    # Refresh the podcast list
                    self.refresh_downloaded_episodes()
                    self.page.update()
                    # Refresh the podcast list
                    self.refresh_downloaded_episodes()
                    self.page.update()

                def define_empty_values(self, name_text, title_text, desc_text):
                    pod_name = name_text
                    ep_title = title_text
                    pub_date = ""
                    ep_desc = desc_text
                    ep_url = ""
                    entry_title = ft.Text(f'{pod_name} - {ep_title}', width=600, style=ft.TextThemeStyle.TITLE_MEDIUM)
                    entry_description = ft.Text(ep_desc, width=800)
                    entry_released = ft.Text(pub_date)
                    artwork_no = random.randint(1, 12)
                    artwork_url = os.path.join(script_dir, "images", "logo_random", f"{artwork_no}.jpeg")
                    art_url_parsed = check_image(artwork_url)
                    entry_artwork_url = ft.Image(src=art_url_parsed, width=150, height=150)
                    ep_play_button = ft.IconButton(
                        icon=ft.icons.PLAY_DISABLED,
                        icon_color=active_user.accent_color,
                        icon_size=40,
                        tooltip="No Episodes Listened to yet"
                    )
                    # Creating column and row for home layout
                    ep_column = ft.Column(
                        controls=[entry_title, entry_description, entry_released]
                    )

                    ep_row_content = ft.ResponsiveRow([
                        ft.Column(col={"md": 2}, controls=[entry_artwork_url]),
                        ft.Column(col={"md": 10}, controls=[ep_column, ep_play_button]),
                    ])
                    div_row = ft.Divider(color=active_user.accent_color)
                    ep_column = ft.Column(controls=[ep_row_content, div_row])
                    ep_row = ft.Container(content=ep_column)
                    ep_row.padding = padding.only(left=70, right=50)
                    self.local_download_row_list.controls.append(ep_row)

                def generate_layout(self, episode_list):
                    self.local_download_row_list.controls.clear()
                    episode_list.reverse()
                    podcasts_by_local_name = defaultdict(list)
                    for entry in episode_list:
                        podcasts_by_local_name[entry['PodcastName']].append(entry)

                    for podcast_name, podcasts in podcasts_by_local_name.items():
                        podcast_id = podcasts[0]['PodcastID']

                        download_pod_art_no = random.randint(1, 12)
                        download_pod_art_fallback = os.path.join(script_dir, "images", "logo_random",
                                                                 f"{download_pod_art_no}.jpeg")

                        download_pod_art_url = podcasts[0]['ArtworkURL'] if podcasts[0][
                            'ArtworkURL'] else download_pod_art_fallback
                        download_pod_art_parsed = check_image(download_pod_art_url)
                        download_pod_entry_artwork_url = ft.Image(src=download_pod_art_parsed, width=150, height=150)
                        download_pod_entry_title = ft.Text(f'{podcast_name}',
                                                           style=ft.TextThemeStyle.TITLE_MEDIUM,
                                                           color=active_user.font_color,
                                                           size=18)
                        local_download_div_row = ft.Divider(color=active_user.accent_color)
                        download_pod_entry_check = ft.Checkbox()
                        self.checkboxes.append(download_pod_entry_check)

                        def append_deletion(e, podcast_id=podcast_id,
                                            download_pod_entry_check=download_pod_entry_check):
                            if download_pod_entry_check.value:
                                self.delete_list.append(podcast_id)
                            else:
                                if podcast_id in self.delete_list:  # check if podcast_id is in the list
                                    self.delete_list.remove(podcast_id)

                        download_pod_entry_check.on_change = append_deletion
                        download_pod_entry_check.visible = False

                        episode_column = ft.Column()
                        for podcast in podcasts:
                            episode_check = ft.Checkbox()
                            self.checkboxes.append(episode_check)

                            def toggle_episode(e, episode_id=podcast['EpisodeID'], episode_check=episode_check):
                                if episode_check.value:
                                    self.selected_episodes.append(episode_id)
                                else:
                                    if episode_id in self.selected_episodes:  # check if episode_id is in the list
                                        self.selected_episodes.remove(episode_id)

                            episode_check.on_change = toggle_episode
                            episode_check.visible = False

                            # do something with the episode information
                            local_download_ep_title = podcast['EpisodeTitle']
                            local_download_ep_url = podcast['EpisodeURL']
                            local_download_ep_desc = podcast['EpisodeDescription']
                            local_download_ep_artwork = podcast['EpisodeArtwork']
                            unfilt_download_pub_date = podcast['EpisodePubDate']
                            # Parse the string into a datetime object
                            dt = datetime.datetime.strptime(unfilt_download_pub_date, "%Y-%m-%d")
                            # Format it in your desired format
                            local_download_pub_date = dt.strftime("%b %d, %Y")
                            local_download_ep_duration = podcast['EpisodeDuration']
                            if self.download_type == "server":
                                local_download_ep_local_url = podcast['DownloadedLocation']
                            if self.download_type == "local":
                                local_download_ep_id = podcast['EpisodeID']
                                local_download_ep_local_url = podcast['EpisodeLocalPath']

                            # do something with the episode information
                            local_download_entry_title_button = ft.Text(f'{local_download_ep_title}',
                                                                        style=ft.TextThemeStyle.TITLE_MEDIUM,
                                                                        color=active_user.font_color)
                            local_download_entry_title = ft.TextButton(content=local_download_entry_title_button,
                                                                       on_click=lambda x, url=local_download_ep_url,
                                                                                       title=local_download_ep_title: open_episode_select(
                                                                           page, url, title))

                            num_lines = local_download_ep_desc.count('\n')
                            if num_lines > 15:
                                if is_html(local_download_ep_desc):
                                    # convert HTML to Markdown
                                    markdown_desc = html2text.html2text(local_download_ep_desc)
                                    if num_lines > 15:
                                        # Split into lines, truncate to 15 lines, and join back into a string
                                        lines = markdown_desc.splitlines()[:15]
                                        markdown_desc = '\n'.join(lines)
                                    # add inline style to change font color
                                    local_download_entry_description = ft.Markdown(markdown_desc,
                                                                                   on_tap_link=launch_clicked_url)
                                    local_download_entry_seemore = ft.TextButton(text="See More...", on_click=lambda x,
                                                                                                                     url=local_download_ep_url,
                                                                                                                     title=local_download_ep_title: open_episode_select(
                                        page, url, title))
                                else:
                                    if num_lines > 15:
                                        # Split into lines, truncate to 15 lines, and join back into a string
                                        lines = local_download_ep_desc.splitlines()[:15]
                                        local_download_ep_desc = '\n'.join(lines)
                                    # display plain text
                                    local_download_entry_description = ft.Text(local_download_ep_desc)

                            else:
                                if is_html(local_download_ep_desc):
                                    # convert HTML to Markdown
                                    markdown_desc = html2text.html2text(local_download_ep_desc)
                                    # add inline style to change font color
                                    local_download_entry_description = ft.Markdown(markdown_desc,
                                                                                   on_tap_link=launch_clicked_url)
                                else:
                                    # display plain text
                                    local_download_entry_description = ft.Text(local_download_ep_desc)
                            check_episode_playback, listen_duration = api_functions.functions.call_check_episode_playback(
                                app_api.url, app_api.headers, active_user.user_id, local_download_ep_title,
                                local_download_ep_url)
                            local_download_entry_released = ft.Text(f'Released on: {local_download_pub_date}',
                                                                    color=active_user.font_color)
                            local_download_art_no = random.randint(1, 12)
                            local_download_art_fallback = os.path.join(script_dir, "images", "logo_random",
                                                                       f"{local_download_art_no}.jpeg")
                            local_download_art_url = local_download_ep_artwork if local_download_ep_artwork else local_download_art_fallback
                            local_download_art_parsed = check_image(local_download_art_url)
                            local_download_entry_artwork_url = ft.Image(src=local_download_art_parsed, width=150,
                                                                        height=150)
                            local_download_ep_resume_button = ft.IconButton(
                                icon=ft.icons.PLAY_CIRCLE,
                                icon_color=active_user.accent_color,
                                icon_size=40,
                                tooltip="Resume Episode",
                                on_click=lambda x,
                                                url=f'{proxy_url}{urllib.parse.quote(local_download_ep_local_url)}',
                                                title=local_download_ep_title,
                                                artwork=local_download_ep_artwork,
                                                listen_duration=listen_duration: resume_selected_episode(url, title,
                                                                                                         artwork,
                                                                                                         listen_duration)
                            )
                            local_download_ep_play_button = ft.IconButton(
                                icon=ft.icons.NOT_STARTED,
                                icon_color=active_user.accent_color,
                                icon_size=40,
                                tooltip="Play Episode",
                                on_click=lambda x,
                                                url=f'{proxy_url}{urllib.parse.quote(local_download_ep_local_url)}',
                                                title=local_download_ep_title,
                                                artwork=local_download_ep_artwork: play_selected_episode(url, title,
                                                                                                         artwork)
                            )
                            local_download_popup_button = ft.PopupMenuButton(
                                content=ft.Icon(ft.icons.ARROW_DROP_DOWN_CIRCLE_ROUNDED,
                                                color=active_user.accent_color, size=40, tooltip="Play Episode"),
                                items=[
                                    ft.PopupMenuItem(icon=ft.icons.QUEUE, text="Queue",
                                                     on_click=lambda x, url=local_download_ep_url,
                                                                     title=local_download_ep_title,
                                                                     artwork=local_download_ep_artwork: queue_selected_episode(
                                                         url, title, artwork, page)),
                                    ft.PopupMenuItem(icon=ft.icons.DOWNLOAD, text="Delete Downloaded Episode",
                                                     on_click=lambda x, url=local_download_ep_url,
                                                                     title=local_download_ep_title: self.delete_selected_episode(
                                                         url, title)),
                                    ft.PopupMenuItem(icon=ft.icons.SAVE, text="Save Episode",
                                                     on_click=lambda x, url=local_download_ep_url,
                                                                     title=local_download_ep_title: save_selected_episode(
                                                         url, title, page))
                                ]
                                )
                            if check_episode_playback == True:
                                listen_prog = seconds_to_time(listen_duration)
                                local_download_ep_prog = seconds_to_time(local_download_ep_duration)
                                progress_value = get_progress(listen_duration, local_download_ep_duration)
                                local_download_entry_progress = ft.Row(
                                    controls=[ft.Text(listen_prog, color=active_user.font_color),
                                              ft.ProgressBar(expand=True, value=progress_value,
                                                             color=active_user.main_color),
                                              ft.Text(local_download_ep_prog, color=active_user.font_color)])
                                if num_lines > 15:
                                    local_download_ep_row_content = ft.ResponsiveRow([
                                        ft.Column(col={"md": 2}, controls=[local_download_entry_artwork_url]),
                                        ft.Column(col={"md": 10}, controls=[episode_check, local_download_entry_title,
                                                                            local_download_entry_description,
                                                                            local_download_entry_seemore,
                                                                            local_download_entry_released,
                                                                            local_download_entry_progress, ft.Row(
                                                controls=[local_download_ep_play_button,
                                                          local_download_ep_resume_button,
                                                          local_download_popup_button])]),
                                    ])
                                else:
                                    local_download_ep_row_content = ft.ResponsiveRow([
                                        ft.Column(col={"md": 2}, controls=[local_download_entry_artwork_url]),
                                        ft.Column(col={"md": 10}, controls=[episode_check, local_download_entry_title,
                                                                            local_download_entry_description,
                                                                            local_download_entry_released,
                                                                            local_download_entry_progress, ft.Row(
                                                controls=[local_download_ep_play_button,
                                                          local_download_ep_resume_button,
                                                          local_download_popup_button])]),
                                    ])
                            else:
                                local_download_ep_dur = seconds_to_time(local_download_ep_duration)
                                local_download_dur_display = ft.Text(f'Episode Duration: {local_download_ep_dur}',
                                                                     color=active_user.font_color)
                                if num_lines > 15:
                                    local_download_ep_row_content = ft.ResponsiveRow([
                                        ft.Column(col={"md": 2}, controls=[local_download_entry_artwork_url]),
                                        ft.Column(col={"md": 10}, controls=[episode_check, local_download_entry_title,
                                                                            local_download_entry_description,
                                                                            local_download_entry_seemore,
                                                                            local_download_entry_released,
                                                                            local_download_dur_display, ft.Row(
                                                controls=[local_download_ep_play_button,
                                                          local_download_popup_button])]),
                                    ])
                                else:
                                    local_download_ep_row_content = ft.ResponsiveRow([
                                        ft.Column(col={"md": 2}, controls=[local_download_entry_artwork_url]),
                                        ft.Column(col={"md": 10}, controls=[episode_check, local_download_entry_title,
                                                                            local_download_entry_description,
                                                                            local_download_entry_released,
                                                                            local_download_dur_display, ft.Row(
                                                controls=[local_download_ep_play_button,
                                                          local_download_popup_button])]),
                                    ])
                            local_download_ep_column = ft.Column(
                                controls=[local_download_ep_row_content, local_download_div_row])

                            local_download_ep_row = ft.Container(content=local_download_ep_column)
                            local_download_ep_row.padding = padding.only(left=20, right=50)

                            episode_column.visible = False
                            episode_column.controls.append(
                                local_download_ep_row)

                        local_rotate_button = ft.IconButton(
                            icon=ft.icons.ARROW_FORWARD_IOS,
                            icon_color=active_user.accent_color,
                            tooltip="Pause record",
                            rotate=ft.transform.Rotate(0, alignment=ft.alignment.center),
                            animate_rotation=ft.animation.Animation(300, ft.AnimationCurve.BOUNCE_OUT),
                        )
                        local_rotate_iteration = AnimatedButton(local_rotate_button, episode_column,
                                                                local_download_entry_seemore)
                        local_rotate_button.on_click = local_rotate_iteration.animate

                        download_pod_data_group = ft.Row(
                            controls=[download_pod_entry_artwork_url, download_pod_entry_title, local_rotate_button,
                                      download_pod_entry_check])

                        podcast_group = ft.Column(
                            controls=[download_pod_data_group, episode_column, local_download_div_row])
                        podcast_group.padding = padding.only(left=70, right=50)
                        self.local_download_row_list.controls.append(podcast_group)
                        self.local_download_row_list.padding = padding.only(left=70, right=50)

            server_text = ft.Text("Server Downloaded Episodes:", size=18, color=active_user.font_color)
            download_title_row = ft.Row(controls=[server_text])
            download_title_row_container = ft.Container(content=download_title_row)
            download_title_row_container.padding=padding.only(left=70, right=50)

            download_list = DownloadLayout(page, "server")
            if download_episode_list:

                download_list.generate_layout(download_episode_list)
            else:
                download_list.define_empty_values(
                    "No Podcasts downloaded yet",
                    "Podcasts you download will display here.",
                    "Click the dropdown on podcasts and select server download. This will download the podcast to the server for local storage. Good for when you'd like to archive episodes. You can even mount the storage location to a nas or other network storage option. See the wiki for more details."
                )

            download_row_contain = ft.Container(content=download_list.local_download_row_list)

            # Current Downloads Display
            class DownloadingDisplay:

                def __init__(self, page):
                    self.page = page
                    self.previous_list = None
                    self.stop_thread = False
                    self.active_downloader = ft.Text(color=active_user.font_color)
                    self.active_download_count = ft.Text(color=active_user.font_color)
                    self.active_download_column = ft.Column()

                    def mass_delete_mode(e):
                        self.mass_delete_button.visible = False
                        self.mass_delete_button_perm.visible = True
                        self.mass_delete_button_cancel.visible = True
                        for checkbox in download_list.checkboxes:
                            checkbox.visible = True
                        # download_list.download_pod_entry_check.visible = True
                        self.page.snack_bar = ft.SnackBar(content=ft.Text(
                            f"Entered delete mode. Select podcasts or episodes to delete en mass, then confirm your select by clicking the trash can."))
                        self.page.snack_bar.open = True
                        self.page.update()

                    def mass_delete_confirm(e):
                        # only call selective_delete if there is something to delete
                        if download_list.delete_list or download_list.selected_episodes:
                            self.selective_delete()
                        else:
                            self.page.snack_bar = ft.SnackBar(
                                content=ft.Text("No downloads, episodes, or podcasts selected for deletion."))
                            self.page.snack_bar.open = True
                            self.page.update()

                        self.mass_delete_button.visible = True
                        self.mass_delete_button_perm.visible = False
                        self.mass_delete_button_cancel.visible = False
                        for checkbox in download_list.checkboxes:
                            checkbox.visible = False
                        self.page.update()

                    def mass_delete_cancel(e):
                        self.mass_delete_button.visible = True
                        self.mass_delete_button_perm.visible = False
                        self.mass_delete_button_cancel.visible = False
                        for checkbox in download_list.checkboxes:
                            checkbox.visible = False
                        self.page.update()

                    self.mass_delete_button = ft.IconButton(icon=ft.icons.DELETE, on_click=mass_delete_mode,
                                                            bgcolor=active_user.main_color, tooltip="Enter Delete Mode")
                    self.mass_delete_button_perm = ft.IconButton(icon=ft.icons.DELETE_FOREVER,
                                                                 on_click=mass_delete_confirm,
                                                                 bgcolor=active_user.main_color,
                                                                 tooltip='Confirm Deletion of Selected Episodes')
                    self.mass_delete_button_perm.visible = False
                    self.mass_delete_button_cancel = ft.IconButton(icon=ft.icons.CANCEL, on_click=mass_delete_cancel,
                                                                   bgcolor=active_user.main_color,
                                                                   tooltip='Cancel Selection and return to normal mode')
                    self.mass_delete_button_cancel.visible = False
                    self.mass_delete_row = ft.Row(controls=[self.mass_delete_button, self.mass_delete_button_perm,
                                                            self.mass_delete_button_cancel])
                    self.active_download_row = ft.Row()
                    self.active_download_container = ft.Container(content=self.active_download_column)
                    self.active_download_container.padding = padding.only(left=80, right=50)
                    self.layout_created = False

                    # Create initial layout
                    self.create_downloading_layout()

                    # Start the monitoring thread
                    self.monitor_thread = threading.Thread(target=self.monitor_changes)
                    self.monitor_thread.start()

                def selective_delete(self):
                    # delete selected episodes first
                    if download_list.selected_episodes or download_list.delete_list:
                        # delete selected episodes first
                        if download_list.selected_episodes:
                            api_functions.functions.call_delete_selected_episodes(app_api.url, app_api.headers,
                                                                                  download_list.selected_episodes,
                                                                                  active_user.user_id)
                            self.page.snack_bar = ft.SnackBar(content=ft.Text(f"Episodes have been deleted!"))
                            self.page.snack_bar.open = True
                            # Refresh the podcast list
                            download_list.refresh_downloaded_episodes()
                            self.page.update()

                        # then delete the entire podcast (if needed)
                        if download_list.delete_list:
                            api_functions.functions.call_delete_selected_podcasts(app_api.url, app_api.headers,
                                                                                  download_list.delete_list,
                                                                                  active_user.user_id)

                            self.page.snack_bar = ft.SnackBar(content=ft.Text(f"Podcasts have been deleted!"))
                            self.page.snack_bar.open = True
                            # Refresh the podcast list
                            download_list.refresh_downloaded_episodes()
                            self.page.update()
                    else:
                        print("No episodes or podcasts selected for deletion.")

                    if download_list.selected_episodes or download_list.delete_list:
                        # delete selected episodes first
                        if download_list.selected_episodes:
                            download_list.selected_episodes = []

                    else:
                        print("No episodes or podcasts selected for deletion.")

                def create_downloading_layout(self):
                    self.active_downloader.value = active_user.downloading[
                        0] if active_user.downloading else "No active downloads"
                    self.active_download_count.value = f'Number of other Podcasts currently downloading: {len(active_user.downloading) - 1 if len(active_user.downloading) > 1 else 0}'
                    self.active_download_column.controls.append(self.active_downloader)
                    self.active_download_column.controls.append(self.active_download_count)
                    self.active_download_row.controls.extend([self.active_download_container, self.mass_delete_row])
                    self.active_download_row.alignment = ft.MainAxisAlignment.SPACE_BETWEEN

                    # Set this flag true before calling the page update method
                    self.layout_created = True

                    # Ensure that the active download container is added to the page before trying to update it
                    self.page.controls.append(self.active_download_row)
                    self.page.update()

                    return self.active_download_container

                def update_downloading_layout(self):
                    if not self.layout_created:
                        return
                    else:

                        # Update the text elements
                        self.active_downloader.value = active_user.downloading_name[
                            0] if active_user.downloading_name else "No active downloads"
                        self.active_download_count.value = f'Number of other Podcasts currently downloading: {len(active_user.downloading) - 1 if len(active_user.downloading) > 1 else 0}'

                        self.active_downloader.update()
                        self.active_download_count.update()

                        # Add this line
                        self.active_download_container.update()

                        # Update the page
                        time.sleep(0.1)  # Add a slight delay
                        self.page.update()

                def monitor_changes(self):
                    while not self.stop_thread:
                        # If the list has changed, update the downloading list
                        if active_user.downloading != self.previous_list:
                            self.update_downloading_layout()
                            self.previous_list = active_user.downloading.copy()
                        time.sleep(1)

                def stop_monitoring(self):
                    self.stop_thread = True

            current_download_text = ft.Text('Currently Downloading Episodes:', size=18, color=active_user.font_color)
            current_download_text_con = ft.Container(content=current_download_text)
            current_download_text_con.padding = padding.only(left=70, right=50)
            current_downloads = DownloadingDisplay(page)
            downloading_row = current_downloads.active_download_row
            current_downloads.active_download_row.visible = True
            # Create search view object
            ep_download_view = ft.View("/downloads",
                                       [
                                           download_list.top_bar,
                                           current_download_text_con,
                                           current_downloads.active_download_row,
                                           ft.Divider(color=active_user.accent_color),
                                           download_title_row_container,
                                           download_row_contain,
                                       ]

                                       )
            ep_download_view.bgcolor = active_user.bgcolor
            ep_download_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                ep_download_view

            )

        if page.route == "/poddisplay" or page.route == "/poddisplay":
            # Check if podcast is already in database for user
            podcast_status = api_functions.functions.call_check_podcast(app_api.url, app_api.headers,
                                                                        active_user.user_id, clicked_podcast.name)
            # Creating attributes for page layout
            # First Podcast Info
            display_pod_art_no = random.randint(1, 12)
            display_pod_art_fallback = os.path.join(script_dir, "images", "logo_random", f"{display_pod_art_no}.jpeg")
            display_pod_art_url = clicked_podcast.artwork if clicked_podcast.artwork else display_pod_art_fallback
            display_pod_art_parsed = check_image(display_pod_art_url)
            pod_image = ft.Image(src=display_pod_art_parsed, width=300, height=300)
            pod_feed_title = ft.Text(clicked_podcast.name, style=ft.TextThemeStyle.HEADLINE_MEDIUM)
            pod_feed_desc = ft.Text(clicked_podcast.description)
            pod_feed_site = ft.ElevatedButton(text=clicked_podcast.website, on_click=launch_pod_site)
            pod_feed_add_button = ft.IconButton(
                icon=ft.icons.ADD_BOX,
                icon_color=active_user.accent_color,
                icon_size=40,
                tooltip="Add Podcast",
                on_click=lambda x: send_podcast(clicked_podcast.name, clicked_podcast.artwork, clicked_podcast.author,
                                                clicked_podcast.categories, clicked_podcast.description,
                                                clicked_podcast.episode_count, clicked_podcast.feedurl,
                                                clicked_podcast.website, page)
            )
            pod_feed_remove_button = ft.IconButton(
                icon=ft.icons.INDETERMINATE_CHECK_BOX,
                icon_color="red400",
                icon_size=40,
                tooltip="Remove Podcast",
                on_click=lambda x, title=clicked_podcast.name: api_functions.functions.call_remove_podcast(app_api.url,
                                                                                                           app_api.headers,
                                                                                                           title,
                                                                                                           active_user.user_id)
            )
            pod_download_button = ft.IconButton(
                icon=ft.icons.CLOUD_DOWNLOAD,
                icon_color=active_user.accent_color,
                icon_size=40,
                tooltip="Download Podcast Episodes to the Server",
                on_click=lambda x, title=clicked_podcast.name, url=clicked_podcast.feedurl: download_full_podcast(title,
                                                                                                           url, page)
            )
            if podcast_status == True:
                feed_row_content = ft.ResponsiveRow([
                    ft.Column(col={"md": 4}, controls=[pod_image]),
                    ft.Column(col={"md": 7}, controls=[pod_feed_title, pod_feed_desc, pod_feed_site]),
                    ft.Column(col={"md": 1}, controls=[pod_feed_remove_button, pod_download_button]),
                ])
            else:
                feed_row_content = ft.ResponsiveRow([
                    ft.Column(col={"md": 4}, controls=[pod_image]),
                    ft.Column(col={"md": 7}, controls=[pod_feed_title, pod_feed_desc, pod_feed_site]),
                    ft.Column(col={"md": 1}, controls=[pod_feed_add_button]),
                ])
            feed_row = ft.Container(content=feed_row_content)
            feed_row.padding = padding.only(left=70, right=50)

            # Episode Info
            # Run Function to get episode data
            ep_number = 1
            ep_row_list = ft.ListView(divider_thickness=3, auto_scroll=True)

            episode_results = app_functions.functions.parse_feed(clicked_podcast.feedurl)

            for entry in episode_results.entries:
                if hasattr(entry, "title") and hasattr(entry, "summary") and hasattr(entry, "enclosures"):
                    # get the episode title
                    parsed_title = entry.title

                    # get the episode description
                    parsed_description = entry.get('content', [{}])[0].get('value', entry.summary)

                    # get the URL of the audio file for the episode
                    if entry.enclosures:
                        parsed_audio_url = entry.enclosures[0].href
                    else:
                        parsed_audio_url = ""

                    # Parse the date string into a datetime object
                    dt = parser.parse(entry.published)

                    # Convert it to the user's timezone
                    user_tz = pytz.timezone(active_user.timezone)
                    dt = dt.astimezone(user_tz)

                    # Format it in 12-hour or 24-hour format based on the user's preference
                    if active_user.hour_pref == 12:
                        parsed_release_date = dt.strftime("%b %d, %Y %I:%M %p")  # 12-hour format with date
                    else:
                        parsed_release_date = dt.strftime("%b %d, %Y %H:%M")  # 24-hour format with date

                    # get the URL of the episode artwork, or use the podcast image URL if not available
                    parsed_artwork_url = entry.get('itunes_image', {}).get('href', None) or entry.get('image', {}).get(
                        'href', None)
                    if parsed_artwork_url == None:
                        parsed_artwork_url = clicked_podcast.artwork
                    display_art_no = random.randint(1, 12)
                    display_art_fallback = os.path.join(script_dir, "images", "logo_random", f"{display_art_no}.jpeg")
                    display_art_url = parsed_artwork_url if parsed_artwork_url else display_art_fallback

                else:
                    print("Skipping entry without required attributes or enclosures")
                entry_title = ft.Text(f'{parsed_title}', style=ft.TextThemeStyle.TITLE_MEDIUM,
                                      color=active_user.font_color)
                entry_audio_url = ft.Text(parsed_audio_url)
                entry_released = ft.Text(parsed_release_date)
                display_art_entry_parsed = check_image(display_art_url)
                entry_artwork_url = ft.Image(src=display_art_entry_parsed, width=150, height=150)

                if is_html(parsed_description):
                    # convert HTML to Markdown
                    markdown_desc = html2text.html2text(parsed_description)
                    # add inline style to change font color
                    entry_description = ft.Markdown(markdown_desc, on_tap_link=launch_clicked_url)
                else:
                    # display plain text
                    markdown_desc = parsed_description
                    entry_description = ft.Text(markdown_desc)
                rotate_button = ft.IconButton(
                    icon=ft.icons.ARROW_FORWARD_IOS,
                    icon_color=active_user.accent_color,
                    tooltip="Show Description",
                    rotate=ft.transform.Rotate(0, alignment=ft.alignment.center),
                    animate_rotation=ft.animation.Animation(300, ft.AnimationCurve.BOUNCE_OUT),
                )
                if podcast_status == True:
                    ep_resume_button = ft.IconButton(
                        icon=ft.icons.NOT_STARTED,
                        icon_color=active_user.accent_color,
                        icon_size=40,
                        tooltip="Play Episode",
                        on_click=lambda x, url=entry_audio_url.value, title=entry_title.value,
                                        artwork=display_art_entry_parsed: play_selected_episode(url, title, artwork)
                    )
                    ep_popup_button = ft.PopupMenuButton(
                        content=ft.Icon(ft.icons.ARROW_DROP_DOWN_CIRCLE_ROUNDED, color=active_user.accent_color,
                                        size=40, tooltip="Play Episode"),
                        items=[
                            ft.PopupMenuItem(icon=ft.icons.QUEUE, text="Queue",
                                             on_click=lambda x, url=entry_audio_url.value, title=entry_title.value,
                                                             artwork=display_art_entry_parsed: queue_selected_episode(
                                                 url, title, artwork, page)),
                            ft.PopupMenuItem(icon=ft.icons.DOWNLOAD, text="Server Download",
                                             on_click=lambda x, url=entry_audio_url.value,
                                                             title=entry_title.value: download_selected_episode(url, title,
                                                                                                          page)),
                            ft.PopupMenuItem(icon=ft.icons.SAVE, text="Save Episode",
                                             on_click=lambda x, url=entry_audio_url.value,
                                                             title=entry_title.value: save_selected_episode(url, title, page))
                        ]
                        )
                    ep_controls_row = ft.Row(controls=[ep_resume_button, ep_popup_button])
                    ep_row_content = ft.ResponsiveRow([
                        ft.Column(col={"md": 2}, controls=[entry_artwork_url]),
                        ft.Column(col={"md": 8},
                                  controls=[entry_title, rotate_button, entry_description, entry_released]),
                        ft.Column(col={"md": 2}, controls=[ep_controls_row])
                    ])
                else:
                    ep_row_content = ft.ResponsiveRow([
                        ft.Column(col={"md": 2}, controls=[entry_artwork_url]),
                        ft.Column(col={"md": 10}, controls=[entry_title, rotate_button, entry_description, entry_released]),
                    ])

                entry_description.visible = False
                rotate_iteration = AnimatedButton(rotate_button, entry_description)
                rotate_button.on_click = rotate_iteration.animate_poddisplay

                div_row = ft.Divider(color=active_user.accent_color)
                ep_row_final = ft.Column(controls=[ep_row_content, div_row])
                ep_row_list.controls.append(ep_row_final)
                ep_number += 1

            ep_row_contain = ft.Container(content=ep_row_list)
            ep_row_contain.padding = padding.only(left=70, right=50)

            pr_instance.rm_stack()            # Create search view object
            pod_view = ft.View(
                "/poddisplay",
                [
                    feed_row,
                    ep_row_contain
                ]

            )
            pod_view.bgcolor = active_user.bgcolor
            pod_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                pod_view
            )

        if page.route == "/pod_list" or page.route == "/pod_list":
            class Podlayout:
                def __init__(self, page):
                    self.page = page
                    self.pod_row_list = ft.ListView(divider_thickness=3, auto_scroll=True)
                    self.refresh_btn = ft.IconButton(icon=ft.icons.REFRESH, icon_color=active_user.font_color,
                                                     tooltip="Refresh Podcast List", on_click=self.refresh_podcasts)
                    self.refresh_btn.icon_color = active_user.font_color
                    self.refresh_ctn = ft.Container(
                        content=self.refresh_btn,
                        alignment=ft.alignment.top_left
                    )
                    self.banner_button = ft.ElevatedButton("Help!", on_click=show_banner_click)
                    self.banner_button.bgcolor = active_user.accent_color
                    self.banner_button.color = active_user.main_color
                    self.settings_row = ft.Row(vertical_alignment=ft.CrossAxisAlignment.START,
                                               controls=[self.refresh_ctn, self.banner_button])
                    self.search_row = ft.Row(spacing=25,
                                             controls=[page_items.search_pods, page_items.search_location, search_btn])
                    self.top_row = ft.Row(alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
                                          vertical_alignment=ft.CrossAxisAlignment.START,
                                          controls=[self.settings_row, self.search_row])
                    self.top_row_container = ft.Container(content=self.top_row, expand=True)
                    self.top_row_container.padding = ft.padding.only(left=60)
                    self.top_bar = ft.Row(vertical_alignment=ft.CrossAxisAlignment.START,
                                          controls=[self.top_row_container])
                    if current_episode.audio_playing == True:
                        pod_controls.audio_container.visible = True

                def refresh_podcasts(self):
                    # Fetch new podcast episodes from the server.
                    pod_list_data = api_functions.functions.call_return_pods(app_api.url, app_api.headers,
                                                                             active_user.user_id)
                    self.generate_layout(pod_list_data)

                def remove_selected_podcast(self, title):
                    # Call the API function to remove the podcast
                    response = api_functions.functions.call_remove_podcast(app_api.url, app_api.headers, title,
                                                                           active_user.user_id)

                    # Check if the podcast was removed successfully
                    if response:
                        # Display a success message
                        self.page.snack_bar = ft.SnackBar(content=ft.Text(f"{title} has been removed!"))
                        self.page.snack_bar.open = True

                        # Refresh the podcast list
                        self.refresh_podcasts()

                        # Update the page
                        self.page.update()
                    else:
                        # Display an error message if the podcast couldn't be removed
                        self.page.snack_bar = ft.SnackBar(content=ft.Text(f"Unable to remove {title}!"))
                        self.page.snack_bar.open = True

                def generate_layout(self, pod_list_data):
                    self.pod_row_list.controls.clear()

                    def on_pod_list_title_click(e, title, artwork, author, categories, desc, ep_count, feed, website):
                        evaluate_podcast(title, artwork, author, categories, desc, ep_count, feed, website)
                        open_poddisplay(e)

                    if pod_list_data is None:
                        pod_list_title = 'No Podcasts added yet'
                        artwork_no = random.randint(1, 12)
                        pod_list_artwork = os.path.join(script_dir, "images", "logo_random", f"{artwork_no}.jpeg")
                        pod_list_desc = "Looks like you haven't added any podcasts yet. Search for podcasts you enjoy in the upper right portion of the screen and click the plus button to add them. They will begin to show up here and new episodes will be put into the main feed. You'll also be able to start downloading and saving episodes. Enjoy the listening!"
                        pod_list_ep_count = 'Start Searching!'
                        pod_list_website = "https://github.com/madeofpendletonwool/PinePods"
                        pod_list_feed = ""
                        pod_list_author = "PinePods"
                        pod_list_categories = ""

                        # Parse webpages needed to extract podcast artwork
                        pod_list_art_parsed = check_image(pod_list_artwork)
                        pod_list_artwork_image = ft.Image(src=pod_list_art_parsed, width=150, height=150)

                        # Defining the attributes of each podcast that will be displayed on screen
                        pod_list_title_display = ft.Text(pod_list_title)
                        pod_list_desc_display = ft.Text(pod_list_desc)
                        # Episode Count and subtitle
                        pod_list_ep_title = ft.Text('PinePods:', weight=ft.FontWeight.BOLD)
                        pod_list_ep_count_display = ft.Text(pod_list_ep_count)
                        pod_list_ep_info = ft.Row(controls=[pod_list_ep_title, pod_list_ep_count_display])
                        remove_pod_button = ft.IconButton(
                            icon=ft.icons.EMOJI_EMOTIONS,
                            icon_color=active_user.accent_color,
                            icon_size=40,
                            tooltip="Start Adding Podcasts!"
                        )

                        # Creating column and row for search layout
                        pod_list_column = ft.Column(
                            controls=[pod_list_title_display, pod_list_desc_display, pod_list_ep_info]
                        )
                        pod_list_row_content = ft.ResponsiveRow([
                            ft.Column(col={"md": 2}, controls=[pod_list_artwork_image]),
                            ft.Column(col={"md": 10}, controls=[pod_list_column, remove_pod_button]),
                        ])
                        pod_list_row = ft.Container(content=pod_list_row_content)
                        pod_list_row.padding = padding.only(left=70, right=50)
                        self.pod_row_list.controls.append(pod_list_row)

                    else:

                        for entry in pod_list_data:
                            pod_list_title = entry['PodcastName']
                            pod_list_artwork = entry['ArtworkURL']
                            pod_list_desc = entry['Description']
                            pod_list_ep_count = entry['EpisodeCount']
                            pod_list_website = entry['WebsiteURL']
                            pod_list_feed = entry['FeedURL']
                            pod_list_author = entry['Author']
                            pod_list_categories = entry['Categories']

                            # Parse webpages needed to extract podcast artwork
                            pod_list_art_parsed = check_image(pod_list_artwork)
                            pod_list_artwork_image = ft.Image(src=pod_list_art_parsed, width=150, height=150)

                            # Defining the attributes of each podcast that will be displayed on screen
                            pod_list_title_display = ft.TextButton(
                                text=pod_list_title,
                                on_click=lambda x, e=e, title=pod_list_title, artwork=pod_list_artwork,
                                                author=pod_list_author,
                                                categories=pod_list_categories, desc=pod_list_desc,
                                                ep_count=pod_list_ep_count,
                                                feed=pod_list_feed, website=pod_list_website: on_pod_list_title_click(e,
                                                                                                                      title,
                                                                                                                      artwork,
                                                                                                                      author,
                                                                                                                      categories,
                                                                                                                      desc,
                                                                                                                      ep_count,
                                                                                                                      feed,
                                                                                                                      website)
                            )
                            pod_list_desc_display = ft.Text(pod_list_desc)
                            # Episode Count and subtitle
                            pod_list_ep_title = ft.Text('Episode Count:', weight=ft.FontWeight.BOLD,
                                                        color=active_user.font_color)
                            pod_list_ep_count_display = ft.Text(pod_list_ep_count, color=active_user.font_color)
                            pod_list_ep_info = ft.Row(controls=[pod_list_ep_title, pod_list_ep_count_display])
                            remove_pod_button = ft.IconButton(
                                icon=ft.icons.INDETERMINATE_CHECK_BOX,
                                icon_color="red400",
                                icon_size=40,
                                tooltip="Remove Podcast",
                                on_click=lambda x, title=pod_list_title: self.remove_selected_podcast(title)
                            )

                            # Creating column and row for search layout
                            pod_list_column = ft.Column(
                                controls=[pod_list_title_display, pod_list_desc_display, pod_list_ep_info]
                            )

                            pod_list_row_content = ft.ResponsiveRow([
                                ft.Column(col={"md": 2}, controls=[pod_list_artwork_image]),
                                ft.Column(col={"md": 10}, controls=[pod_list_column, remove_pod_button]),
                            ])
                            div_row = ft.Divider(color=active_user.accent_color)
                            pod_row_column = ft.Column(controls=[pod_list_row_content, div_row])
                            pod_list_row = ft.Container(content=pod_row_column)
                            pod_list_row.padding = padding.only(left=70, right=50)
                            self.pod_row_list.controls.append(pod_list_row)

            # Get Pod info
            pod_list_data = api_functions.functions.call_return_pods(app_api.url, app_api.headers, active_user.user_id)
            pod_list_instance = Podlayout(page)
            pod_list_instance.generate_layout(pod_list_data)

            pod_view_title = ft.Text(
                "Added Podcasts:",
                size=30,
                font_family="RobotoSlab",
                color=active_user.font_color,
                weight=ft.FontWeight.W_300,
            )
            pod_add_url = ft.ElevatedButton("Add Podcast from URL feed", bgcolor=active_user.main_color, color=active_user.accent_color, on_click=lambda x: (pod_url_add(page)))
            pod_view_row = ft.Row(controls=[pod_view_title], alignment=ft.MainAxisAlignment.CENTER)
            pod_add_row = ft.Row(controls=[pod_add_url], alignment=ft.MainAxisAlignment.END)
            # Create search view object
            pod_list_view = ft.View("/pod_list",
                                    [
                                        pod_list_instance.top_bar,
                                        pod_add_row,
                                        pod_view_row,
                                        pod_list_instance.pod_row_list

                                    ]

                                    )
            pod_list_view.bgcolor = active_user.bgcolor
            pod_list_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                pod_list_view

            )

        if page.route == "/searchpod" or page.route == "/searchpod":
            # Get Pod info
            podcast_value = new_search.searchvalue

            def get_podcast_description(feed_url):
                try:
                    response = urllib.request.urlopen(feed_url, timeout=10)
                    feed = feedparser.parse(response)
                    return feed.feed.get('description', '')
                except (socket.timeout, Exception):
                    return ''

            def map_search_result(result, source):
                mapped = {}

                if source == 'itunes':
                    mapped['title'] = result['collectionName']
                    mapped['url'] = result['feedUrl']
                    mapped['link'] = result['collectionViewUrl']
                    mapped['description'] = get_podcast_description(result['feedUrl'])
                    mapped['author'] = result['artistName']
                    mapped['artwork'] = result['artworkUrl600']
                    mapped['categories'] = result['genres']
                    mapped['episodeCount'] = result['trackCount']
                else:  # podcastindex
                    mapped = result

                return mapped

            search_results = internal_functions.functions.searchpod(podcast_value, api_url, new_search.searchlocation)

            # Create a ThreadPoolExecutor.
            with ThreadPoolExecutor(max_workers=20) as executor:
                futures = [executor.submit(map_search_result, result, new_search.searchlocation) for result in
                           search_results['results' if new_search.searchlocation == 'itunes' else 'feeds']]
                return_results = [future.result() for future in as_completed(futures)]

            pr_instance.rm_stack()

            if search_results.get('feeds') or search_results.get('results'):
                # Get and format list
                pod_number = 1
                search_row_list = ft.ListView(divider_thickness=3, auto_scroll=True)
                for d in return_results:
                    for k, v in d.items():
                        if k == 'title':
                            # Parse webpages needed to extract podcast artwork
                            search_art_no = random.randint(1, 12)
                            search_art_fallback = os.path.join(script_dir, "images", "logo_random",
                                                               f"{search_art_no}.jpeg")
                            search_art_url = d['artwork'] if d['artwork'] else search_art_fallback
                            podimage_parsed = check_image(search_art_url)
                            pod_image = ft.Image(src=podimage_parsed, width=150, height=150)

                            # Defining the attributes of each podcast that will be displayed on screen
                            pod_title_button = ft.Text(d['title'], style=ft.TextThemeStyle.TITLE_MEDIUM,
                                                       color=active_user.font_color)
                            pod_title = ft.TextButton(
                                content=pod_title_button,
                                on_click=lambda x, d=d: (
                                    evaluate_podcast(d['title'], d['artwork'], d['author'], d['categories'],
                                                     d['description'],
                                                     d['episodeCount'], d['url'], d['link']), open_poddisplay(e))
                            )
                            pod_desc = ft.Text(d['description'])
                            # Episode Count and subtitle
                            pod_ep_title = ft.Text('Episode Count:', weight=ft.FontWeight.BOLD,
                                                   color=active_user.font_color)
                            pod_ep_count = ft.Text(d['episodeCount'], color=active_user.font_color)
                            pod_ep_info = ft.Row(controls=[pod_ep_title, pod_ep_count])
                            add_pod_button = ft.IconButton(
                                icon=ft.icons.ADD_BOX,
                                icon_color=active_user.accent_color,
                                icon_size=40,
                                tooltip="Add Podcast",
                                on_click=lambda x, d=d: send_podcast(d['title'], d['artwork'], d['author'],
                                                                     d['categories'],
                                                                     d['description'], d['episodeCount'], d['url'],
                                                                     d['link'], page)
                            )
                            # Creating column and row for search layout
                            search_column = ft.Column(
                                controls=[pod_title, pod_desc, pod_ep_info]
                            )
                            search_row_content = ft.ResponsiveRow([
                                ft.Column(col={"md": 2}, controls=[pod_image]),
                                ft.Column(col={"md": 10}, controls=[search_column, add_pod_button]),
                            ])
                            div_row = ft.Divider(color=active_user.accent_color)
                            search_row_column = ft.Column(controls=[search_row_content, div_row])
                            search_row = ft.Container(content=search_row_column)
                            search_row.padding = padding.only(left=70, right=50)
                            search_row_list.controls.append(search_row)
                            pod_number += 1
            else:
                search_row_text = ft.Text("No results found. Please adjust your query and try again", size=18)
                search_row_column = ft.Column(controls=[search_row_text])
                search_row = ft.Container(content=search_row_column)
                search_row.padding = padding.only(left=70, right=50)
                search_row_list = search_row
            # Create search view object
            search_view = ft.View("/searchpod",
                                  [
                                      search_row_list
                                  ]

                                  )
            search_view.bgcolor = active_user.bgcolor
            search_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                search_view

            )

        if page.route == "/userstats" or page.route == "/userstats":
            user_stats = api_functions.functions.call_get_stats(app_api.url, app_api.headers, active_user.user_id)

            stats_created_date = user_stats['UserCreated']
            stats_pods_played = user_stats['PodcastsPlayed']
            stats_time_listened = user_stats['TimeListened']
            stats_pods_added = user_stats['PodcastsAdded']
            stats_eps_saved = user_stats['EpisodesSaved']
            stats_eps_downloaded = user_stats['EpisodesDownloaded']

            user_ep_count = api_functions.functions.call_get_user_episode_count(app_api.url, app_api.headers,
                                                                                active_user.user_id)

            user_title = ft.Text(f"Stats for {active_user.fullname}:", size=20, weight="bold")
            date_display = ft.Text(f'{active_user.username} created on {stats_created_date}', size=16)
            pods_played_display = ft.Text(f'{stats_pods_played} Podcasts listened to', size=16)
            time_listened_display = ft.Text(f'{stats_time_listened} Minutes spent listening', size=16)
            pods_added_display = ft.Text(f'{stats_pods_added} Podcasts added', size=16)
            eps_added_display = ft.Text(
                f'{user_ep_count} Episodes associated with {active_user.fullname} in the database', size=16)
            eps_saved_display = ft.Text(f'{stats_eps_saved} Podcasts episodes currently saved', size=16)
            eps_downloaded_display = ft.Text(f'{stats_eps_downloaded} Podcasts episodes currently downloaded', size=16)
            stats_column = ft.Column(
                controls=[user_title, date_display, pods_played_display, time_listened_display, pods_added_display,
                          eps_added_display, eps_saved_display, eps_downloaded_display])
            stats_container = ft.Container(content=stats_column)
            stats_container.padding = padding.only(left=70, right=50)

            def highlight_link(e):
                e.control.style.color = ft.colors.BLUE
                e.control.update()

            def unhighlight_link(e):
                e.control.style.color = None
                e.control.update()

            # Creator info
            coffee_info = ft.Column([ft.Text('PinePods is a creation of Collin Pendleton.', ft.TextAlign.CENTER),
                                     ft.Text('A lot of work has gone into making this app.', ft.TextAlign.CENTER),
                                     ft.Text('Thank you for using it!', ft.TextAlign.CENTER),
                                     ft.Text(
                                         disabled=False,
                                         spans=[
                                             ft.TextSpan("If you'd like, you can buy me a coffee "),
                                             ft.TextSpan(
                                                 "here",
                                                 ft.TextStyle(decoration=ft.TextDecoration.UNDERLINE),
                                                 url="https://www.buymeacoffee.com/collinscoffee",
                                                 on_enter=highlight_link,
                                                 on_exit=unhighlight_link,
                                             ),
                                         ],
                                     ),
                                     ], ft.MainAxisAlignment.CENTER, ft.CrossAxisAlignment.CENTER)
            coffee_contain = ft.Container(content=coffee_info)
            coffee_contain.alignment = alignment.bottom_center
            coffee_script_dir = os.path.dirname(os.path.realpath(__file__))
            image_path = os.path.join(coffee_script_dir, "assets", "pinepods-appicon.png")
            pinepods_img = ft.Image(
                src=image_path,
                width=100,
                height=100,
                fit=ft.ImageFit.CONTAIN,
            )
            pine_contain = ft.Container(content=pinepods_img)
            pine_contain.alignment = alignment.bottom_center
            pine_div_row = ft.Divider(color=active_user.accent_color)
            pine_contain.padding = padding.only(top=40)

            stats_view = ft.View("/userstats",
                                 [
                                     stats_container,
                                     pine_div_row,
                                     pine_contain,
                                     coffee_contain
                                 ]

                                 )
            stats_view.bgcolor = active_user.bgcolor
            stats_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                stats_view
            )

        if page.route == "/login" or page.route == "/login":
            guest_enabled = api_functions.functions.call_guest_status(app_api.url, app_api.headers)
            retain_session = ft.Switch(label="Stay Signed in", value=False)
            retain_session_contained = ft.Container(content=retain_session)
            retain_session_contained.padding = padding.only(left=70)
            login_button = ft.FilledButton(
                content=ft.Text(
                    "Login",
                    weight="w700",
                ),
                width=160,
                height=40,
                # Now, if we want to login, we also need to send some info back to the server and check if the credentials are correct or if they even exists.
                on_click=lambda e: active_user.login(login_username, login_password, retain_session.value)
                # on_click=lambda e: go_homelogin(e)
            )
            if page.web:
                retain_session.visible = False
            if guest_enabled == True:
                login_startpage = ft.Column(
                    alignment=ft.MainAxisAlignment.CENTER,
                    horizontal_alignment=ft.CrossAxisAlignment.CENTER,
                    controls=[
                        ft.Card(
                            elevation=15,
                            content=ft.Container(
                                width=550,
                                height=650,
                                padding=padding.all(30),
                                gradient=GradientGenerator(
                                    "#2f2937", "#251867"
                                ),
                                border_radius=border_radius.all(12),
                                content=ft.Column(
                                    horizontal_alignment="center",
                                    alignment="start",
                                    controls=[
                                        ft.Text(
                                            "PinePods",
                                            size=32,
                                            weight="w700",
                                            text_align="center",
                                        ),
                                        ft.Text(
                                            "A Forest of Podcasts, Rooted in the Spirit of Self-Hosting",
                                            size=22,
                                            weight="w700",
                                            text_align="center",
                                        ),
                                        ft.Text(
                                            "Please login with your user account to start listening to podcasts. If you didn't set a default user up please check the docker logs for a default account and credentials",
                                            size=14,
                                            weight="w700",
                                            text_align="center",
                                            color="#64748b",
                                        ),
                                        ft.Container(
                                            padding=padding.only(bottom=20)
                                        ),
                                        login_username,
                                        ft.Container(
                                            padding=padding.only(bottom=10)
                                        ),
                                        login_password,
                                        ft.Container(
                                            padding=padding.only(bottom=20)
                                        ),
                                        retain_session_contained,
                                        ft.Row(
                                            alignment="center",
                                            spacing=20,
                                            controls=[
                                                login_button,
                                                ft.FilledButton(
                                                    content=ft.Text(
                                                        "Guest Login",
                                                        weight="w700",
                                                    ),
                                                    width=160,
                                                    height=40,
                                                    # Now, if we want to login, we also need to send some info back to the server and check if the credentials are correct or if they even exists.
                                                    on_click=lambda e: go_homelogin_guest(page)
                                                    # on_click=lambda e: go_homelogin(e)
                                                ),
                                            ],
                                        ),
                                        ft.Row(
                                            alignment="center",
                                            spacing=20,
                                            controls=[
                                                ft.Text("Haven't created a user yet?"),
                                                ft.OutlinedButton(text="Create New User", on_click=self_service_user)

                                            ]

                                        ),
                                        ft.Row(
                                            alignment="center",
                                            spacing=20,
                                            controls=[
                                                ft.Text("Forgot Password?"),
                                                ft.OutlinedButton(
                                                    content=ft.Text(
                                                        "Reset Password",
                                                        weight="w700",
                                                    ),
                                                    width=160,
                                                    height=40,
                                                    # Now, if we want to login, we also need to send some info back to the server and check if the credentials are correct or if they even exists.
                                                    on_click=lambda e: reset_credentials(page)
                                                    # on_click=lambda e: go_homelogin(e)
                                                ),

                                            ]

                                        )
                                    ],
                                ),
                            ),
                        )
                    ],
                )
            else:
                login_startpage = ft.Column(
                    alignment=ft.MainAxisAlignment.CENTER,
                    horizontal_alignment=ft.CrossAxisAlignment.CENTER,
                    controls=[
                        ft.Card(
                            elevation=15,
                            content=ft.Container(
                                width=550,
                                height=650,
                                padding=ft.padding.all(30),
                                gradient=GradientGenerator(
                                    "#2f2937", "#251867"
                                ),
                                border_radius=ft.border_radius.all(12),
                                content=ft.Column(
                                    horizontal_alignment="center",
                                    alignment="start",
                                    controls=[
                                        ft.Text(
                                            "PinePods",
                                            size=32,
                                            weight="w700",
                                            text_align="center",
                                        ),
                                        ft.Text(
                                            "A Forest of Podcasts, Rooted in the Spirit of Self-Hosting",
                                            size=22,
                                            weight="w700",
                                            text_align="center",
                                        ),
                                        ft.Text(
                                            "Please login with your user account to start listening to podcasts. If you didn't set a default user up please check the docker logs for a default account and credentials",
                                            size=14,
                                            weight="w700",
                                            text_align="center",
                                            color="#64748b",
                                        ),
                                        ft.Container(
                                            padding=ft.padding.only(bottom=20)
                                        ),
                                        login_username,
                                        ft.Container(
                                            padding=ft.padding.only(bottom=10)
                                        ),
                                        login_password,
                                        ft.Container(
                                            padding=ft.padding.only(bottom=20)
                                        ),
                                        retain_session_contained,
                                        ft.Row(
                                            alignment="center",
                                            spacing=20,
                                            controls=[
                                                login_button,
                                            ],
                                        ),
                                        ft.Row(
                                            alignment="center",
                                            spacing=20,
                                            controls=[
                                                ft.Text("Haven't created a user yet?"),
                                                ft.OutlinedButton(text="Create New User", on_click=self_service_user)

                                            ]

                                        ),
                                        ft.Row(
                                            alignment="center",
                                            spacing=20,
                                            controls=[
                                                ft.Text("Forgot Password?"),
                                                ft.OutlinedButton(
                                                    content=ft.Text(
                                                        "Reset Password",
                                                        weight="w700",
                                                    ),
                                                    width=160,
                                                    height=40,
                                                    # Now, if we want to login, we also need to send some info back to the server and check if the credentials are correct or if they even exists.
                                                    on_click=lambda e: reset_credentials(page)
                                                    # on_click=lambda e: go_homelogin(e)
                                                ),

                                            ]

                                        )
                                    ],
                                ),
                            ),
                        )
                    ],
                )

            # Create search view object
            login_startpage_view = ft.View("/login",
                                           horizontal_alignment="center",
                                           vertical_alignment="center",
                                           controls=[
                                               login_startpage
                                           ]

                                           )
            # search_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                login_startpage_view

            )

        if page.route == "/mfalogin" or page.route == "/mfalogin":
            retain_session = ft.Switch(label="Save API Key", value=False)
            retain_session_contained = ft.Container(content=retain_session)
            retain_session_contained.padding = padding.only(left=70)

            server_configpage = ft.Column(
                alignment=ft.MainAxisAlignment.CENTER,
                horizontal_alignment=ft.CrossAxisAlignment.CENTER,
                controls=[
                    ft.Card(
                        elevation=15,
                        content=ft.Container(
                            width=550,
                            height=450,
                            padding=padding.all(30),
                            gradient=GradientGenerator(
                                "#2f2937", "#251867"
                            ),
                            border_radius=border_radius.all(12),
                            content=ft.Column(
                                horizontal_alignment="center",
                                alignment="start",
                                controls=[
                                    ft.Text(
                                        "PinePods",
                                        size=32,
                                        weight="w700",
                                        text_align="center",
                                    ),
                                    ft.Text(
                                        "A Forest of Podcasts, Rooted in the Spirit of Self-Hosting",
                                        size=22,
                                        weight="w700",
                                        text_align="center",
                                    ),
                                    ft.Text(
                                        "Please enter the MFA code for Pinepods from your authenticator app",
                                        size=14,
                                        weight="w700",
                                        text_align="center",
                                        color="#64748b",
                                    ),
                                    ft.Container(
                                        padding=padding.only(bottom=20)
                                    ),
                                    mfa_prompt,
                                    ft.Container(
                                        padding=padding.only(bottom=10)
                                    ),
                                    ft.Row(
                                        alignment="center",
                                        spacing=20,
                                        controls=[
                                            ft.FilledButton(
                                                content=ft.Text(
                                                    "Login",
                                                    weight="w700",
                                                ),
                                                width=160,
                                                height=40,
                                                # Now, if we want to login, we also need to send some info back to the server and check if the credentials are correct or if they even exists.
                                                on_click=lambda e: active_user.mfa_login(mfa_prompt)
                                                # on_click=lambda e: go_homelogin(e)
                                            ),
                                        ],
                                    ),
                                    ft.Row(
                                        alignment="center",
                                        spacing=20,
                                        controls=[
                                            ft.FilledButton(
                                                content=ft.Text(
                                                    "Cancel",
                                                    weight="w700",
                                                ),
                                                width=160,
                                                height=40,
                                                # Now, if we want to login, we also need to send some info back to the server and check if the credentials are correct or if they even exists.
                                                on_click=active_user.logout_pinepods
                                                # on_click=lambda e: go_homelogin(e)
                                            ),
                                        ],
                                    ),
                                ],
                            ),
                        ),
                    )
                ],
            )

            # Create search view object
            server_configpage_view = ft.View("/server_config",
                                             horizontal_alignment="center",
                                             vertical_alignment="center",
                                             controls=[
                                                 server_configpage
                                             ]

                                             )
            # search_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                server_configpage

            )

        if page.route == "/first_time_config" or page.route == "/first_time_config":
            tz_text = ft.Text('Select TimeZone:', color=active_user.font_color, size=16)
            timezones = pytz.all_timezones
            tz_drop = ft.Dropdown(border_color=active_user.accent_color, color=active_user.font_color,
                                  focused_bgcolor=active_user.main_color, focused_border_color=active_user.accent_color,
                                  focused_color=active_user.accent_color,
                                  options=[ft.dropdown.Option(tz) for tz in timezones]
                                  )
            clock_text = ft.Text('Select Time Preference:', color=active_user.font_color, size=16)
            clock_drop = ft.Dropdown(border_color=active_user.accent_color, color=active_user.font_color,
                                     focused_bgcolor=active_user.main_color,
                                     focused_border_color=active_user.accent_color,
                                     focused_color=active_user.accent_color,
                                     options=[
                                         ft.dropdown.Option("12-hour"),
                                         ft.dropdown.Option("24-hour"),
                                     ]
                                     )

            first_time_page = ft.Column(
                alignment=ft.MainAxisAlignment.CENTER,
                horizontal_alignment=ft.CrossAxisAlignment.CENTER,
                controls=[
                    ft.Card(
                        elevation=15,
                        content=ft.Container(
                            width=550,
                            height=600,
                            padding=padding.all(30),
                            gradient=GradientGenerator(
                                "#2f2937", "#251867"
                            ),
                            border_radius=border_radius.all(12),
                            content=ft.Column(
                                horizontal_alignment="center",
                                alignment="start",
                                controls=[
                                    ft.Text(
                                        "PinePods",
                                        size=32,
                                        weight="w700",
                                        text_align="center",
                                    ),
                                    ft.Text(
                                        "A Forest of Podcasts, Rooted in the Spirit of Self-Hosting",
                                        size=22,
                                        weight="w700",
                                        text_align="center",
                                    ),
                                    ft.Text(
                                        "Hello! This appears to be your first time logging in. Let's get some basic information so that we can display podcasts in the way you prefer. This information is stored on your own server only.",
                                        size=14,
                                        weight="w700",
                                        text_align="center",
                                        color="#64748b",
                                    ),
                                    ft.Container(
                                        padding=padding.only(bottom=20)
                                    ),
                                    tz_text,
                                    tz_drop,
                                    ft.Container(
                                        padding=padding.only(bottom=10)
                                    ),
                                    clock_text,
                                    clock_drop,
                                    ft.Container(
                                        padding=padding.only(bottom=10)
                                    ),
                                    ft.Row(
                                        alignment="center",
                                        spacing=20,
                                        controls=[
                                            ft.FilledButton(
                                                content=ft.Text(
                                                    "Submit",
                                                    weight="w700",
                                                ),
                                                width=160,
                                                height=40,
                                                # Now, if we want to login, we also need to send some info back to the server and check if the credentials are correct or if they even exists.
                                                on_click=lambda e: active_user.setup_timezone(tz_drop.value,
                                                                                              clock_drop.value)
                                            ),
                                        ],
                                    ),
                                ],
                            ),
                        ),
                    )
                ],
            )

            # Create search view object
            first_time_view = ft.View("/first_time_config",
                                      horizontal_alignment="center",
                                      vertical_alignment="center",
                                      controls=[
                                          first_time_page
                                      ]

                                      )
            # search_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                first_time_view

            )

        if page.route == "/settings" or page.route == "/settings":

            class Settings:
                def __init__(self, page):
                    self.page = page
                    self.app_api = app_api
                    # Guest login Setup
                    self.guest_status_bool = api_functions.functions.call_guest_status(app_api.url, app_api.headers)
                    self.disable_guest_notify = ft.Text(
                        f'Guest user is currently {"enabled" if self.guest_status_bool else "disabled"}')
                    self.guest_check()
                    # Self Service user create setup
                    self.self_service_bool = api_functions.functions.call_self_service_status(app_api.url,
                                                                                              app_api.headers)
                    self.self_service_notify = ft.Text(
                        f'Self Service user creation is currently {"enabled" if self.self_service_bool else "disabled"}')
                    self.self_service_check()
                    # Backup Settings Setup
                    self.settings_backup_data()
                    # Import Settings Setup
                    self.settings_import_data()
                    # Server Downloads Setup
                    self.download_status_bool = api_functions.functions.call_download_status(app_api.url,
                                                                                             app_api.headers)
                    self.disable_download_notify = ft.Text(
                        f'Downloads are currently {"enabled" if self.download_status_bool else "disabled"}')
                    self.downloads_check()

                    # MFA Settings Setup
                    self.check_mfa_status = api_functions.functions.call_check_mfa_enabled(app_api.url, app_api.headers,
                                                                                           active_user.user_id)
                    self.mfa_check()
                    # New User Creation Setup
                    self.user_table_rows = []
                    self.user_table_load()
                    # Email Settings Setup
                    self.email_information = api_functions.functions.call_get_email_info(app_api.url, app_api.headers)
                    self.email_table_rows = []
                    self.email_table_load()

                def settings_backup_data(self):
                    backup_option_text = Text('Backup Data:', color=active_user.font_color, size=16)
                    backup_option_desc = Text(
                        "Note: This option allows you to backup data in Pinepods. This can be used to backup podcasts to an opml file, or if you're an admin, it can also backup server information for a full restore. Like users, and current server settings.",
                        color=active_user.font_color)
                    self.settings_backup_button = ft.ElevatedButton(f'Backup Data',
                                                                   on_click=self.backup_data,
                                                                   bgcolor=active_user.main_color,
                                                                   color=active_user.accent_color)
                    setting_backup_col = ft.Column(
                        controls=[backup_option_text, backup_option_desc, self.settings_backup_button])
                    self.setting_backup_con = ft.Container(content=setting_backup_col)
                    self.setting_backup_con.padding = padding.only(left=70, right=50)

                def settings_import_data(self):
                    import_option_text = Text('Import Data:', color=active_user.font_color, size=16)
                    import_option_desc = Text(
                        "Note: This option allows you to import backed up data into Pinepods. You can import OPML files for podcast rss feeds and, if you're an admin, you can import entire server information.",
                        color=active_user.font_color)
                    self.settings_import_button = ft.ElevatedButton(f'Import Data',
                                                                   on_click=self.import_data,
                                                                   bgcolor=active_user.main_color,
                                                                   color=active_user.accent_color)
                    setting_import_col = ft.Column(
                        controls=[import_option_text, import_option_desc, self.settings_import_button])
                    self.setting_import_con = ft.Container(content=setting_import_col)
                    self.setting_import_con.padding = padding.only(left=70, right=50)

                def update_mfa_status(self):
                    self.check_mfa_status = api_functions.functions.call_check_mfa_enabled(
                        self.app_api.url, self.app_api.headers, active_user.user_id
                    )
                    if self.check_mfa_status:
                        self.mfa_button.text = f'Re-Setup MFA for your account'
                        self.mfa_button.on_click = self.mfa_option_change
                        if 'mfa_remove_button' not in dir(self):  # create mfa_remove_button if it doesn't exist
                            self.mfa_remove_button = ft.ElevatedButton(f'Remove MFA for your account',
                                                                       on_click=self.remove_mfa,
                                                                       bgcolor=active_user.main_color,
                                                                       color=active_user.accent_color)
                        if self.mfa_button_row is None:
                            self.mfa_button_row = ft.Row()
                        self.mfa_button_row.controls = [self.mfa_button, self.mfa_remove_button]
                    else:
                        self.mfa_button.text = f'Setup MFA for your account'
                        self.mfa_button.on_click = self.setup_mfa
                        if 'mfa_remove_button' in dir(
                                self):  # remove mfa_remove_button from mfa_button_row.controls if it exists
                            self.mfa_button_row.controls = [self.mfa_button]
                    self.mfa_container.content = self.mfa_column
                    self.page.update()

                def remove_mfa(self, e):
                    delete_confirm = api_functions.functions.call_delete_mfa_secret(app_api.url, app_api.headers,
                                                                                    active_user.user_id)
                    if delete_confirm:
                        self.page.snack_bar = ft.SnackBar(content=ft.Text(
                            f"MFA now removed from your account. You'll no longer be prompted at login"))
                        self.page.snack_bar.open = True
                        self.update_mfa_status()
                        self.page.update()
                    else:
                        self.page.snack_bar = ft.SnackBar(
                            content=ft.Text(f"Error removing MFA settings. Maybe it's not already setup?"))
                        self.page.snack_bar.open = True
                        self.page.update()

                def setup_mfa(self, e):
                    def close_mfa_dlg(e):
                        mfa_dlg.open = False
                        os.remove(f"{user_data_dir}/{active_user.user_id}_qrcode_{active_user.mfa_timestamp}.png")
                        self.page.update()

                    def close_validate_mfa_dlg(page):
                        validate_mfa_dlg.open = False
                        try:
                            os.remove(f"{user_data_dir}/{active_user.user_id}_qrcode_{active_user.mfa_timestamp}.png")
                        except:
                            pass
                        self.page.update()

                    def complete_mfa(e):
                        # Get the OTP entered by the user
                        close_validate_mfa_dlg(self.page)
                        self.page.update()

                        entered_otp = mfa_confirm_box.value

                        # Verify the OTP
                        totp = pyotp.TOTP(active_user.mfa_secret)
                        if totp.verify(entered_otp, valid_window=1):
                            # If the OTP is valid, save the MFA secret
                            api_functions.functions.call_save_mfa_secret(app_api.url, app_api.headers,
                                                                         active_user.user_id, active_user.mfa_secret)

                            # Close the dialog and show a success message
                            close_validate_mfa_dlg(self.page)
                            self.page.snack_bar = ft.SnackBar(
                                content=ft.Text(f"MFA now configured! On next login you'll be prompted for your code!"))
                            self.page.snack_bar.open = True
                            self.update_mfa_status()
                            return True
                        else:
                            # If the OTP is not valid, show an error message
                            self.page.snack_bar = ft.SnackBar(content=ft.Text(
                                f"The entered OTP is incorrect. It also may have timed out before you entered it. Please cancel and try again."))
                            self.page.snack_bar.open = True
                        self.page.update()

                    mfa_confirm_box = ft.TextField(label="MFA Code", icon=ft.icons.LOCK_CLOCK, hint_text='123456')
                    mfa_validate_select_row = ft.Row(
                        controls=[
                            ft.TextButton("Confirm", on_click=complete_mfa),
                            ft.TextButton("Cancel", on_click=lambda x: (close_validate_mfa_dlg(page)))
                        ],
                        alignment=ft.MainAxisAlignment.END
                    )
                    validate_mfa_dlg = ft.AlertDialog(
                        modal=True,
                        title=ft.Text(f"Confirm MFA:"),
                        content=ft.Column(controls=[
                            ft.Text(f'Please confirm the code from your authenticator app.', selectable=True),
                            # ], tight=True),
                            mfa_confirm_box,
                            # actions=[
                            mfa_validate_select_row
                        ],
                            tight=True),
                        actions_alignment=ft.MainAxisAlignment.END,
                    )

                    def validate_mfa(e):
                        close_mfa_dlg(self.page)
                        self.page.update()
                        time.sleep(.3)

                        self.page.dialog = validate_mfa_dlg
                        validate_mfa_dlg.open = True
                        self.page.update()

                    img_data_url = setup_user_for_otp()
                    mfa_select_row = ft.Row(
                        controls=[
                            ft.TextButton("Continue", on_click=validate_mfa),
                            ft.TextButton("Close", on_click=lambda x: (close_mfa_dlg(self.page)))
                        ],
                        alignment=ft.MainAxisAlignment.END
                    )
                    mfa_dlg = ft.AlertDialog(
                        modal=True,
                        title=ft.Text(f"Setup MFA:"),
                        content=ft.Column(controls=[
                            ft.Text(
                                f'Scan the code below with your authenticator app and then click continue to validate your code.',
                                selectable=True),
                            ft.Image(src=img_data_url, width=200, height=200),
                            ft.Text(f'MFA Secret for manual entry: {active_user.mfa_secret}', selectable=True),
                            ft.Text('Enter TOTP as the type if doing manual entry', selectable=True),
                            mfa_select_row
                        ],
                            tight=True),
                        actions_alignment=ft.MainAxisAlignment.END,
                    )
                    self.page.dialog = mfa_dlg
                    mfa_dlg.open = True
                    self.page.update()

                def guest_check(self):
                    if self.guest_status_bool:
                        self.guest_status = 'enabled'
                        self.guest_info_button = ft.ElevatedButton(f'Disable Guest User',
                                                                   on_click=self.guest_user_change,
                                                                   bgcolor=active_user.main_color,
                                                                   color=active_user.accent_color)
                    else:
                        self.guest_status = 'disabled'
                        self.guest_info_button = ft.ElevatedButton(f'Enable Guest User',
                                                                   on_click=self.guest_user_change,
                                                                   bgcolor=active_user.main_color,
                                                                   color=active_user.accent_color)

                def self_service_check(self):
                    if self.self_service_bool:
                        self.self_service_status = 'enabled'
                        self.self_service_button = ft.ElevatedButton(f'Disable Self Service User Creation',
                                                                     on_click=self.self_service_change,
                                                                     bgcolor=active_user.main_color,
                                                                     color=active_user.accent_color)
                    else:
                        self.self_service_status = 'disabled'
                        self.self_service_button = ft.ElevatedButton(f'Enable Self Service User Creation',
                                                                     on_click=self.self_service_change,
                                                                     bgcolor=active_user.main_color,
                                                                     color=active_user.accent_color)

                def downloads_check(self):
                    if self.download_status_bool:
                        self.download_info_button = ft.ElevatedButton(f'Disable Podcast Downloads',
                                                                      on_click=self.download_option_change,
                                                                      bgcolor=active_user.main_color,
                                                                      color=active_user.accent_color)
                    else:
                        self.download_info_button = ft.ElevatedButton(f'Enable Podcast Downloads',
                                                                      on_click=self.download_option_change,
                                                                      bgcolor=active_user.main_color,
                                                                      color=active_user.accent_color)

                def mfa_check(self):
                    self.mfa_warning = ft.Text(
                        'Note: when setting up MFA you have 1 minute to enter the code or it will expire. If it expires just cancel and try again.',
                        color=active_user.font_color, size=12)

                    if self.check_mfa_status:
                        self.mfa_text = ft.Text(f'Setup MFA', color=active_user.font_color,
                                                size=16)
                        self.mfa_button = ft.ElevatedButton(f'Re-Setup MFA for your account',
                                                            on_click=self.mfa_option_change,
                                                            bgcolor=active_user.main_color,
                                                            color=active_user.accent_color)
                        self.mfa_remove_button = ft.ElevatedButton(f'Remove MFA for your account',
                                                                   on_click=self.remove_mfa,
                                                                   bgcolor=active_user.main_color,
                                                                   color=active_user.accent_color)
                        self.mfa_button_row = ft.Row(
                            controls=[self.mfa_button, self.mfa_remove_button])
                        self.mfa_column = ft.Column(controls=[self.mfa_text, self.mfa_warning, self.mfa_button_row])
                    else:
                        self.mfa_text = ft.Text(f'Setup MFA', color=active_user.font_color,
                                                size=16)
                        self.mfa_button = ft.ElevatedButton(f'Setup MFA for your account', on_click=self.setup_mfa,
                                                            bgcolor=active_user.main_color,
                                                            color=active_user.accent_color)
                        self.mfa_column = ft.Column(controls=[self.mfa_text, self.mfa_warning, self.mfa_button])

                    # Update mfa_container content
                    self.mfa_container = ft.Container(content=self.mfa_column)
                    self.mfa_container.padding = padding.only(left=70, right=50)
                    self.mfa_container.content = self.mfa_column
                    self.page.update()

                def email_table_load(self):
                    server_info = self.email_information['Server_Name'] + ':' + str(
                        self.email_information['Server_Port'])
                    from_email = self.email_information['From_Email']
                    send_mode = self.email_information['Send_Mode']
                    encryption = self.email_information['Encryption']
                    auth = self.email_information['Auth_Required']

                    if auth == 1:
                        auth_user = self.email_information['Username']
                    else:
                        auth_user = 'Auth not defined!'

                    # Create a new data row with the user information
                    row = ft.DataRow(
                        cells=[
                            ft.DataCell(ft.Text(server_info)),
                            ft.DataCell(ft.Text(from_email)),
                            ft.DataCell(ft.Text(send_mode)),
                            ft.DataCell(ft.Text(encryption)),
                            ft.DataCell(ft.Text(auth_user))
                        ]
                    )

                    # Append the row to the list of data rows
                    self.email_table_rows.append(row)

                    self.email_table = ft.DataTable(
                        bgcolor=active_user.main_color,
                        border=ft.border.all(2, active_user.main_color),
                        border_radius=10,
                        vertical_lines=ft.border.BorderSide(3, active_user.tertiary_color),
                        horizontal_lines=ft.border.BorderSide(1, active_user.tertiary_color),
                        heading_row_color=active_user.nav_color1,
                        heading_row_height=100,
                        data_row_color={"hovered": active_user.font_color},
                        # show_checkbox_column=True,
                        columns=[
                            ft.DataColumn(ft.Text("Server Name"), numeric=True),
                            ft.DataColumn(ft.Text("From Email")),
                            ft.DataColumn(ft.Text("Send Mode")),
                            ft.DataColumn(ft.Text("Encryption?")),
                            ft.DataColumn(ft.Text("Username"))
                        ],
                        rows=self.email_table_rows
                    )
                    pw_reset_current = Text('Existing Email Server Values:', color=active_user.font_color, size=16)
                    self.email_edit_column = ft.Column(controls=[pw_reset_current, self.email_table])
                    self.email_edit_container = ft.Container(content=self.email_edit_column)
                    self.email_edit_container.padding = padding.only(left=70, right=50)

                def create_email_table(self):
                    return ft.DataTable(
                        bgcolor=active_user.main_color,
                        border=ft.border.all(2, active_user.main_color),
                        border_radius=10,
                        vertical_lines=ft.border.BorderSide(3, active_user.tertiary_color),
                        horizontal_lines=ft.border.BorderSide(1, active_user.tertiary_color),
                        heading_row_color=active_user.nav_color1,
                        heading_row_height=100,
                        data_row_color={"hovered": active_user.font_color},
                        # show_checkbox_column=True,
                        columns=[
                            ft.DataColumn(ft.Text("Server Name"), numeric=True),
                            ft.DataColumn(ft.Text("From Email")),
                            ft.DataColumn(ft.Text("Send Mode")),
                            ft.DataColumn(ft.Text("Encryption?")),
                            ft.DataColumn(ft.Text("Username"))
                        ],
                        rows=self.email_table_rows
                    )

                def email_table_update(self):
                    self.email_information = api_functions.functions.call_get_email_info(app_api.url, app_api.headers)
                    self.email_table_rows.clear()
                    server_info = self.email_information['Server_Name'] + ':' + str(
                        self.email_information['Server_Port'])
                    from_email = self.email_information['From_Email']
                    send_mode = self.email_information['Send_Mode']
                    encryption = self.email_information['Encryption']
                    auth = self.email_information['Auth_Required']

                    if auth == 1:
                        auth_user = self.email_information['Username']
                    else:
                        auth_user = 'Auth not defined!'

                    # Create a new data row with the user information
                    row = ft.DataRow(
                        cells=[
                            ft.DataCell(ft.Text(server_info)),
                            ft.DataCell(ft.Text(from_email)),
                            ft.DataCell(ft.Text(send_mode)),
                            ft.DataCell(ft.Text(encryption)),
                            ft.DataCell(ft.Text(auth_user))
                        ]
                    )
                    # Append the row to the list of data rows
                    self.email_table_rows.append(row)
                    self.email_table = self.create_email_table()
                    self.page.update()

                def user_table_load(self):
                    edit_user_text = ft.Text('Modify existing Users (Select a user to modify properties):',
                                             color=active_user.font_color, size=16)
                    user_information = api_functions.functions.call_get_user_info(app_api.url, app_api.headers)

                    for entry in user_information:
                        user_id = entry['UserID']
                        fullname = entry['Fullname']
                        username = entry['Username']
                        email = entry['Email']
                        is_admin_numeric = entry['IsAdmin']
                        if is_admin_numeric == 1:
                            is_admin = 'yes'
                        else:
                            is_admin = 'no'

                        # Create a new data row with the user information
                        row = ft.DataRow(
                            cells=[
                                ft.DataCell(ft.Text(user_id)),
                                ft.DataCell(ft.Text(fullname)),
                                ft.DataCell(ft.Text(username)),
                                ft.DataCell(ft.Text(email)),
                                ft.DataCell(ft.Text(str(is_admin))),
                            ],
                            on_select_changed=(
                                lambda username_copy, is_admin_numeric_copy, fullname_copy, email_copy,
                                       user_id_copy:
                                lambda x: (modify_user.open_edit_user(username_copy, is_admin_numeric_copy,
                                                                      fullname_copy, email_copy, user_id_copy),
                                           self.user_table_update())
                            )(username, is_admin_numeric, fullname, email, user_id)
                        )

                        # Append the row to the list of data rows
                        self.user_table_rows.append(row)

                    self.user_table = ft.DataTable(
                        bgcolor=active_user.main_color,
                        border=ft.border.all(2, active_user.main_color),
                        border_radius=10,
                        vertical_lines=ft.border.BorderSide(3, active_user.tertiary_color),
                        horizontal_lines=ft.border.BorderSide(1, active_user.tertiary_color),
                        heading_row_color=active_user.nav_color1,
                        heading_row_height=100,
                        data_row_color={"hovered": active_user.font_color},
                        columns=[
                            ft.DataColumn(ft.Text("User ID"), numeric=True),
                            ft.DataColumn(ft.Text("Fullname")),
                            ft.DataColumn(ft.Text("Username")),
                            ft.DataColumn(ft.Text("Email")),
                            ft.DataColumn(ft.Text("Admin User"))
                        ],
                        rows=self.user_table_rows
                    )
                    self.user_edit_column = ft.Column(controls=[edit_user_text, self.user_table])
                    self.user_edit_container = ft.Container(content=self.user_edit_column)
                    self.user_edit_container.padding = padding.only(left=70, right=50)

                def user_table_update(self):
                    user_information = api_functions.functions.call_get_user_info(app_api.url, app_api.headers)
                    self.user_table_rows.clear()

                    for entry in user_information:
                        user_id = entry['UserID']
                        fullname = entry['Fullname']
                        username = entry['Username']
                        email = entry['Email']
                        is_admin_numeric = entry['IsAdmin']
                        if is_admin_numeric == 1:
                            is_admin = 'yes'
                        else:
                            is_admin = 'no'

                        # Create a new data row with the user information
                        row = ft.DataRow(
                            cells=[
                                ft.DataCell(ft.Text(user_id)),
                                ft.DataCell(ft.Text(fullname)),
                                ft.DataCell(ft.Text(username)),
                                ft.DataCell(ft.Text(email)),
                                ft.DataCell(ft.Text(str(is_admin))),
                            ],
                            on_select_changed=(
                                lambda username_copy, is_admin_numeric_copy, fullname_copy, email_copy, user_id_copy:
                                lambda x: (modify_user.open_edit_user(username_copy, is_admin_numeric_copy,
                                                                      fullname_copy, email_copy, user_id_copy),
                                           self.user_table_update())
                            )(username, is_admin_numeric, fullname, email, user_id)
                        )

                        self.user_table_rows.append(row)
                    self.user_table = self.create_user_table()
                    self.page.update()

                def create_user_table(self):
                    return ft.DataTable(
                        bgcolor=active_user.main_color,
                        border=ft.border.all(2, active_user.main_color),
                        border_radius=10,
                        vertical_lines=ft.border.BorderSide(3, active_user.tertiary_color),
                        horizontal_lines=ft.border.BorderSide(1, active_user.tertiary_color),
                        heading_row_color=active_user.nav_color1,
                        heading_row_height=100,
                        data_row_color={"hovered": active_user.font_color},
                        columns=[
                            ft.DataColumn(ft.Text("User ID"), numeric=True),
                            ft.DataColumn(ft.Text("Fullname")),
                            ft.DataColumn(ft.Text("Username")),
                            ft.DataColumn(ft.Text("Email")),
                            ft.DataColumn(ft.Text("Admin User"))
                        ],
                        rows=self.user_table_rows
                    )

                def import_data(self, e):
                    def close_import_dlg(page):
                        import_dlg.open = False
                        self.page.update()

                    def import_user():
                        import xml.etree.ElementTree as ET

                        def import_pick_result(e: ft.FilePickerResultEvent):
                            if e.files:
                                active_user.import_file = e.files[0].path

                            print('testing')
                            tree = ET.parse(active_user.import_file)
                            root = tree.getroot()

                            podcasts = []
                            for outline in root.findall('.//outline'):
                                podcast_data = {
                                    'title': outline.get('title'),
                                    'xmlUrl': outline.get('xmlUrl')
                                }
                                podcasts.append(podcast_data)

                            pr_instance.touch_stack()
                            close_import_dlg(page)
                            page.update()
                            for podcast in podcasts:

                                if not podcast.get('title') or not podcast.get('xmlUrl'):
                                    close_import_dlg(page)
                                    page.snack_bar = ft.SnackBar(
                                        content=ft.Text(f"This does not appear to be a valid opml file"))
                                    page.snack_bar.open = True
                                    self.page.update()
                                    return False

                                # Get the podcast values
                                podcast_values = internal_functions.functions.get_podcast_values(podcast['xmlUrl'],
                                                                                                 active_user.user_id)

                                # Call add_podcast for each podcast
                                return_value = api_functions.functions.call_add_podcast(app_api.url, app_api.headers, podcast_values,
                                                                         active_user.user_id)
                                if return_value:
                                    page.snack_bar = ft.SnackBar(
                                        content=ft.Text(f"{podcast_values[0]} Imported!")
                                    )
                                else:
                                    page.snack_bar = ft.SnackBar(
                                        content=ft.Text(f"{podcast_values[0]} already added!")
                                    )
                                page.snack_bar.open = True
                                self.page.update()

                            if pr_instance.active_pr == True:
                                pr_instance.rm_stack()
                            page.snack_bar = ft.SnackBar(
                                content=ft.Text(
                                    f"OPML Successfully imported! You should now be subscribed to podcasts defined in the file!"))
                            page.snack_bar.open = True
                            self.page.update()

                            return True

                        file_picker = ft.FilePicker(on_result=import_pick_result)
                        self.page.overlay.append(file_picker)
                        self.page.update()
                        file_picker.pick_files()

                    def import_server():
                        file_picker = ft.FilePicker(on_result=import_pick_result)
                        self.page.overlay.append(file_picker)
                        self.page.update()
                        file_picker.pick_files()

                    user_import_select = ft.TextButton("Import OPML of Podcasts", on_click=lambda x: (import_user()))
                    server_import_select = ft.TextButton("Import Entire Server Information", on_click=lambda x: (import_server()))

                    import_select_row = ft.Row(
                        controls=[
                            ft.TextButton("Close", on_click=lambda x: (close_import_dlg(self.page)))
                        ],
                        alignment=ft.MainAxisAlignment.END
                    )

                    import_dlg = ft.AlertDialog(
                        modal=True,
                        title=ft.Text(f"Backup Data:"),
                        content=ft.Column(controls=[
                            ft.Text(
                                f'Select an option below to import data.',
                                selectable=True),
                            user_import_select,
                            server_import_select,
                            import_select_row
                        ],
                            tight=True),
                        actions_alignment=ft.MainAxisAlignment.END,
                    )
                    self.page.dialog = import_dlg
                    import_dlg.open = True
                    self.page.update()


                def backup_data(self, e):
                    def close_backup_dlg(page):
                        backup_dlg.open = False
                        self.page.update()

                    def backup_user():
                        backup_status = api_functions.functions.call_backup_user(app_api.url, app_api.headers,
                                                                                 active_user.user_id, backup_dir)
                        close_backup_dlg(self.page)
                        self.page.update()

                        def open_backups():
                            import subprocess
                            import platform

                            def open_folder(path):
                                if platform.system() == "Windows":
                                    os.startfile(path)
                                elif platform.system() == "Darwin":
                                    subprocess.Popen(["open", path])
                                else:
                                    subprocess.Popen(["xdg-open", path])
                            print(backup_dir)
                            open_folder(backup_dir)

                        def close_backup_status_win(page):
                            backup_stat_dlg.open = False
                            self.page.update()

                        if backup_status == True:
                            backup_status_text = ft.Text(f"Backup Successful! File Saved to: {backup_dir}", selectable=True)
                            folder_location = ft.TextButton("Open Backup Location",
                                                                 on_click=lambda x: (open_backups()))
                        else:
                            backup_status_text = ft.Text("Backup was not successful. Try again!")
                            folder_location = ft.Text("N/A")

                        backup_select_status_row = ft.Row(
                            controls=[
                                ft.TextButton("Close", on_click=lambda x: (close_backup_status_win(self.page)))
                            ],
                            alignment=ft.MainAxisAlignment.END
                        )

                        backup_stat_dlg = ft.AlertDialog(
                            modal=True,
                            title=ft.Text(f"Backup Data:"),
                            content=ft.Column(controls=[
                                backup_status_text,
                                folder_location,
                                backup_select_status_row
                            ],
                                tight=True),
                            actions_alignment=ft.MainAxisAlignment.END,
                        )
                        self.page.dialog = backup_stat_dlg
                        backup_stat_dlg.open = True
                        self.page.update()

                    def backup_server():
                        backup_status = api_functions.functions.call_backup_server(app_api.url, app_api.headers, backup_dir)


                    user_backup_select = ft.TextButton("Export OPML of Podcasts", on_click=lambda x: (backup_user()))
                    server_backup_select = ft.TextButton("Backup Entire Server", on_click=lambda x: (backup_server()))

                    backup_select_row = ft.Row(
                        controls=[
                            ft.TextButton("Close", on_click=lambda x: (close_backup_dlg(self.page)))
                        ],
                        alignment=ft.MainAxisAlignment.END
                    )

                    backup_dlg = ft.AlertDialog(
                        modal=True,
                        title=ft.Text(f"Backup Data:"),
                        content=ft.Column(controls=[
                            ft.Text(
                                f'Select an option below to backup information.',
                                selectable=True),
                            user_backup_select,
                            server_backup_select,
                            backup_select_row
                        ],
                            tight=True),
                        actions_alignment=ft.MainAxisAlignment.END,
                    )
                    self.page.dialog = backup_dlg
                    backup_dlg.open = True
                    self.page.update()

                def guest_user_change(self, e):
                    api_functions.functions.call_enable_disable_guest(app_api.url, app_api.headers)
                    self.page.snack_bar = ft.SnackBar(content=ft.Text(f"Guest user modified!"))
                    self.page.snack_bar.open = True
                    self.guest_status_bool = api_functions.functions.call_guest_status(app_api.url, app_api.headers)
                    if self.guest_status_bool:
                        self.guest_info_button.text = 'Disable Guest User'
                        self.guest_info_button.on_click = self.guest_user_change
                        self.guest_status = 'enabled'
                    else:
                        self.guest_info_button.text = 'Enable Guest User'
                        self.guest_info_button.on_click = self.guest_user_change
                        self.guest_status = 'disabled'

                    self.disable_guest_notify.visible = False
                    self.page.update()

                def self_service_change(self, e):
                    api_functions.functions.call_enable_disable_self_service(app_api.url, app_api.headers)
                    self.page.snack_bar = ft.SnackBar(content=ft.Text(f"Self Service Settings Adjusted!"))
                    self.page.snack_bar.open = True
                    self.self_service_bool = api_functions.functions.call_self_service_status(app_api.url,
                                                                                              app_api.headers)
                    if self.self_service_bool:
                        self.self_service_button.text = 'Disable Self Service User Creation'
                        self.self_service_button.on_click = self.self_service_change
                        self.self_service_status = 'enabled'
                    else:
                        self.self_service_button.text = 'Enable Self Service User Creation'
                        self.self_service_button.on_click = self.self_service_change
                        self.self_service_status = 'disabled'

                    self.self_service_notify.visible = False
                    self.page.update()

                def download_option_change(self, e):
                    api_functions.functions.call_enable_disable_downloads(app_api.url, app_api.headers)
                    self.page.snack_bar = ft.SnackBar(content=ft.Text(f"Download Option Modified!"))
                    self.page.snack_bar.open = True
                    self.download_status_bool = api_functions.functions.call_download_status(app_api.url,
                                                                                             app_api.headers)
                    if self.download_status_bool:
                        self.download_info_button.text = 'Disable Podcast Server Downloads'
                        self.download_info_button.on_click = self.download_option_change
                    else:
                        self.download_info_button.text = 'Enable Podcast Server Downloads'
                        self.download_info_button.on_click = self.download_option_change

                    self.disable_download_notify.visible = False
                    self.page.update()

                def mfa_option_change(self, e):
                    mfa_setup_check = self.setup_mfa()
                    if mfa_setup_check == True:
                        self.mfa_check()
                        self.page.update()
                    else:
                        self.page.update()

            settings_data = Settings(page)

            # User Settings
            user_setting = ft.Text(
                "Personal Settings:", color=active_user.font_color,
                size=30,
                font_family="RobotoSlab",
                weight=ft.FontWeight.W_300,
            )
            user_setting_text = ft.Container(content=user_setting)
            user_setting_text.padding = padding.only(left=70, right=50)

            # Theme Select Elements
            theme_text = ft.Text('Select Theme:', color=active_user.font_color, size=16)
            theme_drop = ft.Dropdown(border_color=active_user.accent_color, color=active_user.font_color,
                                     focused_bgcolor=active_user.main_color,
                                     focused_border_color=active_user.accent_color,
                                     focused_color=active_user.accent_color,
                                     options=[
                                         ft.dropdown.Option("light"),
                                         ft.dropdown.Option("dark"),
                                         ft.dropdown.Option("nordic"),
                                         ft.dropdown.Option("abyss"),
                                         ft.dropdown.Option("dracula"),
                                         ft.dropdown.Option("kimbie"),
                                         ft.dropdown.Option("neon"),
                                         ft.dropdown.Option("greenie meanie"),
                                         ft.dropdown.Option("wildberries"),
                                         ft.dropdown.Option("hotdogstand - MY EYES"),
                                     ]
                                     )
            theme_submit = ft.ElevatedButton("Submit", bgcolor=active_user.main_color, color=active_user.accent_color,
                                             on_click=lambda event: active_user.set_theme(theme_drop.value))
            theme_column = ft.Column(controls=[theme_text, theme_drop, theme_submit])
            theme_row = ft.Row(
                vertical_alignment=ft.CrossAxisAlignment.START,
                alignment=ft.MainAxisAlignment.START,
                controls=[theme_column])
            theme_row_container = ft.Container(content=theme_row)
            theme_row_container.padding = padding.only(left=70, right=50)

            # Admin Only Settings

            admin_setting = ft.Text(
                "Administration Settings:", color=active_user.font_color,
                size=30,
                font_family="RobotoSlab",
                weight=ft.FontWeight.W_300,
            )
            admin_setting_text = ft.Container(content=admin_setting)
            admin_setting_text.padding = padding.only(left=70, right=50)

            # New User Creation Elements
            new_user = User(page)
            user_text = Text('Create New User:', color=active_user.font_color, size=16)
            user_name = ft.TextField(label="Full Name", icon=ft.icons.CARD_MEMBERSHIP, hint_text='John PinePods',
                                     border_color=active_user.accent_color, color=active_user.accent_color,
                                     focused_bgcolor=active_user.accent_color, focused_color=active_user.accent_color,
                                     focused_border_color=active_user.accent_color,
                                     cursor_color=active_user.accent_color)
            user_email = ft.TextField(label="Email", icon=ft.icons.EMAIL, hint_text='ilovepinepods@pinepods.com',
                                      border_color=active_user.accent_color, color=active_user.accent_color,
                                      focused_bgcolor=active_user.accent_color, focused_color=active_user.accent_color,
                                      focused_border_color=active_user.accent_color,
                                      cursor_color=active_user.accent_color)
            user_username = ft.TextField(label="Username", icon=ft.icons.PERSON, hint_text='pinepods_user1999',
                                         border_color=active_user.accent_color, color=active_user.accent_color,
                                         focused_bgcolor=active_user.accent_color,
                                         focused_color=active_user.accent_color,
                                         focused_border_color=active_user.accent_color,
                                         cursor_color=active_user.accent_color)
            user_password = ft.TextField(label="password", icon=ft.icons.PASSWORD, password=True,
                                         can_reveal_password=True, hint_text='mY_SuPeR_S3CrEt!',
                                         border_color=active_user.accent_color, color=active_user.accent_color,
                                         focused_bgcolor=active_user.accent_color,
                                         focused_color=active_user.accent_color,
                                         focused_border_color=active_user.accent_color,
                                         cursor_color=active_user.accent_color)
            user_submit = ft.ElevatedButton(text="Submit!", bgcolor=active_user.main_color,
                                            color=active_user.accent_color, on_click=lambda x: (
                    new_user.set_username(user_username.value),
                    new_user.set_password(user_password.value),
                    new_user.set_email(user_email.value),
                    new_user.set_name(user_name.value),
                    new_user.verify_user_values(),
                    # new_user.popup_user_values(e),
                    new_user.create_user(),
                    new_user.user_created_prompt(),
                    settings_data.user_table_update()))
            user_column = ft.Column(
                controls=[user_text, user_name, user_email, user_username, user_password, user_submit]
            )
            user_row = ft.Row(
                vertical_alignment=ft.CrossAxisAlignment.START,
                alignment=ft.MainAxisAlignment.START,
                controls=[user_column])
            user_row_container = ft.Container(content=user_row)
            user_row_container.padding = padding.only(left=70, right=50)
            # Download Disable Settings
            settings_data.disable_download_text = ft.Text(
                'Download Podcast Options (You may consider disabling the ability to download podcasts to the server if your server is open to the public):',
                color=active_user.font_color, size=16)
            download_info_col = ft.Column(
                controls=[settings_data.disable_download_text, settings_data.disable_download_notify,
                          settings_data.download_info_button])
            download_info = ft.Container(content=download_info_col)
            download_info.padding = padding.only(left=70, right=50)

            # Guest User Settings
            settings_data.disable_guest_text = ft.Text(
                'Guest User Settings (Disabling is highly recommended if PinePods is exposed to the internet):',
                color=active_user.font_color, size=16)
            guest_info_col = ft.Column(controls=[settings_data.disable_guest_text, settings_data.disable_guest_notify,
                                                 settings_data.guest_info_button])
            guest_info = ft.Container(content=guest_info_col)
            guest_info.padding = padding.only(left=70, right=50)

            # User Self Service Creation
            settings_data.self_service_text = ft.Text(
                'Self Service Settings (Disabling is highly recommended if PinePods is exposed to the internet):',
                color=active_user.font_color, size=16)
            self_service_info_col = ft.Column(
                controls=[settings_data.self_service_text, settings_data.self_service_notify,
                          settings_data.self_service_button])
            self_service_info = ft.Container(content=self_service_info_col)
            self_service_info.padding = padding.only(left=70, right=50)

            # User Self Service PW Resets

            def auth_box_check(e):
                if new_user.auth_enabled == True:
                    pw_reset_auth_user.disabled = True
                    pw_reset_auth_pw.disabled = True
                    new_user.auth_enabled = 0
                else:
                    pw_reset_auth_user.disabled = False
                    pw_reset_auth_pw.disabled = False
                    new_user.auth_enabled = 1
                page.update()

            pw_reset_text = Text('Set Email Settings for Self Service Password Resets', color=active_user.font_color,
                                 size=16)
            pw_reset_change = Text('Change Existing values:', color=active_user.font_color, size=16)

            pw_reset_server_name = ft.TextField(label="Server Address", icon=ft.icons.COMPUTER,
                                                hint_text='smtp.pinepods.online', border_color=active_user.accent_color,
                                                color=active_user.accent_color,
                                                focused_bgcolor=active_user.accent_color,
                                                focused_color=active_user.accent_color,
                                                focused_border_color=active_user.accent_color,
                                                cursor_color=active_user.accent_color)
            pw_reset_port = ft.TextField(label="Port", hint_text='587', border_color=active_user.accent_color,
                                         color=active_user.accent_color, focused_bgcolor=active_user.accent_color,
                                         focused_color=active_user.accent_color,
                                         focused_border_color=active_user.accent_color,
                                         cursor_color=active_user.accent_color)
            pw_reset_email = ft.TextField(label="From Address", icon=ft.icons.EMAIL,
                                          hint_text='pwresets@pinepods.online', border_color=active_user.accent_color,
                                          color=active_user.accent_color, focused_bgcolor=active_user.accent_color,
                                          focused_color=active_user.accent_color,
                                          focused_border_color=active_user.accent_color,
                                          cursor_color=active_user.accent_color)
            pw_reset_send_mode = ft.Dropdown(width=250, label="Send Mode",
                                             options=[
                                                 ft.dropdown.Option("SMTP"),
                                                 # ft.dropdown.Option("Sendmail"),
                                             ], icon=ft.icons.SEND, border_color=active_user.accent_color,
                                             color=active_user.accent_color, focused_bgcolor=active_user.accent_color,
                                             focused_color=active_user.accent_color,
                                             focused_border_color=active_user.accent_color)
            pw_reset_encryption = ft.Dropdown(width=250, label="Encryption",
                                              options=[
                                                  ft.dropdown.Option("None"),
                                                  ft.dropdown.Option("STARTTLS"),
                                                  ft.dropdown.Option("SSL/TLS"),
                                              ], icon=ft.icons.ENHANCED_ENCRYPTION,
                                              border_color=active_user.accent_color, color=active_user.accent_color,
                                              focused_bgcolor=active_user.accent_color,
                                              focused_color=active_user.accent_color,
                                              focused_border_color=active_user.accent_color)
            pw_reset_auth = ft.Checkbox(label="Authentication Required", value=False, on_change=auth_box_check,
                                        check_color=active_user.accent_color)
            pw_reset_auth_user = ft.TextField(label="Username", icon=ft.icons.PERSON, hint_text='user@pinepods.online',
                                              border_color=active_user.accent_color, color=active_user.accent_color,
                                              focused_bgcolor=active_user.accent_color,
                                              focused_color=active_user.accent_color,
                                              focused_border_color=active_user.accent_color,
                                              cursor_color=active_user.accent_color)
            pw_reset_auth_pw = ft.TextField(label="Password", icon=ft.icons.LOCK, hint_text='Ema1L!P@$$', password=True,
                                            can_reveal_password=True, border_color=active_user.accent_color,
                                            color=active_user.accent_color, focused_bgcolor=active_user.accent_color,
                                            focused_color=active_user.accent_color,
                                            focused_border_color=active_user.accent_color,
                                            cursor_color=active_user.accent_color)
            pw_reset_auth_user.disabled = True
            pw_reset_auth_pw.disabled = True
            pw_reset_test = ft.ElevatedButton(text="Test Send and Submit", bgcolor=active_user.main_color,
                                              color=active_user.accent_color, on_click=lambda x: (
                    new_user.test_email_settings(pw_reset_server_name.value, pw_reset_port.value, pw_reset_email.value,
                                                 pw_reset_send_mode.value, pw_reset_encryption.value,
                                                 pw_reset_auth.value, pw_reset_auth_user.value, pw_reset_auth_pw.value),
                    settings_data.email_table_update()
                ))
            pw_reset_server_row = ft.Row(
                vertical_alignment=ft.CrossAxisAlignment.START,
                alignment=ft.MainAxisAlignment.START,
                controls=[pw_reset_server_name, ft.Text(':', size=24), pw_reset_port])
            pw_reset_send_row = ft.Row(
                vertical_alignment=ft.CrossAxisAlignment.START,
                alignment=ft.MainAxisAlignment.START,
                controls=[pw_reset_send_mode, pw_reset_encryption])
            pw_reset_auth_row = ft.Row(
                vertical_alignment=ft.CrossAxisAlignment.START,
                alignment=ft.MainAxisAlignment.START,
                controls=[pw_reset_auth_user, pw_reset_auth_pw])

            pw_reset_buttons = ft.Row(
                vertical_alignment=ft.CrossAxisAlignment.START,
                alignment=ft.MainAxisAlignment.START,
                controls=[pw_reset_test])

            pw_reset_column = ft.Column(
                controls=[pw_reset_text, pw_reset_change, pw_reset_server_row, pw_reset_send_row, pw_reset_email,
                          pw_reset_auth, pw_reset_auth_row, pw_reset_buttons]
            )
            pw_reset_row = ft.Row(
                vertical_alignment=ft.CrossAxisAlignment.START,
                alignment=ft.MainAxisAlignment.START,
                controls=[pw_reset_column])
            pw_reset_container = ft.Container(content=pw_reset_row)
            pw_reset_container.padding = padding.only(left=70, right=50)

            ### API Key Settings

            edit_api_text = ft.Text('Create or remove API keys for clients:', color=active_user.font_color, size=16)

            def create_api(e):
                def close_api_dlg(e):
                    create_api_dlg.open = False
                    page.update()

                new_key = api_functions.functions.call_create_api_key(app_api.url, app_api.headers, active_user.user_id)

                create_api_dlg = ft.AlertDialog(
                modal=True,
                title=ft.Text(f"New API key listed below"),
                content=ft.Column(controls=[
                ft.Text("Be sure to copy your key. There's no way to ever see it again (You can always create a new one if you forget)"),
                ft.Text(f'Api key: {new_key}', selectable=True),
                    ], tight=True),
                actions=[
                ft.TextButton("Close", on_click=close_api_dlg)
                ],
                actions_alignment=ft.MainAxisAlignment.END
                )
                page.dialog = create_api_dlg
                create_api_dlg.open = True
                page.update()

            def open_edit_api(e):
                def close_api_dlg(e):
                    modify_api_dlg.open = False
                    page.update()

                def delete_api(e):
                    api_functions.functions.call_delete_api_key(app_api.url, app_api.headers, active_user.api_id)
                    modify_api_dlg.open = False
                    page.update()

                modify_api_dlg = ft.AlertDialog(
                modal=True,
                title=ft.Text(f"Would you like to delete api {active_user.api_id}?"),
                actions=[
                ft.TextButton(content=ft.Text("Delete API", color=ft.colors.RED_400), on_click=delete_api),
                ft.TextButton("Cancel", on_click=close_api_dlg)
                ],
                actions_alignment=ft.MainAxisAlignment.END
                )

                page.dialog = modify_api_dlg
                modify_api_dlg.open = True
                page.update()

            create_api_button = ft.ElevatedButton(f'Generate New API Key for Current User', on_click=create_api, bgcolor=active_user.main_color, color=active_user.accent_color)

            api_information = api_functions.functions.call_get_api_info(app_api.url, app_api.headers)

            # Skip the first entry in api_information
            api_information = api_information[1:]

            api_table_rows = []
            def create_on_select_changed_lambda(api_id, pages):
                return lambda e: (setattr(active_user, 'api_id', api_id), open_edit_api(e))


            for entry in api_information:
                api_id = entry['APIKeyID']
                api_key = '...' + entry['LastFourDigits']
                username = entry['Username']
                api_created = entry['Created']
                
                # Create a new data row with the user information
                row = ft.DataRow(
                    cells=[
                        ft.DataCell(ft.Text(api_id)),
                        ft.DataCell(ft.Text(api_key)),
                        ft.DataCell(ft.Text(username)),
                        ft.DataCell(ft.Text(api_created))
                    ],
                    on_select_changed=create_on_select_changed_lambda(api_id, page)
                )
                
                # Append the row to the list of data rows
                api_table_rows.append(row)

            api_table = ft.DataTable(
                bgcolor=active_user.main_color, 
                border=ft.border.all(2, active_user.main_color),
                border_radius=10,
                vertical_lines=ft.border.BorderSide(3, active_user.tertiary_color),
                horizontal_lines=ft.border.BorderSide(1, active_user.tertiary_color),
                heading_row_color=active_user.nav_color1,
                heading_row_height=100,
                data_row_color={"hovered": active_user.font_color},
                # show_checkbox_column=True,
                columns=[
                ft.DataColumn(ft.Text("API ID"), numeric=True),
                ft.DataColumn(ft.Text("API Last Four Digits")),
                ft.DataColumn(ft.Text("User Who Created")),
                ft.DataColumn(ft.Text("Created At")),
            ],
                rows=api_table_rows
                )
            api_edit_column = ft.Column(controls=[edit_api_text, create_api_button, api_table])
            api_edit_container = ft.Container(content=api_edit_column)
            api_edit_container.padding=padding.only(left=70, right=50)

            # Check if admin settings should be displayed 
            div_row = ft.Divider(color=active_user.accent_color)
            user_div_row = ft.Divider(color=active_user.accent_color)
            user_is_admin = api_functions.functions.call_user_admin_check(app_api.url, app_api.headers,
                                                                          int(active_user.user_id))
            if user_is_admin == True:
                pass
            else:
                admin_setting_text.visible = False
                user_row_container.visible = False
                settings_data.user_edit_container.visible = False
                pw_reset_container.visible = False
                settings_data.email_edit_container.visible = False
                guest_info.visible = False
                download_info.visible = False
                self_service_info.visible = False
                api_edit_container.visible = False
                div_row.visible = False

            if active_user.user_id == 0:
                settings_data.mfa_container.visible = False

            # Create search view object
            settings_view = ft.View("/settings",
                    [
                        user_setting_text,
                        theme_row_container,
                        user_div_row,
                        settings_data.mfa_container,
                        user_div_row,
                        settings_data.setting_backup_con,
                        user_div_row,
                        settings_data.setting_import_con,
                        user_div_row,
                        admin_setting_text,
                        user_row_container,
                        settings_data.user_edit_container,
                        div_row,
                        pw_reset_container,
                        settings_data.email_edit_container,
                        div_row,
                        guest_info,
                        div_row,
                        self_service_info,
                        div_row,
                        download_info,
                        div_row,
                        api_edit_container
                    ]
                    
                )
            settings_view.bgcolor = active_user.bgcolor
            settings_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                settings_view

            )

        if page.route == "/episode_display" or page.route == "/episode_display":
            # Creating attributes for page layout
            episode_info = api_functions.functions.call_return_selected_episode(app_api.url, app_api.headers,
                                                                                active_user.user_id,
                                                                                current_episode.title,
                                                                                current_episode.url)

            for entry in episode_info:
                ep_title = entry['EpisodeTitle']
                ep_pod_name = entry['PodcastName']
                ep_pod_site = entry['WebsiteURL']
                ep_pub_date = entry['EpisodePubDate']
                ep_desc = entry['EpisodeDescription']
                ep_artwork = entry['EpisodeArtwork']
                ep_url = entry['EpisodeURL']
                ep_duration = entry['EpisodeDuration']

            ep_podcast_name = ft.Text("ep_pod_name")
            display_pod_art_no = random.randint(1, 12)
            display_pod_art_fallback = os.path.join(script_dir, "images", "logo_random", f"{display_pod_art_no}.jpeg")
            display_pod_art_url = ep_artwork if ep_artwork else display_pod_art_fallback
            display_pod_art_parsed = check_image(display_pod_art_url)
            pod_image = ft.Image(src=display_pod_art_parsed, width=300, height=300)
            pod_feed_title = ft.Text(ep_title, color=active_user.font_color, style=ft.TextThemeStyle.HEADLINE_MEDIUM)
            pod_feed_date = ft.Text(ep_pub_date, color=active_user.font_color)
            pod_duration = seconds_to_time(ep_duration)
            pod_dur_display = ft.Text(f'Episode Duration: {pod_duration}', color=active_user.font_color)
            podcast_feed_name = ft.Text(ep_pod_name, color=active_user.font_color,
                                        style=ft.TextThemeStyle.DISPLAY_MEDIUM)
            pod_feed_site = ft.ElevatedButton(text=ep_pod_site, on_click=launch_pod_site)

            ep_play_button = ft.IconButton(
                icon=ft.icons.PLAY_CIRCLE,
                icon_color=active_user.accent_color,
                icon_size=40,
                tooltip="Play Episode",
                on_click=lambda x, url=ep_url, title=ep_title, artwork=ep_artwork: play_selected_episode(url, title,
                                                                                                         artwork)
            )
            ep_popup_button = ft.PopupMenuButton(
                content=ft.Icon(ft.icons.ARROW_DROP_DOWN_CIRCLE_ROUNDED, color=active_user.accent_color, size=40,
                                tooltip="Play Episode"),
                items=[
                    ft.PopupMenuItem(icon=ft.icons.QUEUE, text="Queue", on_click=lambda x, url=ep_url, title=ep_title,
                                                                                        artwork=ep_artwork: queue_selected_episode(
                        url, title, artwork, page)),
                    ft.PopupMenuItem(icon=ft.icons.DOWNLOAD, text="Server Download",
                                     on_click=lambda x, url=ep_url, title=ep_title: download_selected_episode(url,
                                                                                                              title,
                                                                                                              page)),
                    ft.PopupMenuItem(icon=ft.icons.SAVE, text="Save Episode",
                                     on_click=lambda x, url=ep_url, title=ep_title: save_selected_episode(url, title,
                                                                                                          page))
                ]
                )
            ep_play_options = ft.Row(controls=[ep_play_button, ep_popup_button])

            feed_row_content = ft.ResponsiveRow([
                ft.Column(col={"md": 4}, controls=[pod_image]),
                ft.Column(col={"md": 8}, controls=[pod_feed_title, pod_feed_date, pod_dur_display, ep_play_options]),
            ])
            podcast_row = ft.Container(content=podcast_feed_name)
            podcast_row.padding = padding.only(left=70, right=50)
            feed_row = ft.Container(content=feed_row_content)
            feed_row.padding = padding.only(left=70, right=50)
            # Check for html in description
            if is_html(ep_desc):
                # convert HTML to Markdown
                markdown_desc = html2text.html2text(ep_desc)

                # add inline style to change font color

                pod_feed_desc = ft.Markdown(markdown_desc, on_tap_link=launch_clicked_url)
                desc_row = ft.Container(content=pod_feed_desc)
                desc_row.padding = padding.only(left=70, right=50)
            else:
                # display plain text
                markdown_desc = ep_desc
                pod_feed_desc = ft.Text(ep_desc, color=active_user.font_color)
                desc_row = ft.Container(content=pod_feed_desc)
                desc_row.padding = padding.only(left=70, right=50)
            ep_display_defaults = Pod_View(page)
            # Create search view object
            pod_view = ft.View(
                "/poddisplay",
                [
                    ep_display_defaults.top_bar,
                    podcast_row,
                    feed_row,
                    desc_row

                ]

            )
            pod_view.bgcolor = active_user.bgcolor
            pod_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                pod_view
            )

        if page.route == "/playing" or page.route == "/playing":
            pod_controls.audio_container.visible = False
            pod_controls.audio_container.update()
            page.update()
            fs_container_image = current_episode.audio_con_art_url_parsed
            fs_container_image_landing = ft.Image(src=fs_container_image, width=300, height=300)
            fs_container_image_landing.border_radius = ft.border_radius.all(45)
            fs_container_image_row = ft.Row(controls=[fs_container_image_landing],
                                            alignment=ft.MainAxisAlignment.CENTER)
            fs_currently_playing = ft.Container(content=ft.Text(current_episode.name_truncated, size=16),
                                                on_click=open_currently_playing, alignment=ft.alignment.center)

            # Create the audio controls
            if current_episode.audio_playing == True:
                current_episode.fs_play_button.visible = False
            else:
                current_episode.fs_pause_button.visible = False
            fs_seek_button = ft.IconButton(
                icon=ft.icons.FAST_FORWARD,
                tooltip="Seek 10 seconds",
                icon_color="white",
                on_click=lambda e: current_episode.seek_episode()
            )
            fs_seek_back_button = ft.IconButton(
                icon=ft.icons.FAST_REWIND,
                tooltip="Seek 10 seconds",
                icon_color="white",
                on_click=lambda e: current_episode.seek_back_episode()
            )
            fs_ep_audio_controls = ft.Row(
                controls=[fs_seek_back_button, current_episode.fs_play_button, current_episode.fs_pause_button,
                          fs_seek_button], alignment=ft.MainAxisAlignment.CENTER)
            fs_scrub_bar_row = ft.Row(controls=[pod_controls.current_time, pod_controls.audio_scrubber_column, pod_controls.podcast_length],
                                      alignment=ft.MainAxisAlignment.CENTER)
            fs_volume_adjust_column = ft.Row(controls=[pod_controls.volume_down_icon, pod_controls.volume_slider, pod_controls.volume_up_icon],
                                             alignment=ft.MainAxisAlignment.CENTER)
            fs_volume_container = ft.Container(
                height=35,
                width=275,
                bgcolor=ft.colors.WHITE,
                border_radius=45,
                padding=6,
                content=fs_volume_adjust_column,
                alignment=ft.alignment.center)
            fs_volume_container.adding = ft.padding.all(50)
            fs_volume_adjust_row = ft.Row(controls=[fs_volume_container], alignment=ft.MainAxisAlignment.CENTER)

            def toggle_second_status(status):
                if current_episode.state == 'playing':
                    pod_controls.audio_scrubber.update()
                    pod_controls.current_time.update()

            total_seconds = current_episode.media_length // 1000

            def update_function():
                for i in range(total_seconds):
                    toggle_second_status(current_episode.audio_element.data)
                    time.sleep(1)

            update_thread = threading.Thread(target=update_function)
            update_thread.start()

            fs_volume_container.bgcolor = active_user.main_color
            current_episode.fs_play_button.icon_color = active_user.accent_color
            current_episode.fs_pause_button.icon_color = active_user.accent_color
            fs_seek_button.icon_color = active_user.accent_color

            show_notes_button = ft.OutlinedButton("Show Notes", on_click=lambda x, url=current_episode.url,
                                                                                title=current_episode.name: open_episode_select(
                page, url, title))
            fs_show_notes_row = ft.Row(controls=[show_notes_button], alignment=ft.MainAxisAlignment.CENTER)

            current_column = ft.Column(controls=[
                fs_container_image_row, fs_currently_playing, fs_show_notes_row, fs_scrub_bar_row, fs_ep_audio_controls,
                fs_volume_adjust_row
            ])

            current_container = ft.Container(content=current_column, alignment=ft.alignment.center)
            current_container.padding = padding.only(left=70, right=50)
            current_container.alignment = alignment.center
            current_play_defaults = Pod_View(page)

            # Create search view object
            ep_playing_view = ft.View("/playing",
                                      [
                                          current_play_defaults.top_bar,
                                          current_container

                                      ]

                                      )
            ep_playing_view.bgcolor = active_user.bgcolor
            ep_playing_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                ep_playing_view

            )

    page.on_route_change = route_change
    page.on_view_pop = view_pop

    # -Create Help Banner-----------------------------------------------------------------------
    def close_banner(e):
        page.banner.open = False
        page.update()

    def open_repo(e):
        page.launch_url('https://github.com/madeofpendletonwool/PinePods')

    def open_doc_site(e):
        page.launch_url('https://pinepods.online')

    page.banner = ft.Banner(
        bgcolor=ft.colors.BLUE,
        leading=ft.Icon(ft.icons.WAVING_HAND, color=ft.colors.DEEP_ORANGE_500, size=40),
        content=ft.Text("""
    Welcome to PinePods! PinePods is an app built to save, listen, download, organize, and manage a selection of podcasts. Using the search function you can search for your favorite podcast, from there, click the add button to save your podcast to the database. PinePods will begin displaying new episodes of that podcast from then on to the homescreen when released. In addition, from search you can click on a podcast to view and listen to specific episodes. From the sidebar you can select your saved podcasts and manage them, view and manage your downloaded podcasts, edit app settings, check your listening history, and listen through episodes from your saved 'queue.' For comments, feature requests, pull requests, and bug reports please open an issue, for fork PinePods from the repository:
    """, color=colors.BLACK
                        ),
        actions=[
            ft.TextButton('Open PinePods Repo', on_click=open_repo),
            ft.IconButton(icon=ft.icons.EXIT_TO_APP, on_click=close_banner)
        ],
    )

    def show_banner_click(e):
        page.banner.open = True
        page.update()

    # Login/User Changes------------------------------------------------------
    class User:
        email_regex = r"^[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+$"

        def __init__(self, page):
            self.username = None
            self.password = None
            self.email = None
            self.main_color = 'colors.BLUE_GREY'
            self.bgcolor = 'colors.BLUE_GREY'
            self.accent_color = 'colors.BLUE_GREY'
            self.tertiary_color = 'colors.BLUE_GREY'
            self.font_color = 'colors.BLUE_GREY'
            self.user_id = None
            self.page = page
            self.fullname = 'Login First'
            self.isadmin = None
            self.navbar_stack = None
            self.new_user_valid = False
            self.invalid_value = False
            self.api_id = 0
            self.mfa_secret = None
            self.downloading = []
            self.downloading_name = []
            self.auth_enabled = 0
            self.timezone = 'UTC'
            self.hour_pref = 24
            self.first_login_finished = 0
            self.first_start = 0
            self.search_term = ""
            self.feed_url = None
            self.import_file = None
            # global current_pod_view
            self.current_pod_view = None  # This global variable will hold the current active Pod_View instance

        # New User Stuff ----------------------------

        def set_username(self, new_username):
            if new_username is None or not new_username.strip():
                self.username = None
            else:
                self.username = new_username

        def set_password(self, new_password):
            if new_password is None or not new_password.strip():
                self.password = None
            else:
                self.password = new_password

        def set_email(self, new_email):
            if new_email is None or not new_email.strip():
                self.email = None
            else:
                self.email = new_email

        def set_name(self, new_name):
            if new_name is None or not new_name.strip():
                self.fullname = None
            else:
                self.fullname = new_name

        def set_admin(self, new_admin):
            self.isadmin = new_admin

        def verify_user_values(self):
            self.valid_username = self.username is not None and len(self.username) >= 6
            self.valid_password = self.password is not None and len(self.password) >= 8 and any(
                c.isupper() for c in self.password) and any(c.isdigit() for c in self.password)
            regex = r"^[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+$"
            self.valid_email = self.email is not None and re.match(self.email_regex, self.email) is not None
            invalid_value = False
            if not self.valid_username:
                self.page.dialog = username_invalid_dlg
                username_invalid_dlg.open = True
                self.page.update()
                invalid_value = True
            elif not self.valid_password:
                self.page.dialog = password_invalid_dlg
                password_invalid_dlg.open = True
                self.page.update()
                invalid_value = True
            elif not self.valid_email:
                self.page.dialog = email_invalid_dlg
                email_invalid_dlg.open = True
                self.page.update()
                invalid_value = True
            elif api_functions.functions.call_check_usernames(app_api.url, app_api.headers, self.username):
                self.page.dialog = username_exists_dlg
                username_exists_dlg.open = True
                self.page.update()
                invalid_value = True
            self.new_user_valid = not invalid_value

        def verify_user_values_snack(self):
            self.valid_username = self.username is not None and len(self.username) >= 6
            self.valid_password = self.password is not None and len(self.password) >= 8 and any(
                c.isupper() for c in self.password) and any(c.isdigit() for c in self.password)
            regex = r"^[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+$"
            self.valid_email = self.email is not None and re.match(self.email_regex, self.email) is not None
            invalid_value = False
            if not self.valid_username:
                page.snack_bar = ft.SnackBar(
                    content=ft.Text(f"Usernames must be unique and require at least 6 characters"))
                page.snack_bar.open = True
                self.page.update()
                self.invalid_value = True
            elif not self.valid_password:
                page.snack_bar = ft.SnackBar(content=ft.Text(
                    f"Passwords require at least 8 characters, a number, a capital letter and a special character!"))
                page.snack_bar.open = True
                self.page.update()
                self.invalid_value = True
            elif not self.valid_email:
                page.snack_bar = ft.SnackBar(content=ft.Text(f"Email appears to be non-standard email layout!"))
                page.snack_bar.open = True
                self.page.update()
                self.invalid_value = True
            elif api_functions.functions.call_check_usernames(app_api.url, app_api.headers, self.username):
                page.snack_bar = ft.SnackBar(content=ft.Text(f"This username appears to be already taken"))
                page.snack_bar.open = True
                self.page.update()
                self.invalid_value = True
            if self.invalid_value:
                self.new_user_valid = False
            else:
                self.new_user_valid = not invalid_value

        def user_created_prompt(self):
            if self.new_user_valid:
                self.page.dialog = user_dlg
                user_dlg.open = True
                self.page.update()

        def user_created_snack(self):
            if self.new_user_valid:
                page.snack_bar = ft.SnackBar(content=ft.Text(
                    f"New user created successfully. You may now login and begin using Pinepods. Enjoy!"))
                page.snack_bar.open = True
                page.update()

        def popup_user_values(self, e):
            pass

        def create_user(self):
            if self.new_user_valid:
                salt, hash_pw = Auth.Passfunctions.hash_password(self.password)
                hash_pw_str = base64.b64encode(hash_pw).decode()
                salt_str = base64.b64encode(salt).decode()
                api_functions.functions.call_add_user(app_api.url, app_api.headers, self.fullname, self.username,
                                                      self.email, hash_pw_str, salt_str)

        def test_email_settings(self, server_name, server_port, from_email, send_mode, encryption, auth_required,
                                username=None, password=None):
            def close_email_dlg(e):
                send_email_dlg.open = False
                page.update()

            pr_instance.touch_stack()
            page.update()

            def save_email_settings(e):
                encryption_key = api_functions.functions.call_get_encryption_key(app_api.url, app_api.headers)
                encryption_key_bytes = base64.b64decode(encryption_key)
                api_functions.functions.call_save_email_settings(
                    app_api.url,
                    app_api.headers,
                    self.server_name,
                    self.server_port,
                    self.from_email,
                    self.send_mode,
                    self.encryption,
                    self.auth_required,
                    self.email_username,
                    self.email_password,
                    encryption_key_bytes
                )
                send_email_dlg.open = False
                page.update()

            self.server_name = server_name
            self.server_port = int(server_port)
            self.from_email = from_email
            self.send_mode = send_mode
            self.encryption = encryption
            self.auth_required = auth_required
            self.email_username = username
            self.email_password = password

            subject = "Test email from pinepods"
            body = "If you got this your email settings are working! Great Job! Don't forget to hit save."
            to_email = active_user.email
            email_result = app_functions.functions.send_email(server_name, server_port, from_email, to_email, send_mode,
                                                              encryption, auth_required, username, password, subject,
                                                              body)

            pr_instance.rm_stack()
            send_email_dlg = ft.AlertDialog(
                modal=True,
                title=ft.Text(f"Email Send Test"),
                content=ft.Column(controls=[
                    ft.Text(f"Test email send result: {email_result}", selectable=True),
                    ft.Text(
                        f'If the email sent successfully be sure to hit save. This will save your settings to the database for later use with resetting passwords.',
                        selectable=True),
                ], tight=True),
                actions=[
                    ft.TextButton("Save", on_click=save_email_settings),
                    ft.TextButton("Close", on_click=close_email_dlg)
                ],
                actions_alignment=ft.MainAxisAlignment.END
            )
            page.dialog = send_email_dlg
            send_email_dlg.open = True
            page.update()

        def adjust_email_settings(self, server_name, server_port, from_email, send_mode, encryption, auth_required,
                                  username, password):
            self.server_name = server_name
            self.server_port = server_port
            self.from_email = from_email
            self.send_mode = send_mode
            self.encryption = encryption
            self.auth_required = auth_required
            self.email_username = username
            self.email_password = password
            api_functions.functions.call_save_email_settings(app_api.url, app_api.headers, self.server_name,
                                                             self.server_port, self.from_email, self.send_mode,
                                                             self.encryption, self.auth_required, self.email_username,
                                                             self.email_password)

        # Modify User Stuff---------------------------
        def open_edit_user(self, username, admin, fullname, email, user_id):
            def close_modify_dlg():
                modify_user_dlg.open = False
                self.page.update()

            def close_modify_dlg_auto(e):
                modify_user_dlg.open = False
                page.update()

            if username == 'guest':
                modify_user_dlg = ft.AlertDialog(
                    modal=True,
                    title=ft.Text(f"Guest user cannot be changed"),
                    actions=[
                        ft.TextButton("Cancel", on_click=close_modify_dlg)
                    ],
                    actions_alignment=ft.MainAxisAlignment.END
                )
                self.page.dialog = modify_user_dlg
                modify_user_dlg.open = True
                self.page.update()
            else:
                self.user_id = user_id
                if admin == 1:
                    admin_box = True
                else:
                    admin_box = False

                self.username = username
                user_modify_name = ft.TextField(label="Full Name", icon=ft.icons.CARD_MEMBERSHIP,
                                                hint_text='John PinePods')
                user_modify_email = ft.TextField(label="Email", icon=ft.icons.EMAIL,
                                                 hint_text='ilovepinepods@pinepods.com')
                user_modify_username = ft.TextField(label="Username", icon=ft.icons.PERSON,
                                                    hint_text='pinepods_user1999')
                user_modify_password = ft.TextField(label="Password", icon=ft.icons.PASSWORD, password=True,
                                                    can_reveal_password=True, hint_text='mY_SuPeR_S3CrEt!')
                user_modify_admin = ft.Checkbox(label="Set User as Admin", value=admin_box)
                modify_user_dlg = ft.AlertDialog(
                    modal=True,
                    title=ft.Text(f"Modify User: {modify_user.username}"),
                    content=ft.Column(controls=[
                        user_modify_name,
                        user_modify_email,
                        user_modify_username,
                        user_modify_password,
                        user_modify_admin
                    ], tight=True),
                    actions=[
                        ft.TextButton(content=ft.Text("Delete User", color=ft.colors.RED_400), on_click=lambda x: (
                            modify_user.delete_user(user_id),
                            close_modify_dlg()
                        )),
                        ft.TextButton("Confirm Changes", on_click=lambda x: (
                            modify_user.set_username(user_modify_username.value),
                            modify_user.set_password(user_modify_password.value),
                            modify_user.set_email(user_modify_email.value),
                            modify_user.set_name(user_modify_name.value),
                            modify_user.set_admin(user_modify_admin.value),
                            modify_user.change_user_attributes(),
                            close_modify_dlg()
                        )),

                        ft.TextButton("Cancel", on_click=close_modify_dlg_auto)
                    ],
                    actions_alignment=ft.MainAxisAlignment.SPACE_EVENLY
                )
                self.page.dialog = modify_user_dlg
                modify_user_dlg.open = True
                self.page.update()

        def change_user_attributes(self):
            if self.fullname is not None:
                api_functions.functions.call_set_fullname(app_api.url, app_api.headers, self.user_id, self.fullname)

            if self.password is not None:
                if len(self.password) < 8 or not any(c.isupper() for c in self.password) or not any(
                        c.isdigit() for c in self.password):
                    page.snack_bar = ft.SnackBar(
                        content=ft.Text(f"Passwords must contain a number, a capital letter and a special character"))
                    page.snack_bar.open = True
                    page.update()
                else:
                    salt, hash_pw = Auth.Passfunctions.hash_password(self.password)
                    api_functions.functions.call_set_password(app_api.url, app_api.headers, self.user_id, salt, hash_pw)

            if self.email is not None:
                if not re.match(self.email_regex, self.email):
                    page.snack_bar = ft.SnackBar(
                        content=ft.Text(f"This does not appear to be a properly formatted email"))
                    page.snack_bar.open = True
                    page.update()
                else:
                    api_functions.functions.call_set_email(app_api.url, app_api.headers, self.user_id, self.email)

            if self.username is not None:
                if len(self.username) < 6:
                    page.snack_bar = ft.SnackBar(content=ft.Text(f"Username must be at least 6 characters"))
                    page.snack_bar.open = True
                    page.update()
                else:
                    api_functions.functions.call_set_username(app_api.url, app_api.headers, self.user_id, self.username)

            api_functions.functions.call_set_isadmin(app_api.url, app_api.headers, self.user_id, self.isadmin)
            user_changed = True

            if user_changed == True:
                page.snack_bar = ft.SnackBar(content=ft.Text(f"User Changed!"))
                page.snack_bar.open = True
                page.update()

        def delete_user(self, user_id):
            admin_check = api_functions.functions.call_final_admin(app_api.url, app_api.headers, user_id)
            if user_id == active_user.user_id:
                page.snack_bar = ft.SnackBar(content=ft.Text(f"Cannot delete your own user"))
                page.snack_bar.open = True
                page.update()
            elif admin_check == True:
                page.snack_bar = ft.SnackBar(content=ft.Text(f"Cannot delete the final admin user"))
                page.snack_bar.open = True
                page.update()
            else:
                api_functions.functions.call_delete_user(app_api.url, app_api.headers, user_id)
                page.snack_bar = ft.SnackBar(content=ft.Text(f"User Deleted!"))
                page.snack_bar.open = True
                page.update()

        # Active User Stuff --------------------------

        def setup_timezone(self, tz, hour_pref):
            if hour_pref == '12-hour':
                self.hour_pref = 12
            else:
                self.hour_pref = 24
            self.timezone = tz
            api_functions.functions.call_setup_time_info(app_api.url, app_api.headers, self.user_id, self.timezone,
                                                         self.hour_pref)
            if self.user_id == 1:
                global new_nav
                new_nav = NavBar(page)
                new_nav.navbar.border = ft.border.only(right=ft.border.BorderSide(2, active_user.tertiary_color))
                new_nav.navbar_stack = ft.Stack([new_nav.navbar], expand=True)
                page.overlay.append(new_nav.navbar_stack)
                page.update()
                page.go("/")
            else:
                go_homelogin(page)

        def get_timezone(self):
            self.timezone, self.hour_pref = api_functions.functions.call_get_time_info(app_api.url, app_api.headers,
                                                                                       self.user_id)

        def first_login_done(self):
            self.first_login_finished = api_functions.functions.call_first_login_done(app_api.url, app_api.headers,
                                                                                      self.user_id)

        def get_initials(self):
            # split the full name into separate words
            words = self.fullname.split()

            # extract the first letter of each word and combine them
            initials_lower = "".join(word[0] for word in words)

            # return the initials as uppercase
            self.initials = initials_lower.upper()

        def login(self, username_field, password_field, retain_session):
            username = username_field.value
            password = password_field.value
            username_field.value = ''
            password_field.value = ''
            username_field.update()
            password_field.update()
            if not username or not password:
                on_click_novalues(page)
                return
            pass_correct = api_functions.functions.call_verify_password(app_api.url, app_api.headers, username,
                                                                        password)
            if pass_correct == True:
                login_details = api_functions.functions.call_get_user_details(app_api.url, app_api.headers, username)
                self.user_id = login_details['UserID']
                self.fullname = login_details['Fullname']
                self.username = login_details['Username']
                self.email = login_details['Email']

                check_mfa_status = api_functions.functions.call_check_mfa_enabled(app_api.url, app_api.headers,
                                                                                  self.user_id)
                if check_mfa_status:
                    open_mfa_login(page)

                else:
                    self.first_login_done()
                    if self.first_login_finished == 1:
                        self.get_timezone()
                        go_homelogin(page)
                    else:
                        first_time_config(page)
            else:
                on_click_wronguser(page)

        def mfa_login(self, mfa_prompt):
            mfa_secret = mfa_prompt.value

            mfa_verify = api_functions.functions.call_verify_mfa(app_api.url, app_api.headers, self.user_id, mfa_secret)

            if mfa_verify:
                self.first_login_done()
                if self.first_login_finished == 1:
                    self.get_timezone()
                    go_homelogin(page)
                else:
                    first_time_config(page)
            else:
                page.snack_bar = ft.SnackBar(content=ft.Text(f"MFA Code incorrect"))
                page.snack_bar.open = True
                self.page.update()

        def saved_login(self, user_id):
            login_details = api_functions.functions.call_get_user_details_id(app_api.url, app_api.headers, user_id)
            self.user_id = login_details['UserID']
            self.fullname = login_details['Fullname']
            self.username = login_details['Username']
            self.email = login_details['Email']
            self.first_login_done()
            if self.first_login_finished == 1:
                self.get_timezone()
                go_homelogin(page)
            else:
                first_time_config(page)

        def logout_pinepods(self, e):
            active_user = User(page)
            pr_instance.rm_stack()
            login_username.visible = True
            login_password.visible = True
            if login_screen == True:

                start_login(page)
                new_nav.navbar.border = ft.border.only(right=ft.border.BorderSide(2, active_user.tertiary_color))
                new_nav.navbar_stack = ft.Stack([new_nav.navbar], expand=True)
                page.overlay.append(new_nav.navbar_stack)
                new_nav.navbar.visible = False
                self.page.update()
            else:
                active_user.user_id = 1
                active_user.fullname = 'Guest User'
                go_homelogin(page)

        def logout_pinepods_clear_local(self, e):
            active_user = User(page)
            pr_instance.rm_stack()
            login_username.visible = True
            login_password.visible = True
            if login_screen == True:
                app_name = 'pinepods'
                data_dir = appdirs.user_data_dir(app_name)
                for filename in os.listdir(data_dir):
                    file_path = os.path.join(data_dir, filename)
                    try:
                        if os.path.isfile(file_path) or os.path.islink(file_path):
                            os.unlink(file_path)
                        elif os.path.isdir(file_path):
                            shutil.rmtree(file_path)
                    except Exception as e:
                        print(f'Failed to delete {file_path}. Reason: {e}')

                start_config(page)
            else:
                active_user.user_id = 1
                active_user.fullname = 'Guest User'
                go_homelogin(page)

        def clear_guest(self, e):
            if self.user_id == 1:
                api_functions.functions.call_clear_guest_data(app_api.url, app_api.headers)

        # Setup Theming-------------------------------------------------------
        def theme_select(self):
            active_theme = api_functions.functions.call_get_theme(app_api.url, app_api.headers, self.user_id)
            if active_theme == 'light':
                page.theme_mode = "light"
                self.main_color = '#E1E1E1'
                self.accent_color = colors.BLACK
                self.tertiary_color = '#C7C7C7'
                self.font_color = colors.BLACK
                self.bonus_color = colors.BLACK
                self.nav_color1 = colors.BLACK
                self.nav_color2 = colors.BLACK
                self.bgcolor = '#ECECEC'
                page.bgcolor = '#3C4252'
                page.window_bgcolor = '#ECECEC'
            elif active_theme == 'dark':
                page.theme_mode = "dark"
                self.main_color = '#010409'
                self.accent_color = '#8B949E'
                self.tertiary_color = '#8B949E'
                self.font_color = '#F5F5F5'
                self.bonus_color = colors.BLACK
                self.nav_color1 = colors.BLACK
                self.nav_color2 = colors.BLACK
                self.bgcolor = '#0D1117'
                page.bgcolor = '#3C4252'
                page.window_bgcolor = '#3C4252'
            elif active_theme == 'nordic':
                page.theme_mode = "dark"
                self.main_color = '#323542'
                self.accent_color = colors.WHITE
                self.tertiary_color = colors.WHITE
                self.font_color = colors.WHITE
                self.bonus_color = colors.BLACK
                self.nav_color1 = colors.BLACK
                self.nav_color2 = colors.BLACK
                self.bgcolor = '#3C4252'
                page.bgcolor = '#3C4252'
                page.window_bgcolor = '#3C4252'
            elif active_theme == 'abyss':
                page.theme_mode = "dark"
                self.main_color = '#051336'
                self.accent_color = '#FFFFFF'
                self.tertiary_color = '#13326A'
                self.font_color = '#42A5F5'
                self.bonus_color = colors.BLACK
                self.nav_color1 = colors.BLACK
                self.nav_color2 = colors.WHITE
                self.bgcolor = '#000C18'
                page.bgcolor = '#3C4252'
                page.window_bgcolor = '#3C4252'
            elif active_theme == 'dracula':
                page.theme_mode = "dark"
                self.main_color = '#262626'
                self.accent_color = '#5196B2'
                self.tertiary_color = '#5196B2'
                self.font_color = colors.WHITE
                self.bonus_color = '#D5BC5C'
                self.nav_color1 = '#D5BC5C'
                self.nav_color2 = colors.BLACK
                self.bgcolor = '#282A36'
                page.bgcolor = '#282A36'
                page.window_bgcolor = '#3C4252'
            elif active_theme == 'kimbie':
                page.theme_mode = "dark"
                self.main_color = '#362712'
                self.accent_color = '#B23958'
                self.tertiary_color = '#AC8E2F'
                self.font_color = '#B1AD86'
                self.bonus_color = '#221A1F'
                self.nav_color1 = '#221A1F'
                self.nav_color2 = '#B1AD86'
                self.bgcolor = '#221A0F'
                page.bgcolor = '#282A36'
                page.window_bgcolor = '#3C4252'
            elif active_theme == 'hotdogstand - MY EYES':
                page.theme_mode = "dark"
                self.main_color = '#EEB911'
                self.accent_color = '#C3590D'
                self.tertiary_color = '#730B1B'
                self.font_color = colors.WHITE
                self.bonus_color = '#D5BC5C'
                self.nav_color1 = '#D5BC5C'
                self.nav_color2 = colors.BLACK
                self.bgcolor = '#E31836'
                page.bgcolor = '#282A36'
                page.window_bgcolor = '#3C4252'
            elif active_theme == 'neon':
                page.theme_mode = "dark"
                self.main_color = '#161C26'
                self.accent_color = '#7000FF'
                self.tertiary_color = '#5196B2'
                self.font_color = '#9F9DA1'
                self.bonus_color = '##01FFF4'
                self.nav_color1 = '#FF1178'
                self.nav_color2 = '#3544BD'
                self.bgcolor = '#120E16'
                page.bgcolor = '#282A36'
                page.window_bgcolor = '#3C4252'
            elif active_theme == 'wildberries':
                page.theme_mode = "dark"
                self.main_color = '#19002E'
                self.accent_color = '#F55385'
                self.tertiary_color = '#5196B2'
                self.font_color = '#CF8B3E'
                self.bonus_color = '#C79BFF'
                self.nav_color1 = '#00FFB7'
                self.nav_color2 = '#44433A'
                self.bgcolor = '#282A36'
                page.bgcolor = '#240041'
                page.window_bgcolor = '#3C4252'
            elif active_theme == 'greenie meanie':
                page.theme_mode = "dark"
                self.main_color = '#292A2E'
                self.accent_color = '#737373'
                self.tertiary_color = '#489D50'
                self.font_color = '#489D50'
                self.bonus_color = '#849CA0'
                self.nav_color1 = '#446448'
                self.nav_color2 = '#43603D'
                self.bgcolor = '#1E1F21'
                page.bgcolor = '#3C4252'
                page.window_bgcolor = '#3C4252'

        def set_theme(self, theme):
            api_functions.functions.call_set_theme(app_api.url, app_api.headers, self.user_id, theme)
            self.theme_select
            go_theme_rebuild(self.page)
            self.page.update()

    # Initial user value set
    modify_user = User(page)

    # Searhcing Class

    class SearchPods:
        def __init__(self, page):
            self.searchvalue = None
            self.searchlocation = None

    new_search = SearchPods(page)

    def GradientGenerator(start, end):
        ColorGradient = ft.LinearGradient(
            begin=alignment.bottom_left,
            end=alignment.top_right,
            colors=[
                start,
                end,
            ],
        )

        return ColorGradient

    login_username = ft.TextField(
        label="Username",
        border="underline",
        width=320,
        text_size=14,
    )

    login_password = ft.TextField(
        label="Password",
        border="underline",
        width=320,
        text_size=14,
        password=True,
        can_reveal_password=True,
    )

    mfa_prompt = ft.TextField(
        label="MFA code",
        border="underline",
        hint_text="ex. 123456",
        width=320,
        text_size=14,
    )

    active_user = User(page)

    # Create Sidebar------------------------------------------------------

    class NavBar:
        def __init__(self, page):
            self.page = page
            self.navbar = self.create_navbar()
            self.navbar_stack = ft.Stack([self.navbar], expand=True)

        def HighlightContainer(self, e):
            if e.data == "true":
                e.control.bgcolor = "white10"
                e.control.update()

                e.control.content.controls[0].icon_color = "white"
                e.control.content.controls[1].color = "white"
                e.control.content.update()
            else:
                e.control.bgcolor = None
                e.control.update()

                e.control.content.controls[0].icon_color = active_user.accent_color
                e.control.content.controls[1].color = active_user.accent_color
                e.control.content.update()

        def ContainedIcon(self, tooltip, icon_name, text, destination):
            return ft.Container(
                width=180,
                height=45,
                border_radius=10,
                on_hover=lambda e: self.HighlightContainer(e),
                ink=True,
                content=Row(
                    controls=[
                        ft.IconButton(
                            icon=icon_name,
                            icon_size=18,
                            icon_color=active_user.accent_color,
                            tooltip=tooltip,
                            selected=False,
                            on_click=destination,
                            style=ButtonStyle(
                                shape={
                                    "": ft.RoundedRectangleBorder(radius=7),
                                },
                                overlay_color={"": "transparent"},
                            ),
                        ),
                        ft.Text(
                            value=text,
                            color="white54",
                            size=11,
                            opacity=0,
                            animate_opacity=200,
                        ),
                    ],
                ),
            )

        def create_navbar(self):
            def get_gravatar_url(email, size=42, default='mp'):
                email_hash = hashlib.md5(email.lower().encode('utf-8')).hexdigest()
                gravatar_url = f'https://www.gravatar.com/avatar/{email_hash}?s={size}&d={default}'
                profile_url = f'https://www.gravatar.com/{email_hash}.json'

                try:
                    response = requests.get(profile_url)
                    response.raise_for_status()
                except requests.exceptions.RequestException:
                    return None

                return gravatar_url

            gravatar_url = None
            if active_user.user_id != 1:
                gravatar_url = get_gravatar_url(active_user.email)
            active_user.get_initials()

            user_content = ft.Image(src=gravatar_url, width=42, height=45, border_radius=8) if gravatar_url else Text(
                value=active_user.initials,
                color=active_user.nav_color2,
                size=20,
                weight="bold"
            )

            return ft.Container(
                width=62,
                # height=580,
                expand=True,
                animate=animation.Animation(500, "decelerate"),
                bgcolor=active_user.main_color,
                padding=10,
                content=ft.Column(
                    alignment=MainAxisAlignment.START,
                    horizontal_alignment="center",
                    controls=[
                        Text(
                            value=(f'PinePods'),
                            size=8,
                            weight="bold",
                            color=active_user.accent_color
                        ),
                        ft.Divider(color="white24", height=5),
                        ft.Container(
                            width=42,
                            height=40,
                            border_radius=8,
                            bgcolor=active_user.tertiary_color,
                            alignment=alignment.center,
                            content=user_content,
                            on_click=open_user_stats
                        ),
                        ft.Divider(height=5, color="transparent"),
                        self.ContainedIcon('Home', icons.HOME, "Home", go_home),
                        self.ContainedIcon('Queue', icons.QUEUE, "Queue", open_queue),
                        self.ContainedIcon('Saved Episodes', icons.SAVE, "Saved Epsiodes", open_saved_pods),
                        self.ContainedIcon('Downloaded', icons.DOWNLOAD, "Downloaded", open_downloads),
                        self.ContainedIcon('Podcast History', icons.HISTORY, "Podcast History", open_history),
                        self.ContainedIcon('Added Podcasts', icons.PODCASTS, "Added Podcasts", open_pod_list),
                        self.ContainedIcon('Search', icons.SEARCH, 'Search', open_search),
                        ft.Divider(color="white24", height=5),
                        self.ContainedIcon('Settings', icons.SETTINGS, "Settings", open_settings),
                        self.ContainedIcon('Logout', icons.LOGOUT_ROUNDED, "Logout", active_user.logout_pinepods),
                    ],
                ),
            )

    # Create Page--------------------------------------------------------
    # get the absolute path of the current script
    current_dir = os.path.dirname(os.path.abspath(__file__))

    parsed_audio_url = os.path.join(current_dir, "Audio", "750-milliseconds-of-silence.mp3")
    parsed_title = 'nothing playing'
    current_episode = Toggle_Pod(page, go_home, parsed_audio_url, parsed_title)
    class PodcastControls:

        def __init__(self, page, go_home, parsed_audio_url, parsed_title):
            self.page = page
            self.go_home = go_home
            self.parsed_audio_url = parsed_audio_url
            self.parsed_title = parsed_title
            # self.current_episode = Toggle_Pod(page, go_home, parsed_audio_url, parsed_title)
            self.init_controls()

        def init_controls(self):
            self.create_audio_controls()
            self.setup_audio_scrubber()
            self.setup_audio_container()
            self.setup_volume_control()

        def create_audio_controls(self):
            self.play_button = ft.IconButton(
                icon=ft.icons.PLAY_ARROW,
                tooltip="Play Podcast",
                icon_color="white",
                on_click=lambda e: current_episode.resume_podcast()
            )
            self.pause_button = ft.IconButton(
                icon=ft.icons.PAUSE,
                tooltip="Pause Playback",
                icon_color="white",
                on_click=lambda e: current_episode.pause_episode()
            )
            self.pause_button.visible = False
            self.seek_button = ft.IconButton(
                icon=ft.icons.FAST_FORWARD,
                tooltip="Seek 10 seconds",
                icon_color="white",
                on_click=lambda e: current_episode.seek_episode()
            )
            self.ep_audio_controls = ft.Row(controls=[self.play_button, self.pause_button, self.seek_button])

        def setup_audio_scrubber(self):
            def format_time(time):
                hours, remainder = divmod(int(time), 3600)
                minutes, seconds = divmod(remainder, 60)
                return f"{hours:02d}:{minutes:02d}:{seconds:02d}"

            def slider_changed(e):
                formatted_scrub = format_time(self.audio_scrubber.value)
                self.current_time.content = ft.Text(formatted_scrub)
                self.current_time.update()
                current_episode.time_scrub(self.audio_scrubber.value)

            self.podcast_length = ft.Container(content=ft.Text('doesntmatter'))
            self.current_time_text = ft.Text('placeholder')
            self.current_time = ft.Container(content=self.current_time_text)
            self.audio_scrubber = ft.Slider(min=0, expand=True, max=current_episode.seconds, label="{value}",
                                            on_change=slider_changed)
            self.audio_scrubber.width = '100%'
            self.audio_scrubber_column = ft.Column(controls=[self.audio_scrubber])
            self.audio_scrubber_column.horizontal_alignment.STRETCH
            self.audio_scrubber_column.width = '100%'

        def setup_audio_container(self):
            self.currently_playing = ft.Container(content=ft.Text('test'), on_click=open_currently_playing)
            self.audio_container_image_landing = ft.Image(
                src=f"/home/collinp/Documents/GitHub/PyPods/images/pinepods-logo.jpeg",
                width=40, height=40)
            self.audio_container_image = ft.Container(content=self.audio_container_image_landing,
                                                      on_click=open_currently_playing)
            self.audio_container_image.border_radius = ft.border_radius.all(25)
            self.currently_playing_container = ft.Row(
                controls=[self.audio_container_image, self.currently_playing])
            self.scrub_bar_row = ft.Row(controls=[self.current_time, self.audio_scrubber_column, self.podcast_length])
            self.volume_button = ft.IconButton(icon=ft.icons.VOLUME_UP_ROUNDED, tooltip="Adjust Volume",
                                               on_click=lambda x: current_episode.volume_view())
            self.audio_controls_row = ft.Row(alignment=ft.MainAxisAlignment.CENTER,
                                             controls=[self.scrub_bar_row, self.ep_audio_controls, self.volume_button])
            self.audio_container_row_landing = ft.Row(
                vertical_alignment=ft.CrossAxisAlignment.END,
                alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
                controls=[self.currently_playing_container, self.audio_controls_row])
            self.audio_container_row = ft.Container(content=self.audio_container_row_landing)
            self.audio_container_row.padding = ft.padding.only(left=10)
            self.audio_container_pod_details = ft.Row(
                controls=[self.audio_container_image, self.currently_playing],
                alignment=ft.MainAxisAlignment.CENTER)
            ep_height = 50
            ep_width = 4000
            self.audio_container = ft.Container(
                height=ep_height,
                width=ep_width,
                bgcolor=active_user.main_color,
                border_radius=45,
                padding=6,
                content=self.audio_container_row
            )

        def setup_volume_control(self):
            self.volume_slider = ft.Slider(value=1, on_change=lambda x: current_episode.volume_adjust())
            self.volume_down_icon = ft.Icon(name=ft.icons.VOLUME_MUTE)
            self.volume_up_icon = ft.Icon(name=ft.icons.VOLUME_UP_ROUNDED)
            self.volume_adjust_column = ft.Row(
                controls=[self.volume_down_icon, self.volume_slider, self.volume_up_icon], expand=True)
            self.volume_container = ft.Container(
                height=35,
                width=275,
                bgcolor=ft.colors.WHITE,
                border_radius=45,
                padding=6,
                content=self.volume_adjust_column)
            self.volume_container.adding = ft.padding.all(50)
            self.volume_container.alignment = ft.alignment.top_right
            self.volume_container.visible = False

            self.page.overlay.append(ft.Stack([self.volume_container], bottom=75, right=25, expand=True))
            self.page.overlay.append(ft.Stack([self.audio_container], bottom=20, right=20, left=70, expand=True))
            self.audio_container.visible = False

    # Usage:
    page.title = "PinePods - A Forest of Podcasts, Rooted in the Spirit of Self-Hosting"
    current_dir = os.path.dirname(os.path.abspath(__file__))
    pod_controls = PodcastControls(page, go_home, parsed_audio_url, parsed_title)

    def play_selected_episode(url, title, artwork):
        current_episode.url = url
        current_episode.name = title
        current_episode.artwork = artwork
        current_episode.play_episode()

    def resume_selected_episode(url, title, artwork, listen_duration):
        current_episode.url = url
        current_episode.name = title
        current_episode.artwork = artwork
        current_episode.play_episode(listen_duration=listen_duration)

    def download_selected_episode(url, title, page):
        # First, check if downloads are enabled
        download_status = api_functions.functions.call_download_status(app_api.url, app_api.headers)
        if not download_status:
            page.snack_bar = ft.SnackBar(content=ft.Text(
                f"Downloads are currently disabled! If you'd like to download episodes ask your administrator to enable the option."))
            page.snack_bar.open = True
            page.update()
        else:
            # Proceed with the rest of the process
            check_downloads = api_functions.functions.call_check_downloaded(app_api.url, app_api.headers,
                                                                            active_user.user_id, title, url)
            if check_downloads:
                page.snack_bar = ft.SnackBar(content=ft.Text(f"Episode is already downloaded!"))
                page.snack_bar.open = True
                page.update()
            else:
                pr_instance.touch_stack()
                page.update()
                current_episode.url = url
                current_episode.title = title
                current_episode.download_pod()
                page.snack_bar = ft.SnackBar(content=ft.Text(f"Episode: {title} has been downloaded!"))
                page.snack_bar.open = True
                pr_instance.rm_stack()
                page.update()

    def queue_selected_episode(url, title, artwork, page):
        current_episode.url = url
        current_episode.title = title
        current_episode.artwork = artwork
        current_episode.name = title
        current_episode.queue_pod(url, title)
        page.update()

    def save_selected_episode(url, title, page):
        check_saved = api_functions.functions.call_check_saved(app_api.url, app_api.headers, active_user.user_id, title,
                                                               url)
        if check_saved:
            page.snack_bar = ft.SnackBar(content=ft.Text(f"Episode is already saved!"))
            page.snack_bar.open = True
            page.update()
        else:
            current_episode.url = url
            current_episode.title = title
            current_episode.save_pod()
            page.snack_bar = ft.SnackBar(content=ft.Text(f"Episode: {title} has been added to saved podcasts!"))
            page.snack_bar.open = True
            page.update()

    page.on_disconnect = active_user.clear_guest

# Starting Page Layout
    page.theme_mode = "dark"

    app_api.api_verify()


# Browser Version
ft.app(target=main, view=ft.WEB_BROWSER, port=8034)
# App version
# ft.app(target=main, port=8034)
