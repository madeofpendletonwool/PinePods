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

    def user_created_prompt(e):
        page.dialog = user_dlg
        user_dlg.open = True
        page.update() 

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

    def launch_pod_site(e):
        page.launch_url(clicked_podcast.website)

    def check_image(artwork_path):
        if artwork_path.startswith('http'):
            # It's a URL, so return the path with the proxy URL appended
            return f"{proxy_url}{artwork_path}"
        else:
            # It's a local file path, so return the path as is
            return artwork_path

    class Toggle_Pod:
        initialized = False

        def __init__(self, page, go_home, url=None, name=None):
            if not Toggle_Pod.initialized:
                self.page = page
                self.go_home = go_home
                self.url = url
                self.name = name or ""
                self.audio_playing = False
                self.episode_file = url
                self.episode_name = name
                self.instance = vlc.Instance("--no-xlib") # Use "--no-xlib" option to run on server without GUI
                self.player = self.instance.media_player_new()
                self.thread = None
                # self.episode_name = self.name
                if url is None or name is None:
                    self.active_pod = 'Initial Value'
                else:
                    self.active_pod = self.name
                Toggle_Pod.initialized = True
            else:
                self.page = page
                self.go_home = go_home
                self.url = url
                self.name = name or ""
                self.audio_playing = False
                self.active_pod = self.name
                self.episode_file = url
                self.episode_name = name
                self.instance = vlc.Instance("--no-xlib") # Use "--no-xlib" option to run on server without GUI
                self.player = self.instance.media_player_new()
                self.thread = None
                # self.episode_name = self.name

        def play_episode(self, e=None):
            media = self.instance.media_new(self.url)
            self.player.set_media(media)
            self.player.play()
            self.thread = threading.Thread(target=self._monitor_audio)
            self.thread.start()
            self.audio_playing = True
            self.toggle_current_status()
            self.record_history()

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
                audio_container.visible = True
                currently_playing.content = ft.Text(self.name)
                self.page.update()
            else:
                pause_button.visible = False
                play_button.visible = True
                currently_playing.content = ft.Text(self.name)
                self.page.update()

        def seek_episode(self):
            seconds = 10
            time = self.player.get_time()
            self.player.set_time(time + seconds * 1000) # VLC seeks in milliseconds

        def record_history(self):
            user_id = get_user_id()
            database_functions.functions.record_podcast_history(cnx, self.name, user_id, 0)

        def download_pod(self):
            database_functions.functions.download_podcast(cnx, self.url, self.title, active_user.user_id)

        def delete_pod(self):
            database_functions.functions.delete_podcast(cnx, self.url, self.title, active_user.user_id)


        # def queue_pod(self):

    def refresh_podcasts(e):
        pr = ft.ProgressRing()
        page.overlay.append(ft.Stack([pr], bottom=25, right=30, left=20, expand=True))
        page.update()
        database_functions.functions.refresh_pods(cnx)
        print('refresh complete')
        page.overlay.pop(2)
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
            ft.TextButton("Okay", on_click=close_dlg),
        ],
        actions_alignment=ft.MainAxisAlignment.END,
        on_dismiss=lambda e: go_home
    )   

