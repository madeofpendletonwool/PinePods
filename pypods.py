import flet as ft
import time
import InternalFunctions.functions
import database_functions.functions
import app_functions.functions
import mysql.connector

cnx = mysql.connector.connect(
    host="127.0.0.1",
    port="3306",
    user="root",
    password="password",
    database="pypods_database"
)

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

    def return_home(e):
        # print('clicked once')
        # page.update()

        page.clean()
        page.add(ft.Text("Returning to Homepage!"))
        main(e)   

    def evaluate_podcast(e):
        page.clean()
        page.add(ft.Text("evaluating feed"))


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
            # Allow scrolling otherwise the page will overflow
            page.scroll = "auto"
            page.update()

            # Create back button
            back_button = ft.IconButton(
                icon=ft.icons.ARROW_BACK_IOS_NEW_ROUNDED,
                icon_color='blue400',
                icon_size=30,
                tooltip='Return to Homepage',
                on_click=return_home,
                data=True
            )
            page.add(back_button)
            #cycle through podcasts and add results to page
            pod_number = 1

            for d in return_results:
                # print(d['title'])
                for k, v in d.items():
                    if k == 'title':
                    # Defining the attributes of each podcast that will be displayed on screen
                        pod_image = ft.Image(src=d['image'], width=150, height=150)
                        pod_title = ft.TextButton(
                            text=d['title'], 
                            on_click=evaluate_podcast
                            )
                        pod_desc = ft.Text(d['description'], no_wrap=False)
                        # Episode Count and subtitle
                        pod_ep_title = ft.Text('Episode Count:', weight=ft.FontWeight.BOLD)
                        pod_ep_count = ft.Text(d['episodeCount'])
                        pod_ep_info = ft.Row(controls=[pod_ep_title, pod_ep_count])
                    # Creating column and row for search layout
                        search_column = ft.Column(
                            wrap=True,
                            controls=[pod_title, pod_desc, pod_ep_info]
                        )
                        search_row = ft.Row(
                            wrap=True,
                            alignment=ft.MainAxisAlignment.START, 
                            controls=[pod_image, search_column])
                        

                        page.add(search_row)
                        pod_number += 1
                    # if k == 'description':
                    #     page.add(ft.Text(f"{v}"))
                    #     page.add(ft.Text('next pod ---------'))
                    
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

# Browser Version
ft.app(target=main, view=ft.WEB_BROWSER, port=38356)
# App version
# ft.app(target=main, port=8034)

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