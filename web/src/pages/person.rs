use crate::components::app_drawer::App_drawer;
use crate::components::audio::AudioPlayer;
use crate::components::click_events::create_on_title_click;
use crate::components::context::{AppState, UIState};
use crate::components::episode_list_view::EpisodeListView;
use crate::components::gen_components::{FallbackImage, Search_nav, UseScrollToTop};
use crate::components::loading::Loading;
use crate::components::people::{proxied_image, use_auth, FollowButton, PersonAvatar};
use crate::requests::episode::Episode;
use crate::requests::people_req::{
    call_get_person_profile, call_lookup_person, PersonPodcast, PersonProfile,
};
use gloo_timers::future::TimeoutFuture;
use i18nrs::yew::use_translation;
use std::rc::Rc;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;

const PAGE_SIZE: i64 = 50;

/// How the URL names the person: `/person/12` (PinePods id), `/person/pp-34` (a PodPeopleDB id,
/// from discover) or `/person/Jane%20Doe` (old links by name).
#[derive(Clone, PartialEq, Debug)]
enum PersonKey {
    Id(i32),
    PodPeople(i64),
    Name(String),
}

fn parse_key(key: &str) -> PersonKey {
    if let Ok(id) = key.parse::<i32>() {
        return PersonKey::Id(id);
    }
    if let Some(id) = key.strip_prefix("pp-").and_then(|s| s.parse::<i64>().ok()) {
        return PersonKey::PodPeople(id);
    }
    let name = urlencoding::decode(key).map(|s| s.into_owned()).unwrap_or_else(|_| key.to_string());
    PersonKey::Name(name)
}

#[derive(Clone, Copy, PartialEq)]
enum EpisodesTab {
    Library,
    Elsewhere,
}

#[derive(Clone, PartialEq)]
enum Load {
    Loading,
    NotFound,
    Ready,
}

#[derive(Clone, Properties, PartialEq)]
pub struct PersonProps {
    /// PinePods person id, `pp-<PodPeopleDB id>`, or a name.
    pub person_key: String,
}

