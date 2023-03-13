    # Various flet imports
import flet as ft
from flet import *
from flet import AppBar, ElevatedButton, Page, Text, View, colors, icons, ProgressBar, ButtonStyle, IconButton, TextButton, Row, alignment
# Internal Functions
import internal_functions.functions
import database_functions.functions
import app_functions.functions
import Auth.Passfunctions
import Audio.functions
# Others
import time
import mysql.connector
import json
import re
import feedparser
import urllib.request
import requests
from functools import partial
import os
import requests
import tempfile
import time
import threading
import vlc
import random

    
# def main(page):
#     page.add(
#         ft.Text("Slider with value:"),
#         ft.Slider(value=0.3),
#         ft.Text("Slider with a custom range and label:"),
#         ft.Slider(min=0, max=100, divisions=10, label="{value}%"))
def main(page: ft.Page):
# ft.app(target=main):
    class Toggle_Pod:
        initialized = False

        def __init__(self, page, go_home, url=None, name=None):
            if not Toggle_Pod.initialized:
                self.page = page
                self.go_home = go_home
                self.url = url
                self.name = name or ""
                self.audio_playing = False
                self.episode_file = url
                self.episode_name = name
                self.instance = vlc.Instance("--no-xlib") # Use "--no-xlib" option to run on server without GUI
                self.player = self.instance.media_player_new()
                self.thread = None
                self.length = .1
                self.length_min = 0
                self.length_max = 2.45
                # self.episode_name = self.name
                if url is None or name is None:
                    self.active_pod = 'Initial Value'
                else:
                    self.active_pod = self.name
                self.queue = []
                Toggle_Pod.initialized = True
            else:
                self.page = page
                self.go_home = go_home
                self.url = url
                self.name = name or ""
                self.audio_playing = False
                self.active_pod = self.name
                self.episode_file = url
                self.episode_name = name
                self.instance = vlc.Instance("--no-xlib") # Use "--no-xlib" option to run on server without GUI
                self.player = self.instance.media_player_new()
                self.thread = None
                self.length = .1
                self.length_min = 0
                self.length_max = 2.45
                # self.episode_name = self.name
                self.queue = []

        def play_episode(self, e=None):
            media = self.instance.media_new(self.url)
            self.player.set_media(media)
            self.player.play()
            self.thread = threading.Thread(target=self._monitor_audio)
            self.thread.start()
            self.audio_playing = True
            self.toggle_current_status()
            self.record_history()
            self.length = 1

        def _monitor_audio(self):
            while True:
                state = self.player.get_state()
                if state == vlc.State.Ended:
                    self.thread = None
                    break
                time.sleep(1)

        def pause_episode(self, e=None):
            self.player.pause()
            self.audio_playing = False
            self.toggle_current_status()
            self.page.update()

        def resume_podcast(self, e=None):
            self.player.play()
            self.audio_playing = True
            self.toggle_current_status()
            self.page.update()

        def toggle_current_status(self):
            if self.audio_playing:
                play_button.visible = False
                pause_button.visible = True
                audio_container.visible = True
                currently_playing.content = ft.Text(self.name)
                self.page.update()
            else:
                pause_button.visible = False
                play_button.visible = True
                currently_playing.content = ft.Text(self.name)
                self.page.update()

        def seek_episode(self):
            seconds = 10
            time = self.player.get_time()
            self.player.set_time(time + seconds * 1000) # VLC seeks in milliseconds

        def record_history(self):
            user_id = get_user_id()
            database_functions.functions.record_podcast_history(cnx, self.name, user_id, 0)

        def download_pod(self):
            database_functions.functions.download_podcast(cnx, self.url, self.title, active_user.user_id)

        def delete_pod(self):
            database_functions.functions.delete_podcast(cnx, self.url, self.title, active_user.user_id)


        def queue_pod(self):
            if self.audio_playing:
                # Add the new episode URL to the vlc playlist
                media = self.instance.media_new(self.url)
                media_list = self.instance.media_list_new([media])
                media_list_player = self.instance.media_list_player_new()
                media_list_player.set_media_list(media_list)

                # Update the internal queue list
                self.queue.append(self.url)

                print(f"Added episode '{self.title}' to the queue")
                print(self.queue)
            else:
                self.play_episode()

        def get_queue(self):
            return self.queue

    def refresh_podcasts(e):
        pr = ft.ProgressRing()
        page.overlay.append(ft.Stack([pr], bottom=25, right=30, left=20, expand=True))
        page.update()
        database_functions.functions.refresh_pods(cnx)
        print('refresh complete')
        page.overlay.pop(2)
        page.update()

    def evaluate_podcast(pod_title, pod_artwork, pod_author, pod_categories, pod_description, pod_episode_count, pod_feed_url, pod_website):
        global clicked_podcast
        clicked_podcast = Podcast(name=pod_title, artwork=pod_artwork, author=pod_author, description=pod_description, feedurl=pod_feed_url, website=pod_website)
        return clicked_podcast

    def get_user_id():
        current_username = active_user.username
        user_id = database_functions.functions.get_user_id(cnx, current_username)
        return user_id

    class Podcast:
        def __init__(self, name=None, artwork=None, author=None, description=None, feedurl=None, website=None):
            self.name = name
            self.artwork = artwork
            self.author = author
            self.description = description
            self.feedurl = feedurl
            self.website = website

    def go_home():
        pass

    

    current_episode = Toggle_Pod(page, go_home)

    
    
    audio_scrubber = ft.Slider(min=current_episode.length_min, divisions=3600, max=current_episode.length_max, label="{value}")

    page.add(audio_scrubber)


# Browser Version
# ft.app(target=main, view=ft.WEB_BROWSER)
# App version
ft.app(target=main, port=8034)