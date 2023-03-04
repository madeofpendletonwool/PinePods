import vlc
import time
import threading

class Audio:
    def __init__(self, episode_url, episode_name):
        self.episode_file = episode_url
        self.episode_name = episode_name
        self.instance = vlc.Instance("--no-xlib") # Use "--no-xlib" option to run on server without GUI
        self.player = self.instance.media_player_new()
        self.thread = None

    def play_podcast(self):
        media = self.instance.media_new(self.episode_file)
        self.player.set_media(media)
        self.player.play()
        self.thread = threading.Thread(target=self._monitor_audio)
        self.thread.start()

    def pause_podcast(self):
        self.player.pause()
        audio_playing = False
        return audio_playing

    def resume_podcast(self):
        if self.player:
            self.player.play()
            audio_playing = True
        else:
            audio_playing = None
        return audio_playing

    def seek_podcast(self, seconds):
        if self.player:
            time = self.player.get_time()
            self.player.set_time(time + seconds * 1000) # VLC seeks in milliseconds
        else:
            return None
    
    def _monitor_audio(self):
        while True:
            state = self.player.get_state()
            if state == vlc.State.Ended:
                self.thread = None
                break
            time.sleep(1)

if __name__ == '__main__':
    url = "https://op3.dev/e/https://cdn.changelog.com/uploads/practicalai/1/practical-ai-1.mp3"
    nme = 'nothing'
    testep1 = Audio(url, nme)
    testep1.play_podcast()

    while True:
        # Do other things here while audio is playing
        time.sleep(1)
        print("Still playing audio...")

        # Pause audio after 10 seconds
        if testep1.player.get_state() == vlc.State.Playing and testep1.thread is not None and testep1.player.get_time() >= 10000:
            testep1.pause_podcast()
            print("Paused audio")
            break

    while True:
        # Resume audio after 5 seconds
        time.sleep(5)
        print("Resuming audio...")
        testep1.resume_podcast()
        if testep1.player.get_state() == vlc.State.Playing:
            break

    while True:
        # Seek audio after 5 seconds
        time.sleep(5)
        print("Seeking audio...")
        testep1.seek_podcast(30)
        if testep1.player.get_time() >= 30000:
            break

    # Stop audio and wait for thread to finish
    testep1.player.stop()
    if testep1.thread is not None:
        testep1.thread.join()
    print("Finished playing audio")
