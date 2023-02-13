# Various flet imports
import flet as ft
from flet import AppBar, ElevatedButton, Page, Text, View, colors, icons, ProgressBar, ButtonStyle, IconButton, TextButton, Row
# from flet.control_event import ControlEvent
from flet.auth.providers.github_oauth_provider import GitHubOAuthProvider
# Internal Functions
import internal_functions.functions
import database_functions.functions
import app_functions.functions
# Others
import time
import mysql.connector
import json

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

    def go_home(e):
        page.go("/")

    def route_change(e):
        print("Route change:", e.route)
        page.views.clear()
        page.views.append(
            View(
                "/",
                [
                    AppBar(title=Text("Pypods - Alerting and Monitoring", color="white"), center_title=True, bgcolor="blue",
                        actions=[theme_icon_button], ),

                    top_row, audio_controls_column
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
                            on_click=evaluate_podcast
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
                        AppBar(title=Text("PyPods - Alerting and Monitoring", color="white"), center_title=True, bgcolor="blue",
                        actions=[theme_icon_button], ),
                        *[search_row_dict[f'search_row{i+1}'] for i in range(len(search_rows))]
                    ],
                    
                )
                
            )

    page.on_route_change = route_change
    page.on_view_pop = view_pop

    def send_podcast(pod_title, pod_artwork, pod_author, pod_categories, pod_description, pod_episode_count, pod_feed_url, pod_website):

        categories = json.dumps(pod_categories)
        podcast_values = (None, pod_title, pod_artwork, pod_author, categories, pod_description, pod_episode_count, pod_feed_url, pod_website, 1)
        database_functions.functions.add_podcast(cnx, podcast_values)

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

# Create Page--------------------------------------------------------

    page.title = "PyPods"
    page.theme_mode = "dark"
    theme_icon_button = ft.IconButton(icons.DARK_MODE, selected_icon=icons.LIGHT_MODE, icon_color=colors.BLACK,
                                   icon_size=35, tooltip="change theme", on_click=change_theme,
                                   style=ButtonStyle(color={"": colors.BLACK, "selected": colors.WHITE}, ), )

    page.appbar = AppBar(title=Text("Pypods - A Python based podcast app!", color="white"), center_title=True, bgcolor="blue",
                        actions=[theme_icon_button], )

    page.title = "pyPods - A python based podcast app!"
    
    
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
 

    def evaluate_podcast(e):
        page.clean()
        page.add(ft.Text("evaluating feed"))

     

    # Podcast Search Function

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
    play_button = ft.ElevatedButton("Start playing", on_click=lambda _: audio1.play())
    pause_button = ft.ElevatedButton("Stop playing", on_click=lambda _: audio1.pause())
    seek_button = ft.ElevatedButton("Seek 2s", on_click=lambda _: audio1.seek(2000))


    search_row = ft.Row(spacing=25, controls=[search_pods, search_btn])
    top_row = ft.Row(alignment=ft.MainAxisAlignment.SPACE_BETWEEN, controls=[refresh_ctn, search_row])
    top_row_container = ft.Container(content=top_row, expand=True)
    audio_row = ft.Row(spacing=25, alignment=ft.MainAxisAlignment.CENTER, controls=[play_button, pause_button, seek_button])
    audio_controls_column = ft.Column(alignment=ft.MainAxisAlignment.END, controls=[audio_row])
    test_text = Text('This is a test')
    test_column = ft.Container(alignment=ft.alignment.bottom_center, border=ft.border.all(1, ft.colors.OUTLINE), content=test_text)

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