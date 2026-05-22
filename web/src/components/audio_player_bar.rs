use crate::components::audio::AudioPlayer;
use crate::components::context::UIState;
use yew::prelude::*;
use yew::{function_component, html, Html};
use yewdux::prelude::*;

/// Isolated AudioPlayer wrapper — owns the UIState subscription so that
/// audio state changes never cause parent page components to re-render.
#[function_component(AudioPlayerBar)]
pub fn audio_player_bar() -> Html {
    let (audio_state, _) = use_store::<UIState>();
    if let Some(audio_props) = &audio_state.currently_playing {
        html! {
            <AudioPlayer
                episode={audio_props.episode.clone()}
                src={audio_props.src.clone()}
                title={audio_props.title.clone()}
                description={audio_props.description.clone()}
                release_date={audio_props.release_date.clone()}
                artwork_url={audio_props.artwork_url.clone()}
                duration={audio_props.duration.clone()}
                episode_id={audio_props.episode_id.clone()}
                duration_sec={audio_props.duration_sec.clone()}
                start_pos_sec={audio_props.start_pos_sec.clone()}
                end_pos_sec={audio_props.end_pos_sec.clone()}
                offline={audio_props.offline.clone()}
                is_youtube={audio_props.is_youtube.clone()}
                is_video={audio_props.is_video.clone()}
            />
        }
    } else {
        html! {}
    }
}
