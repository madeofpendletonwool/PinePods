
import requests
import tempfile
import os
# Supress pygame welcome message
os.environ['PYGAME_HIDE_SUPPORT_PROMPT'] = "hide"
import pygame

class Audio:
    def __init__(self, episode_url):
        self.episode_file = episode_url

    def play_podcast(self):
        response = requests.get(self.episode_file)
        with tempfile.NamedTemporaryFile(delete=False) as temp_file:
            temp_file.write(response.content)
        pygame.mixer.init()
        pygame.mixer.music.load(temp_file.name)
        pygame.mixer.music.play()
        os.unlink(temp_file.name)

    def pause_podcast(self):
        pygame.mixer.music.pause()

    def resume_podcast(self):
        pygame.mixer.music.unpause()

    def seek_podcast(self, start_time_ms, end_time_ms):
        pass

if __name__ == '__main__':
    import time

    url = "https://op3.dev/e/https://cdn.changelog.com/uploads/practicalai/1/practical-ai-1.mp3"

    testep1 = Audio(url)
    testep1.play_podcast()

    for i in range(10):
        print("Doing something else while audio is playing...")
        time.sleep(1)

    testep1.pause_podcast()