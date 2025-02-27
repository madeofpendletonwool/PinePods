use crate::components::app_drawer::App_drawer;
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, UIState};
use crate::components::feed::VirtualList;
use crate::components::gen_components::{Search_nav, UseScrollToTop};
use crate::requests::pod_req;
use yew::prelude::*;
use yewdux::prelude::*;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub id: i32,
}

#[function_component(PlaylistDetail)]
pub fn playlist_detail(props: &Props) -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
    let loading = use_state(|| true);
    let error = use_state(|| None::<String>);

    // Fetch playlist episodes
    {
        let loading = loading.clone();
        let error = error.clone();
        let playlist_id = props.id;
        let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
        let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
        let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());

        use_effect_with(
            (api_key.clone(), user_id.clone(), server_name.clone()),
            move |_| {
                if let (Some(api_key), Some(user_id), Some(server_name)) =
                    (api_key.clone(), user_id.clone(), server_name.clone())
                {
                    wasm_bindgen_futures::spawn_local(async move {
                        match pod_req::call_get_playlist_episodes(
                            &server_name,
                            &api_key.unwrap(),
                            &user_id,
                            playlist_id,
                        )
                        .await
                        {
                            Ok(response) => {
                                // Update state with playlist info and episodes
                                dispatch.reduce_mut(move |state| {
                                    state.current_playlist_episodes = Some(response.episodes);
                                    state.current_playlist_info = Some(response.playlist_info);
                                });
                                loading.set(false);
                            }
                            Err(e) => {
                                error.set(Some(e.to_string()));
                                loading.set(false);
                            }
                        }
                    });
                }
                || ()
            },
        );
    }

    html! {
        <>
            <div class="main-container">
                <Search_nav />
                <UseScrollToTop />

                if *loading {
                    <div class="loading-animation">
                        <div class="frame1"></div>
                        <div class="frame2"></div>
                        <div class="frame3"></div>
                        <div class="frame4"></div>
                        <div class="frame5"></div>
                        <div class="frame6"></div>
                    </div>
                } else {
                    if let Some(error_msg) = &*error {
                        <div class="error-message">
                            {error_msg}
                        </div>
                    } else {
                        <div class="section-container">
                            // Playlist header
                            if let Some(playlist_info) = &state.current_playlist_info {
                                <div class="playlist-header mb-6">
                                    <div class="flex items-center gap-4">
                                        <i class={classes!("text-4xl", playlist_info.icon_name.clone())}></i>
                                        <div>
                                            <h1 class="text-2xl font-bold item_container-text">{&playlist_info.name}</h1>
                                            if let Some(desc) = &playlist_info.description {
                                                <p class="text-gray-600 dark:text-gray-400">{desc}</p>
                                            }
                                            <p class="text-sm item_container-text mt-1">
                                                {format!("{} episodes", playlist_info.episode_count.unwrap_or(0))}
                                            </p>
                                        </div>
                                    </div>
                                </div>
                            }

                            if let Some(episodes) = &state.current_playlist_episodes {
                                if episodes.is_empty() {
                                    <div class="flex flex-col items-center justify-center p-8 mt-4 item-container rounded-lg shadow-md">
                                        <i class="ph ph-playlist-x text-6xl mb-4"></i>
                                        <h3 class="text-xl font-semibold mb-2 item_container-text">{"No Episodes Found"}</h3>
                                        <p class="text-center item_container-text text-sm max-w-md">
                                            {match &state.current_playlist_info {
                                                Some(playlist_info) => match playlist_info.name.as_str() {
                                                    "Fresh Releases" => "No new episodes have been released in the last 24 hours. Check back later for fresh content!",
                                                    "Currently Listening" => "Start listening to some episodes and they'll appear here for easy access.",
                                                    "Almost Done" => "You don't have any episodes that are near completion. Keep listening!",
                                                    _ => "No episodes match the current playlist criteria. Try adjusting the filters or add more podcasts."
                                                },
                                                None => "No episodes match the current playlist criteria. Try adjusting the filters or add more podcasts."
                                            }}
                                        </p>
                                    </div>
                                } else {
                                    <VirtualList
                                        episodes={episodes.clone()}
                                        page_type="playlist"
                                    />
                                }
                            }
                        </div>
                    }
                }

                if let Some(audio_props) = &audio_state.currently_playing {
                    <AudioPlayer
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
                    />
                }
            </div>
            <App_drawer />
        </>
    }
}
