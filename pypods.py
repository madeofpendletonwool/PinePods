# Various flet imports
import flet as ft
from flet import *
from flet import AppBar, ElevatedButton, Page, Text, View, colors, icons, ProgressBar, ButtonStyle, IconButton, TextButton, Row, alignment
# Internal Functions
import internal_functions.functions
import database_functions.functions
import app_functions.functions
import Auth.Passfunctions
import Audio.functions
# Others
import time
import mysql.connector
import json
import re
import feedparser
import urllib.request
import requests
from functools import partial
import os
import requests
import tempfile
import time
import random
import datetime
import html2text
import threading
from html.parser import HTMLParser
from flask import Flask
from flask_caching import Cache

app = Flask(__name__)
cache = Cache(app, config={'CACHE_TYPE': 'simple'})

@app.route('/preload/<path:url>')
def preload_audio_file(url):
    # Try to get the response from cache
    response = requests.get('http://10.0.0.15:5000/proxy', params={'url': url})
    if response.status_code == 200:
        # Cache the file content
        cache.set(url, response.content)
    return ""

# Make login Screen start on boot
login_screen = True

#Initial Vars needed to start and used throughout
proxy_url = 'http://localhost:8000/proxy?url='
audio_playing = False
active_pod = 'Set at start'
script_dir = os.path.dirname(os.path.abspath(__file__))



# Create database connector
cnx = mysql.connector.connect(
    host="127.0.0.1",
    port="3306",
    user="root",
    password="password",
    database="pypods_database"
)

def main(page: ft.Page):

