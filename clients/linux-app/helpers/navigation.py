# Various flet imports
import flet as ft
from flet import Text, colors, icons, ButtonStyle, Row, alignment, border_radius, animation, \
    MainAxisAlignment, padding
import logging
import os
import appdirs
import requests
import hashlib

login_screen = True
user_home_dir = os.path.expanduser("~")
audio_playing = False
active_pod = 'Set at start'
initial_script_dir = os.path.dirname(os.path.realpath(__file__))
script_dir = os.path.dirname(os.path.dirname(initial_script_dir))

appname = "pinepods"
appauthor = "Gooseberry Development"

# user_data_dir would be the equivalent to the home directory you were using
user_data_dir = appdirs.user_data_dir(appname, appauthor)
metadata_dir = os.path.join(user_data_dir, 'metadata')
backup_dir = os.path.join(user_data_dir, 'backups')
assets_dir = os.path.join(user_data_dir, 'assets')


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


def start_login(page):
    page.go("/login")


def start_config(page):
    page.go("/server_config")


def first_time_config(page):
    page.go("/first_time_config")


def start_login_e(e):
    page.go("/login")


def open_mfa_login(e):
    page.go("/mfalogin")


def view_pop(e):
    page.views.pop()
    top_view = page.views[-1]
    page.go(top_view.route)


def open_poddisplay(e):
    print('open poddisplay')
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


# def open_currently_playing(e):
#     active_user.show_audio_container = False
#     page.go("/playing")


# def open_episode_select(page, url, title):
#     current_episode.url = url
#     current_episode.title = title
#     page.go("/episode_display")


def open_pod_list(e):
    page.update()
    page.go("/pod_list")


def open_search(e):
    page.go("/user_search")

    # Create Sidebar------------------------------------------------------


class NavBar:
    def __init__(self, page, active_user):
        self.active_user = active_user
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

            e.control.content.controls[0].icon_color = self.active_user.accent_color
            e.control.content.controls[1].color = self.active_user.accent_color
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
                        icon_color=self.active_user.accent_color,
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
        def go_home(e):
            self.page.update()
            self.page.go("/")

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
        if self.active_user.user_id != 1:
            gravatar_url = get_gravatar_url(self.active_user.email)
        self.active_user.get_initials()

        user_content = ft.Image(src=gravatar_url, width=42, height=45, border_radius=8) if gravatar_url else Text(
            value=self.active_user.initials,
            color=self.active_user.nav_color2,
            size=20,
            weight="bold"
        )

        return ft.Container(
            width=62,
            # height=580,
            expand=True,
            animate=animation.Animation(500, "decelerate"),
            bgcolor=self.active_user.main_color,
            padding=10,
            content=ft.Column(
                alignment=MainAxisAlignment.START,
                horizontal_alignment="center",
                controls=[
                    Text(
                        value=(f'PinePods'),
                        size=8,
                        weight="bold",
                        color=self.active_user.accent_color
                    ),
                    ft.Divider(color="white24", height=5),
                    ft.Container(
                        width=42,
                        height=40,
                        border_radius=8,
                        bgcolor=self.active_user.tertiary_color,
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
                    self.ContainedIcon('Logout', icons.LOGOUT_ROUNDED, "Logout", self.active_user.logout_pinepods),
                ],
            ),
        )


def download_image(img_url, save_path):
    response = requests.get(img_url)
    if response.status_code == 200:  # OK
        with open(save_path, 'wb') as img_file:
            img_file.write(response.content)
    else:
        print(f"Failed to download {img_url}. Status code: {response.status_code}")


def ensure_images_are_downloaded(server_name, proxy_url):
    logging.info("Starting to ensure images are downloaded")

    if not os.path.exists(assets_dir):
        logging.info(f"Assets directory {assets_dir} not found, creating it.")
        os.makedirs(assets_dir)

    for i in range(1, 14):  # images 1.jpeg to 13.jpeg
        image_filename = f"{i}.jpeg"
        image_filepath = os.path.join(assets_dir, image_filename)

        if not os.path.exists(image_filepath):
            image_url = f"{proxy_url}/pinepods/images/logo_random/{image_filename}"
            logging.info(f"Downloading image from {image_url}")
            try:
                download_image(image_url, image_filepath)
            except Exception as e:
                logging.error(f"Error downloading {image_url}: {e}")

    logo_filepath = os.path.join(assets_dir, "pinepods-appicon.png")
    if not os.path.exists(logo_filepath):
        logo_image_url = f"{proxy_url}/pinepods/images/pinepods-appicon.png"
        logging.info(f"Downloading logo image from {logo_image_url}")
        try:
            download_image(logo_image_url, logo_filepath)
        except Exception as e:
            logging.error(f"Error downloading {logo_image_url}: {e}")


def close_banner(e):
    page.banner.open = False
    page.update()


def open_repo(e):
    page.launch_url('https://github.com/madeofpendletonwool/PinePods')


def open_doc_site(e):
    page.launch_url('https://pinepods.online')


class Navigation:
    def __init__(self, page, active_user=None):
        self.new_nav = None
        self.page = page
        self.active_user = active_user

    def create_navbar(self):
        self.new_nav = NavBar(self.page, self.active_user)

    def go_homelogin(self, page, active_user, app_api):
        ensure_images_are_downloaded(app_api.server_name, app_api.proxy_url)
        print('image fail')
        active_user.theme_select()
        # Theme user elements
        print('in home')
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
        self.create_navbar()
        self.new_nav.navbar.border = ft.border.only(right=ft.border.BorderSide(2, active_user.tertiary_color))
        self.new_nav.navbar_stack = ft.Stack([self.new_nav.navbar], expand=True)
        self.page.overlay.append(self.new_nav.navbar_stack)
        self.page.update()
        page.go("/")


def go_theme_rebuild(page, active_user, pod_controls, new_nav):
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
