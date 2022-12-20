import flet as ft
import time

def main(page: ft.Page):
    page.title = "pyPods - A python based podcast app!"
    
    
    # page.controls.append(testtx)
    # page.update()

    def search_podcast(e):
        if not search_pods.value:
            search_pods.error_text = "Please enter a podcast to seach for - Cannot search nothing"
            page.update()   
        else:
            podcast_value = search_pods.value
            page.clean()
            page.add(ft.Text(f"Searching for {podcast_value}!"))

    # Page Header

    header = ft.Container(
        content=ft.Text(value="pyPods", color="blue", size=50, expand=True),
        alignment=ft.alignment.center
        # podsear_btn

    )

    # Podcast Search Function

    search_pods = ft.TextField(label="Search for new podcast", content_padding=5, width=200)
    search_box = ft.Container(
        content=search_pods,
        alignment=ft.alignment.top_right
    )
    search_btn = ft.ElevatedButton("Search!", on_click=search_podcast)
    search_btn_ctn = ft.Container(
        content=search_btn,
        alignment=ft.alignment.top_right
    )

    page.add(
        ft.Row([header, search_box, search_btn_ctn])
    )


ft.app(target=main, port=8034)