#---Code for Theme Change----------------------------------------------------------------

    def change_theme(e):
        """
        When the button(to change theme) is clicked, the progress bar is made visible, the theme is changed,
        the progress bar is made invisible, and the page is updated

        :param e: The event that triggered the function
        """
        # page.splash.visible = True
        page.update()
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

    def open_pod_list(e):
        pr = ft.ProgressRing()
        page.overlay.append(ft.Stack([pr], bottom=25, right=30, left=20, expand=True))
        page.update()
        page.go("/pod_list")

    def go_homelogin(page):
        # navbar.visible = True
        page.appbar.visible = True
        navbar = NavBar(page).create_navbar()
        page.overlay.append(ft.Stack([navbar], expand=True))
        page.update()
        page.go("/")

    def go_home(e):
        page.update()
        page.go("/")

    def route_change(e):

        page.views.clear()
        if page.route == "/" or page.route == "/":

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
                home_play_options = ft.Column(controls=[home_ep_play_button, home_popup_button])
                # Creating column and row for home layout
                home_ep_column = ft.Column(
                    controls=[home_entry_title, home_entry_description, home_entry_released]
                )
                home_ep_row = ft.Row(
                    alignment=ft.MainAxisAlignment.CENTER,
                    controls=[home_entry_artwork_url, home_ep_column, home_ep_play_button]
                )
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

                    home_entry_title = ft.Text(f'{home_pod_name} - {home_ep_title}', width=600, style=ft.TextThemeStyle.TITLE_MEDIUM)
                    home_entry_description = ft.Text(home_ep_desc, width=800)
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
                        on_click=lambda x, url=home_ep_url, title=home_ep_title: play_selected_episode(url, title)
                    )
                    home_popup_button = ft.PopupMenuButton(icon=ft.icons.ARROW_DROP_DOWN_CIRCLE_ROUNDED, 
                    # icon_size=40, icon_color="blue400", tooltip="Options",
                        items=[
                            ft.PopupMenuItem(icon=ft.icons.QUEUE, text="Queue", on_click=lambda x, url=home_ep_url, title=home_ep_title: queue_selected_episode(url, title)),
                            ft.PopupMenuItem(icon=ft.icons.DOWNLOAD, text="Download", on_click=lambda x, url=home_ep_url, title=home_ep_title: download_selected_episode(url, title))
                        ]
                    )
                    home_play_options = ft.Column(controls=[home_ep_play_button, home_popup_button])
                    # Creating column and row for home layout
                    home_ep_column = ft.Column(
                        controls=[home_entry_title, home_entry_description, home_entry_released]
                    )
                    home_ep_row = ft.Row(
                        alignment=ft.MainAxisAlignment.CENTER,
                        controls=[home_entry_artwork_url, home_ep_column, home_play_options]
                    )
                    home_ep_rows.append(home_ep_row)
                    home_ep_row_dict[f'search_row{home_ep_number}'] = home_ep_row
                    home_pods_active = True
                    home_ep_number += 1

            home_view = ft.View("/",                 [
                        AppBar(title=Text("Pypods - A Python based podcast app!", color="white"), center_title=True, bgcolor="blue",
                            actions=[theme_icon_button], ),

                        top_bar,
                        *[home_ep_row_dict.get(f'search_row{i+1}') for i in range(len(home_ep_rows))]
                    ]
                )
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
                            text=d['title'], width=600,
                            on_click=lambda x, d=d: (evaluate_podcast(d['title'], d['artwork'], d['author'], d['categories'], d['description'], d['episodeCount'], d['url'], d['link']), open_poddisplay(e))
                        )
                        pod_desc = ft.Text(d['description'], width=700)
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
                        search_row = ft.Row(
                            alignment=ft.MainAxisAlignment.CENTER,
                            controls=[pod_image, search_column, add_pod_button])
                        search_rows.append(search_row)
                        search_row_dict[f'search_row{pod_number}'] = search_row
                        pod_number += 1
            # Create search view object
            search_view = ft.View("/searchpod",
                    [
                        AppBar(title=Text("PyPods - A Python based podcast app!", color="white"), center_title=True, bgcolor="blue",
                        actions=[theme_icon_button], ),
                        *[search_row_dict[f'search_row{i+1}'] for i in range(len(search_rows))]
                    ]
                    
                )
            search_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                search_view
                
            )

        if page.route == "/settings" or page.route == "/settings":

            # New User Creation Elements
            new_user = User(page)
            user_text = Text('Enter New User Information:')
            user_name = ft.TextField(label="Full Name", icon=ft.icons.CARD_MEMBERSHIP, hint_text='John Pypods')
            user_email = ft.TextField(label="email", icon=ft.icons.EMAIL, hint_text='ilovepypods@pypods.com') 
            user_username = ft.TextField(label="Username", icon=ft.icons.PERSON, hint_text='pypods_user1999')
            user_password = ft.TextField(label="password", icon=ft.icons.PASSWORD, password=True, can_reveal_password=True, hint_text='mY_SuPeR_S3CrEt!')
            user_submit = ft.ElevatedButton(text="Submit!", on_click=lambda x: (
                new_user.set_username(user_username.value), 
                new_user.set_password(user_password.value), 
                new_user.set_email(user_email.value),
                new_user.set_name(user_name.value),
                new_user.verify_user_values(),
                new_user.popup_user_values(e),
                new_user.create_user(), 
                user_created_prompt(e)))
            user_column = ft.Column(
                            controls=[user_text, user_name, user_email, user_username, user_password, user_submit]
                        )
            user_row = ft.Row(
                            vertical_alignment=ft.CrossAxisAlignment.START,
                            alignment=ft.MainAxisAlignment.CENTER,
                            controls=[user_column])

            # Theme Select Elements
            theme_text = ft.Text('Select Custom Theme:')
            theme_drop = ft.Dropdown(width=150,
             options=[
                ft.dropdown.Option("Abyss"),
                ft.dropdown.Option("Dracula"),
                ft.dropdown.Option("Dracula Light"),
                ft.dropdown.Option("Greenie Meanie"),
                ft.dropdown.Option("HotDogStand"),
             ]
             )
            theme_column = ft.Column(controls=[theme_text, theme_drop])
            theme_row = ft.Row(
                            vertical_alignment=ft.CrossAxisAlignment.START,
                            alignment=ft.MainAxisAlignment.CENTER,
                            controls=[theme_column])

            # Create search view object
            settings_view = ft.View("/searchpod",
                    [
                        AppBar(title=Text("PyPods - A Python based podcast app!", color="white"), center_title=True, bgcolor="blue",
                        actions=[theme_icon_button], ),
                        user_row,
                        theme_row
                    ]
                    
                )
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
            pod_feed_desc = ft.Text(clicked_podcast.description, width=700)
            pod_feed_site = ft.ElevatedButton(text=clicked_podcast.website, on_click=launch_pod_site)
            # pod_feed_site1 = ft.Text(clicked_podcast.website, style=ft.TextThemeStyle.TITLE_SMALL)
            
            feed_column = ft.Column(
                controls=[pod_feed_title, pod_feed_desc, pod_feed_site]
            )
            feed_row = ft.Row(
                alignment=ft.MainAxisAlignment.CENTER,
                controls=[pod_image, feed_column])

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

                entry_title = ft.Text(parsed_title, width=600, style=ft.TextThemeStyle.TITLE_MEDIUM)
                entry_description = ft.Text(parsed_description, width=800)
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
                    on_click = lambda x, url=parsed_audio_url, title=parsed_title: play_selected_episode(url, title)
                )
                ep_popup_button = ft.PopupMenuButton(icon=ft.icons.ARROW_DROP_DOWN_CIRCLE_ROUNDED, 
                # icon_size=40, icon_color="blue400", tooltip="Options",
                    items=[
                        ft.PopupMenuItem(icon=ft.icons.QUEUE, text="Queue", on_click=lambda x, url=entry_audio_url, title=entry_title: queue_selected_episode(url, title)),
                        ft.PopupMenuItem(icon=ft.icons.DOWNLOAD, text="Download", on_click=lambda x, url=entry_audio_url, title=entry_title: download_selected_episode(url, title))
                    ]
                )
                ep_play_options = ft.Column(controls=[ep_play_button, ep_popup_button])
            
                # Creating column and row for search layout
                ep_column = ft.Column(
                    controls=[entry_title, entry_description, entry_released]
                )
                ep_row = ft.Row(
                    alignment=ft.MainAxisAlignment.CENTER,
                    controls=[entry_artwork_url, ep_column, ep_play_options]
                )
                ep_rows.append(ep_row)
                ep_row_dict[f'search_row{ep_number}'] = ep_row
                ep_number += 1

            page.overlay.pop(2)
            # Create search view object
            pod_view = ft.View(
                    "/poddisplay",
                    [
                        AppBar(title=Text("PyPods - A Python based podcast app!", color="white"), center_title=True, bgcolor="blue",
                        actions=[theme_icon_button], ),
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
                    text=pod_list_title, width=600,
                    on_click=lambda x, e=e, title=pod_list_title, artwork=pod_list_artwork, author=pod_list_author, categories=pod_list_categories, desc=pod_list_desc, ep_count=pod_list_ep_count, feed=pod_list_feed, website=pod_list_website: on_pod_list_title_click(e, title, artwork, author, categories, desc, ep_count, feed, website)
                )
                pod_list_desc_display = ft.Text(pod_list_desc, width=700)
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
                pod_list_row = ft.Row(
                    alignment=ft.MainAxisAlignment.CENTER,
                    controls=[pod_list_artwork_image, pod_list_column, remove_pod_button])
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
                        AppBar(title=Text("PyPods - A Python based podcast app!", color="white"), center_title=True, bgcolor="blue",
                        actions=[theme_icon_button], ),
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
                hist_ep_row = ft.Row(
                    alignment=ft.MainAxisAlignment.CENTER,
                    controls=[hist_entry_artwork_url, hist_ep_column, hist_ep_play_button]
                )
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
                    # do something with the episode information
                    hist_entry_title = ft.Text(f'{hist_pod_name} - {hist_ep_title}', width=600, style=ft.TextThemeStyle.TITLE_MEDIUM)
                    hist_entry_description = ft.Text(hist_ep_desc, width=800)
                    hist_entry_audio_url = ft.Text(hist_ep_url)
                    hist_entry_listened = ft.Text(f'Listened on: {hist_ep_listen_date}')

                    hist_art_no = random.randint(1, 12)
                    hist_art_fallback = os.path.join(script_dir, "images", "logo_random", f"{hist_art_no}.jpeg")
                    hist_art_url = hist_ep_artwork if hist_ep_artwork else hist_art_fallback
                    hist_art_url_parsed = check_image(hist_art_url)
                    hist_entry_artwork_url = ft.Image(src=hist_art_url_parsed, width=150, height=150)
                    hist_ep_play_button = ft.IconButton(
                        icon=ft.icons.PLAY_CIRCLE,
                        icon_color="blue400",
                        icon_size=40,
                        tooltip="Play Episode",
                        on_click=lambda x, url=hist_ep_url, title=hist_ep_title: play_selected_episode(url, title)
                    )
                    hist_popup_button = ft.PopupMenuButton(icon=ft.icons.ARROW_DROP_DOWN_CIRCLE_ROUNDED, 
                    # icon_size=40, icon_color="blue400", tooltip="Options",
                        items=[
                            ft.PopupMenuItem(icon=ft.icons.QUEUE, text="Queue", on_click=lambda x, url=hist_ep_url, title=hist_ep_title: queue_selected_episode(url, title)),
                            ft.PopupMenuItem(icon=ft.icons.DOWNLOAD, text="Download", on_click=lambda x, url=hist_ep_url, title=hist_ep_title: download_selected_episode(url, title))
                        ]
                    )
                    hist_play_options = ft.Column(controls=[hist_ep_play_button, hist_popup_button])
                    
                    # Creating column and row for search layout
                    hist_ep_column = ft.Column(
                        controls=[hist_entry_title, hist_entry_description, hist_entry_listened]
                    )
                    hist_ep_row = ft.Row(
                        alignment=ft.MainAxisAlignment.CENTER,
                        controls=[hist_entry_artwork_url, hist_ep_column, hist_play_options]
                    )
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
                        AppBar(title=Text("PyPods - A Python based podcast app!", color="white"), center_title=True, bgcolor="blue",
                        actions=[theme_icon_button], ),
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
                download_ep_row = ft.Row(
                    alignment=ft.MainAxisAlignment.CENTER,
                    controls=[download_entry_artwork_url, download_ep_column, download_ep_play_button]
                )
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

                    download_entry_title = ft.Text(f'{download_pod_name} - {download_ep_title}', width=600, style=ft.TextThemeStyle.TITLE_MEDIUM)
                    download_entry_description = ft.Text(download_ep_desc, width=800)
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
                        on_click=lambda x, url=download_ep_local_url, title=download_ep_title: play_selected_episode(url, title)
                    )
                    download_popup_button = ft.PopupMenuButton(icon=ft.icons.ARROW_DROP_DOWN_CIRCLE_ROUNDED, 
                    # icon_size=40, icon_color="blue400", tooltip="Options",
                        items=[
                            ft.PopupMenuItem(icon=ft.icons.QUEUE, text="Queue", on_click=lambda x, url=download_ep_url, title=download_ep_title: queue_selected_episode(url, title)),
                            ft.PopupMenuItem(icon=ft.icons.DOWNLOAD, text="Delete Download", on_click=lambda x, url=download_ep_url, title=download_ep_title: delete_selected_episode(url, title))
                        ]
                    )
                    download_play_options = ft.Column(controls=[download_ep_play_button, download_popup_button])
                    # Creating column and row for download layout
                    download_ep_column = ft.Column(
                        controls=[download_entry_title, download_entry_description, download_entry_released]
                    )
                    download_ep_row = ft.Row(
                        alignment=ft.MainAxisAlignment.CENTER,
                        controls=[download_entry_artwork_url, download_ep_column, download_play_options]
                    )
                    download_ep_rows.append(download_ep_row)
                    download_ep_row_dict[f'search_row{download_ep_number}'] = download_ep_row
                    download_pods_active = True
                    download_ep_number += 1

            # Create search view object
            ep_hist_view = ft.View("/downloads",
                    [
                        AppBar(title=Text("PyPods - A Python based podcast app!", color="white"), center_title=True, bgcolor="blue",
                        actions=[theme_icon_button], ),

                        top_bar,
                        *[download_ep_row_dict.get(f'search_row{i+1}') for i in range(len(download_ep_rows))]

                    ]
                    
                )
            ep_hist_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                ep_hist_view
                    
                )

        if page.route == "/queue" or page.route == "/queue":

            # Get Pod info
            user_id = get_user_id()
            print(user_id)
            hist_episodes = database_functions.functions.user_history(cnx, user_id)
            hist_episodes.reverse()

            # page.overlay.pop(2)

            if hist_episodes is None:
                hist_ep_number = 1
                hist_ep_rows = []
                hist_ep_row_dict = {}


            else:
                hist_ep_number = 1
                hist_ep_rows = []
                hist_ep_row_dict = {}


            # Create search view object
            ep_hist_view = ft.View("/queue",
                    [
                        AppBar(title=Text("PyPods - A Python based podcast app!", color="white"), center_title=True, bgcolor="blue",
                        actions=[theme_icon_button], )

                    ]
                    
                )
            ep_hist_view.scroll = ft.ScrollMode.AUTO
            # Create final page
            page.views.append(
                ep_hist_view
                    
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
            print(f'email is valid {self.valid_email}')

        def popup_user_values(self, e):
            pass

        def create_user(self):
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
                # navbar.visible = True
                # page.appbar.visible = True
                go_homelogin(page)
            else:
                on_click_wronguser(page)

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

                e.control.content.controls[0].icon_color = "white54"
                e.control.content.controls[1].color = "white54"
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
                            icon_color="white54",
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
            height=580,
            animate=animation.Animation(500, "decelerate"),
            bgcolor="black",
            border_radius=10,
            padding=10,
            content=ft.Column(
                alignment=MainAxisAlignment.START,
                horizontal_alignment="center",
                controls=[
                Text(
                        value=(f'PyPods'),
                        size=10,
                        weight="bold",
                        color="white"
                    ),
                Divider(color="white24", height=5),
                Container(
                    width=42,
                    height=42,
                    border_radius=8,
                    bgcolor="bluegrey900",
                    alignment=alignment.center,
                    content=Text(
                        value=active_user.initials,
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

    # navbar = NavBar(page).create_navbar()
    

# Create Page--------------------------------------------------------

    page.title = "PyPods"
    page.theme_mode = "dark"
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
    refresh_btn = ft.IconButton(icon=ft.icons.REFRESH, icon_color="blue400", tooltip="Refresh Podcast List", on_click=refresh_podcasts)
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
    currently_playing = ft.Container(content=ft.Text(current_episode.name))
    currently_playing.padding=ft.padding.only(left=20)
    currently_playing.padding=ft.padding.only(top=10)

    ep_height = 50
    ep_width = 4000
    audio_container = ft.Container(
        height=ep_height,
        width=ep_width,
        bgcolor='black',
        border_radius=45,
        padding=6,
        content=ft.Row(
            vertical_alignment=ft.CrossAxisAlignment.END,  
            alignment=ft.MainAxisAlignment.SPACE_BETWEEN,          
            controls=[currently_playing, ep_audio_controls]
        )
    )
        
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


    def play_selected_episode(url, title):
        current_episode.url = url
        current_episode.name = title
        current_episode.play_episode()

    def download_selected_episode(url, title):
        current_episode.url = url
        current_episode.title = title
        current_episode.download_pod()
        
    def delete_selected_episode(url, title):
        current_episode.url = url
        current_episode.title = title
        current_episode.delete_pod()

    def queue_selected_episode(url, title):
        current_episode.url = url
        current_episode.title = title
        

# Starting Page Layout

    top_bar = ft.Row(vertical_alignment=ft.CrossAxisAlignment.START, controls=[top_row_container])

    # page.overlay.append(ft.Stack([navbar], expand=True))
    # navbar.visible = False
    page.appbar.visible = False
    
    start_login(page)

# Browser Version
# ft.app(target=main, view=ft.WEB_BROWSER)
# App version
ft.app(target=main, port=8034)