#---Flet Various Functions---------------------------------------------------------------
    def send_podcast(pod_title, pod_artwork, pod_author, pod_categories, pod_description, pod_episode_count, pod_feed_url, pod_website):
        categories = json.dumps(pod_categories)
        podcast_values = (pod_title, pod_artwork, pod_author, categories, pod_description, pod_episode_count, pod_feed_url, pod_website, active_user.user_id)
        database_functions.functions.add_podcast(cnx, podcast_values, active_user.user_id)
            
    def invalid_username():
        page.dialog = username_invalid_dlg
        username_invalid_dlg.open = True
        page.update() 

    def validate_user(input_username, input_pass):
        return Auth.Passfunctions.verify_password(cnx, input_username, input_pass) 

    def close_dlg(e):
        user_dlg.open = False
        page.update() 
        go_home 

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

    def guest_user_change(e):
        database_functions.functions.enable_disable_guest(cnx)
        page.snack_bar = ft.SnackBar(content=ft.Text(f"Guest user modified!"))
        page.snack_bar.open = True
        page.update()

    def self_service_change(e):
        database_functions.functions.enable_disable_self_service(cnx)
        page.snack_bar = ft.SnackBar(content=ft.Text(f"Self Service Settings Adjusted!"))
        page.snack_bar.open = True
        page.update()

    def display_hello(e):
        page.snack_bar = ft.SnackBar(content=ft.Text(f"Hello {active_user.fullname}! Click name for information"))
        page.snack_bar.open = True
        page.update()


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

    def evaluate_podcast(pod_title, pod_artwork, pod_author, pod_categories, pod_description, pod_episode_count, pod_feed_url, pod_website):
        global clicked_podcast
        clicked_podcast = Podcast(name=pod_title, artwork=pod_artwork, author=pod_author, description=pod_description, feedurl=pod_feed_url, website=pod_website, categories=pod_categories, episode_count=pod_episode_count)
        return clicked_podcast

    class Podcast:
        def __init__(self, name=None, artwork=None, author=None, description=None, feedurl=None, website=None, categories=None, episode_count=None):
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
            page.update()

        self_service_status = database_functions.functions.self_service_status(cnx)

        if self_service_status == 0:
            self_service_dlg = ft.AlertDialog(
                modal=True,
                title=ft.Text(f"User Creation"),
                content=ft.Column(controls=[
                        ft.Text("Self Service User Creation is disabled. If you'd like an account please contact the admin or have them enable self service.")
                    ]),
                actions=[
                    ft.TextButton("Close", on_click=close_self_serv_dlg)
                    ],
                actions_alignment=ft.MainAxisAlignment.SPACE_EVENLY
            )
            self.page.dialog = self_service_dlg
            self_service_dlg.open = True
            self.page.update()

        elif self_service_status == 1:
            new_user = User(page)

            self_service_name = ft.TextField(label="Full Name", icon=ft.icons.CARD_MEMBERSHIP, hint_text='John PinePods') 
            self_service_email = ft.TextField(label="Email", icon=ft.icons.EMAIL, hint_text='ilovepinepods@pinepods.com')
            self_service_username = ft.TextField(label="Username", icon=ft.icons.PERSON, hint_text='pinepods_user1999') 
            self_service_password = ft.TextField(label="Password", icon=ft.icons.PASSWORD, password=True, can_reveal_password=True, hint_text='mY_SuPeR_S3CrEt!')
            self_service_dlg = ft.AlertDialog(
            modal=True,
            title=ft.Text(f"Create User:"),
            content=ft.Column(controls=[
                    self_service_name,
                    self_service_email,
                    self_service_username,
                    self_service_password
                ]),
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

                ft.TextButton("Cancel", on_click=close_self_serv_dlg)
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
                # self.episode_name = self.name
                if url is None or name is None:
                    self.active_pod = 'Initial Value'
                else:
                    self.active_pod = self.name
                self.queue = []
                self.state = 'stopped'
                Toggle_Pod.initialized = True
            else:
                self.page = page
                self.go_home = go_home
                self.url = url
                self.name = name or ""
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
                # self.episode_name = self.name
                self.queue = []
                self.state = 'stopped'

        def run_function_every_60_seconds(self):
            while True:
                time.sleep(60)
                print('adding time entry')
                if self.audio_playing:
                    database_functions.functions.increment_listen_time(cnx, active_user.user_id)

        def play_episode(self, e=None, listen_duration=None):
            pr = ft.ProgressRing()
            progress_stack = ft.Stack([pr], bottom=25, right=30, left=20, expand=True)
            page.overlay.append(progress_stack)
            page.update()
            # release audio_element if it exists
            if self.audio_element:
                self.audio_element.release()

            # Preload the audio file and cache it
            preload_audio_file(self.url)

            self.audio_element = ft.Audio(src=f'{self.url}', autoplay=True, volume=1, on_state_changed=lambda e: self.on_state_changed(e.data))
            page.overlay.append(self.audio_element)
            # self.audio_element.play()
            
            # Set the playback position to the given listen duration
            if listen_duration:
                self.audio_element.seek(listen_duration)

            self.audio_playing = True
            page.update()
            waittime = 1
        
            tries = 0
            while True:
                try:
                    duration = self.audio_element.get_duration()
                    time.sleep(.5)
                    if duration > 0:
                        media_length = duration
                        break
                except Exception as e:
                    tries += 1
                    if tries >= 5:
                        print("Max retries exceeded, unable to load audio")
                        return

            self.record_history()
            database_functions.functions.increment_played(cnx, active_user.user_id)

            # convert milliseconds to a timedelta object
            delta = datetime.timedelta(milliseconds=media_length)

            # convert timedelta object to datetime object
            datetime_obj = datetime.datetime(1, 1, 1) + delta

            # format datetime object to hh:mm:ss format with two decimal places
            total_length = datetime_obj.strftime('%H:%M:%S')
            time.sleep(1)
            self.length = total_length
            self.toggle_current_status()

            page.overlay.remove(progress_stack)
            page.update()
            
            # convert milliseconds to seconds
            total_seconds = media_length // 1000
            self.seconds = total_seconds
            audio_scrubber.max = self.seconds

            threading.Thread(target=self.run_function_every_60_seconds, daemon=True).start()
            for i in range(total_seconds):
                self.current_progress = self.get_current_time()
                self.toggle_second_status(self.audio_element.data)
                time.sleep(1)
                
                if (datetime.datetime.now() - self.last_listen_duration_update).total_seconds() > 15:
                    print(total_seconds)
                    self.record_listen_duration()
                    self.last_listen_duration_update = datetime.datetime.now()

        def on_state_changed(self, status):
            self.state = status
            if status == 'completed':
                # If there are episodes in the queue, play the next episode
                if len(self.queue) > 0:
                    next_episode_url = self.queue.pop(0)
                    self.play_episode(next_episode_url)
                else:
                    # Stop playing when the queue is empty
                    self.audio_element.release()
                    self.audio_playing = False
                    self.toggle_current_status()

        def _monitor_audio(self):
            while True:
                state = self.player.get_state()
                if state == vlc.State.Ended:
                    self.thread = None
                    break
                time.sleep(1)

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

        def toggle_current_status(self):
            if self.audio_playing:
                play_button.visible = False
                pause_button.visible = True
                audio_container.bgcolor = active_user.main_color
                audio_container.visible = True
                currently_playing.content = ft.Text(self.name, size=16)
                current_time.content = ft.Text(self.length, color=active_user.font_color)
                podcast_length.content = ft.Text(self.length)
                audio_container_image_landing.src = self.artwork
                audio_container_image_landing.width = 40
                audio_container_image_landing.height = 40
                audio_container_image_landing.border_radius = ft.border_radius.all(100)
                audio_container_image.border_radius = ft.border_radius.all(75)
                audio_container_image_landing.update()
                audio_scrubber.active_color = active_user.nav_color2
                audio_scrubber.inactive_color = active_user.nav_color2
                audio_scrubber.thumb_color = active_user.accent_color
                play_button.icon_color = active_user.accent_color
                pause_button.icon_color = active_user.accent_color
                seek_button.icon_color = active_user.accent_color
                currently_playing.color = active_user.font_color
                # current_time_text.color = active_user.font_color
                podcast_length.color = active_user.font_color
                self.page.update()
            else:
                pause_button.visible = False
                play_button.visible = True
                currently_playing.content = ft.Text(self.name, color=active_user.nav_color1)
                self.page.update()
                
        def toggle_second_status(self, status):
            if self.state == 'playing':
                audio_scrubber.value = self.get_current_seconds()
                audio_scrubber.update()
                current_time.content = ft.Text(self.current_progress, color=active_user.font_color)
                current_time.update()

            # self.page.update()

        def seek_episode(self):
            seconds = 10
            time = self.audio_element.get_current_position()
            seek_position = time + 10000
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
            database_functions.functions.record_podcast_history(cnx, self.name, active_user.user_id, 0)

        def download_pod(self):
            database_functions.functions.download_podcast(cnx, self.url, self.title, active_user.user_id)

        def delete_pod(self):
            database_functions.functions.delete_podcast(cnx, self.url, self.title, active_user.user_id)


        def queue_pod(self, url):
            self.queue.append(url)

        def remove_queued_pod(self):
            try:
                self.queue.remove(self.url)
            except ValueError:
                print(f"Episode '{url}' not found in queue")

        def save_pod(self):
            database_functions.functions.save_episode(cnx, self.url, self.title, active_user.user_id)

        def remove_saved_pod(self):
            database_functions.functions.remove_saved_episode(cnx, self.url, self.title, active_user.user_id)

        def get_queue(self):
            return self.queue

        def get_current_time(self):
            try:
                time = self.audio_element.get_current_position() // 1000  # convert milliseconds to seconds
            except Exception as e:
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
            database_functions.functions.record_listen_duration(cnx, self.url, self.name, active_user.user_id, listen_duration)

        def seek_to_second(self, second):
            """
            Set the media position to the specified second.
            """
            self.player.set_time(int(second * 1000))


    def refresh_podcasts(e):
        pr = ft.ProgressRing()
        page.overlay.append(ft.Stack([pr], bottom=25, right=30, left=20, expand=True))
        page.update()
        database_functions.functions.refresh_pods(cnx)
        print('refresh complete')
        page.overlay.pop(2)
        page.snack_bar = ft.SnackBar(content=ft.Text(f"Refresh Complete!"))
        page.snack_bar.open = True
        page.update()
        # Reset current view if on homepage
        if page.route == "/" or page.route == "/":
            page.bgcolor = colors.BLUE_GREY

            # Home Screen Podcast Layout (Episodes in Newest order)

            home_episodes = database_functions.functions.return_episodes(cnx, active_user.user_id)

            if home_episodes is None:
                home_ep_number = 1
                home_ep_rows = []
                home_ep_row_dict = {}

                home_pod_name = "No Podcasts added yet"
                home_ep_title = "Podcasts you add will display new episodes here."
                home_pub_date = ""
                home_ep_desc = "You can search podcasts in the upper right. Then click the plus button to add podcasts to the add. Click around on the navbar to manage podcasts you've added. Enjoy the listening!"
                home_ep_url = ""
                home_entry_title = ft.Text(f'{home_pod_name} - {home_ep_title}', width=600, style=ft.TextThemeStyle.TITLE_MEDIUM)
                home_entry_description = ft.Text(home_ep_desc, width=800)
                home_entry_audio_url = ft.Text(home_ep_url)
                home_entry_released = ft.Text(home_pub_date)
                home_artwork_no = random.randint(1, 12)
                home_artwork_url = os.path.join(script_dir, "images", "logo_random", f"{home_artwork_no}.jpeg")
                home_art_url_parsed = check_image(home_artwork_url)
                home_entry_artwork_url = ft.Image(src=home_art_url_parsed, width=150, height=150)
                home_ep_play_button = ft.IconButton(
                    icon=ft.icons.PLAY_DISABLED,
                    icon_color=active_user.accent_color,
                    icon_size=40,
                    tooltip="No Episodes Listened to yet"
                )
                # Creating column and row for home layout
                home_ep_column = ft.Column(
                    controls=[home_entry_title, home_entry_description, home_entry_released]
                )

                home_ep_row_content = ft.ResponsiveRow([
                    ft.Column(col={"md": 2}, controls=[home_entry_artwork_url]),
                    ft.Column(col={"md": 10}, controls=[home_ep_column, home_ep_play_button]),
                ])
                home_ep_row = ft.Container(content=home_ep_row_content)
                home_ep_row.padding=padding.only(left=70, right=50)
                home_ep_rows.append(home_ep_row)
                home_ep_row_dict[f'search_row{home_ep_number}'] = home_ep_row
                home_pods_active = True
                home_ep_number += 1
            else:
                home_ep_number = 1
                home_ep_rows = []
                home_ep_row_dict = {}

                for entry in home_episodes:
                    home_ep_title = entry['EpisodeTitle']
                    home_pod_name = entry['PodcastName']
                    home_pub_date = entry['EpisodePubDate']
                    home_ep_desc = entry['EpisodeDescription']
                    home_ep_artwork = entry['EpisodeArtwork']
                    home_ep_url = entry['EpisodeURL']
                    home_ep_duration = entry['EpisodeDuration']
                    # do something with the episode information
                    home_entry_title_button = ft.Text(f'{home_pod_name} - {home_ep_title}', style=ft.TextThemeStyle.TITLE_MEDIUM, color=active_user.font_color)
                    home_entry_title = ft.TextButton(content=home_entry_title_button, on_click=lambda x, url=home_ep_url, title=home_ep_title: open_episode_select(page, url, title))
                    home_entry_row = ft.ResponsiveRow([
    ft.Column(col={"sm": 6}, controls=[home_entry_title]),
])

                    num_lines = home_ep_desc.count('\n')
                    if num_lines > 15:
                        if is_html(home_ep_desc):
                            # convert HTML to Markdown
                            markdown_desc = html2text.html2text(home_ep_desc)
                            if num_lines > 15:
                                # Split into lines, truncate to 15 lines, and join back into a string
                                lines = markdown_desc.splitlines()[:15]
                                markdown_desc = '\n'.join(lines)
                            # add inline style to change font color                            
                            home_entry_description = ft.Markdown(markdown_desc, on_tap_link=launch_clicked_url)
                            home_entry_seemore = ft.TextButton(text="See More...", on_click=lambda x, url=home_ep_url, title=home_ep_title: open_episode_select(page, url, title))
                        else:
                            if num_lines > 15:
                                # Split into lines, truncate to 15 lines, and join back into a string
                                lines = home_ep_desc.splitlines()[:15]
                                home_ep_desc = '\n'.join(lines)
                            # display plain text
                            home_entry_description = ft.Text(home_ep_desc)

                    else:
                        if is_html(home_ep_desc):
                            # convert HTML to Markdown
                            markdown_desc = html2text.html2text(home_ep_desc)
                            # add inline style to change font color
                            home_entry_description = ft.Markdown(markdown_desc, on_tap_link=launch_clicked_url)
                        else:
                            # display plain text
                            markdown_desc = home_ep_desc
                            home_entry_description = ft.Text(home_ep_desc)

                    home_entry_audio_url = ft.Text(home_ep_url, color=active_user.font_color)
                    check_episode_playback, listen_duration = database_functions.functions.check_episode_playback(cnx, active_user.user_id, home_ep_title, home_ep_url)
                    home_entry_released = ft.Text(f'Released on: {home_pub_date}', color=active_user.font_color)

                    home_art_no = random.randint(1, 12)
                    home_art_fallback = os.path.join(script_dir, "images", "logo_random", f"{home_art_no}.jpeg")
                    home_art_url = home_ep_artwork if home_ep_artwork else home_art_fallback
                    home_art_parsed = check_image(home_art_url)
                    home_entry_artwork_url = ft.Image(src=home_art_parsed, width=150, height=150)
                    home_ep_play_button = ft.IconButton(
                        icon=ft.icons.NOT_STARTED,
                        icon_color=active_user.accent_color,
                        icon_size=40,
                        tooltip="Play Episode",
                        on_click=lambda x, url=home_ep_url, title=home_ep_title, artwork=home_ep_artwork: play_selected_episode(url, title, artwork)
                    )
                    home_ep_resume_button = ft.IconButton(
                        icon=ft.icons.PLAY_CIRCLE,
                        icon_color=active_user.accent_color,
                        icon_size=40,
                        tooltip="Resume Episode",
                        on_click=lambda x, url=home_ep_url, title=home_ep_title, artwork=home_ep_artwork, listen_duration=listen_duration: resume_selected_episode(url, title, artwork, listen_duration)
                    )
                    home_popup_button = ft.PopupMenuButton(content=ft.Icon(ft.icons.ARROW_DROP_DOWN_CIRCLE_ROUNDED, color=active_user.accent_color, size=40, tooltip="Play Episode"), 
                        items=[
                            ft.PopupMenuItem(icon=ft.icons.QUEUE, text="Queue", on_click=lambda x, url=home_ep_url, title=home_ep_title, artwork=home_ep_artwork: queue_selected_episode(url, title, artwork, page)),
                            ft.PopupMenuItem(icon=ft.icons.DOWNLOAD, text="Download", on_click=lambda x, url=home_ep_url, title=home_ep_title: download_selected_episode(url, title, page)),
                            ft.PopupMenuItem(icon=ft.icons.SAVE, text="Save Episode", on_click=lambda x, url=home_ep_url, title=home_ep_title: save_selected_episode(url, title, page))
                        ]
                    )
                    if check_episode_playback == True:
                        listen_prog = seconds_to_time(listen_duration)
                        home_ep_prog = seconds_to_time(home_ep_duration)
                        progress_value = get_progress(listen_duration, home_ep_duration)
                        home_entry_progress = ft.Row(controls=[ft.Text(listen_prog, color=active_user.font_color), ft.ProgressBar(expand=True, value=progress_value, color=active_user.main_color), ft.Text(home_ep_prog, color=active_user.font_color)])
                        if num_lines > 15:
                            home_ep_row_content = ft.ResponsiveRow([
                                ft.Column(col={"md": 2}, controls=[home_entry_artwork_url]),
                                ft.Column(col={"md": 10}, controls=[home_entry_title, home_entry_description, home_entry_seemore, home_entry_released, home_entry_progress, ft.Row(controls=[home_ep_play_button, home_ep_resume_button, home_popup_button])]),
                            ])
                        else:
                            home_ep_row_content = ft.ResponsiveRow([
                                ft.Column(col={"md": 2}, controls=[home_entry_artwork_url]),
                                ft.Column(col={"md": 10}, controls=[home_entry_title, home_entry_description, home_entry_released, home_entry_progress, ft.Row(controls=[home_ep_play_button, home_ep_resume_button, home_popup_button])]),
                            ]) 
                    else:
                        if num_lines > 15:
                            home_ep_row_content = ft.ResponsiveRow([
                                ft.Column(col={"md": 2}, controls=[home_entry_artwork_url]),
                                ft.Column(col={"md": 10}, controls=[home_entry_title, home_entry_description, home_entry_seemore, home_entry_released, ft.Row(controls=[home_ep_play_button, home_popup_button])]),
                            ])
                        else:
                            home_ep_row_content = ft.ResponsiveRow([
                                ft.Column(col={"md": 2}, controls=[home_entry_artwork_url]),
                                ft.Column(col={"md": 10}, controls=[home_entry_title, home_entry_description, home_entry_released, ft.Row(controls=[home_ep_play_button, home_popup_button])]),
                            ]) 
                    home_div_row = ft.Divider(color=active_user.accent_color)
                    home_ep_column = ft.Column(controls=[home_ep_row_content, home_div_row])
                    home_ep_row = ft.Container(content=home_ep_column)
                    home_ep_row.padding=padding.only(left=70, right=50)
                    home_ep_rows.append(home_ep_row)
                    # home_ep_rows.append(ft.Text('test'))
                    home_ep_row_dict[f'search_row{home_ep_number}'] = home_ep_row
                    home_pods_active = True
                    home_ep_number += 1

            home_view = ft.View("/", [
                        pypods_appbar,
                        top_bar,
                        *[home_ep_row_dict.get(f'search_row{i+1}') for i in range(len(home_ep_rows))]
                    ]
                )
            home_view.bgcolor = active_user.bgcolor
            home_view.scroll = ft.ScrollMode.AUTO
            page.views.append(
                    home_view
            )

#---Flet Various Elements----------------------------------------------------------------
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
    # username = 'placeholder'




#---Code for Theme Change----------------------------------------------------------------


    def change_theme(e):
        """
        When the button(to change theme) is clicked, the progress bar is made visible, the theme is changed,
        the progress bar is made invisible, and the page is updated

        :param e: The event that triggered the function
        """
        page.splash.visible = True
        page.theme_mode = "light" if page.theme_mode == "dark" else "dark"
        # page.splash.visible = False
        theme_icon_button.selected = not theme_icon_button.selected
        time.sleep(.3)
        page.update()

#--Defining Routes---------------------------------------------------

    def start_login(page):
        page.go("/login")

    def view_pop(e):
        page.views.pop()
        top_view = page.views[-1]
        page.go(top_view.route)

    def open_search(e):
        pr = ft.ProgressRing()
        page.overlay.append(ft.Stack([pr], bottom=25, right=30, left=20, expand=True))
        page.update()
        page.go("/searchpod")

    def open_poddisplay(e):
        pr = ft.ProgressRing()
        page.overlay.append(ft.Stack([pr], bottom=25, right=30, left=20, expand=True))
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

    def open_episode_select(page, url, title):
        current_episode.url = url
        current_episode.title = title
        page.go("/episode_display")

    def open_pod_list(e):
        pr = ft.ProgressRing()
        page.overlay.append(ft.Stack([pr], bottom=25, right=30, left=20, expand=True))
        page.update()
        page.go("/pod_list")

    def go_homelogin_guest(page):
        active_user.user_id = 1
        active_user.fullname = 'Guest User'
        # navbar.visible = True
        active_user.theme_select()
        # Theme user elements
        pypods_appbar.bgcolor = active_user.main_color
        pypods_appbar.color = active_user.accent_color
        refresh_btn.icon_color = active_user.font_color
        banner_button.bgcolor = active_user.accent_color
        banner_button.color = active_user.main_color
        page.banner.bgcolor = active_user.accent_color
        page.banner.leading = ft.Icon(ft.icons.WAVING_HAND, color=active_user.main_color, size=40)
        page.banner.content = ft.Text("""
    Welcome to PinePods! PinePods is an app built to save, listen, download, organize, and manage a selection of podcasts. Using the search function you can search for your favorite podcast, from there, click the add button to save your podcast to the database. PinePods will begin displaying new episodes of that podcast from then on to the homescreen when released. In addition, from search you can click on a podcast to view and listen to specific episodes. From the sidebar you can select your saved podcasts and manage them, view and manage your downloaded podcasts, edit app settings, check your listening history, and listen through episodes from your saved 'queue.' For comments, feature requests, pull requests, and bug reports please open an issue, for fork PinePods from the repository:
    """, color=active_user.main_color
        )
        page.banner.actions = [
            ft.ElevatedButton('Open PinePods Repo', on_click=open_repo, bgcolor=active_user.main_color, color=active_user.accent_color),
            ft.IconButton(icon=ft.icons.EXIT_TO_APP, on_click=close_banner, bgcolor=active_user.main_color)
        ]
        search_pods.color = active_user.accent_color
        search_pods.focused_bgcolor = active_user.accent_color
        search_pods.focused_border_color = active_user.accent_color
        search_pods.focused_color = active_user.accent_color
        search_pods.focused_color = active_user.accent_color
        search_pods.cursor_color = active_user.accent_color
        search_btn.bgcolor = active_user.accent_color
        search_btn.color = active_user.main_color
        navbar = NavBar(page).create_navbar()
        navbar.border = ft.border.only(right=ft.border.BorderSide(2, active_user.tertiary_color))
        active_user.navbar_stack = ft.Stack([navbar], expand=True)
        page.overlay.append(active_user.navbar_stack)
        page.update()
        page.go("/")

    def go_homelogin(page):
        # navbar.visible = True
        active_user.theme_select()
        # Theme user elements
        pypods_appbar.bgcolor = active_user.main_color
        pypods_appbar.color = active_user.accent_color
        refresh_btn.icon_color = active_user.font_color
        banner_button.bgcolor = active_user.accent_color
        banner_button.color = active_user.main_color
        page.banner.bgcolor = active_user.accent_color
        page.banner.leading = ft.Icon(ft.icons.WAVING_HAND, color=active_user.main_color, size=40)
        page.banner.content = ft.Text("""
    Welcome to PinePods! PinePods is an app built to save, listen, download, organize, and manage a selection of podcasts. Using the search function you can search for your favorite podcast, from there, click the add button to save your podcast to the database. PinePods will begin displaying new episodes of that podcast from then on to the homescreen when released. In addition, from search you can click on a podcast to view and listen to specific episodes. From the sidebar you can select your saved podcasts and manage them, view and manage your downloaded podcasts, edit app settings, check your listening history, and listen through episodes from your saved 'queue.' For comments, feature requests, pull requests, and bug reports please open an issue, for fork PinePods from the repository:
    """, color=active_user.main_color
        )
        page.banner.actions = [
            ft.ElevatedButton('Open PinePods Repo', on_click=open_repo, bgcolor=active_user.main_color, color=active_user.accent_color),
            ft.IconButton(icon=ft.icons.EXIT_TO_APP, on_click=close_banner, bgcolor=active_user.main_color)
        ]
        search_pods.color = active_user.accent_color
        search_pods.focused_bgcolor = active_user.accent_color
        search_pods.focused_border_color = active_user.accent_color
        search_pods.focused_color = active_user.accent_color
        search_pods.focused_color = active_user.accent_color
        search_pods.cursor_color = active_user.accent_color
        search_btn.bgcolor = active_user.accent_color
        search_btn.color = active_user.main_color
        navbar = NavBar(page).create_navbar()
        navbar.border = ft.border.only(right=ft.border.BorderSide(2, active_user.tertiary_color))
        active_user.navbar_stack = ft.Stack([navbar], expand=True)
        page.overlay.append(active_user.navbar_stack)
        page.update()
        page.go("/")

    def go_theme_rebuild(page):
        # navbar.visible = True
        active_user.theme_select()
        # Theme user elements
        pypods_appbar.bgcolor = active_user.main_color
        pypods_appbar.color = active_user.accent_color
        refresh_btn.icon_color = active_user.font_color
        banner_button.bgcolor = active_user.accent_color
        banner_button.color = active_user.main_color
        page.banner.bgcolor = active_user.accent_color
        page.banner.leading = ft.Icon(ft.icons.WAVING_HAND, color=active_user.main_color, size=40)
        page.banner.content = ft.Text("""
    Welcome to PinePods! PinePods is an app built to save, listen, download, organize, and manage a selection of podcasts. Using the search function you can search for your favorite podcast, from there, click the add button to save your podcast to the database. PinePods will begin displaying new episodes of that podcast from then on to the homescreen when released. In addition, from search you can click on a podcast to view and listen to specific episodes. From the sidebar you can select your saved podcasts and manage them, view and manage your downloaded podcasts, edit app settings, check your listening history, and listen through episodes from your saved 'queue.' For comments, feature requests, pull requests, and bug reports please open an issue, for fork PinePods from the repository:
    """, color=active_user.main_color
        )
        page.banner.actions = [
            ft.ElevatedButton('Open PinePods Repo', on_click=open_repo, bgcolor=active_user.main_color, color=active_user.accent_color),
            ft.IconButton(icon=ft.icons.EXIT_TO_APP, on_click=close_banner, bgcolor=active_user.main_color)
        ]
        search_pods.color = active_user.accent_color
        search_pods.focused_bgcolor = active_user.accent_color
        search_pods.focused_border_color = active_user.accent_color
        search_pods.focused_color = active_user.accent_color
        search_pods.focused_color = active_user.accent_color
        search_pods.cursor_color = active_user.accent_color
        search_btn.bgcolor = active_user.accent_color
        search_btn.color = active_user.main_color
        audio_container.bgcolor = active_user.main_color
        audio_scrubber.active_color = active_user.nav_color2
        audio_scrubber.inactive_color = active_user.nav_color2
        audio_scrubber.thumb_color = active_user.accent_color
        play_button.icon_color = active_user.accent_color
        pause_button.icon_color = active_user.accent_color
        seek_button.icon_color = active_user.accent_color
        currently_playing.color = active_user.font_color
        current_time.color = active_user.font_color
        podcast_length.color = active_user.font_color

        navbar = NavBar(page).create_navbar()
        navbar.border = ft.border.only(right=ft.border.BorderSide(2, active_user.tertiary_color))
        active_user.navbar_stack = ft.Stack([navbar], expand=True)
        page.overlay.append(active_user.navbar_stack)
        page.update()
        page.go("/")

    def go_home(e):
        page.update()
        page.go("/")

    def route_change(e):

        # page.views.clear()
        if page.route == "/" or page.route == "/":
            page.bgcolor = colors.BLUE_GREY

            # Home Screen Podcast Layout (Episodes in Newest order)

            home_episodes = database_functions.functions.return_episodes(cnx, active_user.user_id)

            if home_episodes is None:
                home_ep_number = 1
                home_ep_rows = []
                home_ep_row_dict = {}

                home_pod_name = "No Podcasts added yet"
                home_ep_title = "Podcasts you add will display new episodes here."
                home_pub_date = ""
                home_ep_desc = "You can search podcasts in the upper right. Then click the plus button to add podcasts to the add. Click around on the navbar to manage podcasts you've added. Enjoy the listening!"
                home_ep_url = ""
                home_entry_title = ft.Text(f'{home_pod_name} - {home_ep_title}', width=600, style=ft.TextThemeStyle.TITLE_MEDIUM)
                home_entry_description = ft.Text(home_ep_desc, width=800)
                home_entry_audio_url = ft.Text(home_ep_url)
                home_entry_released = ft.Text(home_pub_date)
                home_artwork_no = random.randint(1, 12)
                home_artwork_url = os.path.join(script_dir, "images", "logo_random", f"{home_artwork_no}.jpeg")
                home_art_url_parsed = check_image(home_artwork_url)
                home_entry_artwork_url = ft.Image(src=home_art_url_parsed, width=150, height=150)
                home_ep_play_button = ft.IconButton(
                    icon=ft.icons.PLAY_DISABLED,
                    icon_color=active_user.accent_color,
                    icon_size=40,
                    tooltip="No Episodes Listened to yet"
                )
                # Creating column and row for home layout
                home_ep_column = ft.Column(
                    controls=[home_entry_title, home_entry_description, home_entry_released]
                )

                home_ep_row_content = ft.ResponsiveRow([
                    ft.Column(col={"md": 2}, controls=[home_entry_artwork_url]),
                    ft.Column(col={"md": 10}, controls=[home_ep_column, home_ep_play_button]),
                ])
                home_ep_row = ft.Container(content=home_ep_row_content)
                home_ep_row.padding=padding.only(left=70, right=50)
                home_ep_rows.append(home_ep_row)
                home_ep_row_dict[f'search_row{home_ep_number}'] = home_ep_row
                home_pods_active = True
                home_ep_number += 1
            else:
                home_ep_number = 1
                home_ep_rows = []
                home_ep_row_dict = {}

                for entry in home_episodes:
                    home_ep_title = entry['EpisodeTitle']
                    home_pod_name = entry['PodcastName']
                    home_pub_date = entry['EpisodePubDate']
                    home_ep_desc = entry['EpisodeDescription']
                    home_ep_artwork = entry['EpisodeArtwork']
                    home_ep_url = entry['EpisodeURL']
                    home_ep_duration = entry['EpisodeDuration']
                    # do something with the episode information
                    home_entry_title_button = ft.Text(f'{home_pod_name} - {home_ep_title}', style=ft.TextThemeStyle.TITLE_MEDIUM, color=active_user.font_color)
                    home_entry_title = ft.TextButton(content=home_entry_title_button, on_click=lambda x, url=home_ep_url, title=home_ep_title: open_episode_select(page, url, title))
                    home_entry_row = ft.ResponsiveRow([
    ft.Column(col={"sm": 6}, controls=[home_entry_title]),
])

                    num_lines = home_ep_desc.count('\n')
                    if num_lines > 15:
                        if is_html(home_ep_desc):
                            # convert HTML to Markdown
                            markdown_desc = html2text.html2text(home_ep_desc)
                            if num_lines > 15:
                                # Split into lines, truncate to 15 lines, and join back into a string
                                lines = markdown_desc.splitlines()[:15]
                                markdown_desc = '\n'.join(lines)
                            # add inline style to change font color                            
                            home_entry_description = ft.Markdown(markdown_desc, on_tap_link=launch_clicked_url)
                            home_entry_seemore = ft.TextButton(text="See More...", on_click=lambda x, url=home_ep_url, title=home_ep_title: open_episode_select(page, url, title))
                        else:
                            if num_lines > 15:
                                # Split into lines, truncate to 15 lines, and join back into a string
                                lines = home_ep_desc.splitlines()[:15]
                                home_ep_desc = '\n'.join(lines)
                            # display plain text
                            home_entry_description = ft.Text(home_ep_desc)

                    else:
                        if is_html(home_ep_desc):
                            # convert HTML to Markdown
                            markdown_desc = html2text.html2text(home_ep_desc)
                            # add inline style to change font color
                            home_entry_description = ft.Markdown(markdown_desc, on_tap_link=launch_clicked_url)
                        else:
                            # display plain text
                            markdown_desc = home_ep_desc
                            home_entry_description = ft.Text(home_ep_desc)

                    home_entry_audio_url = ft.Text(home_ep_url, color=active_user.font_color)
                    check_episode_playback, listen_duration = database_functions.functions.check_episode_playback(cnx, active_user.user_id, home_ep_title, home_ep_url)
                    home_entry_released = ft.Text(f'Released on: {home_pub_date}', color=active_user.font_color)

                    home_art_no = random.randint(1, 12)
                    home_art_fallback = os.path.join(script_dir, "images", "logo_random", f"{home_art_no}.jpeg")
                    home_art_url = home_ep_artwork if home_ep_artwork else home_art_fallback
                    home_art_parsed = check_image(home_art_url)
                    home_entry_artwork_url = ft.Image(src=home_art_parsed, width=150, height=150)
                    home_ep_play_button = ft.IconButton(
                        icon=ft.icons.NOT_STARTED,
                        icon_color=active_user.accent_color,
                        icon_size=40,
                        tooltip="Play Episode",
                        on_click=lambda x, url=home_ep_url, title=home_ep_title, artwork=home_ep_artwork: play_selected_episode(url, title, artwork)
                    )
                    home_ep_resume_button = ft.IconButton(
                        icon=ft.icons.PLAY_CIRCLE,
                        icon_color=active_user.accent_color,
                        icon_size=40,
                        tooltip="Resume Episode",
                        on_click=lambda x, url=home_ep_url, title=home_ep_title, artwork=home_ep_artwork, listen_duration=listen_duration: resume_selected_episode(url, title, artwork, listen_duration)
                    )
                    home_popup_button = ft.PopupMenuButton(content=ft.Icon(ft.icons.ARROW_DROP_DOWN_CIRCLE_ROUNDED, color=active_user.accent_color, size=40, tooltip="Play Episode"), 
                        items=[
                            ft.PopupMenuItem(icon=ft.icons.QUEUE, text="Queue", on_click=lambda x, url=home_ep_url, title=home_ep_title, artwork=home_ep_artwork: queue_selected_episode(url, title, artwork, page)),
                            ft.PopupMenuItem(icon=ft.icons.DOWNLOAD, text="Download", on_click=lambda x, url=home_ep_url, title=home_ep_title: download_selected_episode(url, title, page)),
                            ft.PopupMenuItem(icon=ft.icons.SAVE, text="Save Episode", on_click=lambda x, url=home_ep_url, title=home_ep_title: save_selected_episode(url, title, page))
                        ]
                    )
                    if check_episode_playback == True:
                        listen_prog = seconds_to_time(listen_duration)
                        home_ep_prog = seconds_to_time(home_ep_duration)
                        progress_value = get_progress(listen_duration, home_ep_duration)
                        home_entry_progress = ft.Row(controls=[ft.Text(listen_prog, color=active_user.font_color), ft.ProgressBar(expand=True, value=progress_value, color=active_user.main_color), ft.Text(home_ep_prog, color=active_user.font_color)])
                        if num_lines > 15:
                            home_ep_row_content = ft.ResponsiveRow([
                                ft.Column(col={"md": 2}, controls=[home_entry_artwork_url]),
                                ft.Column(col={"md": 10}, controls=[home_entry_title, home_entry_description, home_entry_seemore, home_entry_released, home_entry_progress, ft.Row(controls=[home_ep_play_button, home_ep_resume_button, home_popup_button])]),
                            ])
                        else:
                            home_ep_row_content = ft.ResponsiveRow([
                                ft.Column(col={"md": 2}, controls=[home_entry_artwork_url]),
                                ft.Column(col={"md": 10}, controls=[home_entry_title, home_entry_description, home_entry_released, home_entry_progress, ft.Row(controls=[home_ep_play_button, home_ep_resume_button, home_popup_button])]),
                            ]) 
                    else:
                        if num_lines > 15:
                            home_ep_row_content = ft.ResponsiveRow([
                                ft.Column(col={"md": 2}, controls=[home_entry_artwork_url]),
                                ft.Column(col={"md": 10}, controls=[home_entry_title, home_entry_description, home_entry_seemore, home_entry_released, ft.Row(controls=[home_ep_play_button, home_popup_button])]),
                            ])
                        else:
                            home_ep_row_content = ft.ResponsiveRow([
                                ft.Column(col={"md": 2}, controls=[home_entry_artwork_url]),
                                ft.Column(col={"md": 10}, controls=[home_entry_title, home_entry_description, home_entry_released, ft.Row(controls=[home_ep_play_button, home_popup_button])]),
                            ]) 
                    home_div_row = ft.Divider(color=active_user.accent_color)
                    home_ep_column = ft.Column(controls=[home_ep_row_content, home_div_row])
                    home_ep_row = ft.Container(content=home_ep_column)
                    home_ep_row.padding=padding.only(left=70, right=50)
                    home_ep_rows.append(home_ep_row)
                    # home_ep_rows.append(ft.Text('test'))
                    home_ep_row_dict[f'search_row{home_ep_number}'] = home_ep_row
                    home_pods_active = True
                    home_ep_number += 1

            home_view = ft.View("/", [
                        pypods_appbar,
                        top_bar,
                        *[home_ep_row_dict.get(f'search_row{i+1}') for i in range(len(home_ep_rows))]
                    ]
                )
            home_view.bgcolor = active_user.bgcolor
            home_view.scroll = ft.ScrollMode.AUTO
            page.views.append(
                    home_view
            )

        if page.route == "/userstats" or page.route == "/userstats":

            user_stats = database_functions.functions.get_stats(cnx, active_user.user_id)

            stats_created_date = user_stats['UserCreated']
            stats_pods_played = user_stats['PodcastsPlayed']
            stats_time_listened = user_stats['TimeListened']
            stats_pods_added = user_stats['PodcastsAdded']
            stats_eps_saved = user_stats['EpisodesSaved']
            stats_eps_downloaded = user_stats['EpisodesDownloaded']

            user_ep_count = database_functions.functions.get_user_episode_count(cnx, active_user.user_id)

            user_title = ft.Text(f"Stats for {active_user.fullname}:", size=16, weight="bold")
            date_display = ft.Text(f'{active_user.username} created on {stats_created_date}')
            pods_played_display = ft.Text(f'{stats_pods_played} Podcasts listened to')
            time_listened_display = ft.Text(f'{stats_time_listened} Minutes spent listening')
            pods_added_display = ft.Text(f'{stats_pods_added} Podcasts added')
            eps_added_display = ft.Text(f'{user_ep_count} Episodes associated with {active_user.fullname} in the database')
            eps_saved_display = ft.Text(f'{stats_eps_saved} Podcast episodes currently saved')
            eps_downloaded_display = ft.Text(f'{stats_eps_downloaded} Podcast episodes currently downloaded')
            stats_column = ft.Column(controls=[user_title, date_display, pods_played_display, time_listened_display, pods_added_display, eps_added_display, eps_saved_display, eps_downloaded_display])
            stats_container = ft.Container(content=stats_column)
            stats_container.padding=padding.only(left=70, right=50)


            stats_view = ft.View("/userstats",
                    [
                        pypods_appbar,
                        stats_container
                    ]
                    
                )
            stats_view.bgcolor = active_user.bgcolor
            stats_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                stats_view
                
            )

        if page.route == "/login" or page.route == "/login":
            guest_enabled = database_functions.functions.guest_status(cnx)
            if guest_enabled == True:
                login_startpage = Column(
                    alignment=ft.MainAxisAlignment.CENTER,
                    horizontal_alignment=ft.CrossAxisAlignment.CENTER,
                    controls=[
                        Card(
                            elevation=15,
                            content=Container(
                                width=550,
                                height=580,
                                padding=padding.all(30),
                                gradient=GradientGenerator(
                                    "#2f2937", "#251867"
                                ),
                                border_radius=border_radius.all(12),
                                content=Column(
                                    horizontal_alignment="center",
                                    alignment="start",
                                    controls=[
                                        Text(
                                            "PinePods",
                                            size=32,
                                            weight="w700",
                                            text_align="center",
                                        ),
                                        Text(
                                            "A Forest of Podcasts, Rooted in the Spirit of Self-Hosting",
                                            size=22,
                                            weight="w700",
                                            text_align="center",
                                        ),
                                        Text(
                                            "Please login with your user account to start listening to podcasts. If you didn't set a default user up please check the docker logs for a default account and credentials",
                                            size=14,
                                            weight="w700",
                                            text_align="center",
                                            color="#64748b",
                                        ),
                                        Container(
                                            padding=padding.only(bottom=20)
                                        ),
                                        login_username,
                                        Container(
                                            padding=padding.only(bottom=10)
                                        ),
                                        login_password,
                                        Container(
                                            padding=padding.only(bottom=20)
                                        ),
                                        Row(
                                            alignment="center",
                                            spacing=20,
                                            controls=[
                                                FilledButton(
                                                    content=Text(
                                                        "Login",
                                                        weight="w700",
                                                    ),
                                                    width=160,
                                                    height=40,
                                                    # Now, if we want to login, we also need to send some info back to the server and check if the credentials are correct or if they even exists.
                                                    on_click=lambda e: active_user.login(login_username, login_password)
                                                    # on_click=lambda e: go_homelogin(e)
                                                ),
                                                FilledButton(
                                                    content=Text(
                                                        "Guest Login",
                                                        weight="w700",
                                                    ),
                                                    width=160,
                                                    height=40,
                                                    # Now, if we want to login, we also need to send some info back to the server and check if the credentials are correct or if they even exists.
                                                    on_click = lambda e: go_homelogin_guest(page)
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

                                        )
                                    ],
                                ),
                            ),
                        )
                    ],
                )
            else:
                login_startpage = Column(
                alignment=ft.MainAxisAlignment.CENTER,
                horizontal_alignment=ft.CrossAxisAlignment.CENTER,
                controls=[
                    Card(
                        elevation=15,
                        content=Container(
                            width=550,
                            height=580,
                            padding=padding.all(30),
                            gradient=GradientGenerator(
                                "#2f2937", "#251867"
                            ),
                            border_radius=border_radius.all(12),
                            content=Column(
                                horizontal_alignment="center",
                                alignment="start",
                                controls=[
                                    Text(
                                        "PinePods",
                                        size=32,
                                        weight="w700",
                                        text_align="center",
                                    ),
                                    Text(
                                        "A Forest of Podcasts, Rooted in the Spirit of Self-Hosting",
                                        size=22,
                                        weight="w700",
                                        text_align="center",
                                    ),
                                    Text(
                                        "Please login with your user account to start listening to podcasts. If you didn't set a default user up please check the docker logs for a default account and credentials",
                                        size=14,
                                        weight="w700",
                                        text_align="center",
                                        color="#64748b",
                                    ),
                                    Container(
                                        padding=padding.only(bottom=20)
                                    ),
                                    login_username,
                                    Container(
                                        padding=padding.only(bottom=10)
                                    ),
                                    login_password,
                                    Container(
                                        padding=padding.only(bottom=20)
                                    ),
                                    Row(
                                        alignment="center",
                                        spacing=20,
                                        controls=[
                                            FilledButton(
                                                content=Text(
                                                    "Login",
                                                    weight="w700",
                                                ),
                                                width=160,
                                                height=40,
                                                # Now, if we want to login, we also need to send some info back to the server and check if the credentials are correct or if they even exists.
                                                on_click=lambda e: active_user.login(login_username, login_password)
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

        if page.route == "/searchpod" or page.route == "/searchpod":
            # Get Pod info
            podcast_value = search_pods.value
            search_results = internal_functions.functions.searchpod(podcast_value)
            return_results = search_results['feeds']
            page.overlay.pop(2)

            # Get and format list
            pod_number = 1
            search_rows = []
            search_row_dict = {}
            for d in return_results:
                for k, v in d.items():
                    if k == 'title':
                        # Parse webpages needed to extract podcast artwork
                        search_art_no = random.randint(1, 12)
                        search_art_fallback = os.path.join(script_dir, "images", "logo_random", f"{search_art_no}.jpeg")
                        search_art_url = d['artwork'] if d['artwork'] else search_art_fallback
                        podimage_parsed = check_image(search_art_url)
                        pod_image = ft.Image(src=podimage_parsed, width=150, height=150)
                        
                        # Defining the attributes of each podcast that will be displayed on screen
                        pod_title = ft.TextButton(
                            text=d['title'],
                            on_click=lambda x, d=d: (evaluate_podcast(d['title'], d['artwork'], d['author'], d['categories'], d['description'], d['episodeCount'], d['url'], d['link']), open_poddisplay(e))
                        )
                        pod_desc = ft.Text(d['description'])
                        # Episode Count and subtitle
                        pod_ep_title = ft.Text('Episode Count:', weight=ft.FontWeight.BOLD, color=active_user.font_color)
                        pod_ep_count = ft.Text(d['episodeCount'], color=active_user.font_color)
                        pod_ep_info = ft.Row(controls=[pod_ep_title, pod_ep_count])
                        add_pod_button = ft.IconButton(
                            icon=ft.icons.ADD_BOX,
                            icon_color=active_user.accent_color,
                            icon_size=40,
                            tooltip="Add Podcast",
                            on_click=lambda x, d=d: send_podcast(d['title'], d['artwork'], d['author'], d['categories'], d['description'], d['episodeCount'], d['url'], d['link'])
                        )
                        # Creating column and row for search layout
                        search_column = ft.Column(
                            controls=[pod_title, pod_desc, pod_ep_info]
                        )
                        search_row_content = ft.ResponsiveRow([
                            ft.Column(col={"md": 2}, controls=[pod_image]),
                            ft.Column(col={"md": 10}, controls=[search_column, add_pod_button]),
                        ])
                        search_row = ft.Container(content=search_row_content)
                        search_row.padding=padding.only(left=70, right=50)
                        search_rows.append(search_row)
                        search_row_dict[f'search_row{pod_number}'] = search_row
                        pod_number += 1
            # Create search view object
            search_view = ft.View("/searchpod",
                    [
                        pypods_appbar,
                        *[search_row_dict[f'search_row{i+1}'] for i in range(len(search_rows))]
                    ]
                    
                )
            search_view.bgcolor = active_user.bgcolor
            search_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                search_view
                
            )

        if page.route == "/settings" or page.route == "/settings":

            # User Settings
            user_setting = ft.Text(
            "Personal Settings:", color=active_user.font_color,
            size=30,
            font_family="RobotoSlab",
            weight=ft.FontWeight.W_300,
        )
            user_setting_text = ft.Container(content=user_setting)
            user_setting_text.padding=padding.only(left=70, right=50)

            # Theme Select Elements
            theme_text = ft.Text('Select Custom Theme:', color=active_user.font_color)
            theme_drop = ft.Dropdown(border_color=active_user.accent_color, color=active_user.font_color, focused_bgcolor=active_user.main_color, focused_border_color=active_user.accent_color, focused_color=active_user.accent_color, 
             options=[
                ft.dropdown.Option("light"),
                ft.dropdown.Option("dark"),
                ft.dropdown.Option("nordic"),
                ft.dropdown.Option("abyss"),
                ft.dropdown.Option("dracula"),
                ft.dropdown.Option("kimbie"),
                ft.dropdown.Option("greenie meanie"),
                ft.dropdown.Option("neon"),
                ft.dropdown.Option("greenie meanie"),
                ft.dropdown.Option("wilderberries"),
                ft.dropdown.Option("hotdogstand - MY EYES"),
             ]
             )
            theme_submit = ft.ElevatedButton("Submit", bgcolor=active_user.main_color, color=active_user.accent_color, on_click=lambda event: active_user.set_theme(theme_drop.value))
            theme_column = ft.Column(controls=[theme_text, theme_drop, theme_submit])
            theme_row = ft.Row(
                            vertical_alignment=ft.CrossAxisAlignment.START,
                            alignment=ft.MainAxisAlignment.START,
                            controls=[theme_column])
            theme_row_container = ft.Container(content=theme_row)
            theme_row_container.padding = padding.only(left=70, right=50)

            # Admin Only Settings

            admin_setting = ft.Text(
            "Administration Settings:", color = active_user.font_color,
            size=30,
            font_family="RobotoSlab",
            weight=ft.FontWeight.W_300,
        )
            admin_setting_text = ft.Container(content=admin_setting)
            admin_setting_text.padding=padding.only(left=70, right=50)

            # New User Creation Elements
            new_user = User(page)
            user_text = Text('Enter New User Information:', color=active_user.font_color)
            user_name = ft.TextField(label="Full Name", icon=ft.icons.CARD_MEMBERSHIP, hint_text='John PinePods', border_color=active_user.accent_color, color=active_user.accent_color, focused_bgcolor=active_user.accent_color, focused_color=active_user.accent_color, focused_border_color=active_user.accent_color, cursor_color=active_user.accent_color )
            user_email = ft.TextField(label="Email", icon=ft.icons.EMAIL, hint_text='ilovepinepods@pinepods.com', border_color=active_user.accent_color, color=active_user.accent_color, focused_bgcolor=active_user.accent_color, focused_color=active_user.accent_color, focused_border_color=active_user.accent_color, cursor_color=active_user.accent_color )
            user_username = ft.TextField(label="Username", icon=ft.icons.PERSON, hint_text='pinepods_user1999', border_color=active_user.accent_color, color=active_user.accent_color, focused_bgcolor=active_user.accent_color, focused_color=active_user.accent_color, focused_border_color=active_user.accent_color, cursor_color=active_user.accent_color )
            user_password = ft.TextField(label="password", icon=ft.icons.PASSWORD, password=True, can_reveal_password=True, hint_text='mY_SuPeR_S3CrEt!', border_color=active_user.accent_color, color=active_user.accent_color, focused_bgcolor=active_user.accent_color, focused_color=active_user.accent_color, focused_border_color=active_user.accent_color, cursor_color=active_user.accent_color )
            user_submit = ft.ElevatedButton(text="Submit!", bgcolor=active_user.main_color, color=active_user.accent_color, on_click=lambda x: (
                new_user.set_username(user_username.value), 
                new_user.set_password(user_password.value), 
                new_user.set_email(user_email.value),
                new_user.set_name(user_name.value),
                new_user.verify_user_values(),
                # new_user.popup_user_values(e),
                new_user.create_user(), 
                new_user.user_created_prompt()))
            user_column = ft.Column(
                            controls=[user_text, user_name, user_email, user_username, user_password, user_submit]
                        )
            user_row = ft.Row(
                            vertical_alignment=ft.CrossAxisAlignment.START,
                            alignment=ft.MainAxisAlignment.START,
                            controls=[user_column])
            user_row_container = ft.Container(content=user_row)
            user_row_container.padding=padding.only(left=70, right=50)
            #User Table Setup - Admin only
            edit_user_text = ft.Text('Modify existing Users (Select a user to modify properties):', color=active_user.font_color)

            user_information = database_functions.functions.get_user_info(cnx)
            user_table_rows = []

            for entry in user_information:
                user_id = entry['UserID']
                fullname = entry['Fullname']
                username = entry['Username']
                email = entry['Email']
                is_admin_numeric = entry['IsAdmin']
                if is_admin_numeric == 1:
                    is_admin = 'yes'
                else: is_admin = 'no'

                
                # Create a new data row with the user information
                row = ft.DataRow(
                    cells=[
                        ft.DataCell(ft.Text(user_id)),
                        ft.DataCell(ft.Text(fullname)),
                        ft.DataCell(ft.Text(username)),
                        ft.DataCell(ft.Text(email)),
                        ft.DataCell(ft.Text(str(is_admin))),
                    ],
                    on_select_changed=(lambda username_copy, is_admin_numeric_copy, fullname_copy, email_copy, user_id_copy: 
                        lambda x: modify_user.open_edit_user(username_copy, is_admin_numeric_copy, fullname_copy, email_copy, user_id_copy)
                    )(username, is_admin_numeric, fullname, email, user_id)
                )
                
                # Append the row to the list of data rows
                user_table_rows.append(row)

            user_table = ft.DataTable(
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
                ft.DataColumn(ft.Text("User ID"), numeric=True),
                ft.DataColumn(ft.Text("Fullname")),
                ft.DataColumn(ft.Text("Username")),
                ft.DataColumn(ft.Text("Email")),
                ft.DataColumn(ft.Text("Admin User"))
            ],
                rows=user_table_rows
                )
            user_edit_column = ft.Column(controls=[edit_user_text, user_table])
            user_edit_container = ft.Container(content=user_edit_column)
            user_edit_container.padding=padding.only(left=70, right=50)

            # Guest User Settings 
            guest_status_bool = database_functions.functions.guest_status(cnx)
            if guest_status_bool == True:
                guest_status = 'enabled'
            else:
                guest_status = 'disabled'
            disable_guest_text = ft.Text('Guest User Settings (Disabling is highly recommended if PinePods is exposed to the internet):', color=active_user.font_color)
            disable_guest_notify = ft.Text(f'Guest user is currently {guest_status}')
            if guest_status_bool == True:
                guest_info_button = ft.ElevatedButton(f'Disable Guest User', on_click=guest_user_change, bgcolor=active_user.main_color, color=active_user.accent_color)
            else:
                guest_info_button = ft.ElevatedButton(f'Enable Guest User', on_click=guest_user_change, bgcolor=active_user.main_color, color=active_user.accent_color)

            guest_info_col = ft.Column(controls=[disable_guest_text, disable_guest_notify, guest_info_button])
            guest_info = ft.Container(content=guest_info_col)
            guest_info.padding=padding.only(left=70, right=50)

            # User Self Service Creation
            self_service_bool = database_functions.functions.self_service_status(cnx)
            if self_service_bool == True:
                self_service_status = 'enabled'
            else:
                self_service_status = 'disabled'
            self_service_text = ft.Text('Self Service Settings (Disabling is highly recommended if PinePods is exposed to the internet):', color=active_user.font_color)
            self_service_notify = ft.Text(f'Self Service user creation is currently {self_service_status}')
            if self_service_bool == True:
                self_service_button = ft.ElevatedButton(f'Disable Self Service User Creation', on_click=self_service_change, bgcolor=active_user.main_color, color=active_user.accent_color)
            else:
                self_service_button = ft.ElevatedButton(f'Enable Self Service User Creation', on_click=self_service_change, bgcolor=active_user.main_color, color=active_user.accent_color)

            self_service_info_col = ft.Column(controls=[self_service_text, self_service_notify, self_service_button])
            self_service_info = ft.Container(content=self_service_info_col)
            self_service_info.padding=padding.only(left=70, right=50)

            # Check if admin settings should be displayed 
            user_is_admin = database_functions.functions.user_admin_check(cnx, int(active_user.user_id))
            if user_is_admin == True:
                pass
            else:
                admin_setting_text.visible = False
                user_row_container.visible = False
                user_edit_container.visible = False
                guest_info.visible = False
                self_service_info.visible = False

            # Create search view object
            settings_view = ft.View("/settings",
                    [
                        pypods_appbar,
                        user_setting_text,
                        theme_row_container,
                        admin_setting_text,
                        user_row_container,
                        user_edit_container,
                        guest_info,
                        self_service_info
                    ]
                    
                )
            settings_view.bgcolor = active_user.bgcolor
            settings_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                settings_view
                    
                )

        if page.route == "/poddisplay" or page.route == "/poddisplay":
            # Check if podcast is already in database for user
            podcast_status = database_functions.functions.check_podcast(cnx, active_user.user_id, clicked_podcast.name)
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
                on_click=lambda x: send_podcast(clicked_podcast.name, clicked_podcast.artwork, clicked_podcast.author, clicked_podcast.categories, clicked_podcast.description, clicked_podcast.episode_count, clicked_podcast.feedurl, clicked_podcast.website)
            )
            pod_feed_remove_button = ft.IconButton(
                icon=ft.icons.INDETERMINATE_CHECK_BOX,
                icon_color="red400",
                icon_size=40,
                tooltip="Remove Podcast",
                on_click=lambda x, title=clicked_podcast.name: database_functions.functions.remove_podcast(cnx, title, active_user.user_id)
            )
            if podcast_status == True:
                feed_row_content = ft.ResponsiveRow([
                ft.Column(col={"md": 4}, controls=[pod_image]),
                ft.Column(col={"md": 7}, controls=[pod_feed_title, pod_feed_desc, pod_feed_site]),
                ft.Column(col={"md": 1}, controls=[pod_feed_remove_button]),
                ])
            else:
                feed_row_content = ft.ResponsiveRow([
                ft.Column(col={"md": 4}, controls=[pod_image]),
                ft.Column(col={"md": 7}, controls=[pod_feed_title, pod_feed_desc, pod_feed_site]),
                ft.Column(col={"md": 1}, controls=[pod_feed_add_button]),
                ])
            feed_row = ft.Container(content=feed_row_content)
            feed_row.padding=padding.only(left=70, right=50)

            # Episode Info
            # Run Function to get episode data
            ep_number = 1
            ep_rows = []
            ep_row_dict = {}

            episode_results = app_functions.functions.parse_feed(clicked_podcast.feedurl)

            for entry in episode_results.entries:
                if hasattr(entry, "title") and hasattr(entry, "summary") and hasattr(entry, "enclosures"):
                    # get the episode title
                    parsed_title = entry.title

                    # get the episode description
                    parsed_description = entry.summary

                    # get the URL of the audio file for the episode
                    if entry.enclosures:
                        parsed_audio_url = entry.enclosures[0].href
                    else:
                        parsed_audio_url = ""


                    # get the release date of the episode
                    parsed_release_date = entry.published

                    # get the URL of the episode artwork, or use the podcast image URL if not available
                    parsed_artwork_url = entry.get('itunes_image', {}).get('href', None) or entry.get('image', {}).get('href', None)
                    if parsed_artwork_url == None:
                        parsed_artwork_url = clicked_podcast.artwork
                    display_art_no = random.randint(1, 12)
                    display_art_fallback = os.path.join(script_dir, "images", "logo_random", f"{display_art_no}.jpeg")
                    display_art_url = parsed_artwork_url if parsed_artwork_url else display_art_fallback

                else:
                    print("Skipping entry without required attributes or enclosures")
                entry_title = ft.Text(f'{parsed_title}', style=ft.TextThemeStyle.TITLE_MEDIUM, color=active_user.font_color)
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
                if podcast_status == True:
                    ep_resume_button = ft.IconButton(
                        icon=ft.icons.NOT_STARTED,
                        icon_color=active_user.accent_color,
                        icon_size=40,
                        tooltip="Play Episode",
                        on_click=lambda x, url=entry_audio_url, title=entry_title, artwork=display_art_entry_parsed: play_selected_episode(url, title, artwork)
                    )
                    ep_popup_button = ft.PopupMenuButton(content=ft.Icon(ft.icons.ARROW_DROP_DOWN_CIRCLE_ROUNDED, color=active_user.accent_color, size=40, tooltip="Play Episode"), 
                        items=[
                            ft.PopupMenuItem(icon=ft.icons.QUEUE, text="Queue", on_click=lambda x, url=entry_audio_url, title=entry_title, artwork=display_art_entry_parsed: queue_selected_episode(url, title, artwork, page)),
                            ft.PopupMenuItem(icon=ft.icons.DOWNLOAD, text="Download", on_click=lambda x, url=entry_audio_url, title=entry_title: download_selected_episode(url, title, page)),
                            ft.PopupMenuItem(icon=ft.icons.SAVE, text="Save Episode", on_click=lambda x, url=entry_audio_url, title=entry_title: save_selected_episode(url, title, page))
                        ]
                    )
                    ep_controls_row = ft.Row(controls=[ep_resume_button, ep_popup_button])
                    ep_row_content = ft.ResponsiveRow([
                    ft.Column(col={"md": 2}, controls=[entry_artwork_url]),
                    ft.Column(col={"md": 8}, controls=[entry_title, entry_description, entry_released]),
                    ft.Column(col={"md": 2}, controls=[ep_controls_row])
                    ])
                else:
                    ep_row_content = ft.ResponsiveRow([
                        ft.Column(col={"md": 2}, controls=[entry_artwork_url]),
                        ft.Column(col={"md": 10}, controls=[entry_title, entry_description, entry_released]),
                        ])

                ep_row_list = ft.ListView(divider_thickness=3, auto_scroll=True)
                ep_row_list.controls.append(ep_row_content)
                div_row = ft.Divider(color=active_user.accent_color)
                ep_column = ft.Column(controls=[ep_row_list, div_row])
                ep_row = ft.Container(content=ep_column)

                ep_row.padding=padding.only(left=70, right=50)
                ep_rows.append(ep_row)
                # home_ep_rows.append(ft.Text('test'))
                ep_row_dict[f'search_row{ep_number}'] = ep_row
                pods_active = True
                ep_number += 1

            page.overlay.pop(2)
            # Create search view object
            pod_view = ft.View(
                    "/poddisplay",
                    [
                        pypods_appbar,
                        feed_row,
                        *[ep_row_dict[f'search_row{i+1}'] for i in range(len(ep_rows))]
                        
                    ]
                    
                )
            pod_view.bgcolor = active_user.bgcolor
            pod_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                    pod_view
        )
        if page.route == "/pod_list" or page.route == "/pod_list":

            # Get Pod info
            pod_list_data = database_functions.functions.return_pods(cnx, active_user.user_id)

            page.overlay.pop(2)

            # Get and format list
            pod_list_number = 1
            pod_list_rows = []
            pod_list_dict = {}

            def on_pod_list_title_click(e, title, artwork, author, categories, desc, ep_count, feed, website):
                evaluate_podcast(title, artwork, author, categories, desc, ep_count, feed, website)
                open_poddisplay(e)
        
            if pod_list_data is None:
                pod_list_title = 'No Podcasts added yet'
                artwork_no = random.randint(1, 12)
                pod_list_artwork = os.path.join(script_dir, "images", "logo_random", f"{artwork_no}.jpeg")
                pod_list_desc = "Looks like you haven't added any podcasts yet. Search for podcasts you enjoy in the upper right portion of the screen and click the plus button to add them. They will begin to show up here and new episodes will be put into the main feed. You'll also be able to start downloading and saving episodes. Enjoy the listening!"
                pod_list_ep_count = 'Start Searching!'
                pod_list_website = "https://github.com/madeofpendletonwool/pypods"
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
                # pod_list_row = ft.Row(
                #     alignment=ft.MainAxisAlignment.CENTER,
                #     controls=[pod_list_artwork_image, pod_list_column, remove_pod_button])
                pod_list_row_content = ft.ResponsiveRow([
                    ft.Column(col={"md": 2}, controls=[pod_list_artwork_image]),
                    ft.Column(col={"md": 10}, controls=[pod_list_column, remove_pod_button]),
                ])
                pod_list_row = ft.Container(content=pod_list_row_content)
                pod_list_row.padding=padding.only(left=70, right=50)
                pod_list_rows.append(pod_list_row)
                pod_list_dict[f'pod_list_row{pod_list_number}'] = pod_list_row
                # pod_list_number += 1

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
                        on_click=lambda x, e=e, title=pod_list_title, artwork=pod_list_artwork, author=pod_list_author, categories=pod_list_categories, desc=pod_list_desc, ep_count=pod_list_ep_count, feed=pod_list_feed, website=pod_list_website: on_pod_list_title_click(e, title, artwork, author, categories, desc, ep_count, feed, website)
                    )
                    pod_list_desc_display = ft.Text(pod_list_desc)
                    # Episode Count and subtitle
                    pod_list_ep_title = ft.Text('Episode Count:', weight=ft.FontWeight.BOLD, color=active_user.font_color)
                    pod_list_ep_count_display = ft.Text(pod_list_ep_count, color=active_user.font_color)
                    pod_list_ep_info = ft.Row(controls=[pod_list_ep_title, pod_list_ep_count_display])
                    remove_pod_button = ft.IconButton(
                        icon=ft.icons.INDETERMINATE_CHECK_BOX,
                        icon_color="red400",
                        icon_size=40,
                        tooltip="Remove Podcast",
                        on_click=lambda x, title=pod_list_title: database_functions.functions.remove_podcast(cnx, title, active_user.user_id)
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
                    pod_list_row.padding=padding.only(left=70, right=50)
                    pod_list_rows.append(pod_list_row)
                    pod_list_dict[f'pod_list_row{pod_list_number}'] = pod_list_row
                    pod_list_number += 1
            pod_view_title = ft.Text(
            "Added Podcasts:",
            size=30,
            font_family="RobotoSlab",
            color=active_user.font_color,
            weight=ft.FontWeight.W_300,
        )
            pod_view_row = ft.Row(controls=[pod_view_title], alignment=ft.MainAxisAlignment.CENTER)


            # Create search view object
            pod_list_view = ft.View("/pod_list",
                    [
                        pypods_appbar,
                        top_bar,
                        pod_view_row,
                        *[pod_list_dict[f'pod_list_row{i+1}'] for i in range(len(pod_list_rows))]

                    ]
                    
                )
            pod_list_view.bgcolor = active_user.bgcolor
            pod_list_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                pod_list_view
                    
                )

        if page.route == "/history" or page.route == "/history":

            # Get Pod info
            hist_episodes = database_functions.functions.user_history(cnx, active_user.user_id)
            hist_episodes.reverse()

            # page.overlay.pop(2)

            if hist_episodes is None:
                hist_ep_number = 1
                hist_ep_rows = []
                hist_ep_row_dict = {}

                hist_pod_name = "No Podcasts history yet"
                hist_ep_title = "Podcasts you add will display here after you listen to them."
                hist_pub_date = ""
                hist_ep_desc = "You can search podcasts in the upper right. Then click the plus button to add podcasts. Once you listen to episodes they will appear here."
                hist_ep_url = ""
                hist_entry_title = ft.Text(f'{hist_pod_name} - {hist_ep_title}', width=600, style=ft.TextThemeStyle.TITLE_MEDIUM)
                hist_entry_description = ft.Text(hist_ep_desc, width=800)
                hist_entry_audio_url = ft.Text(hist_ep_url)
                hist_entry_released = ft.Text(hist_pub_date)
                hist_artwork_no = random.randint(1, 12)
                hist_artwork_url = os.path.join(script_dir, "images", "logo_random", f"{hist_artwork_no}.jpeg")
                hist_art_url_parsed = check_image(hist_artwork_url)
                hist_entry_artwork_url = ft.Image(src=hist_art_url_parsed, width=150, height=150)
                hist_ep_play_button = ft.IconButton(
                    icon=ft.icons.PLAY_DISABLED,
                    icon_color=active_user.accent_color,
                    icon_size=40,
                    tooltip="No Episodes Listened to yet"
                )
                # Creating column and row for home layout
                hist_ep_column = ft.Column(
                    controls=[hist_entry_title, hist_entry_description, hist_entry_released]
                )

                hist_ep_row_content = ft.ResponsiveRow([
                    ft.Column(col={"md": 2}, controls=[hist_entry_artwork_url]),
                    ft.Column(col={"md": 10}, controls=[hist_ep_column, hist_ep_play_button]),
                ])
                hist_ep_row = ft.Container(content=hist_ep_row_content)
                hist_ep_row.padding=padding.only(left=70, right=50)
                hist_ep_rows.append(hist_ep_row)
                hist_ep_row_dict[f'search_row{hist_ep_number}'] = hist_ep_row
                hist_pods_active = True
                hist_ep_number += 1
            else:
                hist_ep_number = 1
                hist_ep_rows = []
                hist_ep_row_dict = {}

                for entry in hist_episodes:
                    hist_ep_title = entry['EpisodeTitle']
                    hist_pod_name = entry['PodcastName']
                    hist_pub_date = entry['EpisodePubDate']
                    hist_ep_desc = entry['EpisodeDescription']
                    hist_ep_artwork = entry['EpisodeArtwork']
                    hist_ep_url = entry['EpisodeURL']
                    hist_ep_listen_date = entry['ListenDate']
                    hist_ep_duration = entry['EpisodeDuration']
                    # do something with the episode information
                    hist_entry_title_button = ft.Text(f'{hist_pod_name} - {hist_ep_title}', style=ft.TextThemeStyle.TITLE_MEDIUM, color=active_user.font_color)
                    hist_entry_title = ft.TextButton(content=hist_entry_title_button, on_click=lambda x, url=hist_ep_url, title=hist_ep_title: open_episode_select(page, url, title))
                    hist_entry_row = ft.ResponsiveRow([
    ft.Column(col={"sm": 6}, controls=[hist_entry_title]),
])

                    num_lines = hist_ep_desc.count('\n')
                    if num_lines > 15:
                        if is_html(hist_ep_desc):
                            # convert HTML to Markdown
                            markdown_desc = html2text.html2text(hist_ep_desc)
                            if num_lines > 15:
                                # Split into lines, truncate to 15 lines, and join back into a string
                                lines = markdown_desc.splitlines()[:15]
                                markdown_desc = '\n'.join(lines)
                            # add inline style to change font color                            
                            hist_entry_description = ft.Markdown(markdown_desc, on_tap_link=launch_clicked_url)
                            hist_entry_seemore = ft.TextButton(text="See More...", on_click=lambda x, url=hist_ep_url, title=hist_ep_title: open_episode_select(page, url, title))
                        else:
                            if num_lines > 15:
                                # Split into lines, truncate to 15 lines, and join back into a string
                                lines = hist_ep_desc.splitlines()[:15]
                                hist_ep_desc = '\n'.join(lines)
                            # display plain text
                            hist_entry_description = ft.Text(hist_ep_desc)

                    else:
                        if is_html(hist_ep_desc):
                            # convert HTML to Markdown
                            markdown_desc = html2text.html2text(hist_ep_desc)
                            # add inline style to change font color
                            hist_entry_description = ft.Markdown(markdown_desc, on_tap_link=launch_clicked_url)
                        else:
                            # display plain text
                            markdown_desc = hist_ep_desc
                            hist_entry_description = ft.Text(hist_ep_desc)

                    hist_entry_audio_url = ft.Text(hist_ep_url)
                    check_episode_playback, listen_duration = database_functions.functions.check_episode_playback(cnx, active_user.user_id, hist_ep_title, hist_ep_url)
                    hist_art_no = random.randint(1, 12)
                    hist_art_fallback = os.path.join(script_dir, "images", "logo_random", f"{hist_art_no}.jpeg")
                    hist_art_url = hist_ep_artwork if hist_ep_artwork else hist_art_fallback
                    hist_art_url_parsed = check_image(hist_art_url)
                    hist_entry_artwork_url = ft.Image(src=hist_art_url_parsed, width=150, height=150)
                    hist_ep_play_button = ft.IconButton(
                        icon=ft.icons.NOT_STARTED,
                        icon_color=active_user.accent_color,
                        icon_size=40,
                        tooltip="Start Episode From Beginning",
                        on_click=lambda x, url=hist_ep_url, title=hist_ep_title, artwork=hist_ep_artwork: play_selected_episode(url, title, artwork)
                    )
                    hist_ep_resume_button = ft.IconButton(
                        icon=ft.icons.PLAY_CIRCLE,
                        icon_color=active_user.accent_color,
                        icon_size=40,
                        tooltip="Resume Episode",
                        on_click=lambda x, url=hist_ep_url, title=hist_ep_title, artwork=hist_ep_artwork, listen_duration=listen_duration: resume_selected_episode(url, title, artwork, listen_duration)
                    )
                    hist_popup_button = ft.PopupMenuButton(content=ft.Icon(ft.icons.ARROW_DROP_DOWN_CIRCLE_ROUNDED, color=active_user.accent_color, size=40, tooltip="Play Episode"), 
                        items=[
                            ft.PopupMenuItem(icon=ft.icons.QUEUE, text="Queue", on_click=lambda x, url=hist_ep_url, title=hist_ep_title, artwork=hist_ep_artwork: queue_selected_episode(url, title, artwork, page)),
                            ft.PopupMenuItem(icon=ft.icons.DOWNLOAD, text="Download", on_click=lambda x, url=hist_ep_url, title=hist_ep_title: download_selected_episode(url, title, page)),
                            ft.PopupMenuItem(icon=ft.icons.SAVE, text="Save Episode", on_click=lambda x, url=hist_ep_url, title=hist_ep_title: save_selected_episode(url, title, page))
                        ]
                    )
                    
                    if check_episode_playback == True:
                        listen_prog = seconds_to_time(listen_duration)
                        hist_ep_prog = seconds_to_time(hist_ep_duration)
                        progress_value = get_progress(listen_duration, hist_ep_duration)
                        hist_entry_listened = ft.Text(f'Listened on: {hist_ep_listen_date}', color=active_user.font_color)
                        hist_entry_progress = ft.Row(controls=[ft.Text(listen_prog, color=active_user.font_color), ft.ProgressBar(expand=True, value=progress_value, color=active_user.main_color), ft.Text(hist_ep_prog, color=active_user.font_color)])
                        if num_lines > 15:
                            hist_ep_row_content = ft.ResponsiveRow([
                                ft.Column(col={"md": 2}, controls=[hist_entry_artwork_url]),
                                ft.Column(col={"md": 10}, controls=[hist_entry_title, hist_entry_description, hist_entry_seemore, hist_entry_listened, hist_entry_progress, ft.Row(controls=[hist_ep_play_button, hist_ep_resume_button, hist_popup_button])]),
                            ])
                        else:
                            hist_ep_row_content = ft.ResponsiveRow([
                                ft.Column(col={"md": 2}, controls=[hist_entry_artwork_url]),
                                ft.Column(col={"md": 10}, controls=[hist_entry_title, hist_entry_description, hist_entry_listened, hist_entry_progress, ft.Row(controls=[hist_ep_play_button, hist_ep_resume_button, hist_popup_button])]),
                            ]) 
                    else:
                        if num_lines > 15:
                            hist_ep_row_content = ft.ResponsiveRow([
                                ft.Column(col={"md": 2}, controls=[hist_entry_artwork_url]),
                                ft.Column(col={"md": 10}, controls=[hist_entry_title, hist_entry_description, hist_entry_seemore, hist_entry_listened, ft.Row(controls=[hist_ep_play_button, hist_popup_button])]),
                            ])
                        else:
                            hist_ep_row_content = ft.ResponsiveRow([
                                ft.Column(col={"md": 2}, controls=[hist_entry_artwork_url]),
                                ft.Column(col={"md": 10}, controls=[hist_entry_title, hist_entry_description, hist_entry_listened, ft.Row(controls=[hist_ep_play_button, hist_popup_button])]),
                            ]) 
                    hist_div_row = ft.Divider(color=active_user.accent_color)
                    hist_ep_column = ft.Column(controls=[hist_ep_row_content, hist_div_row])
                    hist_ep_row = ft.Container(content=hist_ep_column)
                    hist_ep_row.padding=padding.only(left=70, right=50)
                    hist_ep_rows.append(hist_ep_row)
                    # hist_ep_rows.append(ft.Text('test'))
                    hist_ep_row_dict[f'search_row{hist_ep_number}'] = hist_ep_row
                    hist_pods_active = True
                    hist_ep_number += 1

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
                        pypods_appbar,
                        top_bar,
                        history_title_row,
                        *[hist_ep_row_dict.get(f'search_row{i+1}') for i in range(len(hist_ep_rows))]

                    ]
                    
                )
            ep_hist_view.bgcolor = active_user.bgcolor
            ep_hist_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                ep_hist_view
                    
                )

        if page.route == "/saved" or page.route == "/saved":

            # Get Pod info
            saved_episode_list = database_functions.functions.saved_episode_list(cnx, active_user.user_id)

            # page.overlay.pop(2)

            if saved_episode_list is None:
                saved_ep_number = 1
                saved_ep_rows = []
                saved_ep_row_dict = {}
                saved_pod_name = "No podcasts saved yet"
                saved_ep_title = "Podcasts you save will display here."
                saved_pub_date = ""
                saved_ep_desc = "Click the dropdown on podcasts and select save. This will save the podcast in order to easily find them for later listening. Think of this like a permanant queue."
                saved_ep_url = ""
                saved_entry_title = ft.Text(f'{saved_pod_name} - {saved_ep_title}', width=600, style=ft.TextThemeStyle.TITLE_MEDIUM)
                saved_entry_description = ft.Text(saved_ep_desc, width=800)
                saved_entry_audio_url = ft.Text(saved_ep_url)
                saved_entry_released = ft.Text(saved_pub_date)
                artwork_no = random.randint(1, 12)
                saved_artwork_url = os.path.join(script_dir, "images", "logo_random", f"{artwork_no}.jpeg")
                saved_artwork_url_parsed = check_image(saved_artwork_url)
                saved_entry_artwork_url = ft.Image(src=saved_artwork_url_parsed, width=150, height=150)
                saved_ep_play_button = ft.IconButton(
                    icon=ft.icons.PLAY_DISABLED,
                    icon_color=active_user.accent_color,
                    icon_size=40,
                    tooltip="No Episodes Added Yet"
                )
                # Creating column and row for saved layout
                saved_ep_column = ft.Column(
                    controls=[saved_entry_title, saved_entry_description, saved_entry_released]
                )
                # saved_ep_row = ft.Row(
                #     alignment=ft.MainAxisAlignment.CENTER,
                #     controls=[saved_entry_artwork_url, saved_ep_column, saved_ep_play_button]
                # )
                saved_ep_row_content = ft.ResponsiveRow([
                    ft.Column(col={"md": 2}, controls=[saved_entry_artwork_url]),
                    ft.Column(col={"md": 10}, controls=[saved_ep_column, saved_ep_play_button]),
                ])
                saved_ep_row = ft.Container(content=saved_ep_row_content)
                saved_ep_row.padding=padding.only(left=70, right=50)
                saved_ep_rows.append(saved_ep_row)
                saved_ep_row_dict[f'search_row{saved_ep_number}'] = saved_ep_row
                saved_pods_active = True
                saved_ep_number += 1

            else:
                saved_episode_list.reverse()
                saved_ep_number = 1
                saved_ep_rows = []
                saved_ep_row_dict = {}

                for entry in saved_episode_list:
                    saved_ep_title = entry['EpisodeTitle']
                    saved_pod_name = entry['PodcastName']
                    saved_pub_date = entry['EpisodePubDate']
                    saved_ep_desc = entry['EpisodeDescription']
                    saved_ep_artwork = entry['EpisodeArtwork']
                    saved_ep_url = entry['EpisodeURL']
                    saved_ep_duration = entry['EpisodeDuration']
                    
                    # do something with the episode information
                    saved_entry_title_button = ft.Text(f'{saved_pod_name} - {saved_ep_title}', style=ft.TextThemeStyle.TITLE_MEDIUM, color=active_user.font_color)
                    saved_entry_title = ft.TextButton(content=saved_entry_title_button, on_click=lambda x, url=saved_ep_url, title=saved_ep_title: open_episode_select(page, url, title))
                    saved_entry_row = ft.ResponsiveRow([
    ft.Column(col={"sm": 6}, controls=[saved_entry_title]),
])

                    num_lines = saved_ep_desc.count('\n')
                    if num_lines > 15:
                        if is_html(saved_ep_desc):
                            # convert HTML to Markdown
                            markdown_desc = html2text.html2text(saved_ep_desc)
                            if num_lines > 15:
                                # Split into lines, truncate to 15 lines, and join back into a string
                                lines = markdown_desc.splitlines()[:15]
                                markdown_desc = '\n'.join(lines)
                            # add inline style to change font color                            
                            saved_entry_description = ft.Markdown(markdown_desc, on_tap_link=launch_clicked_url)
                            saved_entry_seemore = ft.TextButton(text="See More...", on_click=lambda x, url=saved_ep_url, title=saved_ep_title: open_episode_select(page, url, title))
                        else:
                            if num_lines > 15:
                                # Split into lines, truncate to 15 lines, and join back into a string
                                lines = saved_ep_desc.splitlines()[:15]
                                saved_ep_desc = '\n'.join(lines)
                            # display plain text
                            saved_entry_description = ft.Text(saved_ep_desc)

                    else:
                        if is_html(saved_ep_desc):
                            # convert HTML to Markdown
                            markdown_desc = html2text.html2text(saved_ep_desc)
                            # add inline style to change font color
                            saved_entry_description = ft.Markdown(markdown_desc, on_tap_link=launch_clicked_url)
                        else:
                            # display plain text
                            markdown_desc = saved_ep_desc
                            saved_entry_description = ft.Text(saved_ep_desc)
                    saved_entry_audio_url = ft.Text(saved_ep_url, color=active_user.font_color)
                    check_episode_playback, listen_duration = database_functions.functions.check_episode_playback(cnx, active_user.user_id, saved_ep_title, saved_ep_url)
                    saved_entry_released = ft.Text(f'Released on: {saved_pub_date}', color=active_user.font_color)


                    saved_art_no = random.randint(1, 12)
                    saved_art_fallback = os.path.join(script_dir, "images", "logo_random", f"{saved_art_no}.jpeg")
                    saved_art_url = saved_ep_artwork if saved_ep_artwork else saved_art_fallback
                    saved_art_parsed = check_image(saved_art_url)
                    saved_entry_artwork_url = ft.Image(src=saved_art_parsed, width=150, height=150)
                    saved_ep_play_button = ft.IconButton(
                        icon=ft.icons.NOT_STARTED,
                        icon_color=active_user.accent_color,
                        icon_size=40,
                        tooltip="Play Episode",
                        on_click=lambda x, url=saved_ep_url, title=saved_ep_title, artwork=saved_ep_artwork: play_selected_episode(url, title, artwork)
                    )
                    saved_ep_resume_button = ft.IconButton(
                        icon=ft.icons.PLAY_CIRCLE,
                        icon_color=active_user.accent_color,
                        icon_size=40,
                        tooltip="Resume Episode",
                        on_click=lambda x, url=saved_ep_url, title=saved_ep_title, artwork=saved_ep_artwork, listen_duration=listen_duration: resume_selected_episode(url, title, artwork, listen_duration)
                    )
                    saved_popup_button = ft.PopupMenuButton(content=ft.Icon(ft.icons.ARROW_DROP_DOWN_CIRCLE_ROUNDED, color=active_user.accent_color, size=40, tooltip="Play Episode"), 
                        items=[
                            ft.PopupMenuItem(icon=ft.icons.QUEUE, text="Queue", on_click=lambda x, url=saved_ep_url, title=saved_ep_title, artwork=saved_ep_artwork: queue_selected_episode(url, title, artwork, page)),
                            ft.PopupMenuItem(icon=ft.icons.DOWNLOAD, text="Download", on_click=lambda x, url=saved_ep_url, title=saved_ep_title: download_selected_episode(url, title, page)),         
                            ft.PopupMenuItem(icon=ft.icons.SAVE, text="Remove Saved Episode", on_click=lambda x, url=saved_ep_url, title=saved_ep_title: remove_saved_episode(url, title, page))
                        ]
                    )
                    if check_episode_playback == True:
                        listen_prog = seconds_to_time(listen_duration)
                        saved_ep_prog = seconds_to_time(saved_ep_duration)
                        progress_value = get_progress(listen_duration, saved_ep_duration)
                        saved_entry_progress = ft.Row(controls=[ft.Text(listen_prog, color=active_user.font_color), ft.ProgressBar(expand=True, value=progress_value, color=active_user.main_color), ft.Text(saved_ep_prog, color=active_user.font_color)])
                        if num_lines > 15:
                            saved_ep_row_content = ft.ResponsiveRow([
                                ft.Column(col={"md": 2}, controls=[saved_entry_artwork_url]),
                                ft.Column(col={"md": 10}, controls=[saved_entry_title, saved_entry_description, saved_entry_seemore, saved_entry_released, saved_entry_progress, ft.Row(controls=[saved_ep_play_button, saved_ep_resume_button, saved_popup_button])]),
                            ])
                        else:
                            saved_ep_row_content = ft.ResponsiveRow([
                                ft.Column(col={"md": 2}, controls=[saved_entry_artwork_url]),
                                ft.Column(col={"md": 10}, controls=[saved_entry_title, saved_entry_description, saved_entry_released, saved_entry_progress, ft.Row(controls=[saved_ep_play_button, saved_ep_resume_button, saved_popup_button])]),
                            ]) 
                    else:
                        if num_lines > 15:
                            saved_ep_row_content = ft.ResponsiveRow([
                                ft.Column(col={"md": 2}, controls=[saved_entry_artwork_url]),
                                ft.Column(col={"md": 10}, controls=[saved_entry_title, saved_entry_description, saved_entry_seemore, saved_entry_released, ft.Row(controls=[saved_ep_play_button, saved_popup_button])]),
                            ])
                        else:
                            saved_ep_row_content = ft.ResponsiveRow([
                                ft.Column(col={"md": 2}, controls=[saved_entry_artwork_url]),
                                ft.Column(col={"md": 10}, controls=[saved_entry_title, saved_entry_description, saved_entry_released, ft.Row(controls=[saved_ep_play_button, saved_popup_button])]),
                            ]) 
                    saved_div_row = ft.Divider(color=active_user.accent_color)
                    saved_ep_column = ft.Column(controls=[saved_ep_row_content, saved_div_row])
                    saved_ep_row = ft.Container(content=saved_ep_column)
                    saved_ep_row.padding=padding.only(left=70, right=50)
                    saved_ep_rows.append(saved_ep_row)
                    # saved_ep_rows.append(ft.Text('test'))
                    saved_ep_row_dict[f'search_row{saved_ep_number}'] = saved_ep_row
                    saved_pods_active = True
                    saved_ep_number += 1

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
                        pypods_appbar,
                        top_bar,
                        saved_title_row,
                        *[saved_ep_row_dict.get(f'search_row{i+1}') for i in range(len(saved_ep_rows))]

                    ]
                    
                )
            ep_saved_view.bgcolor = active_user.bgcolor
            ep_saved_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                ep_saved_view
                    
                )

        if page.route == "/downloads" or page.route == "/downloads":

            # Get Pod info
            download_episode_list = database_functions.functions.download_episode_list(cnx, active_user.user_id)

            # page.overlay.pop(2)

            if download_episode_list is None:
                download_ep_number = 1
                download_ep_rows = []
                download_ep_row_dict = {}
                download_pod_name = "No Podcasts added yet"
                download_ep_title = "Podcasts you download will display here."
                download_pub_date = ""
                download_ep_desc = "Click the dropdown on podcasts and select download. This will download the podcast to the server for local storage."
                download_ep_url = ""
                download_entry_title = ft.Text(f'{download_pod_name} - {download_ep_title}', width=600, style=ft.TextThemeStyle.TITLE_MEDIUM)
                download_entry_description = ft.Text(download_ep_desc, width=800)
                download_entry_audio_url = ft.Text(download_ep_url)
                download_entry_released = ft.Text(download_pub_date)
                artwork_no = random.randint(1, 12)
                download_artwork_url = os.path.join(script_dir, "images", "logo_random", f"{artwork_no}.jpeg")
                download_artwork_url_parsed = check_image(download_artwork_url)
                download_entry_artwork_url = ft.Image(src=download_artwork_url_parsed, width=150, height=150)
                download_ep_play_button = ft.IconButton(
                    icon=ft.icons.PLAY_DISABLED,
                    icon_color="blue400",
                    icon_size=40,
                    tooltip="No Episodes Added Yet"
                )
                # Creating column and row for download layout
                download_ep_column = ft.Column(
                    controls=[download_entry_title, download_entry_description, download_entry_released]
                )
                # download_ep_row = ft.Row(
                #     alignment=ft.MainAxisAlignment.CENTER,
                #     controls=[download_entry_artwork_url, download_ep_column, download_ep_play_button]
                # )
                download_ep_row_content = ft.ResponsiveRow([
                    ft.Column(col={"md": 2}, controls=[download_entry_artwork_url]),
                    ft.Column(col={"md": 10}, controls=[download_ep_column, download_ep_play_button]),
                ])
                download_ep_row = ft.Container(content=download_ep_row_content)
                download_ep_row.padding=padding.only(left=70, right=50)
                download_ep_rows.append(download_ep_row)
                download_ep_row_dict[f'search_row{download_ep_number}'] = download_ep_row
                download_pods_active = True
                download_ep_number += 1

            else:
                download_episode_list.reverse()
                download_ep_number = 1
                download_ep_rows = []
                download_ep_row_dict = {}

                for entry in download_episode_list:
                    download_ep_title = entry['EpisodeTitle']
                    download_pod_name = entry['PodcastName']
                    download_pub_date = entry['EpisodePubDate']
                    download_ep_desc = entry['EpisodeDescription']
                    download_ep_artwork = entry['EpisodeArtwork']
                    download_ep_url = entry['EpisodeURL']
                    download_ep_local_url = entry['DownloadedLocation']
                    download_ep_duration = entry['EpisodeDuration']
                    
                    # do something with the episode information
                    download_entry_title_button = ft.Text(f'{download_pod_name} - {download_ep_title}', style=ft.TextThemeStyle.TITLE_MEDIUM, color=active_user.font_color)
                    download_entry_title = ft.TextButton(content=download_entry_title_button, on_click=lambda x, url=download_ep_url, title=download_ep_title: open_episode_select(page, url, title))
                    download_entry_row = ft.ResponsiveRow([
    ft.Column(col={"sm": 6}, controls=[download_entry_title]),
])

                    num_lines = download_ep_desc.count('\n')
                    if num_lines > 15:
                        if is_html(download_ep_desc):
                            # convert HTML to Markdown
                            markdown_desc = html2text.html2text(download_ep_desc)
                            if num_lines > 15:
                                # Split into lines, truncate to 15 lines, and join back into a string
                                lines = markdown_desc.splitlines()[:15]
                                markdown_desc = '\n'.join(lines)
                            # add inline style to change font color                            
                            download_entry_description = ft.Markdown(markdown_desc, on_tap_link=launch_clicked_url)
                            download_entry_seemore = ft.TextButton(text="See More...", on_click=lambda x, url=download_ep_url, title=download_ep_title: open_episode_select(page, url, title))
                        else:
                            if num_lines > 15:
                                # Split into lines, truncate to 15 lines, and join back into a string
                                lines = download_ep_desc.splitlines()[:15]
                                download_ep_desc = '\n'.join(lines)
                            # display plain text
                            download_entry_description = ft.Text(download_ep_desc)

                    else:
                        if is_html(download_ep_desc):
                            # convert HTML to Markdown
                            markdown_desc = html2text.html2text(download_ep_desc)
                            # add inline style to change font color
                            download_entry_description = ft.Markdown(markdown_desc, on_tap_link=launch_clicked_url)
                        else:
                            # display plain text
                            markdown_desc = download_ep_desc
                            download_entry_description = ft.Text(download_ep_desc)
                    download_entry_audio_url = ft.Text(download_ep_url, color=active_user.font_color)
                    check_episode_playback, listen_duration = database_functions.functions.check_episode_playback(cnx, active_user.user_id, download_ep_title, download_ep_url)
                    download_entry_released = ft.Text(f'Released on: {download_pub_date}', color=active_user.font_color)


                    download_art_no = random.randint(1, 12)
                    download_art_fallback = os.path.join(script_dir, "images", "logo_random", f"{download_art_no}.jpeg")
                    download_art_url = download_ep_artwork if download_ep_artwork else download_art_fallback
                    download_art_parsed = check_image(download_art_url)
                    download_entry_artwork_url = ft.Image(src=download_art_parsed, width=150, height=150)
                    download_ep_play_button = ft.IconButton(
                        icon=ft.icons.NOT_STARTED,
                        icon_color=active_user.accent_color,
                        icon_size=40,
                        tooltip="Play Episode",
                        on_click=lambda x, url=download_ep_local_url, title=download_ep_title, artwork=download_ep_artwork: play_selected_episode(url, title, artwork)
                    )
                    download_ep_resume_button = ft.IconButton(
                        icon=ft.icons.PLAY_CIRCLE,
                        icon_color=active_user.accent_color,
                        icon_size=40,
                        tooltip="Resume Episode",
                        on_click=lambda x, url=download_ep_url, title=download_ep_title, artwork=download_ep_artwork, listen_duration=listen_duration: resume_selected_episode(url, title, artwork, listen_duration)
                    )
                    download_popup_button = ft.PopupMenuButton(content=ft.Icon(ft.icons.ARROW_DROP_DOWN_CIRCLE_ROUNDED, color=active_user.accent_color, size=40, tooltip="Play Episode"), 
                        items=[
                            ft.PopupMenuItem(icon=ft.icons.QUEUE, text="Queue", on_click=lambda x, url=download_ep_url, title=download_ep_title, artwork=download_ep_artwork: queue_selected_episode(url, title, artwork, page)),
                            ft.PopupMenuItem(icon=ft.icons.DOWNLOAD, text="Delete Downloaded Episode", on_click=lambda x, url=download_ep_url, title=download_ep_title: delete_selected_episode(url, title, page)),
                            ft.PopupMenuItem(icon=ft.icons.SAVE, text="Save Episode", on_click=lambda x, url=download_ep_url, title=download_ep_title: save_selected_episode(url, title, page))
                        ]
                    )
                    if check_episode_playback == True:
                        listen_prog = seconds_to_time(listen_duration)
                        download_ep_prog = seconds_to_time(download_ep_duration)
                        progress_value = get_progress(listen_duration, download_ep_duration)
                        download_entry_progress = ft.Row(controls=[ft.Text(listen_prog, color=active_user.font_color), ft.ProgressBar(expand=True, value=progress_value, color=active_user.main_color), ft.Text(download_ep_prog, color=active_user.font_color)])
                        if num_lines > 15:
                            download_ep_row_content = ft.ResponsiveRow([
                                ft.Column(col={"md": 2}, controls=[download_entry_artwork_url]),
                                ft.Column(col={"md": 10}, controls=[download_entry_title, download_entry_description, download_entry_seemore, download_entry_released, download_entry_progress, ft.Row(controls=[download_ep_play_button, download_ep_resume_button, download_popup_button])]),
                            ])
                        else:
                            download_ep_row_content = ft.ResponsiveRow([
                                ft.Column(col={"md": 2}, controls=[download_entry_artwork_url]),
                                ft.Column(col={"md": 10}, controls=[download_entry_title, download_entry_description, download_entry_released, download_entry_progress, ft.Row(controls=[download_ep_play_button, download_ep_resume_button, download_popup_button])]),
                            ]) 
                    else:
                        if num_lines > 15:
                            download_ep_row_content = ft.ResponsiveRow([
                                ft.Column(col={"md": 2}, controls=[download_entry_artwork_url]),
                                ft.Column(col={"md": 10}, controls=[download_entry_title, download_entry_description, download_entry_seemore, download_entry_released, ft.Row(controls=[download_ep_play_button, download_popup_button])]),
                            ])
                        else:
                            download_ep_row_content = ft.ResponsiveRow([
                                ft.Column(col={"md": 2}, controls=[download_entry_artwork_url]),
                                ft.Column(col={"md": 10}, controls=[download_entry_title, download_entry_description, download_entry_released, ft.Row(controls=[download_ep_play_button, download_popup_button])]),
                            ]) 
                    download_div_row = ft.Divider(color=active_user.accent_color)
                    download_ep_column = ft.Column(controls=[download_ep_row_content, download_div_row])
                    download_ep_row = ft.Container(content=download_ep_column)
                    download_ep_row.padding=padding.only(left=70, right=50)
                    download_ep_rows.append(download_ep_row)
                    # download_ep_rows.append(ft.Text('test'))
                    download_ep_row_dict[f'search_row{download_ep_number}'] = download_ep_row
                    download_pods_active = True
                    download_ep_number += 1

            download_title = ft.Text(
            "Downloaded Episodes:",
            size=30,
            font_family="RobotoSlab",
            color=active_user.font_color,
            weight=ft.FontWeight.W_300,
        )
            download_title_row = ft.Row(controls=[download_title], alignment=ft.MainAxisAlignment.CENTER)


            # Create search view object
            ep_download_view = ft.View("/downloads",
                    [
                        pypods_appbar,
                        top_bar,
                        download_title_row,
                        *[download_ep_row_dict.get(f'search_row{i+1}') for i in range(len(download_ep_rows))]

                    ]
                    
                )
            ep_download_view.bgcolor = active_user.bgcolor
            ep_download_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                ep_download_view
                    
                )

        if page.route == "/queue" or page.route == "/queue":

            current_queue_list = current_episode.get_queue()
            episode_queue_list = database_functions.functions.get_queue_list(cnx, current_queue_list)

            # database_functions.functions.queue_episode_list(cnx, active_user.user_id)


            if episode_queue_list is None:
                queue_ep_number = 1
                queue_ep_rows = []
                queue_ep_row_dict = {}
                queue_pod_name = "No Podcasts added yet"
                queue_ep_title = "Podcasts you queue will display here."
                queue_pub_date = ""
                queue_ep_desc = "Click the dropdown on podcasts and select queue. This will queue the podcast to play next."
                queue_ep_url = ""
                queue_entry_title = ft.Text(f'{queue_pod_name} - {queue_ep_title}', style=ft.TextThemeStyle.TITLE_MEDIUM)
                queue_entry_description = ft.Text(queue_ep_desc)
                queue_entry_audio_url = ft.Text(queue_ep_url)
                queue_entry_released = ft.Text(queue_pub_date)
                artwork_no = random.randint(1, 12)
                queue_artwork_url = os.path.join(script_dir, "images", "logo_random", f"{artwork_no}.jpeg")
                queue_artwork_url_parsed = check_image(queue_artwork_url)
                queue_entry_artwork_url = ft.Image(src=queue_artwork_url_parsed, width=150, height=150)
                queue_ep_play_button = ft.IconButton(
                    icon=ft.icons.PLAY_DISABLED,
                    icon_color=active_user.accent_color,
                    icon_size=40,
                    tooltip="No Episodes Added Yet"
                )
                # Creating column and row for queue layout
                queue_ep_column = ft.Column(
                    controls=[queue_entry_title, queue_entry_description, queue_entry_released]
                )
                queue_ep_row_content = ft.ResponsiveRow([
                    ft.Column(col={"md": 2}, controls=[queue_entry_artwork_url]),
                    ft.Column(col={"md": 10}, controls=[queue_ep_column, queue_ep_play_button]),
                ])
                queue_ep_row = ft.Container(content=queue_ep_row_content)
                queue_ep_row.padding=padding.only(left=70, right=50)
                queue_ep_rows.append(queue_ep_row)
                queue_ep_row_dict[f'search_row{queue_ep_number}'] = queue_ep_row
                queue_pods_active = True
                queue_ep_number += 1


            else:
                queue_ep_number = 1
                queue_ep_rows = []
                queue_ep_row_dict = {}

                for entry in episode_queue_list:
                    queue_ep_title = entry['EpisodeTitle']
                    queue_pod_name = entry['PodcastName']
                    queue_pub_date = entry['EpisodePubDate']
                    queue_ep_desc = entry['EpisodeDescription']
                    queue_ep_artwork = entry['EpisodeArtwork']
                    queue_ep_url = entry['EpisodeURL']
                    queue_ep_date = entry['QueueDate']
                    queue_ep_duration = entry['EpisodeDuration']
                    
                    # do something with the episode information
                    queue_entry_title_button = ft.Text(f'{queue_pod_name} - {queue_ep_title}', style=ft.TextThemeStyle.TITLE_MEDIUM, color=active_user.font_color)
                    queue_entry_title = ft.TextButton(content=queue_entry_title_button, on_click=lambda x, url=queue_ep_url, title=queue_ep_title: open_episode_select(page, url, title))
                    queue_entry_row = ft.ResponsiveRow([
    ft.Column(col={"sm": 6}, controls=[queue_entry_title]),
])

                    num_lines = queue_ep_desc.count('\n')
                    if num_lines > 15:
                        if is_html(queue_ep_desc):
                            # convert HTML to Markdown
                            markdown_desc = html2text.html2text(queue_ep_desc)
                            if num_lines > 15:
                                # Split into lines, truncate to 15 lines, and join back into a string
                                lines = markdown_desc.splitlines()[:15]
                                markdown_desc = '\n'.join(lines)
                            # add inline style to change font color                            
                            queue_entry_description = ft.Markdown(markdown_desc, on_tap_link=launch_clicked_url)
                            queue_entry_seemore = ft.TextButton(text="See More...", on_click=lambda x, url=queue_ep_url, title=queue_ep_title: open_episode_select(page, url, title))
                        else:
                            if num_lines > 15:
                                # Split into lines, truncate to 15 lines, and join back into a string
                                lines = queue_ep_desc.splitlines()[:15]
                                queue_ep_desc = '\n'.join(lines)
                            # display plain text
                            queue_entry_description = ft.Text(queue_ep_desc)

                    else:
                        if is_html(queue_ep_desc):
                            # convert HTML to Markdown
                            markdown_desc = html2text.html2text(queue_ep_desc)
                            # add inline style to change font color
                            queue_entry_description = ft.Markdown(markdown_desc, on_tap_link=launch_clicked_url)
                        else:
                            # display plain text
                            markdown_desc = queue_ep_desc
                            queue_entry_description = ft.Text(queue_ep_desc)
                    queue_entry_audio_url = ft.Text(queue_ep_url, color=active_user.font_color)
                    check_episode_playback, listen_duration = database_functions.functions.check_episode_playback(cnx, active_user.user_id, queue_ep_title, queue_ep_url)
                    queue_entry_released = ft.Text(queue_pub_date, color=active_user.font_color)

                    queue_art_no = random.randint(1, 12)
                    queue_art_fallback = os.path.join(script_dir, "images", "logo_random", f"{queue_art_no}.jpeg")
                    queue_art_url = queue_ep_artwork if queue_ep_artwork else queue_art_fallback
                    queue_art_parsed = check_image(queue_art_url)
                    queue_entry_artwork_url = ft.Image(src=queue_art_parsed, width=150, height=150)
                    queue_ep_play_button = ft.IconButton(
                        icon=ft.icons.PLAY_CIRCLE,
                        icon_color=active_user.accent_color,
                        icon_size=40,
                        tooltip="Play Episode",
                        on_click=lambda x, url=queue_ep_url, title=queue_ep_title, artwork=queue_ep_artwork: play_selected_episode(url, title, artwork)
                    )
                    queue_ep_resume_button = ft.IconButton(
                        icon=ft.icons.PLAY_CIRCLE,
                        icon_color=active_user.accent_color,
                        icon_size=40,
                        tooltip="Resume Episode",
                        on_click=lambda x, url=queue_ep_url, title=queue_ep_title, artwork=queue_ep_artwork, listen_duration=listen_duration: resume_selected_episode(url, title, artwork, listen_duration)
                    )
                    queue_popup_button = ft.PopupMenuButton(content=ft.Icon(ft.icons.ARROW_DROP_DOWN_CIRCLE_ROUNDED, color=active_user.accent_color, size=40, tooltip="Play Episode"), 
                    # icon_size=40, icon_color="blue400", tooltip="Options",
                        items=[
                            ft.PopupMenuItem(icon=ft.icons.QUEUE, text="Remove From Queue", on_click=lambda x, url=queue_ep_url, title=queue_ep_title: episode_remove_queue(url, title, page)),
                            ft.PopupMenuItem(icon=ft.icons.DOWNLOAD, text="Download Episode", on_click=lambda x, url=queue_ep_url, title=queue_ep_title: download_selected_episode(url, title, page)),
                            ft.PopupMenuItem(icon=ft.icons.SAVE, text="Save Episode", on_click=lambda x, url=queue_ep_url, title=queue_ep_title: save_selected_episode(url, title, page))
                        ]
                    )
                    if check_episode_playback == True:
                        listen_prog = seconds_to_time(listen_duration)
                        queue_ep_prog = seconds_to_time(queue_ep_duration)
                        progress_value = get_progress(listen_duration, queue_ep_duration)
                        queue_entry_progress = ft.Row(controls=[ft.Text(listen_prog, color=active_user.font_color), ft.ProgressBar(expand=True, value=progress_value, color=active_user.main_color), ft.Text(queue_ep_prog, color=active_user.font_color)])
                        if num_lines > 15:
                            queue_ep_row_content = ft.ResponsiveRow([
                                ft.Column(col={"md": 2}, controls=[queue_entry_artwork_url]),
                                ft.Column(col={"md": 10}, controls=[queue_entry_title, queue_entry_description, queue_entry_seemore, queue_entry_released, queue_entry_progress, ft.Row(controls=[queue_ep_play_button, queue_ep_resume_button, queue_popup_button])]),
                            ])
                        else:
                            queue_ep_row_content = ft.ResponsiveRow([
                                ft.Column(col={"md": 2}, controls=[queue_entry_artwork_url]),
                                ft.Column(col={"md": 10}, controls=[queue_entry_title, queue_entry_description, queue_entry_released, queue_entry_progress, ft.Row(controls=[queue_ep_play_button, queue_ep_resume_button, queue_popup_button])]),
                            ]) 
                    else:
                        if num_lines > 15:
                            queue_ep_row_content = ft.ResponsiveRow([
                                ft.Column(col={"md": 2}, controls=[queue_entry_artwork_url]),
                                ft.Column(col={"md": 10}, controls=[queue_entry_title, queue_entry_description, queue_entry_seemore, queue_entry_released, ft.Row(controls=[queue_ep_play_button, queue_popup_button])]),
                            ])
                        else:
                            queue_ep_row_content = ft.ResponsiveRow([
                                ft.Column(col={"md": 2}, controls=[queue_entry_artwork_url]),
                                ft.Column(col={"md": 10}, controls=[queue_entry_title, queue_entry_description, queue_entry_released, ft.Row(controls=[queue_ep_play_button, queue_popup_button])]),
                            ]) 
                    queue_div_row = ft.Divider(color=active_user.accent_color)
                    queue_ep_column = ft.Column(controls=[queue_ep_row_content, queue_div_row])
                    queue_ep_row = ft.Container(content=queue_ep_column)
                    queue_ep_row.padding=padding.only(left=70, right=50)
                    queue_ep_rows.append(queue_ep_row)
                    # queue_ep_rows.append(ft.Text('test'))
                    queue_ep_row_dict[f'search_row{queue_ep_number}'] = queue_ep_row
                    queue_pods_active = True
                    queue_ep_number += 1

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
                        pypods_appbar,
                        top_bar,
                        queue_title_row,
                        *[queue_ep_row_dict.get(f'search_row{i+1}') for i in range(len(queue_ep_rows))]

                    ]
                    
                )
            ep_queue_view.bgcolor = active_user.bgcolor
            ep_queue_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                ep_queue_view
                    
                )


        if page.route == "/episode_display" or page.route == "/episode_display":
            # Creating attributes for page layout
            episode_info = database_functions.functions.return_selected_episode(cnx, active_user.user_id, current_episode.title, current_episode.url)
            
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
            podcast_feed_name = ft.Text(ep_pod_name, color=active_user.font_color, style=ft.TextThemeStyle.DISPLAY_MEDIUM)
            pod_feed_site = ft.ElevatedButton(text=ep_pod_site, on_click=launch_pod_site)

            ep_play_button = ft.IconButton(
                icon=ft.icons.PLAY_CIRCLE,
                icon_color=active_user.accent_color,
                icon_size=40,
                tooltip="Play Episode",
                on_click = lambda x, url=ep_url, title=ep_title, artwork=ep_artwork: play_selected_episode(url, title, artwork)
            )
            ep_popup_button = ft.PopupMenuButton(content=ft.Icon(ft.icons.ARROW_DROP_DOWN_CIRCLE_ROUNDED, color=active_user.accent_color, size=40, tooltip="Play Episode"), 
                    items=[
                    ft.PopupMenuItem(icon=ft.icons.QUEUE, text="Queue", on_click=lambda x, url=ep_url, title=ep_title, artwork=ep_artwork: queue_selected_episode(url, title, artwork, page)),
                    ft.PopupMenuItem(icon=ft.icons.DOWNLOAD, text="Download", on_click=lambda x, url=ep_url, title=ep_title: download_selected_episode(url, title, page)),
                    ft.PopupMenuItem(icon=ft.icons.SAVE, text="Save Episode", on_click=lambda x, url=ep_url, title=ep_title: save_selected_episode(url, title, page))
                ]
            )
            ep_play_options = ft.Row(controls=[ep_play_button, ep_popup_button])

            feed_row_content = ft.ResponsiveRow([
            ft.Column(col={"md": 4}, controls=[pod_image]),
            ft.Column(col={"md": 8}, controls=[pod_feed_title, pod_feed_date, ep_play_options]),
            ])
            podcast_row = ft.Container(content=podcast_feed_name)
            podcast_row.padding=padding.only(left=70, right=50)
            feed_row = ft.Container(content=feed_row_content)
            feed_row.padding=padding.only(left=70, right=50)
            # Check for html in description
            if is_html(ep_desc):
                # convert HTML to Markdown
                markdown_desc = html2text.html2text(ep_desc)

                # add inline style to change font color
                
                pod_feed_desc = ft.Markdown(markdown_desc, on_tap_link=launch_clicked_url)
                desc_row = ft.Container(content=pod_feed_desc)
                desc_row.padding=padding.only(left=70, right=50)
            else:
                # display plain text
                markdown_desc = ep_desc
                pod_feed_desc = ft.Text(ep_desc, color=active_user.font_color)
                desc_row = ft.Container(content=pod_feed_desc)
                desc_row.padding=padding.only(left=70, right=50)



            # page.overlay.pop(2)
            # Create search view object
            pod_view = ft.View(
                    "/poddisplay",
                    [
                        pypods_appbar,
                        top_bar,
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

    page.on_route_change = route_change
    page.on_view_pop = view_pop

#-Create Help Banner-----------------------------------------------------------------------
    def close_banner(e):
        page.banner.open = False
        page.update()

    def open_repo(e):
        page.launch_url('https://github.com/madeofpendletonwool/pypods')

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

    banner_button = ft.ElevatedButton("Help!", on_click=show_banner_click)

# Login/User Changes------------------------------------------------------
    class User:
        email_regex = r"^[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+$"
        def __init__(self, page):
            self.username = None
            self.password = None
            self.email = None
            self.main_color = 'colors.BLUE_GREY'
            self.accent_color = 'colors.BLUE_GREY'
            self.tertiary_color = 'colors.BLUE_GREY'
            self.font_color = 'colors.BLUE_GREY'
            self.user_id = None
            self.page = page
            self.fullname = 'Login First'
            self.isadmin = None
            self.navbar_stack = None

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
            self.valid_password = self.password is not None and len(self.password) >= 8 and any(c.isupper() for c in self.password) and any(c.isdigit() for c in self.password)
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
            elif database_functions.functions.check_usernames(cnx, self.username):
                self.page.dialog = username_exists_dlg
                username_exists_dlg.open = True
                self.page.update()
                invalid_value = True
            self.new_user_valid = not invalid_value

        def verify_user_values_snack(self):
            self.valid_username = self.username is not None and len(self.username) >= 6
            self.valid_password = self.password is not None and len(self.password) >= 8 and any(c.isupper() for c in self.password) and any(c.isdigit() for c in self.password)
            regex = r"^[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+$"
            self.valid_email = self.email is not None and re.match(self.email_regex, self.email) is not None
            invalid_value = False
            if not self.valid_username:
                page.snack_bar = ft.SnackBar(content=ft.Text(f"Usernames must be unique and require at least 6 characters"))
                page.snack_bar.open = True
                self.page.update()
                self.invalid_value = True
            elif not self.valid_password:
                page.snack_bar = ft.SnackBar(content=ft.Text(f"Passwords require at least 8 characters, a number, a capital letter and a special character!"))
                page.snack_bar.open = True
                self.page.update()
                self.invalid_value = True
            elif not self.valid_email:
                page.snack_bar = ft.SnackBar(content=ft.Text(f"Email appears to be non-standard email layout!"))
                page.snack_bar.open = True
                self.page.update()
                self.invalid_value = True
            elif database_functions.functions.check_usernames(cnx, self.username):
                page.snack_bar = ft.SnackBar(content=ft.Text(f"This username appears to be already taken"))
                page.snack_bar.open = True
                self.page.update()
                self.invalid_value = True
            if self.invalid_value == True:
                self.new_user_valid = False
            else:
                self.new_user_valid = not invalid_value

        def user_created_prompt(self):
            if self.new_user_valid == True:
                self.page.dialog = user_dlg
                user_dlg.open = True
                self.page.update()

        def user_created_snack(self):
            if self.new_user_valid == True:
                page.snack_bar = ft.SnackBar(content=ft.Text(f"New user created successfully. You may now login and begin using Pinepods. Enjoy!"))
                page.snack_bar.open = True
                page.update()
                

        def popup_user_values(self, e):
            pass

        def create_user(self):
            if self.new_user_valid == True:
                salt, hash_pw = Auth.Passfunctions.hash_password(self.password)
                user_values = (self.fullname, self.username, self.email, hash_pw, salt)
                database_functions.functions.add_user(cnx, user_values)

    # Modify User Stuff---------------------------
        def open_edit_user(self, username, admin, fullname, email, user_id):
            def close_modify_dlg(e):
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
                else: admin_box = False

                self.username = username
                user_modify_name = ft.TextField(label="Full Name", icon=ft.icons.CARD_MEMBERSHIP, hint_text='John PinePods') 
                user_modify_email = ft.TextField(label="Email", icon=ft.icons.EMAIL, hint_text='ilovepinepods@pinepods.com')
                user_modify_username = ft.TextField(label="Username", icon=ft.icons.PERSON, hint_text='pinepods_user1999') 
                user_modify_password = ft.TextField(label="Password", icon=ft.icons.PASSWORD, password=True, can_reveal_password=True, hint_text='mY_SuPeR_S3CrEt!')
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
                    ]),
                actions=[
                    ft.TextButton(content=ft.Text("Delete User", color=ft.colors.RED_400), on_click=lambda x: (
                        modify_user.delete_user(user_id),
                        close_modify_dlg
                        )),
                    ft.TextButton("Confirm Changes", on_click=lambda x: (
                    modify_user.set_username(user_modify_username.value), 
                    modify_user.set_password(user_modify_password.value), 
                    modify_user.set_email(user_modify_email.value),
                    modify_user.set_name(user_modify_name.value),
                    modify_user.set_admin(user_modify_admin.value),
                    modify_user.change_user_attributes()
                    )),

                    ft.TextButton("Cancel", on_click=close_modify_dlg)
                    ],
                actions_alignment=ft.MainAxisAlignment.SPACE_EVENLY
            )
                self.page.dialog = modify_user_dlg
                modify_user_dlg.open = True
                self.page.update()

        def change_user_attributes(self):
            if self.fullname is not None:
                database_functions.functions.fullname(cnx, self.user_id, self.fullname)
                
            if self.password is not None:
                if len(self.password) < 8 or not any(c.isupper() for c in self.password) or not any(c.isdigit() for c in self.password):
                    page.snack_bar = ft.SnackBar(content=ft.Text(f"Passwords must contain a number, a capital letter and a special character"))
                    page.snack_bar.open = True
                    page.update()
                else:
                    salt, hash_pw = Auth.Passfunctions.hash_password(self.password)
                    database_functions.functions.set_password(cnx, self.user_id, salt, hash_pw)

            if self.email is not None:
                if not re.match(self.email_regex, self.email):
                    page.snack_bar = ft.SnackBar(content=ft.Text(f"This does not appear to be a properly formatted email"))
                    page.snack_bar.open = True
                    page.update()
                else:
                    database_functions.functions.set_email(cnx, self.user_id, self.email)

            if self.username is not None:
                if len(self.username) < 6:
                    page.snack_bar = ft.SnackBar(content=ft.Text(f"Username must be at least 6 characters"))
                    page.snack_bar.open = True
                    page.update()
                else:
                    database_functions.functions.set_username(cnx, self.user_id, self.username)

            database_functions.functions.set_isadmin(cnx, self.user_id, self.isadmin)
            user_changed = True

            if user_changed == True:
                page.snack_bar = ft.SnackBar(content=ft.Text(f"User Changed! Leave the page and return to see changes."))
                page.snack_bar.open = True
                page.update()


        # page.snack_bar = ft.SnackBar(content=ft.Text(f"Episode: {title} has been downloaded!"))
        # page.snack_bar.open = True
        # page.overlay.pop(2)
        # page.update()

        def delete_user(self, user_id):
            admin_check = database_functions.functions.final_admin(cnx, user_id)
            if user_id == active_user.user_id:
                page.snack_bar = ft.SnackBar(content=ft.Text(f"Cannot delete your own user"))
                page.snack_bar.open = True
                page.update()
            elif admin_check == True: 
                page.snack_bar = ft.SnackBar(content=ft.Text(f"Cannot delete the final admin user"))
                page.snack_bar.open = True
                page.update()
            else:
                database_functions.functions.delete_user(cnx, user_id)
                page.snack_bar = ft.SnackBar(content=ft.Text(f"User Deleted!"))
                page.snack_bar.open = True
                page.update()



    # Active User Stuff --------------------------

        def get_initials(self):
            # split the full name into separate words
            words = self.fullname.split()
            
            # extract the first letter of each word and combine them
            initials_lower = "".join(word[0] for word in words)
            
            # return the initials as uppercase
            self.initials = initials_lower.upper()

        def login(self, username_field, password_field):
            username = username_field.value
            password = password_field.value
            username_field.value = ''
            password_field.value = ''
            username_field.update()
            password_field.update()
            if not username or not password:
                on_click_novalues(page)
                return
            pass_correct = Auth.Passfunctions.verify_password(cnx, username, password)
            if pass_correct == True:
                login_details = database_functions.functions.get_user_details(cnx, username)
                self.user_id = login_details['UserID']
                self.fullname = login_details['Fullname']
                self.username = login_details['Username']
                self.email = login_details['Email']
                go_homelogin(page)
            else:
                on_click_wronguser(page)

        def logout_pinepods(self, e):
            active_user = User(page)
            page.overlay.remove(self.navbar_stack)
            if login_screen == True:

                start_login(page)
            else:
                active_user.user_id = 1
                active_user.fullname = 'Guest User'
                go_homelogin(page)

    # Setup Theming-------------------------------------------------------
        def theme_select(self):
            active_theme = database_functions.functions.get_theme(cnx, self.user_id)
            if active_theme == 'light':
                page.theme_mode = "light"
                self.main_color = '#E1E1E1'
                self.accent_color = colors.BLACK
                self.tertiary_color = '#C7C7C7'
                self.font_color = colors.BLACK
                self.bonus_color = colors.BLACK
                self.nav_color1 = colors.BLACK
                self.nav_color2 = '#C7C7C7'
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
                self.tertiary_color = '#23282E'
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
            elif active_theme == 'wildberries':
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
            else:
                page.theme_mode = "greenie meanie"
                self.main_color = '#323542'
                self.accent_color = colors.WHITE
                self.tertiary_color = '#23282E'
                self.font_color = colors.BLACK
                self.bonus_color = colors.BLACK
                self.nav_color1 = colors.BLACK
                self.nav_color2 = colors.BLACK
                self.bgcolor = '#3C4252'
                page.bgcolor = '#3C4252'
                page.window_bgcolor = '#3C4252'

        def set_theme(self, theme):
            database_functions.functions.set_theme(cnx, self.user_id, theme)
            self.theme_select
            go_theme_rebuild(self.page)
            self.page.update()

    # Initial user value set
    modify_user = User(page)

    def GradientGenerator(start, end):
        ColorGradient = LinearGradient(
            begin=alignment.bottom_left,
            end=alignment.top_right,
            colors=[
                start,
                end,
            ],
        )

        return ColorGradient
    
    login_username = TextField(
    label="Username",
    border="underline",
    width=320,
    text_size=14,
    )

    login_password = TextField(
        label="Password",
        border="underline",
        width=320,
        text_size=14,
        password=True,
        can_reveal_password=True,
    )

    active_user = User(page)

