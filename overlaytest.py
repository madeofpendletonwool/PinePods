import flet as ft

def main(page: ft.Page):
    page.overlay.append(ft.Stack([ft.Container(ft.Text("HELLO"), bottom=20, right=20)], expand=True))

    page.add(
        ft.Row(
            [ft.Column([ft.Text(str(number)) for number in range(100)], expand=True, scroll=ft.ScrollMode.AUTO)],
            expand=True,
        ),
    )
    page.update()

ft.app(target=main)