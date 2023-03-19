from time import sleep

import flet as ft
from flet import colors

def main(page: ft.Page):
    page.theme_mode = "dark"
    main_color = colors.DEEP_ORANGE
    accent_color = colors.BLACK
    page.bgcolor = colors.BLUE_GREY
    page.window_bgcolor = colors.BLUE_GREY
    pb = ft.ProgressBar(width=400)
    pb.value = .5

    page.add(
        ft.Text("Linear progress indicator", style="headlineSmall"),
        ft.Column([ ft.Text("Doing something..."), pb]),
        ft.Text("Indeterminate progress bar", style="headlineSmall"),
        ft.ProgressBar(width=400, color="amber", bgcolor="#eeeeee"),
    )

    # for i in range(0, 101):
    #     pb.value = i * 0.01
    #     sleep(0.1)
    #     page.update()

ft.app(target=main)