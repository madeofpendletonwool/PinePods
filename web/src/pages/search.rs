use crate::components::app_drawer::App_drawer;
use crate::components::audio::AudioPlayer;
use crate::components::click_events::create_on_title_click;
use crate::components::context::{AppState, FilterState, NotificationState, UIState};
use crate::components::episode_list_view::EpisodeListView;
use crate::components::gen_components::{FallbackImage, Search_nav, UseScrollToTop};
use crate::components::gen_funcs::format_time;
use crate::requests::episode::Episode;
use crate::requests::pod_req::{
    call_bulk_download_episodes, call_bulk_mark_episodes_completed, call_bulk_queue_episodes,
    call_bulk_save_episodes, call_get_home_overview, call_get_podcasts_extra,
    BulkEpisodeActionRequest, HomePodcast,
};
use yewdux::dispatch::Dispatch;
use crate::requests::search_pods::{call_search_database_paged, SearchRequest};
use gloo_events::EventListener;
use gloo_timers::future::TimeoutFuture;
use i18nrs::yew::use_translation;
use std::collections::HashSet;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::window;
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yew::{function_component, html, use_node_ref, AttrValue, Callback, Html, Properties};
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;
use wasm_bindgen::closure::Closure;

const PAGE_SIZE: i64 = 50;

#[derive(Clone, PartialEq, Debug)]
enum FilterChip {
    All,
    Unplayed,
    InProgress,
    Saved,
    Downloaded,
}

fn filter_episode(ep: &Episode, chip: &FilterChip) -> bool {
    match chip {
        FilterChip::All        => true,
        FilterChip::Unplayed   => !ep.completed && ep.listenduration == 0,
        FilterChip::InProgress => ep.listenduration > 0 && !ep.completed,
        FilterChip::Saved      => ep.saved,
        FilterChip::Downloaded => ep.downloaded,
    }
}

fn highlight_html(text: &str, query: &str) -> String {
    if query.is_empty() || text.is_empty() {
        return html_escape(text);
    }
    let esc = html_escape(text);
    let lo = esc.to_lowercase();
    let q = query.to_lowercase();
    let mut out = String::with_capacity(esc.len() + 32);
    let mut last = 0;
    while let Some(p) = lo[last..].find(&q) {
        let abs = last + p;
        out.push_str(&esc[last..abs]);
        out.push_str("<mark>");
        out.push_str(&esc[abs..abs + q.len()]);
        out.push_str("</mark>");
        last = abs + q.len();
    }
    out.push_str(&esc[last..]);
    out
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
     .replace('\'', "&#39;")
}

fn load_recents_from_storage() -> Vec<String> {
    window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("pp_search_recents").ok().flatten())
        .and_then(|raw| serde_json::from_str::<Vec<String>>(&raw).ok())
        .unwrap_or_default()
}

fn save_recents_to_storage(recents: &[String]) {
    if let Some(s) = window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = s.set_item(
            "pp_search_recents",
            &serde_json::to_string(recents).unwrap_or_default(),
        );
    }
}

#[derive(Properties, Clone, PartialEq)]
pub struct SearchProps {
    pub on_search: Callback<String>,
}

