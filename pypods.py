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
import threading
import vlc
import random
import datetime
import html2text
from html.parser import HTMLParser

# Make login Screen start on boot
login_screen = False

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
        database_functions.functions.add_podcast(cnx, podcast_values)
            
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
                self.instance = vlc.Instance("--no-xlib") # Use "--no-xlib" option to run on server without GUI
                self.player = self.instance.media_player_new()
                self.thread = None
                self.length = length or ""
                self.length_min = 0
                self.length_max = 3000
                self.seconds = 1
                self.last_listen_duration_update = datetime.datetime.now()
                # self.episode_name = self.name
                if url is None or name is None:
                    self.active_pod = 'Initial Value'
                else:
                    self.active_pod = self.name
                self.queue = []
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
                self.instance = vlc.Instance("--no-xlib") # Use "--no-xlib" option to run on server without GUI
                self.player = self.instance.media_player_new()
                self.thread = None
                self.length = length or ""
                self.length_min = 0
                self.length_max = 3000
                self.seconds = 1
                self.last_listen_duration_update = datetime.datetime.now()
                # self.episode_name = self.name
                self.queue = []

        def play_episode(self, e=None, listen_duration=None):
            print(listen_duration)
            media = self.instance.media_new(self.url)
            media.parse_with_options(vlc.MediaParseFlag.network, 1000)  # wait for media to finish loading
            self.player.set_media(media)
            self.player.play()
            
            # Set the playback position to the given listen duration
            if listen_duration:
                print(f'in duration {listen_duration}')
                print(type(listen_duration))
                self.player.set_time(listen_duration * 1000)
            
            self.thread = threading.Thread(target=self._monitor_audio)
            self.thread.start()
            self.audio_playing = True

            self.record_history()

            time.sleep(1)

            # get the length of the media in milliseconds
            media_length = self.player.get_length()

            # convert milliseconds to a timedelta object
            delta = datetime.timedelta(milliseconds=media_length)

            # convert timedelta object to datetime object
            datetime_obj = datetime.datetime(1, 1, 1) + delta

            # format datetime object to hh:mm:ss format with two decimal places
            total_length = datetime_obj.strftime('%H:%M:%S')

            self.length = total_length
            self.toggle_current_status()
            page.update()
            
            # convert milliseconds to seconds
            total_seconds = media_length // 1000
            self.seconds = total_seconds
            audio_scrubber.max = self.seconds
            
            for i in range(total_seconds):
                self.current_progress = self.get_current_time()
                self.toggle_second_status()
                time.sleep(1)
                
                if (datetime.datetime.now() - self.last_listen_duration_update).total_seconds() > 15:
                    self.record_listen_duration()
                    self.last_listen_duration_update = datetime.datetime.now()




        def _monitor_audio(self):
            while True:
                state = self.player.get_state()
                if state == vlc.State.Ended:
                    self.thread = None
                    break
                time.sleep(1)

        def pause_episode(self, e=None):
            self.player.pause()
            self.audio_playing = False
            self.toggle_current_status()
            self.page.update()

        def resume_podcast(self, e=None):
            self.player.play()
            self.audio_playing = True
            self.toggle_current_status()
            self.page.update()

        def toggle_current_status(self):
            if self.audio_playing:
                play_button.visible = False
                pause_button.visible = True
                audio_container.bgcolor = active_user.main_color
                audio_container.visible = True
                currently_playing.content = ft.Text(self.name, color=active_user.nav_color1, size=16)
                current_time.content = ft.Text(self.length, color=active_user.nav_color1)
                podcast_length.content = ft.Text(self.length, color=active_user.nav_color1)
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
                self.page.update()
            else:
                pause_button.visible = False
                play_button.visible = True
                currently_playing.content = ft.Text(self.name, color=active_user.nav_color1)
                self.page.update()
                
        def toggle_second_status(self):
            audio_scrubber.value = self.get_current_seconds()
            audio_scrubber.update()
            current_time.content = ft.Text(self.current_progress, color=active_user.nav_color1)
            current_time.update()

            # self.page.update()

        def seek_episode(self):
            seconds = 10
            time = self.player.get_time()
            self.player.set_time(time + seconds * 1000) # VLC seeks in milliseconds

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
            self.player.set_time(time_ms)

        def record_history(self):
            user_id = get_user_id()
            database_functions.functions.record_podcast_history(cnx, self.name, user_id, 0)

        def download_pod(self):
            database_functions.functions.download_podcast(cnx, self.url, self.title, active_user.user_id)

        def delete_pod(self):
            database_functions.functions.delete_podcast(cnx, self.url, self.title, active_user.user_id)


        def queue_pod(self):
            if self.audio_playing:
                # Add the new episode URL to the vlc playlist
                media = self.instance.media_new(self.url)
                media_list = self.instance.media_list_new([media])
                media_list_player = self.instance.media_list_player_new()
                media_list_player.set_media_list(media_list)

                # Update the internal queue list
                self.queue.append(self.url)

                print(f"Added episode '{self.title}' to the queue")
            else:
                self.play_episode()

        def remove_queued_pod(self):
            # Get the current playlist and media player
            media_list_player = self.instance.media_list_player_new()
            media_list_player.set_media_player(self.player)
            media_list = self.instance.media_list_new()

            # Populate the media list with the current queue
            for url in self.queue:
                media = self.instance.media_new(url)
                media_list.add_media(media)

            media_list_player.set_media_list(media_list)

            # Iterate through the media list and remove the media object that corresponds to the URL
            for i in range(media_list.count()):
                media = media_list.item_at_index(i)
                if media.get_mrl() == self.url:
                    media_list.lock()
                    media_list.remove_index(i)
                    media_list.unlock()
                    break

            # Update the internal queue list
            self.queue.remove(self.url)

            print(f"Removed episode '{self.title}' from the queue")

            # Remove the episode from the database queue
            database_functions.functions.episode_remove_queue(cnx, active_user.user_id, self.url, self.title)


        def get_queue(self):
            return self.queue

        def get_current_time(self):
            time = self.player.get_time() // 1000  # convert milliseconds to seconds
            hours, remainder = divmod(time, 3600)
            minutes, seconds = divmod(remainder, 60)
            return f"{hours:02d}:{minutes:02d}:{seconds:02d}"

        def get_current_seconds(self):
            time_ms = self.player.get_time()  # get current time in milliseconds
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
            print(f'in seek {second}')
            self.player.set_time(int(second * 1000))


    def refresh_podcasts(e):
        pr = ft.ProgressRing()
        page.overlay.append(ft.Stack([pr], bottom=25, right=30, left=20, expand=True))
        page.update()
        database_functions.functions.refresh_pods(cnx)
        print('refresh complete')
        page.overlay.pop(2)
        page.update()
        # Reset current view if on homepage
        if page.route == "/" or page.route == "/":
            page.views.clear()

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
                artwork_no = random.randint(1, 12)
                none_artwork_url = os.path.join(script_dir, "images", "logo_random", f"{artwork_no}.jpeg")
                none_artwork_url_parsed = check_image(none_artwork_url)
                home_entry_artwork_url = ft.Image(src=none_artwork_url_parsed, width=150, height=150)
                home_ep_play_button = ft.IconButton(
                    icon=ft.icons.PLAY_DISABLED,
                    icon_color="blue400",
                    icon_size=40,
                    tooltip="No Episodes Added Yet"
                )
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
                    # do something with the episode information

                    home_entry_title = ft.Text(f'{home_pod_name} - {home_ep_title}', style=ft.TextThemeStyle.TITLE_MEDIUM)
                    home_entry_row = ft.ResponsiveRow([
    ft.Column(col={"sm": 6}, controls=[home_entry_title]),
])
                    home_entry_description = ft.Text(home_ep_desc)
                    home_entry_audio_url = ft.Text(home_ep_url)
                    home_entry_released = ft.Text(home_pub_date)

                    home_art_no = random.randint(1, 12)
                    home_art_fallback = os.path.join(script_dir, "images", "logo_random", f"{home_art_no}.jpeg")
                    home_art_url = home_ep_artwork if home_ep_artwork else home_art_fallback
                    home_art_parsed = check_image(home_art_url)
                    home_entry_artwork_url = ft.Image(src=home_art_parsed, width=150, height=150)
                    home_ep_play_button = ft.IconButton(
                        icon=ft.icons.PLAY_CIRCLE,
                        icon_color="blue400",
                        icon_size=40,
                        tooltip="Play Episode",
                        on_click=lambda x, url=home_ep_url, title=home_ep_title, artwork=home_ep_artwork: play_selected_episode(url, title, artwork)
                    )
                    home_popup_button = ft.PopupMenuButton(content=ft.Icon(ft.icons.ARROW_DROP_DOWN_CIRCLE_ROUNDED, color="blue400", size=40, tooltip="Play Episode"), 
                        items=[
                            ft.PopupMenuItem(icon=ft.icons.QUEUE, text="Queue", on_click=lambda x, url=home_ep_url, title=home_ep_title, artwork=home_ep_artwork: queue_selected_episode(url, title, artwork)),
                            ft.PopupMenuItem(icon=ft.icons.DOWNLOAD, text="Download", on_click=lambda x, url=home_ep_url, title=home_ep_title: download_selected_episode(url, title, page))
                        ]
                    )
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

            home_view = ft.View("/",                 [
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
            page.update()

    def evaluate_podcast(pod_title, pod_artwork, pod_author, pod_categories, pod_description, pod_episode_count, pod_feed_url, pod_website):
        global clicked_podcast
        clicked_podcast = Podcast(name=pod_title, artwork=pod_artwork, author=pod_author, description=pod_description, feedurl=pod_feed_url, website=pod_website)
        return clicked_podcast

    def get_user_id():
        current_username = active_user.username
        user_id = database_functions.functions.get_user_id(cnx, current_username)
        return user_id

    class Podcast:
        def __init__(self, name=None, artwork=None, author=None, description=None, feedurl=None, website=None):
            self.name = name
            self.artwork = artwork
            self.author = author
            self.description = description
            self.feedurl = feedurl
            self.website = website

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
        content=ft.Text("Usernames require at least 6 characters!"),
        actions=[
            ft.TextButton("Okay", on_click=close_invalid_dlg),
        ],
        actions_alignment=ft.MainAxisAlignment.END
    ) 
    password_invalid_dlg = ft.AlertDialog(
        modal=True,
        title=ft.Text("Password Invalid!"),
        content=ft.Text("Passwords require at least 8 characters, a capital letter and a special character!"),
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

    def open_history(e):
        page.go("/history")

    def open_episode_select(page, url, title):
        current_episode.url = url
        current_episode.title = title
        page.go("/episode_display")

    def open_pod_list(e):
        pr = ft.ProgressRing()
        page.overlay.append(ft.Stack([pr], bottom=25, right=30, left=20, expand=True))
        page.update()
        page.go("/pod_list")

    def go_homelogin(page):
        # navbar.visible = True
        active_user.theme_select()
        print(active_user.main_color)
        # Theme user elements
        pypods_appbar.bgcolor = active_user.main_color
        pypods_appbar.color = active_user.accent_color
        refresh_btn.icon_color = active_user.font_color
        banner_button.bgcolor = active_user.accent_color
        banner_button.color = active_user.main_color
        page.banner.bgcolor = active_user.accent_color
        page.banner.leading = ft.Icon(ft.icons.WAVING_HAND, color=active_user.main_color, size=40)
        page.banner.content = ft.Text("""
    Welcome to PyPods! PyPods is an app built to save, listen, download, organize, and manage a selection of podcasts. Using the search function you can search for your favorite podcast, from there, click the add button to save your podcast to the database. Pypods will begin displaying new episodes of that podcast from then on to the homescreen when released. In addition, from search you can click on a podcast to view and listen to specific episodes. From the sidebar you can select your saved podcasts and manage them, view and manage your downloaded podcasts, edit app settings, check your listening history, and listen through episodes from your saved 'queue.' For comments, feature requests, pull requests, and bug reports please open an issue, for fork PyPods from the repository:
    """, color=active_user.main_color
        )
        page.banner.actions = [
            ft.ElevatedButton('Open PyPods Repo', on_click=open_repo, bgcolor=active_user.main_color, color=active_user.accent_color),
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
        page.overlay.append(ft.Stack([navbar], expand=True))
        page.update()
        page.go("/")

    def go_theme_rebuild(page):
        # navbar.visible = True
        active_user.theme_select()
        print(active_user.main_color)
        # Theme user elements
        pypods_appbar.bgcolor = active_user.main_color
        pypods_appbar.color = active_user.accent_color
        refresh_btn.icon_color = active_user.font_color
        banner_button.bgcolor = active_user.accent_color
        banner_button.color = active_user.main_color
        page.banner.bgcolor = active_user.accent_color
        page.banner.leading = ft.Icon(ft.icons.WAVING_HAND, color=active_user.main_color, size=40)
        page.banner.content = ft.Text("""
    Welcome to PyPods! PyPods is an app built to save, listen, download, organize, and manage a selection of podcasts. Using the search function you can search for your favorite podcast, from there, click the add button to save your podcast to the database. Pypods will begin displaying new episodes of that podcast from then on to the homescreen when released. In addition, from search you can click on a podcast to view and listen to specific episodes. From the sidebar you can select your saved podcasts and manage them, view and manage your downloaded podcasts, edit app settings, check your listening history, and listen through episodes from your saved 'queue.' For comments, feature requests, pull requests, and bug reports please open an issue, for fork PyPods from the repository:
    """, color=active_user.main_color
        )
        page.banner.actions = [
            ft.ElevatedButton('Open PyPods Repo', on_click=open_repo, bgcolor=active_user.main_color, color=active_user.accent_color),
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
        page.overlay.append(ft.Stack([navbar], expand=True))
        page.update()
        page.go("/")

    def go_home(e):
        page.update()
        page.go("/")

    def route_change(e):

        page.views.clear()
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
                home_entry_title = ft.TextButton(text=f'{home_pod_name} - {home_ep_title}', style=ft.TextThemeStyle.TITLE_MEDIUM)
                home_entry_description = ft.Text(home_ep_desc)
                home_entry_audio_url = ft.Text(home_ep_url)
                home_entry_released = ft.Text(home_pub_date)
                artwork_no = random.randint(1, 12)
                none_artwork_url = os.path.join(script_dir, "images", "logo_random", f"{artwork_no}.jpeg")
                none_artwork_url_parsed = check_image(none_artwork_url)
                home_entry_artwork_url = ft.Image(src=none_artwork_url_parsed, width=150, height=150)
                home_ep_play_button = ft.IconButton(
                    icon=ft.icons.PLAY_DISABLED,
                    icon_color=active_user.accent_color,
                    icon_size=40,
                    tooltip="No Episodes Added Yet"
                )
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
                            home_entry_description = ft.Text(home_ep_desc, color=active_user.font_color)

                    else:
                        if is_html(home_ep_desc):
                            # convert HTML to Markdown
                            markdown_desc = html2text.html2text(home_ep_desc)
                            # add inline style to change font color
                            home_entry_description = ft.Markdown(markdown_desc, on_tap_link=launch_clicked_url)
                        else:
                            # display plain text
                            markdown_desc = home_ep_desc
                            home_entry_description = ft.Text(home_ep_desc, color=active_user.font_color)

                    home_entry_audio_url = ft.Text(home_ep_url, color=active_user.font_color)
                    check_episode_playback, listen_duration = database_functions.functions.check_episode_playback(cnx, active_user.user_id, home_ep_title, home_ep_url)
                    home_entry_released = ft.Text(home_pub_date, color=active_user.font_color)

                    home_art_no = random.randint(1, 12)
                    home_art_fallback = os.path.join(script_dir, "images", "logo_random", f"{home_art_no}.jpeg")
                    home_art_url = home_ep_artwork if home_ep_artwork else home_art_fallback
                    home_art_parsed = check_image(home_art_url)
                    home_entry_artwork_url = ft.Image(src=home_art_parsed, width=150, height=150)
                    home_ep_play_button = ft.IconButton(
                        icon=ft.icons.PLAY_CIRCLE,
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
                            ft.PopupMenuItem(icon=ft.icons.QUEUE, text="Queue", on_click=lambda x, url=home_ep_url, title=home_ep_title, artwork=home_ep_artwork: queue_selected_episode(url, title, artwork)),
                            ft.PopupMenuItem(icon=ft.icons.DOWNLOAD, text="Download", on_click=lambda x, url=home_ep_url, title=home_ep_title: download_selected_episode(url, title, page))
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

        if page.route == "/login" or page.route == "/login":
            login_startpage = Column(
                alignment=ft.MainAxisAlignment.CENTER,
                horizontal_alignment=ft.CrossAxisAlignment.CENTER,
                controls=[
                    Card(
                        elevation=15,
                        content=Container(
                            width=550,
                            height=550,
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
                                        "Pypods: A podcast app built in python",
                                        size=32,
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
                                                on_click=lambda e: active_user.login(login_username.value, login_password.value)
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
                        pod_ep_title = ft.Text('Episode Count:', weight=ft.FontWeight.BOLD)
                        pod_ep_count = ft.Text(d['episodeCount'])
                        pod_ep_info = ft.Row(controls=[pod_ep_title, pod_ep_count])
                        add_pod_button = ft.IconButton(
                            icon=ft.icons.ADD_BOX,
                            icon_color="blue400",
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
            theme_drop = ft.Dropdown(width=150, border_color=active_user.accent_color, color=active_user.font_color, focused_bgcolor=active_user.main_color, focused_border_color=active_user.accent_color, focused_color=active_user.accent_color, 
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
                            alignment=ft.MainAxisAlignment.CENTER,
                            controls=[theme_column])

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
            user_name = ft.TextField(label="Full Name", icon=ft.icons.CARD_MEMBERSHIP, hint_text='John Pypods', border_color=active_user.accent_color, color=active_user.accent_color, focused_bgcolor=active_user.accent_color, focused_color=active_user.accent_color, focused_border_color=active_user.accent_color, cursor_color=active_user.accent_color )
            user_email = ft.TextField(label="email", icon=ft.icons.EMAIL, hint_text='ilovepypods@pypods.com', border_color=active_user.accent_color, color=active_user.accent_color, focused_bgcolor=active_user.accent_color, focused_color=active_user.accent_color, focused_border_color=active_user.accent_color, cursor_color=active_user.accent_color )
            user_username = ft.TextField(label="Username", icon=ft.icons.PERSON, hint_text='pypods_user1999', border_color=active_user.accent_color, color=active_user.accent_color, focused_bgcolor=active_user.accent_color, focused_color=active_user.accent_color, focused_border_color=active_user.accent_color, cursor_color=active_user.accent_color )
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
                            alignment=ft.MainAxisAlignment.CENTER,
                            controls=[user_column])


            # Create search view object
            settings_view = ft.View("/settings",
                    [
                        pypods_appbar,
                        user_setting_text,
                        theme_row,
                        admin_setting_text,
                        user_row,
                    ]
                    
                )
            settings_view.bgcolor = active_user.bgcolor
            settings_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                settings_view
                    
                )

        if page.route == "/poddisplay" or page.route == "/poddisplay":
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

            feed_row_content = ft.ResponsiveRow([
            ft.Column(col={"md": 4}, controls=[pod_image]),
            ft.Column(col={"md": 8}, controls=[pod_feed_title, pod_feed_desc, pod_feed_site]),
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

                entry_title = ft.Text(parsed_title, style=ft.TextThemeStyle.TITLE_MEDIUM)
                entry_description = ft.Text(parsed_description)
                entry_audio_url = ft.Text(parsed_audio_url)
                entry_released = ft.Text(parsed_release_date)
                display_art_entry_parsed = check_image(display_art_url)
                entry_artwork_url = ft.Image(src=display_art_entry_parsed, width=150, height=150)
                
                # current_episode = Toggle_Pod(page, go_home, parsed_audio_url, parsed_title)
                ep_play_button = ft.IconButton(
                    icon=ft.icons.PLAY_CIRCLE,
                    icon_color="blue400",
                    icon_size=40,
                    tooltip="Play Episode",
                    on_click = lambda x, url=parsed_audio_url, title=parsed_title, artwork=parsed_artwork_url: play_selected_episode(url, title, artwork)
                )
                ep_popup_button = ft.PopupMenuButton(content=ft.Icon(ft.icons.ARROW_DROP_DOWN_CIRCLE_ROUNDED, color="blue400", size=40, tooltip="Play Episode"), 
                        items=[
                        ft.PopupMenuItem(icon=ft.icons.QUEUE, text="Queue", on_click=lambda x, url=entry_audio_url, title=entry_title, artwork=entry_artwork_url: queue_selected_episode(url, title, artwork)),
                        ft.PopupMenuItem(icon=ft.icons.DOWNLOAD, text="Download", on_click=lambda x, url=entry_audio_url, title=entry_title: download_selected_episode(url, title))
                    ]
                )
                ep_play_options = ft.Row(controls=[ep_play_button, ep_popup_button])

                ep_row_content = ft.ResponsiveRow([
                    ft.Column(col={"md": 2}, controls=[entry_artwork_url]),
                    ft.Column(col={"md": 10}, controls=[entry_title, entry_description, entry_released, ep_play_options])])
                
                ep_row = ft.Container(content=ep_row_content)
                ep_row.padding=padding.only(left=70, right=50)
                ep_rows.append(ep_row)
                ep_row_dict[f'search_row{ep_number}'] = ep_row
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
                pod_list_desc = "Looks like you haven't added any podcasts yet. Search for podcasts you enjoy in the upper right portion of the screen and click the plus button to add them. They will begin to show up here and new episodes will be put into the main feed. You'll also be able to start downloading episodes as well as queueing them. Enjoy the listening!"
                pod_list_ep_count = 'Start Searching!'
                pod_list_website = "https://github.com/madeofpendletonwool/pypods"
                pod_list_feed = ""
                pod_list_author = "Pypods"
                pod_list_categories = ""

                # Parse webpages needed to extract podcast artwork
                pod_list_art_parsed = check_image(pod_list_artwork)
                pod_list_artwork_image = ft.Image(src=pod_list_art_parsed, width=150, height=150)

                # Defining the attributes of each podcast that will be displayed on screen
                pod_list_title_display = ft.Text(pod_list_title)
                pod_list_desc_display = ft.Text(pod_list_desc)
                # Episode Count and subtitle
                pod_list_ep_title = ft.Text('Pypods:', weight=ft.FontWeight.BOLD)
                pod_list_ep_count_display = ft.Text(pod_list_ep_count)
                pod_list_ep_info = ft.Row(controls=[pod_list_ep_title, pod_list_ep_count_display])
                remove_pod_button = ft.IconButton(
                    icon=ft.icons.EMOJI_EMOTIONS,
                    icon_color="blue400",
                    icon_size=40,
                    tooltip="Remove Podcast"
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
                    pod_list_ep_title = ft.Text('Episode Count:', weight=ft.FontWeight.BOLD)
                    pod_list_ep_count_display = ft.Text(pod_list_ep_count)
                    pod_list_ep_info = ft.Row(controls=[pod_list_ep_title, pod_list_ep_count_display])
                    remove_pod_button = ft.IconButton(
                        icon=ft.icons.INDETERMINATE_CHECK_BOX,
                        icon_color="red400",
                        icon_size=40,
                        tooltip="Remove Podcast",
                        on_click=lambda x, title=pod_list_title: database_functions.functions.remove_podcast(cnx, title)
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
                    pod_list_number += 1
            pod_view_title = ft.Text(
            "Added Podcasts:",
            size=30,
            font_family="RobotoSlab",
            weight=ft.FontWeight.W_300,
        )
            pod_view_row = ft.Row(controls=[pod_view_title], alignment=ft.MainAxisAlignment.CENTER)


            # Create search view object
            pod_list_view = ft.View("/pod_list",
                    [
                        pypods_appbar,
                        pod_view_row,
                        *[pod_list_dict[f'pod_list_row{i+1}'] for i in range(len(pod_list_rows))]

                    ]
                    
                )
            pod_list_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                pod_list_view
                    
                )

        if page.route == "/history" or page.route == "/history":

            # Get Pod info
            user_id = get_user_id()
            hist_episodes = database_functions.functions.user_history(cnx, user_id)
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
                    icon_color="blue400",
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
                    hist_entry_title = ft.Text(f'{hist_pod_name} - {hist_ep_title}', style=ft.TextThemeStyle.TITLE_MEDIUM)
                    hist_entry_description = ft.Text(hist_ep_desc)
                    hist_entry_audio_url = ft.Text(hist_ep_url)
                    check_episode_playback, listen_duration = database_functions.functions.check_episode_playback(cnx, user_id, hist_ep_title, hist_ep_url)
                    hist_art_no = random.randint(1, 12)
                    hist_art_fallback = os.path.join(script_dir, "images", "logo_random", f"{hist_art_no}.jpeg")
                    hist_art_url = hist_ep_artwork if hist_ep_artwork else hist_art_fallback
                    hist_art_url_parsed = check_image(hist_art_url)
                    hist_entry_artwork_url = ft.Image(src=hist_art_url_parsed, width=150, height=150)
                    hist_ep_play_button = ft.IconButton(
                        icon=ft.icons.NOT_STARTED,
                        icon_color=active_user.main_color,
                        icon_size=40,
                        tooltip="Start Episode From Beginning",
                        on_click=lambda x, url=hist_ep_url, title=hist_ep_title, artwork=hist_ep_artwork: play_selected_episode(url, title, artwork)
                    )
                    hist_ep_resume_button = ft.IconButton(
                        icon=ft.icons.PLAY_CIRCLE,
                        icon_color=active_user.main_color,
                        icon_size=40,
                        tooltip="Resume Episode",
                        on_click=lambda x, url=hist_ep_url, title=hist_ep_title, artwork=hist_ep_artwork, listen_duration=listen_duration: resume_selected_episode(url, title, artwork, listen_duration)
                    )
                    hist_popup_button = ft.PopupMenuButton(content=ft.Icon(ft.icons.ARROW_DROP_DOWN_CIRCLE_ROUNDED, color=active_user.main_color, size=40, tooltip="Play Episode"), 
                        items=[
                            ft.PopupMenuItem(icon=ft.icons.QUEUE, text="Queue", on_click=lambda x, url=hist_ep_url, title=hist_ep_title, artwork=home_ep_artwork: queue_selected_episode(url, title, artwork)),
                            ft.PopupMenuItem(icon=ft.icons.DOWNLOAD, text="Download", on_click=lambda x, url=hist_ep_url, title=hist_ep_title: download_selected_episode(url, title))
                        ]
                    )
                    
                    if check_episode_playback == True:
                        listen_prog = seconds_to_time(listen_duration)
                        hist_ep_prog = seconds_to_time(hist_ep_duration)
                        progress_value = get_progress(listen_duration, hist_ep_duration)
                        hist_entry_listened = ft.Text(f'Listened on: {hist_ep_listen_date}')
                        hist_entry_progress = ft.Row(controls=[ft.Text(listen_prog), ft.ProgressBar(expand=True, value=progress_value, color=active_user.main_color), ft.Text(hist_ep_prog)])
                        hist_ep_row_content = ft.ResponsiveRow([
                            ft.Column(col={"md": 2}, controls=[hist_entry_artwork_url]),
                            ft.Column(col={"md": 10}, controls=[hist_entry_title, hist_entry_description, hist_entry_listened, hist_entry_progress, ft.Row(controls=[hist_ep_play_button, hist_ep_resume_button, hist_popup_button])]),
                        ])
                    else:
                        hist_entry_listened = ft.Text(f'Listened on: {hist_ep_listen_date}')
                        hist_ep_row_content = ft.ResponsiveRow([
                            ft.Column(col={"md": 2}, controls=[hist_entry_artwork_url]),
                            ft.Column(col={"md": 10}, controls=[hist_entry_title, hist_entry_description, hist_entry_listened, ft.Row(controls=[hist_ep_play_button, hist_popup_button])]),
                        ])
                    hist_ep_row = ft.Container(content=hist_ep_row_content)
                    hist_ep_row.padding=padding.only(left=70, right=50)
                    hist_ep_rows.append(hist_ep_row)
                    hist_ep_row_dict[f'search_row{hist_ep_number}'] = hist_ep_row
                    hist_pods_active = True
                    hist_ep_number += 1

            history_title = ft.Text(
            "Listen History:",
            size=30,
            font_family="RobotoSlab",
            weight=ft.FontWeight.W_300,
        )
            history_title_row = ft.Row(controls=[history_title], alignment=ft.MainAxisAlignment.CENTER)

            # Create search view object
            ep_hist_view = ft.View("/history",
                    [
                        pypods_appbar,
                        history_title_row,
                        *[hist_ep_row_dict.get(f'search_row{i+1}') for i in range(len(hist_ep_rows))]

                    ]
                    
                )
            ep_hist_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                ep_hist_view
                    
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
                    
                    # do something with the episode information

                    download_entry_title = ft.Text(f'{download_pod_name} - {download_ep_title}', style=ft.TextThemeStyle.TITLE_MEDIUM)
                    download_entry_description = ft.Text(download_ep_desc)
                    download_entry_audio_url = ft.Text(download_ep_url)
                    download_entry_released = ft.Text(download_pub_date)

                    download_art_no = random.randint(1, 12)
                    download_art_fallback = os.path.join(script_dir, "images", "logo_random", f"{download_art_no}.jpeg")
                    download_art_url = download_ep_artwork if download_ep_artwork else download_art_fallback
                    download_art_parsed = check_image(download_art_url)
                    download_entry_artwork_url = ft.Image(src=download_art_parsed, width=150, height=150)
                    download_ep_play_button = ft.IconButton(
                        icon=ft.icons.PLAY_CIRCLE,
                        icon_color="blue400",
                        icon_size=40,
                        tooltip="Play Episode",
                        on_click=lambda x, url=download_ep_local_url, title=download_ep_title, artwork=download_ep_artwork: play_selected_episode(url, title, artwork)
                    )
                    download_popup_button = ft.PopupMenuButton(content=ft.Icon(ft.icons.ARROW_DROP_DOWN_CIRCLE_ROUNDED, color="blue400", size=40, tooltip="Play Episode"), 
                        items=[
                            ft.PopupMenuItem(icon=ft.icons.QUEUE, text="Queue", on_click=lambda x, url=download_ep_url, title=download_ep_title, artwork=home_ep_artwork: queue_selected_episode(url, title, artwork)),
                            ft.PopupMenuItem(icon=ft.icons.DOWNLOAD, text="Download", on_click=lambda x, url=download_ep_url, title=download_ep_title: delete_selected_episode(url, title))
                        ]
                    )
                    download_ep_row_content = ft.ResponsiveRow([
                        ft.Column(col={"md": 2}, controls=[download_entry_artwork_url]),
                        ft.Column(col={"md": 10}, controls=[download_entry_title, download_entry_description, download_entry_released, ft.Row(controls=[download_ep_play_button, download_popup_button])]),
                    ])
                    download_ep_row = ft.Container(content=download_ep_row_content)
                    download_ep_row.padding=padding.only(left=70, right=50)
                    download_ep_rows.append(download_ep_row)
                    download_ep_row_dict[f'search_row{download_ep_number}'] = download_ep_row
                    download_pods_active = True
                    download_ep_number += 1

            download_title = ft.Text(
            "Downloaded Episodes:",
            size=30,
            font_family="RobotoSlab",
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
                # queue_ep_row = ft.Row(
                #     alignment=ft.MainAxisAlignment.CENTER,
                #     controls=[queue_entry_artwork_url, queue_ep_column, queue_ep_play_button]
                # )
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

                    queue_entry_title = ft.Text(f'{queue_pod_name} - {queue_ep_title}', style=ft.TextThemeStyle.TITLE_MEDIUM, color=active_user.font_color)
                    queue_entry_description = ft.Text(queue_ep_desc, color=active_user.font_color)
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
                            ft.PopupMenuItem(icon=ft.icons.QUEUE, text="Remove From Queue", on_click=lambda x, url=queue_ep_url, title=queue_ep_title: episode_remove_queue(url, title)),
                            ft.PopupMenuItem(icon=ft.icons.DOWNLOAD, text="Download Episode", on_click=lambda x, url=queue_ep_url, title=queue_ep_title: delete_selected_episode(url, title))
                        ]
                    )
                    if check_episode_playback == True:
                        listen_prog = seconds_to_time(listen_duration)
                        queue_ep_prog = seconds_to_time(queue_ep_duration)
                        progress_value = get_progress(listen_duration, queue_ep_duration)
                        queue_entry_progress = ft.Row(controls=[ft.Text(listen_prog, color=active_user.font_color), ft.ProgressBar(expand=True, value=progress_value, color=active_user.main_color), ft.Text(queue_ep_prog, color=active_user.font_color)])
                        queue_ep_row_content = ft.ResponsiveRow([
                            ft.Column(col={"md": 2}, controls=[queue_entry_artwork_url]),
                            ft.Column(col={"md": 10}, controls=[queue_entry_title, queue_entry_description, queue_entry_released, queue_entry_progress, ft.Row(controls=[queue_ep_play_button, queue_ep_resume_button, queue_popup_button])]),
                        ])
                    else:
                        queue_ep_row_content = ft.ResponsiveRow([
                            ft.Column(col={"md": 2}, controls=[queue_entry_artwork_url]),
                            ft.Column(col={"md": 10}, controls=[queue_entry_title, queue_entry_description, queue_entry_released, ft.Row(controls=[queue_ep_play_button, queue_popup_button])]),
                        ])
                    queue_div_row = ft.Divider(color=active_user.accent_color)
                    queue_ep_column = ft.Column(controls=[queue_ep_row_content, queue_div_row])
                    queue_ep_row = ft.Container(content=queue_ep_row_content)
                    queue_ep_row.padding=padding.only(left=70, right=50)
                    queue_ep_rows.append(queue_ep_row)
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
                    ft.PopupMenuItem(icon=ft.icons.QUEUE, text="Queue", on_click=lambda x, url=ep_url, title=ep_title, artwork=ep_artwork: queue_selected_episode(url, title, artwork)),
                    ft.PopupMenuItem(icon=ft.icons.DOWNLOAD, text="Download", on_click=lambda x, url=ep_url, title=ep_title: download_selected_episode(url, title))
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
    Welcome to PyPods! PyPods is an app built to save, listen, download, organize, and manage a selection of podcasts. Using the search function you can search for your favorite podcast, from there, click the add button to save your podcast to the database. Pypods will begin displaying new episodes of that podcast from then on to the homescreen when released. In addition, from search you can click on a podcast to view and listen to specific episodes. From the sidebar you can select your saved podcasts and manage them, view and manage your downloaded podcasts, edit app settings, check your listening history, and listen through episodes from your saved 'queue.' For comments, feature requests, pull requests, and bug reports please open an issue, for fork PyPods from the repository:
    """, color=colors.BLACK
        ),
        actions=[
            ft.TextButton('Open PyPods Repo', on_click=open_repo),
            ft.IconButton(icon=ft.icons.EXIT_TO_APP, on_click=close_banner)
        ],
    )

    def show_banner_click(e):
        page.banner.open = True
        page.update()

    banner_button = ft.ElevatedButton("Help!", on_click=show_banner_click)

# Login/User Changes------------------------------------------------------
    class User:
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

    # New User Stuff ----------------------------

        def set_username(self, new_username):
            self.username = new_username

        def set_password(self, new_password):
            self.password = new_password

        def set_email(self, new_email):
            self.email = new_email

        def set_name(self, new_name):
            self.fullname = new_name

        def verify_user_values(self):
            self.valid_username = len(self.username) >= 6
            self.valid_password = len(self.password) >= 8 and any(c.isupper() for c in self.password) and any(c.isdigit() for c in self.password)
            regex = r"^[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+$"
            self.valid_email = re.match(regex, self.email) is not None
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

        def user_created_prompt(self):
            if self.new_user_valid == True:
                self.page.dialog = user_dlg
                user_dlg.open = True
                self.page.update()
                

        def popup_user_values(self, e):
            pass

        def create_user(self):
            if self.new_user_valid == True:
                salt, hash_pw = Auth.Passfunctions.hash_password(self.password)
                user_values = (self.fullname, self.username, self.email, hash_pw, salt)
                database_functions.functions.add_user(cnx, user_values)

    # Active User Stuff --------------------------

        def get_initials(self):
            # split the full name into separate words
            words = self.fullname.split()
            
            # extract the first letter of each word and combine them
            initials_lower = "".join(word[0] for word in words)
            
            # return the initials as uppercase
            self.initials = initials_lower.upper()

        def login(self, username, password):
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

    # Setup Theming-------------------------------------------------------
        def theme_select(self):
            active_theme = database_functions.functions.get_theme(cnx, self.user_id)
            print(active_theme)
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
            print(theme)
            database_functions.functions.set_theme(cnx, self.user_id, theme)
            self.theme_select
            go_theme_rebuild(self.page)
            self.page.update()

        def logout_pypods(self, e):
            pass

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
                        value=(f'PyPods'),
                        size=10,
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
                        weight="bold",
                    ),
                ),
                    Divider(height=5, color="transparent"),
                    self.ContainedIcon('Home', icons.HOME, "Home", go_home),
                    self.ContainedIcon('Queue', icons.QUEUE, "Queue", open_queue),
                    self.ContainedIcon('Downloaded',icons.DOWNLOAD, "Downloaded", open_downloads),
                    self.ContainedIcon('Podcast History', icons.HISTORY, "Podcast History", open_history),
                    self.ContainedIcon('Added Podcasts', icons.PODCASTS, "Added Podcasts", open_pod_list),
                    Divider(color="white24", height=5),
                    self.ContainedIcon('Settings', icons.SETTINGS, "Settings", open_settings),
                    self.ContainedIcon('Logout', icons.LOGOUT_ROUNDED, "Logout", active_user.logout_pypods),
                ],
            ),
        )



# Create Page--------------------------------------------------------


    page.title = "PyPods"
    theme_icon_button = ft.IconButton(icons.DARK_MODE, selected_icon=icons.LIGHT_MODE, icon_color=colors.BLACK,
                                   icon_size=35, tooltip="change theme", on_click=change_theme,
                                   style=ButtonStyle(color={"": colors.BLACK, "selected": colors.WHITE}, ), )

    page.appbar = AppBar(title=Text("Pypods - A Python based podcast app!", color="white"), center_title=True, bgcolor="green",
                        actions=[theme_icon_button], )

    page.title = "PyPods - A python based podcast app!"

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

    def load_podcast():
        pass

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
    current_time = ft.Container(content=ft.Text('placeholder'))
    audio_scrubber = ft.Slider(min=0, expand=True,  max=current_episode.seconds, label="{value}", on_change=slider_changed)
    audio_scrubber.width = '100%'
    audio_scrubber_column = ft.Column(controls=[audio_scrubber])
    audio_scrubber_column.horizontal_alignment.STRETCH
    audio_scrubber_column.width = '100%'
    # Image for podcast playing
    audio_container_image_landing = ft.Image(src=f"/home/collinp/Documents/GitHub/PyPods/images/Pypods-logos_blue.png", width=40, height=40)
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
        print(f'in resume episode {listen_duration}')
        current_episode.play_episode(listen_duration=listen_duration)


    def download_selected_episode(url, title, page):
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

        
    def delete_selected_episode(url, title):
        current_episode.url = url
        current_episode.title = title
        current_episode.delete_pod()

    def queue_selected_episode(url, title, artwork):
        current_episode.url = url
        current_episode.title = title
        current_episode.artwork = artwork
        current_episode.name = title
        current_episode.queue_pod()

    def episode_remove_queue(url, title):
        current_episode.url = url
        current_episode.title = title
        current_episode.remove_queued_pod()
        

# Starting Page Layout

    top_bar = ft.Row(vertical_alignment=ft.CrossAxisAlignment.START, controls=[top_row_container])

    pypods_appbar = AppBar(title=Text("PyPods - A Python based podcast app!", color=active_user.accent_color), center_title=True, bgcolor=active_user.main_color,
                            actions=[theme_icon_button])
    # pypods_appbar = ft.Container(content=pypods_app)
    # pypods_appbar.border = ft.border.only(bottom=ft.border.BorderSide(4, active_user.tertiary_color))
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