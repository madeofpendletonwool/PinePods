import flet as ft
import datetime
import time
import requests
from flask import Flask
from flask_caching import Cache

app = Flask(__name__)
cache = Cache(app, config={'CACHE_TYPE': 'simple'})

@app.route('/preload/<path:url>')
def preload_audio_file(url):
    # Try to get the response from cache
    response = requests.get('http://10.0.0.15:5000/proxy', params={'url': url})
    if response.status_code == 200:
        # Cache the file content
        cache.set(url, response.content)
    return ""


url = "http://10.0.0.15:5000/proxy/edge2.pod.npr.org/anon.npr-mp3/npr/han/2023/03/20230327_han_f13cb0d3-30d5-4b99-9f3b-d56de09a651e.mp3/20230327_han_f13cb0d3-30d5-4b99-9f3b-d56de09a651e.mp3_ywr3ahjkcgo_a71dad59e711790c2d235d64391cb092_28858347.mp3?awCollectionId=510051&awEpisodeId=1166292893&orgId=1&d=1770&p=510051&story=1166292893&t=podcast&e=1166292893&size=28336005&ft=pod&f=510051&hash_redirect=1&x-total-bytes=28858347&x-ais-classified=unclassified&x-access-range=0-&listeningSessionID=0CD_382_86__558b10f0e6f194f787102271fbff74428ef25109"

# Preload the audio file and cache it
# preload_audio_file(url)

def main(page: ft.Page):
    def volume_down(_):
        audio1.volume -= 0.1
        audio1.update()

    def volume_up(_):
        audio1.volume += 0.1
        audio1.update()

    def balance_left(_):
        audio1.balance -= 0.1
        audio1.update()

    def balance_right(_):
        audio1.balance += 0.1
        audio1.update()

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
    page.add(
        ft.ElevatedButton("Play", on_click=lambda _: audio1.play()),
        ft.ElevatedButton("Pause", on_click=lambda _: audio1.pause()),
        ft.ElevatedButton("Resume", on_click=lambda _: audio1.resume()),
        ft.ElevatedButton("Release", on_click=lambda _: audio1.release()),
        ft.ElevatedButton("Seek 2s", on_click=lambda _: audio1.seek(2000)),
        ft.Row(
            [
                ft.ElevatedButton("Volume down", on_click=volume_down),
                ft.ElevatedButton("Volume up", on_click=volume_up),
            ]
        ),
        ft.Row(
            [
                ft.ElevatedButton("Balance left", on_click=balance_left),
                ft.ElevatedButton("Balance right", on_click=balance_right),
            ]
        ),
        ft.ElevatedButton(
            "Get duration", on_click=lambda _: print("Duration:", audio1.get_duration())
        ),
        ft.ElevatedButton(
            "Get current position",
            on_click=lambda _: print("Current position:", audio1.get_duration()),
        ),
    )

# ft.app(target=main)
# Browser Version
# ft.app(target=main, view=ft.WEB_BROWSER, port=8034)
# App version
ft.app(target=main, port=8034)

# if __name__ == '__main__':
#     app.run(port=5001)