#[function_component(Search)]
pub fn search(_props: &SearchProps) -> Html {
    let (i18n, _) = use_translation();
    let (post_state, _dispatch) = use_store::<AppState>();
    let (audio_state, _audio_dispatch) = use_store::<UIState>();
    let (filter_state, _) = use_store::<FilterState>();
    let dispatch = _dispatch.clone();

    let api_key = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.api_key.clone());
    let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.server_name.clone());

    let input_ref = use_node_ref();

    // Pagination / search results (keep existing logic)
    let episodes = use_state(|| Rc::new(Vec::<Episode>::new()));
    let total = use_state(|| 0i64);
    let offset = use_state(|| 0i64);
    let loading_more = use_state(|| false);
    let current_term = use_state(|| String::new());

    // New state
    let query = use_state(|| String::new());
    let active_filter = use_state(|| FilterChip::All);
    let most_played = use_state(|| Vec::<HomePodcast>::new());
    let categories = use_state(|| Vec::<String>::new());
    let recent_searches = use_state(|| Vec::<String>::new());
    let input_focused = use_state(|| false);
    let is_mobile = use_state(|| false);
    let selected_categories = use_state(|| Vec::<String>::new());
    let is_selecting = use_state(|| false);
    let selected_episodes_set = use_state(|| HashSet::<i32>::new());

    // BrowserHistory for podcast tile clicks
    let history = BrowserHistory::new();

    // ── Resize listener for is_mobile ──────────────────────────────────────
    {
        let is_mobile = is_mobile.clone();
        use_effect_with((), move |_| {
            let update = {
                let is_mobile = is_mobile.clone();
                Callback::from(move |_| {
                    if let Some(w) = window() {
                        if let Ok(width) = w.inner_width() {
                            if let Some(w) = width.as_f64() {
                                is_mobile.set(w <= 500.0);
                            }
                        }
                    }
                })
            };
            update.emit(());
            let listener = EventListener::new(&window().unwrap(), "resize", move |_| {
                update.emit(());
            });
            move || drop(listener)
        });
    }

    // ── Mount: load recents, categories, most-played ───────────────────────
    {
        let recent_searches = recent_searches.clone();
        let categories = categories.clone();
        let most_played = most_played.clone();
        let filter_cat_list = filter_state.category_filter_list.clone();
        let api_key_m = api_key.clone();
        let user_id_m = user_id.clone();
        let server_name_m = server_name.clone();

        use_effect_with((), move |_| {
            recent_searches.set(load_recents_from_storage());

            spawn_local(async move {
                // Categories: prefer FilterState if already populated
                if let Some(list) = filter_cat_list {
                    categories.set(list);
                } else if let (Some(server), Some(ak), Some(uid)) = (
                    server_name_m.clone(),
                    api_key_m.clone().flatten(),
                    user_id_m,
                ) {
                    if let Ok(pods) =
                        call_get_podcasts_extra(&server, &Some(ak), &uid).await
                    {
                        let mut set = HashSet::new();
                        for pod in &pods {
                            if let Some(cats) = &pod.categories {
                                for v in cats.values() {
                                    let t = v.trim().to_string();
                                    if !t.is_empty() { set.insert(t); }
                                }
                            }
                        }
                        let mut list: Vec<String> = set.into_iter().collect();
                        list.sort();
                        categories.set(list);
                    }
                }

                // Most-played shelf
                if let (Some(server), Some(ak), Some(uid)) = (
                    server_name_m,
                    api_key_m.flatten(),
                    user_id_m,
                ) {
                    if let Ok(overview) =
                        call_get_home_overview(&server, &ak, uid).await
                    {
                        most_played.set(overview.top_podcasts);
                    }
                }
            });

            || ()
        });
    }

    // ── Keyboard shortcuts: "/" focuses, Escape clears ─────────────────────
    {
        let input_ref_ks = input_ref.clone();
        let query_ks = query.clone();
        let current_term_ks = current_term.clone();
        let episodes_ks = episodes.clone();
        let total_ks = total.clone();
        let offset_ks = offset.clone();
        let active_filter_ks = active_filter.clone();
        let selected_categories_ks = selected_categories.clone();
        let is_selecting_ks = is_selecting.clone();
        let selected_episodes_set_ks = selected_episodes_set.clone();

        use_effect_with((), move |_| {
            let cb = Closure::<dyn Fn(web_sys::KeyboardEvent)>::wrap(Box::new(
                move |e: web_sys::KeyboardEvent| {
                    let doc = web_sys::window()
                        .and_then(|w| w.document());
                    if e.key() == "/" {
                        let active = doc
                            .and_then(|d| d.active_element())
                            .and_then(|el| el.dyn_into::<HtmlInputElement>().ok());
                        if active.is_none() {
                            e.prevent_default();
                            if let Some(el) = input_ref_ks.cast::<HtmlInputElement>() {
                                let _ = el.focus();
                            }
                        }
                    } else if e.key() == "Escape" && (!(*query_ks).is_empty() || !(*selected_categories_ks).is_empty()) {
                        query_ks.set(String::new());
                        current_term_ks.set(String::new());
                        episodes_ks.set(Rc::new(Vec::new()));
                        total_ks.set(0);
                        offset_ks.set(0);
                        active_filter_ks.set(FilterChip::All);
                        selected_categories_ks.set(vec![]);
                        is_selecting_ks.set(false);
                        selected_episodes_set_ks.set(HashSet::new());
                        if let Some(el) = input_ref_ks.cast::<HtmlInputElement>() {
                            let _ = el.focus();
                        }
                    }
                },
            ));
            let w = web_sys::window().unwrap();
            w.add_event_listener_with_callback(
                "keydown",
                cb.as_ref().unchecked_ref(),
            )
            .ok();
            cb.forget();
            || ()
        });
    }

    // ── Chip counts (computed from all loaded episodes, regardless of active filter) ─────
    let chip_counts = [
        (*episodes).len(),
        (*episodes).iter().filter(|e| !e.completed && e.listenduration == 0).count(),
        (*episodes).iter().filter(|e| e.listenduration > 0 && !e.completed).count(),
        (*episodes).iter().filter(|e| e.saved).count(),
        (*episodes).iter().filter(|e| e.downloaded).count(),
    ];

    // Visible episodes after the FilterChip filter. When the chip is "All", skip the clone
    // and just hand the parent's Rc straight through; otherwise allocate a filtered Vec.
    let display_episodes_rc: Rc<Vec<Episode>> = if *active_filter == FilterChip::All {
        (*episodes).clone()
    } else {
        let filter = (*active_filter).clone();
        Rc::new(
            (*episodes)
                .iter()
                .filter(|ep| filter_episode(ep, &filter))
                .cloned()
                .collect(),
        )
    };
    let visible_count = display_episodes_rc.len();
    let visible_empty = visible_count == 0;
    let visible_ep_ids: Vec<i32> = display_episodes_rc.iter().map(|ep| ep.episodeid).collect();

    // ── Select mode callbacks ──────────────────────────────────────────────
    let toggle_select = {
        let is_selecting = is_selecting.clone();
        let selected_episodes_set = selected_episodes_set.clone();
        Callback::from(move |_: MouseEvent| {
            if *is_selecting {
                selected_episodes_set.set(HashSet::new());
            }
            is_selecting.set(!*is_selecting);
        })
    };

    // on_episode_checkbox / on_select_above / on_select_below pass into EpisodeListItem via
    // EpisodeListView, so their identity has to be stable across renders or every already-
    // mounted card will fail PartialEq and re-run its function body.
    let on_episode_checkbox = {
        let selected_episodes_set = selected_episodes_set.clone();
        use_callback((), move |ep_id: i32, _| {
            let mut current = (*selected_episodes_set).clone();
            if current.contains(&ep_id) {
                current.remove(&ep_id);
            } else {
                current.insert(ep_id);
            }
            selected_episodes_set.set(current);
        })
    };

    let on_select_above = {
        let selected_episodes_set = selected_episodes_set.clone();
        let episodes_handle = episodes.clone();
        let active_filter_handle = active_filter.clone();
        use_callback((), move |cutoff_id: i32, _| {
            let filter = (*active_filter_handle).clone();
            let ids: Vec<i32> = (*episodes_handle)
                .iter()
                .filter(|ep| filter_episode(ep, &filter))
                .map(|ep| ep.episodeid)
                .collect();
            if let Some(pos) = ids.iter().position(|&id| id == cutoff_id) {
                let to_add: HashSet<i32> = ids[..=pos].iter().cloned().collect();
                let mut current = (*selected_episodes_set).clone();
                current.extend(to_add);
                selected_episodes_set.set(current);
            }
        })
    };

    let on_select_below = {
        let selected_episodes_set = selected_episodes_set.clone();
        let episodes_handle = episodes.clone();
        let active_filter_handle = active_filter.clone();
        use_callback((), move |cutoff_id: i32, _| {
            let filter = (*active_filter_handle).clone();
            let ids: Vec<i32> = (*episodes_handle)
                .iter()
                .filter(|ep| filter_episode(ep, &filter))
                .map(|ep| ep.episodeid)
                .collect();
            if let Some(pos) = ids.iter().position(|&id| id == cutoff_id) {
                let to_add: HashSet<i32> = ids[pos..].iter().cloned().collect();
                let mut current = (*selected_episodes_set).clone();
                current.extend(to_add);
                selected_episodes_set.set(current);
            }
        })
    };

    let on_select_all = {
        let selected_episodes_set = selected_episodes_set.clone();
        let ids = visible_ep_ids.clone();
        Callback::from(move |_: MouseEvent| {
            let all: HashSet<i32> = ids.iter().cloned().collect();
            let current = (*selected_episodes_set).clone();
            if current.len() == all.len() && all.iter().all(|id| current.contains(id)) {
                selected_episodes_set.set(HashSet::new());
            } else {
                selected_episodes_set.set(all);
            }
        })
    };

    let on_select_unplayed = {
        let selected_episodes_set = selected_episodes_set.clone();
        let ep_data: Vec<(i32, bool)> = display_episodes_rc
            .iter()
            .map(|ep| (ep.episodeid, !ep.completed && ep.listenduration == 0))
            .collect();
        Callback::from(move |_: MouseEvent| {
            let ids: HashSet<i32> = ep_data
                .iter()
                .filter(|(_, unplayed)| *unplayed)
                .map(|(id, _)| *id)
                .collect();
            selected_episodes_set.set(ids);
        })
    };

    let on_select_in_progress = {
        let selected_episodes_set = selected_episodes_set.clone();
        let ep_data: Vec<(i32, bool)> = display_episodes_rc
            .iter()
            .map(|ep| (ep.episodeid, ep.listenduration > 0 && !ep.completed))
            .collect();
        Callback::from(move |_: MouseEvent| {
            let ids: HashSet<i32> = ep_data
                .iter()
                .filter(|(_, in_prog)| *in_prog)
                .map(|(id, _)| *id)
                .collect();
            selected_episodes_set.set(ids);
        })
    };

    // ── fire_search: low-level executor, accepts (term, cats) explicitly ────
    let fire_search = {
        let current_term = current_term.clone();
        let episodes = episodes.clone();
        let total = total.clone();
        let offset = offset.clone();
        let loading_more = loading_more.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();

        Callback::from(move |(term, cats): (String, Vec<String>)| {
            current_term.set(term.clone());
            episodes.set(Rc::new(Vec::new()));
            total.set(0);
            offset.set(0);
            loading_more.set(true);

            let current_term = current_term.clone();
            let episodes = episodes.clone();
            let total = total.clone();
            let offset = offset.clone();
            let loading_more = loading_more.clone();
            let api_key = api_key.clone();
            let user_id = user_id.clone();
            let server_name = server_name.clone();
            let term_clone = term.clone();

            spawn_local(async move {
                if let (Some(server), Some(ak), Some(uid)) =
                    (server_name, api_key.flatten(), user_id)
                {
                    let req = SearchRequest {
                        search_term: term_clone.clone(),
                        user_id: uid,
                        categories: if cats.is_empty() { None } else { Some(cats) },
                    };
                    match call_search_database_paged(&server, &Some(ak), &req, PAGE_SIZE, 0).await {
                        Ok(page) => {
                            if *current_term != term_clone { return; }
                            total.set(page.total);
                            offset.set(page.data.len() as i64);
                            episodes.set(Rc::new(page.data));
                        }
                        Err(e) => {
                            web_sys::console::log_1(&format!("Search error: {:?}", e).into());
                        }
                    }
                }
                loading_more.set(false);
            });
        })
    };

    // ── commit_search: saves recents then calls fire_search ────────────────
    let commit_search = {
        let query = query.clone();
        let recent_searches = recent_searches.clone();
        let fire_search = fire_search.clone();

        Callback::from(move |(term, cats): (String, Vec<String>)| {
            let t = term.trim().to_string();
            if t.is_empty() && cats.is_empty() { return; }

            query.set(t.clone());

            if !t.is_empty() {
                let mut recents = (*recent_searches).clone();
                recents.retain(|r| r != &t);
                recents.insert(0, t.clone());
                recents.truncate(6);
                recent_searches.set(recents.clone());
                save_recents_to_storage(&recents);
            }

            fire_search.emit((t, cats));
        })
    };

    // ── on_input: update query state + debounced search trigger ───────────
    let on_input = {
        let query = query.clone();
        let query_check = query.clone();
        let current_term = current_term.clone();
        let episodes = episodes.clone();
        let total = total.clone();
        let offset = offset.clone();
        let commit_search = commit_search.clone();
        let selected_categories_check = selected_categories.clone();
        let fire_search_input = fire_search.clone();

        Callback::from(move |e: InputEvent| {
            let value = e.target_unchecked_into::<HtmlInputElement>().value();
            query.set(value.clone());

            if value.trim().is_empty() {
                query.set(String::new());
                if (*selected_categories_check).is_empty() {
                    episodes.set(Rc::new(Vec::new()));
                    total.set(0);
                    offset.set(0);
                    current_term.set(String::new());
                } else {
                    fire_search_input.emit(("".to_string(), (*selected_categories_check).clone()));
                }
                return;
            }

            let term = value.trim().to_string();
            let query_check = query_check.clone();
            let current_term = current_term.clone();
            let commit_search = commit_search.clone();
            let cats_snap = (*selected_categories_check).clone();

            spawn_local(async move {
                TimeoutFuture::new(400).await;
                // Stale check: user kept typing
                if (*query_check).trim() != term { return; }
                // Already searched this term
                if *current_term == term { return; }
                commit_search.emit((term, cats_snap));
            });
        })
    };

    // ── on_keydown: Enter triggers immediate search ─────────────────────
    let on_keydown = {
        let query = query.clone();
        let commit_search = commit_search.clone();
        let selected_categories_kd = selected_categories.clone();
        Callback::from(move |e: KeyboardEvent| {
            if e.key() == "Enter" {
                e.prevent_default();
                let term = (*query).trim().to_string();
                let cats = (*selected_categories_kd).clone();
                if !term.is_empty() || !cats.is_empty() {
                    commit_search.emit((term, cats));
                }
            }
        })
    };

    // ── on_clear ──────────────────────────────────────────────────────────
    let on_clear = {
        let query = query.clone();
        let current_term = current_term.clone();
        let episodes = episodes.clone();
        let total = total.clone();
        let offset = offset.clone();
        let active_filter = active_filter.clone();
        let selected_categories = selected_categories.clone();
        let input_ref = input_ref.clone();
        let is_selecting = is_selecting.clone();
        let selected_episodes_set = selected_episodes_set.clone();
        Callback::from(move |_: MouseEvent| {
            query.set(String::new());
            current_term.set(String::new());
            episodes.set(Rc::new(Vec::new()));
            total.set(0);
            offset.set(0);
            active_filter.set(FilterChip::All);
            selected_categories.set(vec![]);
            is_selecting.set(false);
            selected_episodes_set.set(HashSet::new());
            if let Some(el) = input_ref.cast::<HtmlInputElement>() {
                let _ = el.focus();
            }
        })
    };

    // Load-more handler. EpisodeListView owns the sentinel/observer/display-count/ramp; this
    // callback fires only when the view runs out of buffered episodes and the parent reports
    // `backend_can_load_more`. Snapshot current_term at entry so a stale fetch result that
    // returns after the user has changed the query gets dropped before mutating state.
    let on_load_more = {
        let episodes = episodes.clone();
        let total = total.clone();
        let offset = offset.clone();
        let loading_more = loading_more.clone();
        let current_term = current_term.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let selected_categories_lm = selected_categories.clone();
        use_callback((), move |_: (), _| {
            if *loading_more {
                return;
            }
            let current_offset = *offset;
            let current_total = *total;
            if current_offset >= current_total {
                return;
            }
            let search_term = (*current_term).clone();
            let cats_snap = (*selected_categories_lm).clone();
            if search_term.is_empty() && cats_snap.is_empty() {
                return;
            }
            let Some(server) = server_name.clone() else { return; };
            let Some(ak) = api_key.clone().flatten() else { return; };
            let Some(uid) = user_id else { return; };
            loading_more.set(true);
            let episodes = episodes.clone();
            let total = total.clone();
            let offset = offset.clone();
            let loading_more = loading_more.clone();
            let current_term_for_check = current_term.clone();
            let term_snap = search_term.clone();
            spawn_local(async move {
                let req = SearchRequest {
                    search_term: term_snap.clone(),
                    user_id: uid,
                    categories: if cats_snap.is_empty() { None } else { Some(cats_snap) },
                };
                if let Ok(page) = call_search_database_paged(
                    &server,
                    &Some(ak),
                    &req,
                    PAGE_SIZE,
                    current_offset,
                )
                .await
                {
                    if *current_term_for_check != term_snap {
                        loading_more.set(false);
                        return;
                    }
                    TimeoutFuture::new(0).await;
                    let added = page.data.len() as i64;
                    let mut all = (**episodes).clone();
                    all.extend(page.data);
                    offset.set(current_offset + added);
                    total.set(page.total);
                    episodes.set(Rc::new(all));
                    TimeoutFuture::new(0).await;
                }
                loading_more.set(false);
            });
        })
    };

    // ── Category toggle callback ───────────────────────────────────────────
    let on_cat_toggle = {
        let selected_categories = selected_categories.clone();
        let query = query.clone();
        let fire_search = fire_search.clone();
        Callback::from(move |cat: String| {
            let mut cats = (*selected_categories).clone();
            if cats.contains(&cat) {
                cats.retain(|c| c != &cat);
            } else {
                cats.push(cat);
            }
            selected_categories.set(cats.clone());
            let term = (*query).trim().to_string();
            fire_search.emit((term, cats));
        })
    };

    // ── Derived state ───────────────────────────────────────────────────────
    let is_collapsed = !(*query).is_empty() || !(*selected_categories).is_empty();
    let placeholder_text = if *is_mobile {
        i18n.t("search.mobile_placeholder")
    } else {
        i18n.t("search.desktop_placeholder")
    };

    // ── Chip definitions ──────────────────────────────────────────────────
    let chips: &[(&str, &str, Option<&str>, FilterChip)] = &[
        ("all",  "search.filter_all",         None,                              FilterChip::All),
        ("new",  "search.filter_unplayed",     Some("ph ph-circle"),              FilterChip::Unplayed),
        ("prog", "search.filter_in_progress",  Some("ph ph-hourglass-medium"),    FilterChip::InProgress),
        ("save", "search.filter_saved",        Some("ph ph-star"),                FilterChip::Saved),
        ("dl",   "search.filter_downloaded",   Some("ph ph-download-simple"),     FilterChip::Downloaded),
    ];

    let render_chips = |dispatch_chip: &Callback<FilterChip>| -> Html {
        html! {
            <>
            { chips.iter().enumerate().map(|(idx, (_, label_key, icon, chip))| {
                let label = i18n.t(label_key);
                let count = chip_counts[idx];
                let is_active = *active_filter == *chip;
                let chip_val = chip.clone();
                let dispatch_chip = dispatch_chip.clone();
                let onclick = Callback::from(move |_: MouseEvent| {
                    dispatch_chip.emit(chip_val.clone());
                });
                html! {
                    <button
                        key={*label_key}
                        class={classes!("sp-chip", is_active.then_some("is-active"))}
                        onclick={onclick}
                    >
                        if let Some(ico) = icon {
                            <i class={*ico}></i>
                        }
                        <span>{ label }</span>
                        <span class="sp-chip-count">{ count }</span>
                    </button>
                }
            }).collect::<Html>() }
            </>
        }
    };

    let set_filter = {
        let active_filter = active_filter.clone();
        Callback::from(move |chip: FilterChip| active_filter.set(chip))
    };

    // ── Pre-computed values for select mode UI ───────────────────────────────
    let sel_all_ids: HashSet<i32> = visible_ep_ids.iter().cloned().collect();
    let sel_cur = (*selected_episodes_set).clone();
    let sel_all_selected = !sel_all_ids.is_empty()
        && sel_cur.len() == sel_all_ids.len()
        && sel_all_ids.iter().all(|id| sel_cur.contains(id));
    let sel_count = sel_cur.len();
    let sel_ids: Vec<i32> = sel_cur.iter().cloned().collect();
    let sel_user_id = user_id.unwrap_or(0);

    // ── HTML ─────────────────────────────────────────────────────────────────
    html! {
        <>
        <div class="search-page-container">
            <Search_nav />
            <UseScrollToTop />

            // ── Collapsing sticky header ──────────────────────────────────
            <div class={classes!("sp-head", is_collapsed.then_some("is-collapsed"))}>
                <div class="sp-head-titles">
                    <h1 class="sp-title">{ i18n.t("search.title") }</h1>
                    <p class="sp-subtitle">{ i18n.t("search.subtitle") }</p>
                </div>

                <div class="sp-input-row">
                    <div class={classes!("sp-input", (*input_focused).then_some("is-focused"))}>
                        <i class="ph ph-magnifying-glass sp-search-ico"></i>
                        <input
                            ref={input_ref.clone()}
                            type="text"
                            placeholder={placeholder_text}
                            value={(*query).clone()}
                            oninput={on_input}
                            onkeydown={on_keydown}
                            onfocus={Callback::from({
                                let input_focused = input_focused.clone();
                                move |_| input_focused.set(true)
                            })}
                            onblur={Callback::from({
                                let input_focused = input_focused.clone();
                                move |_| input_focused.set(false)
                            })}
                        />
                        if !(*query).is_empty() || !(*selected_categories).is_empty() {
                            <button class="sp-clear" onclick={on_clear.clone()} aria-label="Clear">
                                <i class="ph ph-x"></i>
                            </button>
                        } else if !*is_mobile {
                            <span class="sp-kbd">{ "/" }</span>
                        }
                    </div>
                </div>

                <div class="sp-chips">
                    { render_chips(&set_filter) }
                    if is_collapsed && !visible_empty {
                        <button
                            class={classes!("sp-chip", (*is_selecting).then_some("is-active"))}
                            onclick={toggle_select.clone()}
                        >
                            <i class={if *is_selecting { "ph ph-x-square" } else { "ph ph-check-square" }}></i>
                            <span>{ if *is_selecting { i18n.t("search.exit_select") } else { i18n.t("search.select") } }</span>
                        </button>
                    }
                </div>

                if !(*selected_categories).is_empty() {
                    <div class="sp-chips sp-active-cats">
                        { (*selected_categories).iter().map(|cat| {
                            let cat_rm = cat.clone();
                            let selected_categories_rm = selected_categories.clone();
                            let query_rm = query.clone();
                            let fire_search_rm = fire_search.clone();
                            html! {
                                <button key={cat.clone()} class="sp-chip is-active"
                                    onclick={Callback::from(move |_: MouseEvent| {
                                        let mut cats = (*selected_categories_rm).clone();
                                        cats.retain(|c| c != &cat_rm);
                                        selected_categories_rm.set(cats.clone());
                                        let term = (*query_rm).trim().to_string();
                                        fire_search_rm.emit((term, cats));
                                    })}>
                                    <i class="ph ph-tag"></i>
                                    <span>{ cat }</span>
                                    <i class="ph ph-x" style="font-size:11px; opacity:.7;"></i>
                                </button>
                            }
                        }).collect::<Html>() }
                    </div>
                }
            </div>

            // ── Body: discovery surface OR results ────────────────────────
            <div class="sp-body">
                if !is_collapsed {
                    // ── Discovery surface (empty / idle state) ────────────

                    // Recent searches
                    if !(*recent_searches).is_empty() {
                        <div class="sp-sec-head">
                            <h3>{ i18n.t("search.recent_searches") }</h3>
                            <a onclick={Callback::from({
                                let recent_searches = recent_searches.clone();
                                move |_: MouseEvent| {
                                    recent_searches.set(vec![]);
                                    save_recents_to_storage(&[]);
                                }
                            })}>
                                { i18n.t("search.clear_all") }
                            </a>
                        </div>
                        <div class="sp-recent">
                            { (*recent_searches).iter().map(|r| {
                                let term = r.clone();
                                let query_cs = query.clone();
                                let commit = commit_search.clone();
                                let selected_categories_pill = selected_categories.clone();
                                let recents_del = recent_searches.clone();
                                let term_del = r.clone();
                                html! {
                                    <div class="sp-recent-pill" key={r.clone()}
                                         onclick={Callback::from(move |_: MouseEvent| {
                                             query_cs.set(term.clone());
                                             let cats = (*selected_categories_pill).clone();
                                             commit.emit((term.clone(), cats));
                                         })}>
                                        <i class="ph ph-clock-counter-clockwise"></i>
                                        <span>{ r }</span>
                                        <span class="sp-recent-x"
                                              onclick={Callback::from(move |e: MouseEvent| {
                                                  e.stop_propagation();
                                                  let mut rs = (*recents_del).clone();
                                                  rs.retain(|x| x != &term_del);
                                                  recents_del.set(rs.clone());
                                                  save_recents_to_storage(&rs);
                                              })}>
                                            <i class="ph ph-x"></i>
                                        </span>
                                    </div>
                                }
                            }).collect::<Html>() }
                        </div>
                    }

                    // Most played shelf
                    if !(*most_played).is_empty() {
                        <div class="sp-sec-head">
                            <h3>{ i18n.t("search.most_played") }</h3>
                        </div>
                        <div class="sp-shelf">
                            { (*most_played).iter().take(8).map(|pod| {
                                let api_key_tile = api_key.clone();
                                let server_tile = server_name.clone().unwrap_or_default();
                                let history_tile = history.clone();
                                let dispatch_tile = dispatch.clone();
                                let on_click = create_on_title_click(
                                    server_tile,
                                    api_key_tile,
                                    &history_tile,
                                    pod.podcastid,
                                    pod.podcastindexid,
                                    pod.podcastname.clone(),
                                    pod.feedurl.clone().unwrap_or_default(),
                                    pod.description.clone().unwrap_or_default(),
                                    pod.author.clone().unwrap_or_default(),
                                    pod.artworkurl.clone().unwrap_or_default(),
                                    pod.explicit.unwrap_or(false),
                                    pod.episodecount.unwrap_or(0),
                                    pod.categories.as_ref().map(|c| c.values().cloned().collect::<Vec<_>>().join(", ")),
                                    pod.websiteurl.clone().unwrap_or_default(),
                                    user_id.unwrap_or(0),
                                    pod.is_youtube,
                                );
                                let ep_count_str = format!(
                                    "{} {}",
                                    pod.episodecount.unwrap_or(0),
                                    if pod.episodecount.unwrap_or(0) == 1 { "episode" } else { "episodes" }
                                );
                                html! {
                                    <div class="sp-tile" key={pod.podcastid} onclick={on_click}>
                                        <div class="sp-tile-cover">
                                            <FallbackImage
                                                src={pod.artworkurl.clone().unwrap_or_default()}
                                                alt={format!("Cover for {}", pod.podcastname)}
                                                class="sp-tile-cover-img"
                                            />
                                        </div>
                                        <div class="sp-tile-title">{ &pod.podcastname }</div>
                                        <div class="sp-tile-sub">{ ep_count_str }</div>
                                    </div>
                                }
                            }).collect::<Html>() }
                        </div>
                    }

                    // Browse by category
                    if !(*categories).is_empty() {
                        <div class="sp-sec-head">
                            <h3>{ i18n.t("search.browse_by_category") }</h3>
                        </div>
                        <div class="sp-cat-grid">
                            { (*categories).iter().map(|cat| {
                                let cat_name = cat.clone();
                                let on_cat_toggle = on_cat_toggle.clone();
                                let is_active = (*selected_categories).contains(cat);
                                html! {
                                    <div class={classes!("sp-cat", is_active.then_some("is-active"))}
                                         key={cat.clone()}
                                         onclick={Callback::from(move |_: MouseEvent| {
                                             on_cat_toggle.emit(cat_name.clone());
                                         })}>
                                        <div class="sp-cat-name">{ cat }</div>
                                    </div>
                                }
                            }).collect::<Html>() }
                        </div>
                    }

                    // Empty discovery (no recents, no most-played yet)
                    if (*recent_searches).is_empty() && (*most_played).is_empty() && (*categories).is_empty() {
                        <div class="sp-noresults" style="padding-top: 80px;">
                            <i class="ph ph-magnifying-glass"></i>
                            <h4>{ i18n.t("search.title") }</h4>
                            <p>{ i18n.t("search.subtitle") }</p>
                        </div>
                    }
                } else {
                    // ── Results view ──────────────────────────────────────

                    // Results count header
                    <div class="sp-sec-head" style="margin-top: 8px;">
                        <h3 style="text-transform: none; letter-spacing: 0;">
                            <span class="sp-results-count">
                                {
                                    if *loading_more && visible_empty {
                                        "Searching…".to_string()
                                    } else if visible_empty {
                                        i18n.t("search.no_matches_header")
                                    } else {
                                        format!(
                                            "{} {} \"{}\"",
                                            visible_count,
                                            if visible_count == 1 { "match for" } else { "matches for" },
                                            *current_term
                                        )
                                    }
                                }
                            </span>
                        </h3>
                        if !*is_mobile && !visible_empty {
                            <a>
                                <i class="ph ph-funnel-simple"></i>
                                { i18n.t("search.sort_relevance") }
                            </a>
                        }
                    </div>

                    // Smart selection row
                    if *is_selecting && !visible_empty {
                        <div class="sp-select-controls">
                            <button class="bulk-select-button" onclick={on_select_all.clone()}>
                                { if sel_all_selected { i18n.t("search.deselect_all") } else { i18n.t("search.select_all") } }
                            </button>
                            <button class="bulk-filter-button" onclick={on_select_unplayed.clone()}>
                                { i18n.t("search.select_unplayed") }
                            </button>
                            <button class="bulk-filter-button" onclick={on_select_in_progress.clone()}>
                                { i18n.t("search.select_in_progress") }
                            </button>
                        </div>
                    }

                    // Bulk actions toolbar
                    if *is_selecting && !selected_episodes_set.is_empty() {
                        <div class="bulk-actions-bar">
                            <div class="bulk-actions-bar__count">
                                <i class="ph ph-check-circle"></i>
                                { format!("{} episode{} selected", sel_count, if sel_count == 1 { "" } else { "s" }) }
                            </div>
                            <div class="bulk-actions-bar__actions">
                                <button
                                    onclick={
                                        let sel_ids = sel_ids.clone();
                                        let api_key = api_key.clone();
                                        let server_name = server_name.clone();
                                        let selected_episodes_set = selected_episodes_set.clone();
                                        Callback::from(move |_| {
                                            let sel_ids = sel_ids.clone();
                                            let api_key = api_key.clone();
                                            let server_name = server_name.clone();
                                            let selected_episodes_set = selected_episodes_set.clone();
                                            spawn_local(async move {
                                                let request = BulkEpisodeActionRequest {
                                                    episode_ids: sel_ids,
                                                    user_id: sel_user_id,
                                                    is_youtube: None,
                                                };
                                                match call_bulk_mark_episodes_completed(
                                                    &server_name.unwrap_or_default(),
                                                    &api_key.flatten(),
                                                    &request,
                                                ).await {
                                                    Ok(msg) => {
                                                        Dispatch::<NotificationState>::global().reduce_mut(|s| s.info_message = Some(msg));
                                                        selected_episodes_set.set(HashSet::new());
                                                    }
                                                    Err(e) => {
                                                        Dispatch::<NotificationState>::global().reduce_mut(|s| s.error_message = Some(format!("Error: {}", e)));
                                                    }
                                                }
                                            });
                                        })
                                    }
                                    class="btn btn-secondary"
                                >
                                    <i class="ph ph-check-circle"></i>
                                    { i18n.t("search.mark_complete") }
                                </button>
                                <button
                                    onclick={
                                        let sel_ids = sel_ids.clone();
                                        let api_key = api_key.clone();
                                        let server_name = server_name.clone();
                                        let selected_episodes_set = selected_episodes_set.clone();
                                        Callback::from(move |_| {
                                            let sel_ids = sel_ids.clone();
                                            let api_key = api_key.clone();
                                            let server_name = server_name.clone();
                                            let selected_episodes_set = selected_episodes_set.clone();
                                            spawn_local(async move {
                                                let request = BulkEpisodeActionRequest {
                                                    episode_ids: sel_ids,
                                                    user_id: sel_user_id,
                                                    is_youtube: None,
                                                };
                                                match call_bulk_save_episodes(
                                                    &server_name.unwrap_or_default(),
                                                    &api_key.flatten(),
                                                    &request,
                                                ).await {
                                                    Ok(msg) => {
                                                        Dispatch::<NotificationState>::global().reduce_mut(|s| s.info_message = Some(msg));
                                                        selected_episodes_set.set(HashSet::new());
                                                    }
                                                    Err(e) => {
                                                        Dispatch::<NotificationState>::global().reduce_mut(|s| s.error_message = Some(format!("Error: {}", e)));
                                                    }
                                                }
                                            });
                                        })
                                    }
                                    class="btn btn-secondary"
                                >
                                    <i class="ph ph-star"></i>
                                    { i18n.t("search.save") }
                                </button>
                                <button
                                    onclick={
                                        let sel_ids = sel_ids.clone();
                                        let api_key = api_key.clone();
                                        let server_name = server_name.clone();
                                        let selected_episodes_set = selected_episodes_set.clone();
                                        Callback::from(move |_| {
                                            let sel_ids = sel_ids.clone();
                                            let api_key = api_key.clone();
                                            let server_name = server_name.clone();
                                            let selected_episodes_set = selected_episodes_set.clone();
                                            spawn_local(async move {
                                                let request = BulkEpisodeActionRequest {
                                                    episode_ids: sel_ids,
                                                    user_id: sel_user_id,
                                                    is_youtube: None,
                                                };
                                                match call_bulk_queue_episodes(
                                                    &server_name.unwrap_or_default(),
                                                    &api_key.flatten(),
                                                    &request,
                                                ).await {
                                                    Ok(msg) => {
                                                        Dispatch::<NotificationState>::global().reduce_mut(|s| s.info_message = Some(msg));
                                                        selected_episodes_set.set(HashSet::new());
                                                    }
                                                    Err(e) => {
                                                        Dispatch::<NotificationState>::global().reduce_mut(|s| s.error_message = Some(format!("Error: {}", e)));
                                                    }
                                                }
                                            });
                                        })
                                    }
                                    class="btn btn-secondary"
                                >
                                    <i class="ph ph-list-plus"></i>
                                    { i18n.t("search.queue") }
                                </button>
                                <button
                                    onclick={
                                        let sel_ids = sel_ids.clone();
                                        let api_key = api_key.clone();
                                        let server_name = server_name.clone();
                                        let selected_episodes_set = selected_episodes_set.clone();
                                        Callback::from(move |_| {
                                            let sel_ids = sel_ids.clone();
                                            let api_key = api_key.clone();
                                            let server_name = server_name.clone();
                                            let selected_episodes_set = selected_episodes_set.clone();
                                            spawn_local(async move {
                                                let request = BulkEpisodeActionRequest {
                                                    episode_ids: sel_ids,
                                                    user_id: sel_user_id,
                                                    is_youtube: None,
                                                };
                                                match call_bulk_download_episodes(
                                                    &server_name.unwrap_or_default(),
                                                    &api_key.flatten(),
                                                    &request,
                                                ).await {
                                                    Ok(msg) => {
                                                        Dispatch::<NotificationState>::global().reduce_mut(|s| s.info_message = Some(msg));
                                                        selected_episodes_set.set(HashSet::new());
                                                    }
                                                    Err(e) => {
                                                        Dispatch::<NotificationState>::global().reduce_mut(|s| s.error_message = Some(format!("Error: {}", e)));
                                                    }
                                                }
                                            });
                                        })
                                    }
                                    class="btn btn-secondary"
                                >
                                    <i class="ph ph-download-simple"></i>
                                    { i18n.t("search.download") }
                                </button>
                            </div>
                        </div>
                    }

                    // No results empty state
                    if visible_empty && !*loading_more {
                        <div class="sp-noresults">
                            <i class="ph ph-binoculars"></i>
                            <h4>{ i18n.t("search.no_matches_header") }</h4>
                            <p>{ i18n.t("search.no_matches_body") }</p>
                        </div>
                    }

                    // Result rows. Key encodes current_term + categories + active filter so
                    // a fresh search or category toggle remounts the view and re-runs the
                    // initial ramp from display_count = 15.
                    if !visible_empty {
                        <div class="search-results-container">
                            <EpisodeListView
                                key={format!(
                                    "search|{}|{}|{:?}",
                                    *current_term,
                                    (*selected_categories).join(","),
                                    *active_filter
                                )}
                                episodes={display_episodes_rc.clone()}
                                backend_can_load_more={*offset < *total}
                                loading_more={*loading_more}
                                on_load_more={on_load_more.clone()}
                                is_delete_mode={*is_selecting}
                                on_checkbox_change={on_episode_checkbox.clone()}
                                on_select_above={on_select_above.clone()}
                                on_select_below={on_select_below.clone()}
                                selected_episodes={Rc::new((*selected_episodes_set).clone())}
                            />
                        </div>
                    }

                }
            </div>

            <App_drawer />

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
        </>
    }
}
