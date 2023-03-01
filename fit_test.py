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

    pr = ft.ProgressRing()
    page.overlay.append(ft.Stack([pr], bottom=25, right=30, left=20, expand=True))

    print(page.overlay)

    page.add(lv_contain)

ft.app(target=main, port=8035)