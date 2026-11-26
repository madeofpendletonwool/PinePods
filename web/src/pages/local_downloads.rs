//! Episodes downloaded to this computer (desktop app). Works without the server: the list
//! and the files are both on this machine.

use crate::components::app_drawer::App_drawer;
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, EpisodeStatusState, NotificationState, UIState};
use crate::components::context_menu_button::PageType;
use crate::components::gen_components::{empty_message, Search_nav, UseScrollToTop};
use crate::components::i18n::use_translation;
use crate::desktop::{self, DeviceDownloads, Phase};
use crate::pages::downloads::render_podcast_with_episodes;
use crate::requests::episode::Episode;
use crate::requests::pod_req::PodcastDownloadSummary;
use std::collections::{BTreeMap, HashMap, HashSet};
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;

#[function_component(LocalDownloads)]
pub fn local_downloads() -> Html {
    let (i18n, _) = use_translation();
    let (state, dispatch) = use_store::<AppState>();
    let (ep_status, _) = use_store::<EpisodeStatusState>();
    let (device, _) = use_store::<DeviceDownloads>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
    let expanded = use_state(HashMap::<i32, bool>::new);
    let selecting = use_state(|| false);
    let search = use_state(String::new);
    let show_completed = use_state(|| false);
    let show_in_progress = use_state(|| false);
    let offline = audio_state.app_offline_mode.unwrap_or(false);

    // The list can change from another window or after a restart; reload it on open.
    use_effect_with((), |_| {
        wasm_bindgen_futures::spawn_local(desktop::refresh());
        || ()
    });

    if !desktop::is_desktop() {
        return html! {
            <div class="main-container">
                { empty_message(&i18n.t("desktop.only_in_app"), &i18n.t("desktop.only_in_app_description")) }
            </div>
        };
    }

    let local: HashSet<i32> = ep_status.downloaded_episodes.local_ids().collect();
    let term = search.to_lowercase();
    let episodes: Vec<Episode> = ep_status
        .downloaded_episodes
        .episodes()
        .filter(|ep| local.contains(&ep.episodeid))
        .filter(|ep| term.is_empty() || ep.episodetitle.to_lowercase().contains(&term) || ep.podcastname.to_lowercase().contains(&term))
        .filter(|ep| {
            let completed = ep.completed || ep_status.completed_episodes.contains(&ep.episodeid);
            if *show_completed {
                completed
            } else if *show_in_progress {
                !completed && ep.listenduration > 0
            } else {
                true
            }
        })
        .cloned()
        .collect();

    // Group by podcast, named from the episodes themselves so this works offline.
    let mut groups: BTreeMap<String, (PodcastDownloadSummary, Vec<Episode>)> = BTreeMap::new();
    for ep in episodes {
        let entry = groups.entry(ep.podcastname.to_lowercase()).or_insert_with(|| {
            let art = [&ep.artworkurl, &ep.episodeartwork].into_iter().find(|a| !a.is_empty()).cloned();
            (
                PodcastDownloadSummary {
                    podcastid: ep.podcastid,
                    podcastname: ep.podcastname.clone(),
                    artworkurl: art,
                    episode_count: 0,
                },
                Vec::new(),
            )
        });
        entry.0.episode_count += 1;
        entry.1.push(ep);
    }

    let toggle = {
        let expanded = expanded.clone();
        Callback::from(move |id: i32| {
            let mut next = (*expanded).clone();
            let open = next.get(&id).copied().unwrap_or(false);
            next.insert(id, !open);
            expanded.set(next);
        })
    };
    let on_checkbox_change = {
        let dispatch = dispatch.clone();
        Callback::from(move |id: i32| {
            dispatch.reduce_mut(move |s| {
                if !s.selected_episodes_for_deletion.remove(&id) {
                    s.selected_episodes_for_deletion.insert(id);
                }
            })
        })
    };
    let delete_selected = {
        let dispatch = dispatch.clone();
        let selecting = selecting.clone();
        let deleted = i18n.t("desktop.deleted_count").to_string();
        let failed = i18n.t("desktop.delete_failed").to_string();
        Callback::from(move |_: MouseEvent| {
            let ids: Vec<i32> = dispatch.get().selected_episodes_for_deletion.iter().copied().collect();
            dispatch.reduce_mut(|s| s.selected_episodes_for_deletion.clear());
            selecting.set(false);
            let (deleted, failed) = (deleted.clone(), failed.clone());
            wasm_bindgen_futures::spawn_local(async move {
                let result = desktop::delete(ids).await;
                Dispatch::<NotificationState>::global().reduce_mut(move |s| match result {
                    Ok(gone) => s.info_message = Some(deleted.replace("{count}", &gone.len().to_string())),
                    Err(e) => s.error_message = Some(format!("{failed}: {e}")),
                });
            });
        })
    };
    let toggle_select = {
        let selecting = selecting.clone();
        let dispatch = dispatch.clone();
        Callback::from(move |_: MouseEvent| {
            dispatch.reduce_mut(|s| s.selected_episodes_for_deletion.clear());
            selecting.set(!*selecting);
        })
    };
    let go_online = {
        let audio_dispatch = audio_dispatch.clone();
        Callback::from(move |_: MouseEvent| {
            audio_dispatch.reduce_mut(|s| s.app_offline_mode = Some(false));
            BrowserHistory::new().push("/");
        })
    };

    let mut active: Vec<_> = device.active.values().cloned().collect();
    active.sort_by_key(|p| p.episode_id);
    let active_section = if active.is_empty() {
        html! {}
    } else {
        html! {
            <div class="pfb-section">
                <h2 class="text-lg font-semibold item_container-text mb-2">{ i18n.t("desktop.downloading_now") }</h2>
                { for active.into_iter().map(|p| {
                    let pct = p.percent();
                    let status = match (&p.phase, pct) {
                        (Phase::Queued, _) => i18n.t("desktop.download_waiting").to_string(),
                        (_, Some(pct)) => format!("{pct}% · {}", desktop::format_size(p.received)),
                        _ => desktop::format_size(p.received),
                    };
                    let id = p.episode_id;
                    let cancel = Callback::from(move |_: MouseEvent| {
                        wasm_bindgen_futures::spawn_local(desktop::cancel(id));
                    });
                    html! {
                        <div key={id} class="flex items-center gap-3 py-2">
                            <div class="flex-1 min-w-0">
                                <p class="item_container-text truncate">{ &p.title }</p>
                                <div class="w-full h-1 rounded overflow-hidden" style="background: var(--secondary-background)">
                                    <div class="h-1" style={format!("width: {}%; background: var(--accent-color)", pct.unwrap_or(0))}></div>
                                </div>
                                <p class="text-sm item_container-text opacity-70">{ status }</p>
                            </div>
                            <button class="sp-chip" onclick={cancel}>
                                <i class="ph ph-x"></i>
                                <span>{ i18n.t("common.cancel") }</span>
                            </button>
                        </div>
                    }
                }) }
            </div>
        }
    };

    let selected_count = state.selected_episodes_for_deletion.len();
    let toolbar = html! {
        <div class="flex gap-2 justify-end mb-4">
            if *selecting {
                <button class="sp-chip" onclick={toggle_select.clone()}>
                    <i class="ph ph-prohibit"></i><span>{ i18n.t("common.cancel") }</span>
                </button>
                <button class="sp-chip is-alert" onclick={delete_selected} disabled={selected_count == 0}>
                    <i class="ph ph-trash"></i><span>{ i18n.t("common.delete") }</span>
                </button>
            } else {
                <button class="sp-chip" onclick={toggle_select}>
                    <i class="ph ph-lasso"></i><span>{ i18n.t("downloads.select") }</span>
                </button>
            }
        </div>
    };

    let chip = |active: &UseStateHandle<bool>, other: &UseStateHandle<bool>, icon: &str, label: String| {
        let (a, o) = (active.clone(), other.clone());
        let onclick = Callback::from(move |_: MouseEvent| {
            a.set(!*a);
            o.set(false);
        });
        html! {
            <button {onclick} class={classes!("sp-chip", (**active).then_some("is-active"))}>
                <i class={classes!("ph", icon.to_string())}></i><span>{ label }</span>
            </button>
        }
    };
    let filters = html! {
        <div class="pfb-section">
            <div class="pfb-bar">
                <div class="sp-input">
                    <i class="ph ph-magnifying-glass sp-search-ico"></i>
                    <input
                        type="text"
                        placeholder={i18n.t("downloads.search_downloaded_episodes")}
                        value={(*search).clone()}
                        oninput={let search = search.clone(); Callback::from(move |e: InputEvent| {
                            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                search.set(input.value());
                            }
                        })}
                    />
                </div>
            </div>
            <div class="sp-chips pfb-chips">
                { chip(&show_completed, &show_in_progress, "ph-check-circle", i18n.t("downloads.completed").to_string()) }
                { chip(&show_in_progress, &show_completed, "ph-hourglass-medium", i18n.t("downloads.in_progress").to_string()) }
            </div>
        </div>
    };

    let list = if groups.is_empty() {
        empty_message(&i18n.t("desktop.no_downloads"), &i18n.t("desktop.no_downloads_description"))
    } else {
        let rendered: Vec<Html> = groups.into_values().map(|(summary, episodes)| {
            let id = summary.podcastid;
            let total = summary.episode_count;
            render_podcast_with_episodes(
                &summary,
                episodes,
                total,
                expanded.get(&id).copied().unwrap_or(false),
                toggle.reform(move |_: MouseEvent| id),
                dispatch.clone(),
                *selecting,
                on_checkbox_change.clone(),
                false,
                false,
                Callback::noop(),
                String::new(),
                Callback::noop(),
                PageType::LocalDownloads,
            )
        }).collect();
        rendered.into_iter().collect::<Html>()
    };

    let folder = device.downloads_dir.clone().map(|dir| html! {
        <p class="text-sm item_container-text opacity-70 mt-6">{ format!("{} {dir}", i18n.t("desktop.saved_in")) }</p>
    });

    html! {
        <>
        <div class="main-container">
            if !offline { <Search_nav /> }
            <UseScrollToTop />
            if offline {
                <div class="w-full p-4 mb-4">
                    <button onclick={go_online} class="sp-chip w-full justify-center">
                        <i class="ph ph-cloud"></i><span>{ i18n.t("desktop.go_online") }</span>
                    </button>
                </div>
            }
            <h1 class="text-2xl item_container-text font-bold mb-4">{ i18n.t("app_drawer.local_downloads") }</h1>
            { active_section }
            { toolbar }
            { filters }
            { list }
            { folder.unwrap_or_default() }
            if let Some(audio_props) = &audio_state.currently_playing {
                <AudioPlayer
                    episode={audio_props.episode.clone()}
                    src={audio_props.src.clone()}
                    title={audio_props.title.clone()}
                    description={audio_props.description.clone()}
                    release_date={audio_props.release_date.clone()}
                    artwork_url={audio_props.artwork_url.clone()}
                    duration={audio_props.duration.clone()}
                    episode_id={audio_props.episode_id}
                    duration_sec={audio_props.duration_sec}
                    start_pos_sec={audio_props.start_pos_sec}
                    end_pos_sec={audio_props.end_pos_sec}
                    offline={audio_props.offline}
                    is_youtube={audio_props.is_youtube}
                    is_video={audio_props.is_video}
                />
            }
        </div>
        if !offline { <App_drawer /> }
        </>
    }
}
