import os
import flet as ft
from flet import *

proxy_url = 'http://localhost:8000/proxy?url='

def main(page: ft.Page):
    text1 = ft.Text('Image With issue below')
    text2 = ft.Text('Image Without issue below')

    # Use the local proxy server to retrieve the image
    url1 = '/home/collinp/Documents/GitHub/PyPods/images/logo_random/11.jpeg'
    url2 = 'https://cdn.changelog.com/uploads/covers/practical-ai-original.png?v=63725770374'
    url3 = 'https://imgv3.fotor.com/images/blog-cover-image/part-blurry-image.jpg'

    # Check if the path is a local file or a URL and pass it to the proxy server accordingly
    if url1.startswith('http'):
        image1 = ft.Image(src=proxy_url + url1, width=150, height=150)
    else:
        image1 = ft.Image(src=url1, width=150, height=150)

    image2 = ft.Image(src=proxy_url + url2, width=150, height=150)

    if url3.startswith('http'):
        image3 = ft.Image(src=proxy_url + url3, width=150, height=150)
    else:
        image3 = ft.Image(src=url3, width=150, height=150)

    print(image1)

    # Create Initial Home Page
    page.add(
        text1,
        image1,
        text2,
        image2,
        image3
    )

# Browser Version
ft.app(target=main, view=ft.WEB_BROWSER)
