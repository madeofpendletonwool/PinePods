# Various flet imports
import flet as ft
from flet import *
from flet import AppBar, ElevatedButton, Page, Text, View, colors, icons, ProgressBar, ButtonStyle, IconButton, TextButton, Row

def main(page: ft.Page):

    text1 = ft.Text('Image With issue below')
    text2 = ft.Text('Image Without issue below')

    imageissue = ft.Image(src='https://imgv3.fotor.com/images/blog-cover-image/part-blurry-image.jpg', width=150, height=150)
    imagefine = ft.Image(src='https://cdn.changelog.com/uploads/covers/practical-ai-original.png?v=63725770374', width=150, height=150)


    # Create Initial Home Page
    page.add(
        text1,
        imageissue,
        text2,
        imagefine
    )
# Browser Version
# ft.app(target=main, view=ft.WEB_BROWSER)
# App version
ft.app(target=main, port=8034)