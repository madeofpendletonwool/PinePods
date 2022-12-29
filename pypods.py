import flet as ft
import time
import InternalFunctions.searchpod

url = "https://github.com/mdn/webaudio-examples/blob/main/audio-analyser/viper.mp3?raw=true"

def main(page: ft.Page):

    def check_item_clicked(e):
        e.control.checked = not e.control.checked
        page.update()

    page.title = "PyPods"
    page.appbar = ft.AppBar(
        leading=ft.Icon(ft.icons.PALETTE),
        leading_width=40,
        title=ft.Text("pyPods"),
        center_title=True,
        bgcolor=ft.colors.SURFACE_VARIANT,
        actions=[
            ft.IconButton(ft.icons.WB_SUNNY_OUTLINED),
            ft.IconButton(ft.icons.FILTER_3),
            ft.PopupMenuButton(
                items=[
                    ft.PopupMenuItem(text="Item 1"),
                    ft.PopupMenuItem(),  # divider
                    ft.PopupMenuItem(
                        text="Checked item", checked=False, on_click=check_item_clicked
                    ),
                ]
            ),
        ],
    )
    page.title = "pyPods - A python based podcast app!"
    
    
    # page.controls.append(testtx)
    # page.update()

    # Audio Setup
    audio1 = ft.Audio(
        src=url,
        autoplay=False,
        volume=1,
        balance=0,
        on_loaded=lambda _: print("Loaded"),
        on_duration_changed=lambda e: print("Duration changed:", e.data),
        on_position_changed=lambda e: print("Position changed:", e.data),
        on_state_changed=lambda e: print("State changed:", e.data),
        on_seek_complete=lambda _: print("Seek complete"),
    )
    page.overlay.append(audio1)

    def search_podcast(e):
        if not search_pods.value:
            search_pods.error_text = "Please enter a podcast to seach for"
            page.update()   
        else:
            podcast_value = search_pods.value
            page.clean()
            page.add(ft.Text(f"Searching for {podcast_value}!"))
            search_results = InternalFunctions.searchpod.searchpod(podcast_value)
            return_results = search_results['feeds']
            page.clean()

            for d in return_results:
                print(d['title'])
                for k, v in d.items():
                    if k == 'title':
                        page.add(ft.Text(f"{v}"))
                    if k == 'description':
                        page.add(ft.Text(f"{v}"))
                        page.add(ft.Text('next pod ---------'))
                    
                    # print("new item: {} = {}".format(k, v))
                    # page.add(ft.Text(f'{k}:'))
                    # page.add(ft.Text(v))

            

    # Podcast Search Function

    search_pods = ft.TextField(label="Search for new podcast", content_padding=5, width=350)
    search_btn = ft.ElevatedButton("Search!", on_click=search_podcast)
    refresh_btn = ft.ElevatedButton(text="Refresh Podcast List")
    search_box = ft.Container(
        content=search_pods,
        alignment=ft.alignment.top_right
    )
    search_btn_ctn = ft.Container(
        content=search_btn,
        alignment=ft.alignment.top_right
    )
    refresh_ctn = ft.Container(
        content=refresh_btn,
        alignment=ft.alignment.top_left
    )

    #Audio Button Setup
    play_button = ft.ElevatedButton("Start playing", on_click=lambda _: audio1.play())
    pause_button = ft.ElevatedButton("Stop playing", on_click=lambda _: audio1.pause())
    seek_button = ft.ElevatedButton("Seek 2s", on_click=lambda _: audio1.seek(2000))


    search_row = ft.Row(spacing=25, alignment=ft.MainAxisAlignment.END, controls=[search_pods, search_btn])
    top_row = ft.Row(alignment=ft.MainAxisAlignment.SPACE_BETWEEN, controls=[refresh_ctn, search_row])
    audio_row = ft.Row(spacing=25, alignment=ft.MainAxisAlignment.CENTER, controls=[play_button, pause_button, seek_button])
    audio_controls_column = ft.Column(alignment=ft.MainAxisAlignment.END, controls=[audio_row])

    page.add(
        #Search Functionality
        top_row,

        # Audio Controls button
        audio_controls_column
    )


ft.app(target=main, port=8034)

                    # Row(
                    #     controls=[
                    #         ElevatedButton(
                    #             text="7",
                    #             bgcolor=colors.WHITE24,
                    #             color=colors.WHITE,
                    #             expand=1,
                    #         ),
                    #         ElevatedButton(
                    #             text="8",
                    #             bgcolor=colors.WHITE24,
                    #             color=colors.WHITE,
                    #             expand=1,
                    #         ),
                    #         ElevatedButton(
                    #             text="9",
                    #             bgcolor=colors.WHITE24,
                    #             color=colors.WHITE,
                    #             expand=1,
                    #         ),
                    #         ElevatedButton(
                    #             text="*",
                    #             bgcolor=colors.ORANGE,
                    #             color=colors.WHITE,
                    #             expand=1,
                    #         ),
                    #     ]
                    # ),