# Create Sidebar------------------------------------------------------

    class NavBar:
        def __init__(self, page):
            self.page = page

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
            return Container(
                width=180,
                height=45,
                border_radius=10,
                on_hover=lambda e: self.HighlightContainer(e),
                ink=True,
                content=Row(
                    controls=[
                        IconButton(
                            icon=icon_name,
                            icon_size=18,
                            icon_color=active_user.accent_color,
                            tooltip=tooltip,
                            selected=False,
                            on_click=destination,
                            style=ButtonStyle(
                                shape={
                                    "": RoundedRectangleBorder(radius=7),
                                },
                                overlay_color={"": "transparent"},
                            ),
                        ),
                        Text(
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
            active_user.get_initials()
            return Container(
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
                Divider(color="white24", height=5),
                Container(
                    width=42,
                    height=42,
                    border_radius=8,
                    bgcolor=active_user.tertiary_color,
                    alignment=alignment.center,
                    content=Text(
                        value=active_user.initials,
                        color=active_user.nav_color2,
                        size=20,
                        weight="bold"
                    ),
                    on_hover=display_hello,
                    on_click=open_user_stats
                ),
                    Divider(height=5, color="transparent"),
                    self.ContainedIcon('Home', icons.HOME, "Home", go_home),
                    self.ContainedIcon('Queue', icons.QUEUE, "Queue", open_queue),
                    self.ContainedIcon('Saved Podcasts',icons.SAVE, "Saved Podcasts", open_saved_pods),
                    self.ContainedIcon('Downloaded',icons.DOWNLOAD, "Downloaded", open_downloads),
                    self.ContainedIcon('Podcast History', icons.HISTORY, "Podcast History", open_history),
                    self.ContainedIcon('Added Podcasts', icons.PODCASTS, "Added Podcasts", open_pod_list),
                    Divider(color="white24", height=5),
                    self.ContainedIcon('Settings', icons.SETTINGS, "Settings", open_settings),
                    self.ContainedIcon('Logout', icons.LOGOUT_ROUNDED, "Logout", active_user.logout_pinepods),
                ],
            ),
        )



# Create Page--------------------------------------------------------


    page.title = "PinePods"
    page.appbar = AppBar(title=Text("PinePods - A Forest of Podcasts, Rooted in the Spirit of Self-Hosting", color="white"), center_title=True)
    page.title = "PinePods - A Forest of Podcasts, Rooted in the Spirit of Self-Hosting"
    def progress_ring(e):
        pr = ft.Column(
            [ft.ProgressRing(), ft.Text("I'm going to run for ages...")],
            horizontal_alignment=ft.CrossAxisAlignment.CENTER,
        ),
        page.overlay.append(pr)
    # Podcast Search Function Setup
    search_pods = ft.TextField(label="Search for new podcast", content_padding=5, width=350)
    search_btn = ft.ElevatedButton("Search!", on_click=open_search)
    refresh_btn = ft.IconButton(icon=ft.icons.REFRESH, icon_color=active_user.font_color, tooltip="Refresh Podcast List", on_click=refresh_podcasts)
    search_box = ft.Container(
        content=search_pods,
        alignment=ft.alignment.top_right
    )
    search_btn_ctn = ft.Container(
        content=search_btn,
        alignment=ft.alignment.top_right
    )
    refresh_ctn = ft.Container(
        content=refresh_btn,
        alignment=ft.alignment.top_left
    )

    # get the absolute path of the current script
    current_dir = os.path.dirname(os.path.abspath(__file__))

    # set the audio file path relative to the current script's directory
    audio_file = os.path.join(current_dir, "Audio", "750-milliseconds-of-silence.mp3")
    audio_file = os.path.join(current_dir, "Audio", "750-milliseconds-of-silence.mp3")

    parsed_audio_url = os.path.join(current_dir, "Audio", "750-milliseconds-of-silence.mp3")
    parsed_title = 'nothing playing'
    # Initialize the current episode
    global current_episode
    current_episode = Toggle_Pod(page, go_home, parsed_audio_url, parsed_title)

    # Create the audio controls
    play_button = ft.IconButton(
        icon=ft.icons.PLAY_ARROW,
        tooltip="Play Podcast",
        icon_color="white",
        on_click=lambda e: current_episode.resume_podcast()
    )
    pause_button = ft.IconButton(
        icon=ft.icons.PAUSE,
        tooltip="Pause Playback",
        icon_color="white",
        on_click=lambda e: current_episode.pause_episode()
    )
    pause_button.visible = False
    seek_button = ft.IconButton(
        icon=ft.icons.FAST_FORWARD,
        tooltip="Seek 10 seconds",
        icon_color="white",
        on_click=lambda e: current_episode.seek_episode()
    )
    ep_audio_controls = ft.Row(controls=[play_button, pause_button, seek_button])
    # Create the currently playing container
    currently_playing = ft.Container(content=ft.Text('test'))
    currently_playing.padding=ft.padding.only(bottom=5)

    def format_time(time):
        hours, remainder = divmod(int(time), 3600)
        minutes, seconds = divmod(remainder, 60)
        return f"{hours:02d}:{minutes:02d}:{seconds:02d}"


    def slider_changed(e):
        formatted_scrub = format_time(audio_scrubber.value)
        current_time.content = ft.Text(formatted_scrub)
        current_time.update()
        current_episode.time_scrub(audio_scrubber.value)

    podcast_length = ft.Container(content=ft.Text('doesntmatter'))
    current_time_text = ft.Text('placeholder')
    current_time = ft.Container(content=current_time_text)
    audio_scrubber = ft.Slider(min=0, expand=True,  max=current_episode.seconds, label="{value}", on_change=slider_changed)
    audio_scrubber.width = '100%'
    audio_scrubber_column = ft.Column(controls=[audio_scrubber])
    audio_scrubber_column.horizontal_alignment.STRETCH
    audio_scrubber_column.width = '100%'
    # Image for podcast playing
    audio_container_image_landing = ft.Image(src=f"/home/collinp/Documents/GitHub/PyPods/images/pinepods-logo.jpeg", width=40, height=40)
    audio_container_image = ft.Container(content=audio_container_image_landing)
    audio_container_image.border_radius = ft.border_radius.all(25)
    currently_playing_container = ft.Row(controls=[audio_container_image, currently_playing])
    scrub_bar_row = ft.Row(controls=[current_time, audio_scrubber_column, podcast_length])
    audio_controls_row = ft.Row(alignment=ft.MainAxisAlignment.CENTER, controls=[scrub_bar_row, ep_audio_controls])
    audio_container_row_landing = ft.Row(
                vertical_alignment=ft.CrossAxisAlignment.END,  
                alignment=ft.MainAxisAlignment.SPACE_BETWEEN,          
                controls=[currently_playing_container, audio_controls_row])
    audio_container_row = ft.Container(content=audio_container_row_landing)
    audio_container_row.padding=ft.padding.only(left=10)
    def page_checksize(e):
        if page.width <= 768:
            ep_height = 100
            ep_width = 4000
            audio_container.height = ep_height
            audio_container.content = ft.Column(
                horizontal_alignment=ft.CrossAxisAlignment.CENTER,          
                controls=[audio_container_image, currently_playing, audio_controls_row])
            audio_container.update()
            page.update()
        else:
            ep_height = 50
            ep_width = 4000
            audio_container.height = ep_height
            audio_container.content = audio_container_row
            audio_container.update()
    if page.width <= 768:
        ep_height = 100
        ep_width = 4000
        audio_container = ft.Container(
            height=ep_height,
            width=ep_width,
            bgcolor=active_user.main_color,
            border_radius=45,
            padding=6,
            content=ft.Column(
                horizontal_alignment=ft.CrossAxisAlignment.CENTER,          
                controls=[audio_container_image, currently_playing, audio_controls_row])
        )
    else:
        ep_height = 50
        ep_width = 4000
        audio_container = ft.Container(
            height=ep_height,
            width=ep_width,
            bgcolor=active_user.main_color,
            border_radius=45,
            padding=6,
            content=audio_container_row
        )

    page.on_resize = page_checksize
        
    # page.overlay.append(audio_container)
    page.overlay.append(ft.Stack([audio_container], bottom=20, right=20, left=70, expand=True))
    audio_container.visible = False


    # Various rows and columns for layout
    settings_row = ft.Row(vertical_alignment=ft.CrossAxisAlignment.START, controls=[refresh_ctn, banner_button])
    search_row = ft.Row(spacing=25, controls=[search_pods, search_btn])
    top_row = ft.Row(alignment=ft.MainAxisAlignment.SPACE_BETWEEN, vertical_alignment=ft.CrossAxisAlignment.START, controls=[settings_row, search_row])
    top_row_container = ft.Container(content=top_row, expand=True)
    top_row_container.padding=padding.only(left=60)
    audio_row = ft.Row(spacing=25, alignment=ft.MainAxisAlignment.CENTER, controls=[play_button, pause_button, seek_button])
    audio_controls_column = ft.Column(alignment=ft.MainAxisAlignment.END, controls=[audio_row])
    test_text = Text('This is a test')
    test_column = ft.Container(alignment=ft.alignment.bottom_center, border=ft.border.all(1, ft.colors.OUTLINE), content=test_text)


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
        check_downloads = database_functions.functions.check_downloaded(cnx, active_user.user_id, title, url)
        if check_downloads:
            page.snack_bar = ft.SnackBar(content=ft.Text(f"Episode is already downloaded!"))
            page.snack_bar.open = True
            page.update()
        else:
            pr = ft.ProgressRing()
            page.overlay.append(ft.Stack([pr], bottom=25, right=30, left=20, expand=True))
            page.update()
            current_episode.url = url
            current_episode.title = title
            current_episode.download_pod()
            page.snack_bar = ft.SnackBar(content=ft.Text(f"Episode: {title} has been downloaded!"))
            page.snack_bar.open = True
            page.overlay.pop(2)
            page.update()

        
    def delete_selected_episode(url, title, page):
        current_episode.url = url
        current_episode.title = title
        current_episode.delete_pod()
        page.snack_bar = ft.SnackBar(content=ft.Text(f"Episode: {title} has deleted!"))
        page.snack_bar.open = True
        page.update()

    def queue_selected_episode(url, title, artwork, page):
        current_episode.url = url
        current_episode.title = title
        current_episode.artwork = artwork
        current_episode.name = title
        current_episode.queue_pod(url)
        page.snack_bar = ft.SnackBar(content=ft.Text(f"Episode: {title} has been added to the queue!"))
        page.snack_bar.open = True
        page.update()

    def episode_remove_queue(url, title, page):
        current_episode.url = url
        current_episode.title = title
        current_episode.remove_queued_pod()
        page.snack_bar = ft.SnackBar(content=ft.Text(f"Episode: {title} has been removed from the queue!"))
        page.snack_bar.open = True
        page.update()

    def save_selected_episode(url, title, page):
        check_saved = database_functions.functions.check_saved(cnx, active_user.user_id, title, url)
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

    def remove_saved_episode(url, title, page):
        current_episode.url = url
        current_episode.title = title
        current_episode.remove_saved_pod()
        page.snack_bar = ft.SnackBar(content=ft.Text(f"Episode: {title} has been removed from saved podcasts!"))
        page.snack_bar.open = True
        page.update()
        

# Starting Page Layout

    top_bar = ft.Row(vertical_alignment=ft.CrossAxisAlignment.START, controls=[top_row_container])

    pypods_appbar = AppBar(title=Text("PinePods - A Forest of Podcasts, Rooted in the Spirit of Self-Hosting", color=active_user.accent_color), center_title=True, bgcolor=active_user.main_color)
    page.add(pypods_appbar)
    # page.appbar.visible = True
    # page.appbar.update()
    page.appbar.visible = False
    
    if login_screen == True:

        start_login(page)
    else:
        active_user.user_id = 1
        active_user.fullname = 'Guest User'
        go_homelogin(page)

# Browser Version
# ft.app(target=main, view=ft.WEB_BROWSER, port=8034)
# App version
ft.app(target=main, port=8034)