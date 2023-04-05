

import flet as ft
import math

def main(page: ft.Page):

    volume_adjust_column = ft.Column(controls=[ft.Icon(name=ft.icons.VOLUME_MUTE), ft.Slider(value=1, expand=True, rotate=ft.Rotate(angle=3*math.pi/2)), ft.Icon(name=ft.icons.VOLUME_UP_ROUNDED)], expand=True)
    volume_container = ft.Container(
            height=250,
            width=30,
            bgcolor=ft.colors.BLUE,
            border_radius=45,
            padding=6,
            content=volume_adjust_column)
    # volume_container.adding=ft.padding.all(50)
    volume_container.alignment = ft.alignment.top_right
    page.add(volume_container)
    # page.overlay.append(ft.Stack([volume_container], bottom=75, right=25, expand=True))



# Browser Version
# ft.app(target=main, view=ft.WEB_BROWSER, port=8034)
# App version
ft.app(target=main, port=8034)
