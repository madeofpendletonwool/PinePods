import flet as ft
from math import pi

rotate_pos = False
def main(page):

    def button_press(e):
        button.text = "testafter"
        page.update()

    button = ft.ElevatedButton("testing", on_click=button_press)

    page.add(button, ft.Text('Testing again')
             )

ft.app(target=main)