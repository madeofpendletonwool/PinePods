# Various flet imports
import flet as ft
from flet import Text, colors, icons, ButtonStyle, Row, alignment, border_radius, animation, \
    MainAxisAlignment, padding

class PodcastControls:

    def __init__(self, page, go_home=None, parsed_audio_url=None, parsed_title=None, current_episode=None, active_user=None):
        self.active_user = active_user
        self.audio_container_image_landing = None
        self.audio_scrubber = None
        self.current_time = None
        self.current_time_text = None
        self.audio_scrubber_column = None
        self.podcast_length = None
        self.ep_audio_controls = None
        self.pause_button = None
        self.play_button = None
        self.seek_button = None
        self.currently_playing = None
        self.current_episode = current_episode
        self.page = page
        self.go_home = go_home
        self.parsed_audio_url = parsed_audio_url
        self.parsed_title = parsed_title
        # self.current_episode = Toggle_Pod(page, go_home, parsed_audio_url, parsed_title)
        self.init_controls()

    def open_currently_playing(self, e):
        self.active_user.show_audio_container = False
        self.page.go("/playing")

    def init_controls(self):
        self.create_audio_controls()
        self.setup_audio_scrubber()
        self.setup_audio_container()
        self.setup_volume_control()

    def create_audio_controls(self):
        self.play_button = ft.IconButton(
            icon=ft.icons.PLAY_ARROW,
            tooltip="Play Podcast",
            icon_color="white",
            on_click=lambda e: self.current_episode.resume_podcast()
        )
        self.pause_button = ft.IconButton(
            icon=ft.icons.PAUSE,
            tooltip="Pause Playback",
            icon_color="white",
            on_click=lambda e: self.current_episode.pause_episode()
        )
        self.pause_button.visible = False
        self.seek_button = ft.IconButton(
            icon=ft.icons.FAST_FORWARD,
            tooltip="Seek 10 seconds",
            icon_color="white",
            on_click=lambda e: self.current_episode.seek_episode()
        )
        self.ep_audio_controls = ft.Row(controls=[self.play_button, self.pause_button, self.seek_button])

    def setup_audio_scrubber(self):
        def format_time(time):
            hours, remainder = divmod(int(time), 3600)
            minutes, seconds = divmod(remainder, 60)
            return f"{hours:02d}:{minutes:02d}:{seconds:02d}"

        def slider_changed(e):
            formatted_scrub = format_time(self.audio_scrubber.value)
            self.current_time.content = ft.Text(formatted_scrub)
            # self.current_time.update()
            self.current_episode.time_scrub(self.audio_scrubber.value)

        self.podcast_length = ft.Container(content=ft.Text('doesntmatter'))
        self.current_time_text = ft.Text('placeholder')
        self.current_time = ft.Container(content=self.current_time_text)
        self.audio_scrubber = ft.Slider(min=0, expand=True, max=self.current_episode.seconds, label="{value}",
                                        on_change=slider_changed)
        self.audio_scrubber.width = '100%'
        self.audio_scrubber_column = ft.Column(controls=[self.audio_scrubber])
        self.audio_scrubber_column.horizontal_alignment.STRETCH
        self.audio_scrubber_column.width = '100%'

    def setup_audio_container(self):
        self.currently_playing = ft.Container(content=ft.Text('test'), on_click=self.open_currently_playing)
        self.audio_container_image_landing = ft.Image(
            src=f"None",
            width=40, height=40)
        self.audio_container_image = ft.Container(content=self.audio_container_image_landing,
                                                  on_click=self.open_currently_playing)
        self.audio_container_image.border_radius = ft.border_radius.all(25)
        self.currently_playing_container = ft.Row(
            controls=[self.audio_container_image, self.currently_playing])
        self.scrub_bar_row = ft.Row(controls=[self.current_time, self.audio_scrubber_column, self.podcast_length])
        self.volume_button = ft.IconButton(icon=ft.icons.VOLUME_UP_ROUNDED, tooltip="Adjust Volume",
                                           on_click=lambda x: self.current_episode.volume_view())
        self.audio_controls_row = ft.Row(alignment=ft.MainAxisAlignment.CENTER,
                                         controls=[self.scrub_bar_row, self.ep_audio_controls, self.volume_button])
        self.audio_container_row_landing = ft.Row(
            vertical_alignment=ft.CrossAxisAlignment.END,
            alignment=ft.MainAxisAlignment.SPACE_BETWEEN,
            controls=[self.currently_playing_container, self.audio_controls_row])
        self.audio_container_row = ft.Container(content=self.audio_container_row_landing)
        self.audio_container_row.padding = ft.padding.only(left=10)
        self.audio_container_pod_details = ft.Row(
            controls=[self.audio_container_image, self.currently_playing],
            alignment=ft.MainAxisAlignment.CENTER)
        ep_height = 50
        ep_width = 4000
        self.audio_container = ft.Container(
            height=ep_height,
            width=ep_width,
            bgcolor=self.active_user.main_color,
            border_radius=45,
            padding=6,
            content=self.audio_container_row
        )

    def setup_volume_control(self):
        self.volume_slider = ft.Slider(value=1, on_change=lambda x: self.current_episode.volume_adjust())
        self.volume_down_icon = ft.Icon(name=ft.icons.VOLUME_MUTE)
        self.volume_up_icon = ft.Icon(name=ft.icons.VOLUME_UP_ROUNDED)
        self.volume_adjust_column = ft.Row(
            controls=[self.volume_down_icon, self.volume_slider, self.volume_up_icon], expand=True)
        self.volume_container = ft.Container(
            height=35,
            width=275,
            bgcolor=ft.colors.WHITE,
            border_radius=45,
            padding=6,
            content=self.volume_adjust_column)
        self.volume_container.adding = ft.padding.all(50)
        self.volume_container.alignment = ft.alignment.top_right
        self.volume_container.visible = False

        self.page.overlay.append(ft.Stack([self.volume_container], bottom=75, right=25, expand=True))
        self.page.overlay.append(ft.Stack([self.audio_container], bottom=20, right=20, left=70, expand=True))
        self.audio_container.visible = False