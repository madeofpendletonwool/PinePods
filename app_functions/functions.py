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
