use crate::components::app_drawer::App_drawer;
use crate::components::audio_player_bar::AudioPlayerBar;
use crate::components::episode_list_view::EpisodeListView;
use crate::components::gen_components::{empty_message, Search_nav, UseScrollToTop};
use crate::components::loading::Loading;
use crate::components::people::{person_path, proxied_image, use_auth, FollowButton, PersonAvatar};
use crate::requests::episode::Episode;
use crate::requests::people_req::{call_get_followed_people, call_get_person_profile, PersonInfo};
use i18nrs::yew::use_translation;
use std::collections::HashSet;
use std::rc::Rc;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};

/// Episodes per person in the combined feed.
const PER_PERSON: i64 = 20;
const FEED_LIMIT: usize = 150;

/// People the user follows, and their latest episodes across every show.
#[function_component(SubscribedPeople)]
pub fn subscribed_people() -> Html {
    let (i18n, _) = use_translation();
    let auth = use_auth();
    let people = use_state(|| None::<Vec<PersonInfo>>);
    let feed = use_state(|| None::<Rc<Vec<Episode>>>);

    {
        let people = people.clone();
        let feed = feed.clone();
        use_effect_with(auth.clone(), move |auth| {
            if let Some((server, key)) = auth.clone() {
                spawn_local(async move {
                    let list = call_get_followed_people(&server, &key).await.unwrap_or_default();
                    people.set(Some(list.clone()));

                    let profiles = futures::future::join_all(
                        list.iter().map(|p| call_get_person_profile(&server, &key, p.id, PER_PERSON, 0)),
                    )
                    .await;
                    let mut seen = HashSet::new();
                    let mut episodes: Vec<Episode> = Vec::new();
                    for profile in profiles.into_iter().flatten() {
                        let outside = profile.external_episodes.into_iter().map(|e| e.into_episode());
                        for ep in profile.episodes.into_iter().chain(outside) {
                            // The same episode can credit several followed people.
                            if seen.insert(ep.episodeurl.clone()) {
                                episodes.push(ep);
                            }
                        }
                    }
                    episodes.sort_by(|a, b| b.episodepubdate.cmp(&a.episodepubdate));
                    episodes.truncate(FEED_LIMIT);
                    feed.set(Some(Rc::new(episodes)));
                });
            }
            || ()
        });
    }

    let server_name = auth.as_ref().map(|(s, _)| s.clone()).unwrap_or_default();

    let content = match &*people {
        None => html! { <Loading /> },
        Some(list) if list.is_empty() => empty_message(
            &i18n.t("people.followed_empty_title"),
            &i18n.t("people.followed_empty_message"),
        ),
        Some(list) => html! {
            <>
            <section class="pp-section">
                <div class="pp-followed">
                    { for list.iter().map(|p| {
                        let id = p.id;
                        let open = Callback::from(move |_: MouseEvent| BrowserHistory::new().push(person_path(id)));
                        html! {
                            <div class="pp-followed-card" key={p.id} onclick={open} title={p.name.clone()}>
                                <PersonAvatar
                                    src={p.img.as_deref().filter(|s| !s.is_empty()).map(|img| AttrValue::from(proxied_image(&server_name, img)))}
                                    alt={p.name.clone()}
                                    class="pp-followed-avatar"
                                    placeholder="is-ph"
                                />
                                <span class="pp-followed-name">{ &p.name }</span>
                                <FollowButton person_id={p.id} followed={p.followed} compact={true} />
                            </div>
                        }
                    }) }
                </div>
            </section>
            <section class="pp-section">
                <h2 class="pp-section-title">{ i18n.t("people.latest_episodes") }</h2>
                { match &*feed {
                    None => html! { <p class="pp-note"><span class="sp-spinner"></span>{ i18n.t("people_subs.loading_episodes") }</p> },
                    Some(episodes) if episodes.is_empty() => html! {
                        <p class="pp-empty-text pp-pad">{ i18n.t("people_subs.no_recent_episodes_found") }</p>
                    },
                    Some(episodes) => html! {
                        <EpisodeListView key="followed-feed" episodes={episodes.clone()} />
                    },
                } }
            </section>
            </>
        },
    };

    html! {
        <>
        <div class="main-container">
            <Search_nav />
            <UseScrollToTop />
            <div class="pp-page">
                <h1 class="pp-page-title">{ i18n.t("people_subs.subscribed_people") }</h1>
                { content }
            </div>
            <AudioPlayerBar />
        </div>
        <App_drawer />
        </>
    }
}
