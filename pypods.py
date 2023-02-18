# Various flet imports
import flet as ft
from flet import AppBar, ElevatedButton, Page, Text, View, colors, icons, ProgressBar, ButtonStyle, IconButton, TextButton, Row
# from flet.control_event import ControlEvent
from flet.auth.providers.github_oauth_provider import GitHubOAuthProvider
# Internal Functions
import internal_functions.functions
import database_functions.functions
import app_functions.functions
import Auth.Passfunctions
# Others
import time
import mysql.connector
import json
import re

# Create database connector
cnx = mysql.connector.connect(
    host="127.0.0.1",
    port="3306",
    user="root",
    password="password",
    database="pypods_database"
)

url = "https://github.com/mdn/webaudio-examples/blob/main/audio-analyser/viper.mp3?raw=true"

def main(page: ft.Page):
    # page.scroll = "auto"

#---Flet Various Functions---------------------------------------------------------------
    def send_podcast(pod_title, pod_artwork, pod_author, pod_categories, pod_description, pod_episode_count, pod_feed_url, pod_website):

        categories = json.dumps(pod_categories)
        podcast_values = (pod_title, pod_artwork, pod_author, categories, pod_description, pod_episode_count, pod_feed_url, pod_website, 1)
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

    def evaluate_podcast(pod_title, pod_artwork, pod_author, pod_categories, pod_description, pod_episode_count, pod_feed_url, pod_website):
        global clicked_podcast
        clicked_podcast = Podcast(name=pod_title, artwork=pod_artwork, author=pod_author, description=pod_description, feedurl=pod_feed_url, website=pod_website)
        print(clicked_podcast.name)
        return clicked_podcast
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

    def view_pop(e):
        print("View pop:", e.view)
        page.views.pop()
        top_view = page.views[-1]
        page.go(top_view.route)

    def open_search(e):
        page.go("/searchpod")

    def open_poddisplay(e):
        page.go("/poddisplay")

    def open_settings(e):
        page.go("/settings")

    def go_home(e):
        page.go("/")

    def route_change(e):
        print("Route change:", e.route)
        page.views.clear()
        page.views.append(
            View(
                "/",
                [
                    AppBar(title=Text("Pypods - A Python based podcast app!", color="white"), center_title=True, bgcolor="blue",
                        actions=[theme_icon_button], ),

                    #Search Functionality
                    top_row_container,

                    # Audio Controls button
                    audio_controls_column
                ],
            )
        )
        if page.route == "/searchpod" or page.route == "/searchpod":
            # Get Pod info
            podcast_value = search_pods.value
            search_results = internal_functions.functions.searchpod(podcast_value)
            return_results = search_results['feeds']
            # Allow scrolling otherwise the page will overflow


            # Get and format list
            pod_number = 1
            search_rows = []
            search_row_dict = {}
            for d in return_results:
                for k, v in d.items():
                    if k == 'title':
                        # Defining the attributes of each podcast that will be displayed on screen
                        pod_image = ft.Image(src=d['image'], width=150, height=150)
                        pod_title = ft.TextButton(
                            text=d['title'], 
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
            page.scroll = "always"
            page.views.append(
                View(
                    "/searchpod",
                    [
                        AppBar(title=Text("PyPods - A Python based podcast app!", color="white"), center_title=True, bgcolor="blue",
                        actions=[theme_icon_button], ),
                        *[search_row_dict[f'search_row{i+1}'] for i in range(len(search_rows))]
                    ],
                    
                )
                
            )

        if page.route == "/settings" or page.route == "/settings":

            # New User Creation Elements
            new_user = User()
            user_text = Text('Enter New User Information:')
            user_email = ft.TextField(label="email", icon=ft.icons.EMAIL, hint_text='ilovepypods@pypods.com') 
            user_username = ft.TextField(label="Username", icon=ft.icons.PERSON, hint_text='pypods_user1999')
            user_password = ft.TextField(label="password", icon=ft.icons.PASSWORD, password=True, can_reveal_password=True, hint_text='mY_SuPeR_S3CrEt!')
            user_submit = ft.ElevatedButton(text="Submit!", on_click=lambda x: (
                new_user.set_username(user_username.value), 
                new_user.set_password(user_password.value), 
                new_user.set_email(user_email.value),
                new_user.verify_user_values(),
                new_user.popup_user_values(e),
                new_user.create_user(), 
                user_created_prompt(e)))
            user_column = ft.Column(
                            controls=[user_text, user_email, user_username, user_password, user_submit]
                        )
            user_row = ft.Row(
                            alignment=ft.MainAxisAlignment.CENTER,
                            controls=[user_column])

            page.views.append(
                View(
                    "/settings",
                    [
                        AppBar(title=Text("PyPods - A Python based podcast app!", color="white"), center_title=True, bgcolor="blue",
                        actions=[theme_icon_button], ),
                        user_row
                        
                    ],
                    
                )
                
            )

        if page.route == "/poddisplay" or page.route == "/poddisplay":
            testname = ft.Text(clicked_podcast.name)

            page.views.append(
                View(
                    "/poddisplay",
                    [
                        AppBar(title=Text("PyPods - A Python based podcast app!", color="white"), center_title=True, bgcolor="blue",
                        actions=[theme_icon_button], ),
                        testname
                        
                    ],
                    
                )
                
            )

    page.on_route_change = route_change
    page.on_view_pop = view_pop

#-Create Help Banner-----------------------------------------------------------------------
    def close_banner(e):
        page.banner.open = False
        page.update()

    page.banner = ft.Banner(
        bgcolor=ft.colors.BLUE,
        leading=ft.Icon(ft.icons.WAVING_HAND, color=ft.colors.DEEP_ORANGE_500, size=40),
        content=ft.Text("""
    Welcome to Pypods
    """, color=colors.BLACK
        ),
        actions=[
            ft.IconButton(icon=ft.icons.EXIT_TO_APP, on_click=close_banner)
        ],
    )

    def show_banner_click(e):
        page.banner.open = True
        page.update()

    banner_button = ft.ElevatedButton("Help!", on_click=show_banner_click)

# Login/User Changes------------------------------------------------------
    class User:
        def __init__(self):
            self.username = None
            self.password = None
            self.email = None

        def set_username(self, new_username):
            self.username = new_username

        def set_password(self, new_password):
            self.password = new_password

        def set_email(self, new_email):
            self.email = new_email
    
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
            user_values = (self.username, self.email, hash_pw, salt)
            database_functions.functions.add_user(cnx, user_values)

    active_user = User()

    print(active_user.username)

    # def login_click(e):
    #     page.login(provider)

    # def logout_button_click(e):
    #     page.logout()

    # def on_logout(e):
    #     toggle_login_session()

    # # def on_login(e: ft.LoginEvent):
    # def on_login(e):
    #     print("Access token:", page.auth.token.access_token)
    #     print("User ID:", page.auth.user.id)
    #     if not e.error:
    #         toggle_login_session()
    #     # Allow Route Changes only after login

    # page.on_login = on_login
    # logout_button = ft.ElevatedButton("Logout", on_click=logout_button_click)
    # login_button = ft.ElevatedButton("Login with GitHub", on_click=login_click)
    # login_row = Row(alignment=ft.MainAxisAlignment.SPACE_BETWEEN, controls=[login_button, banner_button])
    # logout_row = Row(alignment=ft.MainAxisAlignment.SPACE_BETWEEN, controls=[logout_button, banner_button])
    # page.add(login_row, logout_row)

    # def toggle_login_session():
    #     cecil_row.visible = page.auth is None
    #     login_row.visible = page.auth is None
    #     logout_row.visible = page.auth is not None
    #     basic_row.visible = page.auth is not None
    #     basic_modules_row.visible = page.auth is not None
    #     alert_row.visible = page.auth is not None
    #     alert_modules_row.visible = page.auth is not None
    #     monitor_row.visible = page.auth is not None
    #     report_modules_row.visible = page.auth is not None
    #     page.update()
    

# Create Page--------------------------------------------------------

    page.title = "PyPods"
    page.theme_mode = "dark"
    theme_icon_button = ft.IconButton(icons.DARK_MODE, selected_icon=icons.LIGHT_MODE, icon_color=colors.BLACK,
                                   icon_size=35, tooltip="change theme", on_click=change_theme,
                                   style=ButtonStyle(color={"": colors.BLACK, "selected": colors.WHITE}, ), )

    page.appbar = AppBar(title=Text("Pypods - A Python based podcast app!", color="white"), center_title=True, bgcolor="blue",
                        actions=[theme_icon_button], )

    page.title = "PyPods - A python based podcast app!"
    
    
    # page.controls.append(testtx)
    # page.update()

    # Audio Setup
    audio1 = ft.Audio(
        src=url,
        autoplay=False,
        volume=1,
        balance=0,
        on_loaded=lambda _: print("Loaded"),
        on_duration_changed=lambda e: print("Duration changed:", e.data),
        on_position_changed=lambda e: print("Position changed:", e.data),
        on_state_changed=lambda e: print("State changed:", e.data),
        on_seek_complete=lambda _: print("Seek complete"),
    )
    page.overlay.append(audio1)

     
    # Settings Button
    settings_btn = ft.ElevatedButton("PyPods Settings", on_click=open_settings)

    # Podcast Search Function Setup
    search_pods = ft.TextField(label="Search for new podcast", content_padding=5, width=350)
    search_btn = ft.ElevatedButton("Search!", on_click=open_search)
    refresh_btn = ft.ElevatedButton(text="Refresh Podcast List")
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

    #Audio Button Setup
    play_button = ft.IconButton(icon=ft.icons.PLAY_ARROW, tooltip="Play Podcast", on_click=lambda _: audio1.play())
    pause_button = ft.IconButton(icon=ft.icons.PAUSE, tooltip="Pause Playback", on_click=lambda _: audio1.pause())
    seek_button = ft.IconButton(icon=ft.icons.FAST_FORWARD, tooltip="Seek 10 seconds", on_click=lambda _: audio1.seek(10000))

    # Various rows and columns for layout
    settings_row = ft.Row(vertical_alignment=ft.CrossAxisAlignment.START, controls=[refresh_ctn, settings_btn])
    search_row = ft.Row(spacing=25, controls=[search_pods, search_btn])
    top_row = ft.Row(alignment=ft.MainAxisAlignment.SPACE_BETWEEN, vertical_alignment=ft.CrossAxisAlignment.START, controls=[settings_row, search_row])
    top_row_container = ft.Container(content=top_row, expand=True)
    audio_row = ft.Row(spacing=25, alignment=ft.MainAxisAlignment.CENTER, controls=[play_button, pause_button, seek_button])
    audio_controls_column = ft.Column(alignment=ft.MainAxisAlignment.END, controls=[audio_row])
    test_text = Text('This is a test')
    test_column = ft.Container(alignment=ft.alignment.bottom_center, border=ft.border.all(1, ft.colors.OUTLINE), content=test_text)

    # Create Initial Home Page
    page.add(
        #Search Functionality
        top_row_container,

        # Audio Controls button
        audio_controls_column
    )

    page.scroll = "always"

# Browser Version
ft.app(target=main, view=ft.WEB_BROWSER)
# App version
# ft.app(target=main, port=8034)