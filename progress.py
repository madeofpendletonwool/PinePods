

import flet as ft
import math

def main(page: ft.Page):

    volume_slider = ft.Slider(value=1)
    volume_down_icon = ft.Icon(name=ft.icons.VOLUME_MUTE)
    volume_up_icon = ft.Icon(name=ft.icons.VOLUME_UP_ROUNDED)
    volume_adjust_column = ft.Row(controls=[volume_down_icon, volume_slider, volume_up_icon], expand=True)
    volume_container = ft.Container(
            height=35,
            width=250,
            bgcolor=ft.colors.WHITE,
            border_radius=45,
            padding=6,
            content=volume_adjust_column)
    volume_container.adding=ft.padding.all(50)
    volume_container.alignment = ft.alignment.top_right

    page.overlay.append(ft.Stack([volume_container], bottom=75, right=25, expand=True))
    page.add(ft.Text('test'))

# Browser Version
# ft.app(target=main, view=ft.WEB_BROWSER, port=8034)
# App version
ft.app(target=main, port=8034)

