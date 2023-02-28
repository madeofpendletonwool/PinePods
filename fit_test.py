import flet as ft

def main(page: ft.Page):
    lv = ft.ListView(expand=True, spacing=10)
    for i in range(5000):
        lv.controls.append(ft.Text(f"Line {i}"))

    lv_contain = ft.Container(        
        width=200,
        height=200,
        bgcolor="red",
        content=lv)

    page.add(lv_contain)

ft.app(target=main, view=ft.WEB_BROWSER)