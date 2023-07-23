import flet as ft

def main(page: ft.Page):
    textbox = ft.TextField(label='test')
    text_row = ft.Row(controls=[textbox])
    text_row.alignment = ft.MainAxisAlignment.CENTER
    c3 = ft.Container(content=text_row, alignment=ft.alignment.center, top=120, animate_position=500)
    c3.horizontal_alignment = ft.CrossAxisAlignment.CENTER
    c_row = ft.Row(controls=c3)
    c_row.alignment = ft.MainAxisAlignment.CENTER
    # c3.padding = ft.padding.only(top=100)



    def animate_container(e):
        c3.top = 0
        page.update()

    page.add(
        ft.Stack([c_row], height=250),
        ft.ElevatedButton("Animate!", on_click=animate_container),
    )

ft.app(target=main)