#[function_component(Person)]
pub fn person(props: &PersonProps) -> Html {
    let (i18n, _) = use_translation();
    let auth = use_auth();
    let user_id = use_selector(|s: &AppState| s.user_details.as_ref().map(|u| u.UserID)).as_ref().clone();
    let api_key_opt = use_selector(|s: &AppState| s.auth_details.as_ref().map(|a| a.api_key.clone())).as_ref().clone();
    let (audio_state, _) = use_store::<UIState>();

    let load = use_state(|| Load::Loading);
    let profile = use_state(|| None::<PersonProfile>);
    let library_episodes = use_state(|| Rc::new(Vec::<Episode>::new()));
    let can_load_more = use_state(|| false);
    let loading_more = use_state(|| false);
    let tab = use_state(|| None::<EpisodesTab>);

    // Resolve the key and load the profile. A refresh of outside-library episodes may still be
    // running on the server; refetch a few times until it reports done.
    {
        let load = load.clone();
        let profile = profile.clone();
        let library_episodes = library_episodes.clone();
        let can_load_more = can_load_more.clone();
        let tab = tab.clone();
        let auth = auth.clone();
        use_effect_with((props.person_key.clone(), auth.is_some()), move |(key, _)| {
            let cancelled = Rc::new(std::cell::Cell::new(false));
            if let Some((server, api_key)) = auth {
                let key = parse_key(key);
                let cancelled = cancelled.clone();
                load.set(Load::Loading);
                profile.set(None);
                tab.set(None);
                spawn_local(async move {
                    let person_id = match &key {
                        PersonKey::Id(id) => Ok(*id),
                        PersonKey::PodPeople(pp) => call_lookup_person(&server, &api_key, Some(*pp), None).await,
                        PersonKey::Name(name) => call_lookup_person(&server, &api_key, None, Some(name)).await,
                    };
                    let person_id = match person_id {
                        Ok(id) => id,
                        Err(_) => {
                            load.set(Load::NotFound);
                            return;
                        }
                    };
                    if !matches!(key, PersonKey::Id(_)) {
                        // Canonical URL; the route change reloads with the id.
                        BrowserHistory::new().replace(format!("/person/{}", person_id));
                        return;
                    }
                    for attempt in 0..4 {
                        if cancelled.get() {
                            return;
                        }
                        match call_get_person_profile(&server, &api_key, person_id, PAGE_SIZE, 0).await {
                            Ok(p) => {
                                let refreshing = p.refreshing;
                                can_load_more.set(p.episodes.len() as i64 >= PAGE_SIZE);
                                library_episodes.set(Rc::new(p.episodes.clone()));
                                profile.set(Some(p));
                                load.set(Load::Ready);
                                if !refreshing {
                                    return;
                                }
                            }
                            Err(_) if attempt == 0 => {
                                load.set(Load::NotFound);
                                return;
                            }
                            Err(_) => return,
                        }
                        TimeoutFuture::new(4_000).await;
                    }
                });
            }
            move || cancelled.set(true)
        });
    }

    let on_load_more = {
        let auth = auth.clone();
        let profile = profile.clone();
        let library_episodes = library_episodes.clone();
        let can_load_more = can_load_more.clone();
        let loading_more = loading_more.clone();
        use_callback(
            (profile.as_ref().map(|p| p.person.id), library_episodes.len()),
            move |_: (), (person_id, offset)| {
                let (Some((server, api_key)), Some(person_id)) = (auth.clone(), *person_id) else { return };
                if *loading_more {
                    return;
                }
                let offset = *offset as i64;
                let library_episodes = library_episodes.clone();
                let can_load_more = can_load_more.clone();
                let loading_more = loading_more.clone();
                loading_more.set(true);
                spawn_local(async move {
                    if let Ok(p) = call_get_person_profile(&server, &api_key, person_id, PAGE_SIZE, offset).await {
                        can_load_more.set(p.episodes.len() as i64 >= PAGE_SIZE);
                        let mut all = (**library_episodes).clone();
                        all.extend(p.episodes);
                        library_episodes.set(Rc::new(all));
                    }
                    loading_more.set(false);
                });
            },
        )
    };

    let elsewhere_episodes: Rc<Vec<Episode>> = use_memo(profile.clone(), |profile| {
        profile
            .as_ref()
            .as_ref()
            .map(|p| p.external_episodes.iter().cloned().map(|e| e.into_episode()).collect())
            .unwrap_or_default()
    });

    let server_name = auth.as_ref().map(|(s, _)| s.clone()).unwrap_or_default();
    let history = BrowserHistory::new();

    let show_card = |show: &PersonPodcast| -> Html {
        let in_library = show.podcastid.is_some();
        let onclick = create_on_title_click(
            server_name.clone(),
            api_key_opt.clone(),
            &history,
            // A library podcast opens directly; anything else is looked up and parsed like a
            // search result.
            show.podcastid.unwrap_or(0),
            show.podcastindexid.unwrap_or(0) as i32,
            show.podcastname.clone(),
            show.feedurl.clone(),
            String::new(),
            String::new(),
            show.artworkurl.clone().unwrap_or_default(),
            false,
            0,
            None,
            String::new(),
            user_id.unwrap_or_default(),
            false,
        );
        let roles = show.roles.join(", ");
        let detail = if show.podcast_level {
            roles
        } else {
            let count = i18n.t("people.episode_count").replace("{count}", &show.episode_count.to_string());
            if roles.is_empty() { count } else { format!("{} · {}", roles, count) }
        };
        html! {
            <button type="button" class="pp-show" {onclick} title={show.podcastname.clone()}>
                <div class="pp-show-art">
                    <FallbackImage
                        src={show.artworkurl.clone().unwrap_or_default()}
                        alt={show.podcastname.clone()}
                        class={Some("pp-show-img".to_string())}
                    />
                    { if in_library { html! {} } else {
                        html! { <span class="pp-show-badge">{ i18n.t("people.not_in_library") }</span> }
                    } }
                </div>
                <span class="pp-show-name">{ &show.podcastname }</span>
                <span class="pp-show-meta">{ detail }</span>
            </button>
        }
    };

    let body = match (&*load, profile.as_ref()) {
        (Load::NotFound, _) => html! {
            <div class="empty-episodes-container">
                <img src="static/assets/favicon.png" alt="Logo" class="logo"/>
                <h1 class="page-subtitles">{ i18n.t("people.person_not_found") }</h1>
                <p class="page-paragraphs">{ i18n.t("people.person_not_found_message") }</p>
            </div>
        },
        (Load::Ready, Some(p)) => {
            let person = &p.person;
            let library_count = library_episodes.len();
            let elsewhere_count = elsewhere_episodes.len();
            let active_tab = (*tab).unwrap_or(if library_count == 0 && elsewhere_count > 0 {
                EpisodesTab::Elsewhere
            } else {
                EpisodesTab::Library
            });
            let tab_button = |which: EpisodesTab, label: String, count: usize| {
                let tab = tab.clone();
                let active = active_tab == which;
                html! {
                    <button type="button" role="tab" aria-selected={active.to_string()}
                            class={classes!("ep-tab", active.then_some("is-active"))}
                            onclick={Callback::from(move |_: MouseEvent| tab.set(Some(which)))}>
                        <span>{ label }</span>
                        { if count > 0 { html! { <span class="ep-tab-badge">{ count }</span> } } else { html! {} } }
                    </button>
                }
            };
            let library_shows = p.podcasts.iter().filter(|s| s.podcastid.is_some()).count();
            let summary = i18n
                .t("people.profile_summary")
                .replace("{shows}", &p.podcasts.len().to_string())
                .replace("{library}", &library_shows.to_string());
            html! {
                <>
                <header class="pp-profile">
                    <PersonAvatar
                        src={person.img.as_deref().filter(|s| !s.is_empty()).map(|img| AttrValue::from(proxied_image(&server_name, img)))}
                        alt={person.name.clone()}
                        class="pp-profile-avatar"
                        placeholder="is-ph"
                    />
                    <div class="pp-profile-body">
                        <span class="pp-eyebrow">{ i18n.t("people.person") }</span>
                        <h1 class="pp-profile-name">{ &person.name }</h1>
                        <p class="pp-profile-meta">{ summary }</p>
                        <div class="pp-profile-actions">
                            <FollowButton person_id={person.id} followed={person.followed} />
                            { match person.href.as_deref().filter(|h| h.starts_with("http")) {
                                Some(href) => html! {
                                    <a class="pp-btn" href={href.to_string()} target="_blank" rel="noopener noreferrer">
                                        <i class="ph ph-link"></i><span>{ i18n.t("people.website") }</span>
                                    </a>
                                },
                                None => html! {},
                            } }
                        </div>
                    </div>
                </header>

                { if p.podcasts.is_empty() { html! {} } else { html! {
                    <section class="pp-section">
                        <h2 class="pp-section-title">{ i18n.t("people.shows") }</h2>
                        <div class="pp-shows">
                            { for p.podcasts.iter().map(|s| show_card(s)) }
                        </div>
                    </section>
                } } }

                <section class="pp-section">
                    <div class="ep-tabs" role="tablist">
                        { tab_button(EpisodesTab::Library, i18n.t("people.in_your_library").to_string(), library_count) }
                        { tab_button(EpisodesTab::Elsewhere, i18n.t("people.elsewhere").to_string(), elsewhere_count) }
                    </div>
                    { if p.refreshing && active_tab == EpisodesTab::Elsewhere {
                        html! { <p class="pp-note"><span class="sp-spinner"></span>{ i18n.t("people.looking_for_more") }</p> }
                    } else { html! {} } }
                    { match active_tab {
                        EpisodesTab::Library if library_count == 0 => html! {
                            <p class="pp-empty-text pp-pad">{ i18n.t("people.no_library_episodes") }</p>
                        },
                        EpisodesTab::Library => html! {
                            <EpisodeListView
                                key={format!("person-{}-library", person.id)}
                                episodes={(*library_episodes).clone()}
                                backend_can_load_more={*can_load_more}
                                loading_more={*loading_more}
                                on_load_more={on_load_more.clone()}
                            />
                        },
                        EpisodesTab::Elsewhere if elsewhere_count == 0 => html! {
                            <p class="pp-empty-text pp-pad">{
                                if p.refreshing { i18n.t("people.looking_for_more") } else { i18n.t("people.no_elsewhere_episodes") }
                            }</p>
                        },
                        EpisodesTab::Elsewhere => html! {
                            <EpisodeListView
                                key={format!("person-{}-elsewhere", person.id)}
                                episodes={elsewhere_episodes.clone()}
                            />
                        },
                    } }
                </section>
                </>
            }
        }
        _ => html! { <Loading /> },
    };

    html! {
        <>
            <div class="main-container">
                <Search_nav />
                <UseScrollToTop />
                <div class="pp-page">{ body }</div>
                {
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
            </div>
            <App_drawer />
        </>
    }
}
