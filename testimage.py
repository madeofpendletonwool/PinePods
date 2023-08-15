import flet as ft
import os

def main(page: ft.Page):
    coffee_script_dir = os.path.dirname(os.path.realpath(__file__))
    image_path = os.path.join(coffee_script_dir, "images", "pinepods-appicon.png")
    pinepods_img = ft.Image(
        src=image_path,
        width=100,
        height=100,
        fit=ft.ImageFit.CONTAIN,
    )

    page.add(
        pinepods_img
    )


ft.app(target=main, view=ft.WEB_BROWSER)