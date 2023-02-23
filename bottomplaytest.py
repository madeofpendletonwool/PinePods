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
import Audio.functions

class Data:
    def __init__(self) -> None:
        self.counter = 0

d = Data()

def main(page: ft.Page):

    page.title = "PyPods_testing"
    page.theme_mode = "dark"

    page.snack_bar = ft.SnackBar(
        content=ft.Text("Hello, world!"),
        action="Alright!",
    )

    #Audio Button Setup
    play_button = ft.IconButton(icon=ft.icons.PLAY_ARROW, tooltip="Play Podcast", on_click=lambda _: audio1.play())
    pause_button = ft.IconButton(icon=ft.icons.PAUSE, tooltip="Pause Playback", on_click=lambda _: audio1.pause())
    seek_button = ft.IconButton(icon=ft.icons.FAST_FORWARD, tooltip="Seek 10 seconds", on_click=lambda _: audio1.seek(2000))

    audio_contain = ft.Container(content=[play_button, pause_button, seek_button],
                    margin=10,
                    padding=10,
                    alignment=ft.alignment.center,
                    bgcolor=ft.colors.AMBER,
                    width=150,
                    height=150,
                    border_radius=10,
                    )

    def on_click(e):
        page.snack_bar = ft.SnackBar(ft.TextButton(text='test'))
        page.snack_bar.open = True
        d.counter += 1
        page.update()

    page.add(ft.Row(
            [
                ft.Container(
                    content=[play_button, pause_button, seek_button],
                    margin=10,
                    padding=10,
                    alignment=ft.alignment.center,
                    bgcolor=ft.colors.AMBER,
                    width=150,
                    height=150,
                    border_radius=10,
                ),
            ]))
                


ft.app(main)
