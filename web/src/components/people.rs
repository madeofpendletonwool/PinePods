//! People on podcasts and episodes: the shared people strip (podcast banner, mobile chips,
//! episode cards), the follow button, and the add-people sheet that sends contributions to
//! PodPeopleDB through the server.

use std::cell::Cell;
use std::rc::Rc;

use gloo_timers::future::TimeoutFuture;
use i18nrs::yew::use_translation;
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlInputElement, KeyboardEvent};
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;

use crate::components::context::{AppState, NotificationState};
use crate::requests::notify;
use crate::requests::people_req::{
    call_contribute_people, call_search_people, call_set_following, ContributePerson,
    ContributeRequest, CreditedPerson, PersonHit,
};

/// Server name and API key of the signed-in user.
#[hook]
pub fn use_auth() -> Option<(String, String)> {
    let auth = use_selector(|state: &AppState| {
        state
            .auth_details
            .as_ref()
            .and_then(|a| a.api_key.clone().map(|key| (a.server_name.clone(), key)))
    });
    (*auth).clone()
}

pub fn proxied_image(server_name: &str, url: &str) -> String {
    format!(
        "{}/api/proxy/image?url={}",
        server_name,
        urlencoding::encode(url)
    )
}

pub fn person_path(person_id: i32) -> String {
    format!("/person/{}", person_id)
}

fn go_to_person(person_id: i32) -> Callback<MouseEvent> {
    Callback::from(move |_: MouseEvent| BrowserHistory::new().push(person_path(person_id)))
}

fn avatar(server_name: &str, person: &CreditedPerson, class: &'static str, placeholder: &'static str) -> Html {
    html! {
        <PersonAvatar
            src={person.img.as_deref().filter(|s| !s.is_empty()).map(|img| AttrValue::from(proxied_image(server_name, img)))}
            alt={person.name.clone()}
            {class}
            {placeholder}
        />
    }
}

#[derive(Properties, PartialEq, Clone)]
pub struct PersonAvatarProps {
    pub src: Option<AttrValue>,
    #[prop_or_default]
    pub alt: AttrValue,
    pub class: &'static str,
    /// Extra class for the icon placeholder shown when there's no image or it fails to load.
    pub placeholder: &'static str,
}

/// A person's picture, or a user icon when there's none or it doesn't load.
#[function_component(PersonAvatar)]
pub fn person_avatar(props: &PersonAvatarProps) -> Html {
    let failed = use_state(|| false);
    {
        let failed = failed.clone();
        use_effect_with(props.src.clone(), move |_| {
            failed.set(false);
            || ()
        });
    }
    match &props.src {
        Some(src) if !*failed => {
            let failed = failed.clone();
            html! {
                <img src={src.clone()} alt={props.alt.clone()} class={props.class} loading="lazy"
                     onerror={Callback::from(move |_: Event| failed.set(true))} />
            }
        }
        _ => html! { <div class={classes!(props.class, props.placeholder)}><i class="ph ph-user"></i></div> },
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PeopleLayout {
    /// Overlapping avatars plus "Hosted by A & B" (desktop podcast banner).
    Banner,
    /// A labelled, horizontally scrolling row of chips (mobile pages, guests).
    Chips,
    /// A grid of cards with follow buttons (episode People tab).
    Cards,
}

#[derive(Properties, PartialEq, Clone)]
pub struct PeopleStripProps {
    pub people: Rc<Vec<CreditedPerson>>,
    pub layout: PeopleLayout,
    #[prop_or_default]
    pub label: AttrValue,
    /// Shows an "Add people" button; with no people, an invitation to add some.
    #[prop_or_default]
    pub on_add: Option<Callback<()>>,
    /// Text for the empty state (only shown with `on_add`).
    #[prop_or_default]
    pub empty_text: AttrValue,
    /// Wrapper class, so pages keep their own spacing.
    #[prop_or_default]
    pub class: Classes,
}

#[function_component(PeopleStrip)]
pub fn people_strip(props: &PeopleStripProps) -> Html {
    let (i18n, _) = use_translation();
    let server_name = use_auth().map(|(s, _)| s).unwrap_or_default();
    let people = &props.people;
    let unverified_title = i18n.t("people.unverified").to_string();

    let add_button = |compact: bool| -> Html {
        match &props.on_add {
            Some(on_add) => {
                let on_add = on_add.clone();
                let label = i18n.t("people.add_people").to_string();
                html! {
                    <button
                        type="button"
                        class={classes!("pp-add-btn", compact.then_some("is-compact"))}
                        title={label.clone()}
                        aria-label={label.clone()}
                        onclick={Callback::from(move |_: MouseEvent| on_add.emit(()))}
                    >
                        <i class="ph ph-user-plus"></i>
                        { if compact { html! {} } else { html! { <span>{ label }</span> } } }
                    </button>
                }
            }
            None => html! {},
        }
    };

    if people.is_empty() {
        if props.on_add.is_none() {
            return html! {};
        }
        return html! {
            <div class={classes!("pp-empty", props.class.clone())}>
                { if props.label.is_empty() { html! {} } else { html! { <span class="pp-label">{ &props.label }</span> } } }
                <div class="pp-empty-row">
                    <span class="pp-empty-text">{ &props.empty_text }</span>
                    { add_button(false) }
                </div>
            </div>
        };
    }

    let unverified_dot = |p: &CreditedPerson| -> Html {
        if p.verified {
            html! {}
        } else {
            html! { <span class="pp-unverified" title={unverified_title.clone()} aria-label={unverified_title.clone()}></span> }
        }
    };

    match props.layout {
        PeopleLayout::Banner => {
            let shown: Vec<CreditedPerson> = people.iter().take(6).cloned().collect();
            let more = people.len().saturating_sub(shown.len());
            html! {
                <div class={classes!("pd-banner-hosts", props.class.clone())}>
                    <div class="pd-host-stack">
                        { for shown.iter().map(|p| html! {
                            <div class="pd-host-avatar-wrap" title={p.name.clone()} onclick={go_to_person(p.id)}>
                                { avatar(&server_name, p, "pd-host-avatar", "pd-host-avatar-ph") }
                            </div>
                        }) }
                    </div>
                    <div class="pd-host-label">
                        <span class="pd-host-by">{ &props.label }</span>
                        <span class="pd-host-names">
                            { for shown.iter().enumerate().map(|(i, p)| {
                                let sep = if i == 0 { "" } else if i + 1 == shown.len() && more == 0 { " & " } else { ", " };
                                html! {
                                    <>
                                        { sep }
                                        <a class="pp-name-link" onclick={go_to_person(p.id)}>{ &p.name }</a>
                                        { unverified_dot(p) }
                                    </>
                                }
                            }) }
                            { if more > 0 { html! { <span class="pp-more">{ format!(" +{}", more) }</span> } } else { html! {} } }
                        </span>
                    </div>
                    { add_button(true) }
                </div>
            }
        }
        PeopleLayout::Chips => html! {
            <div class={classes!("pp-chips-section", props.class.clone())}>
                <div class="pp-head">
                    { if props.label.is_empty() { html! {} } else { html! { <span class="pp-label">{ &props.label }</span> } } }
                    { add_button(true) }
                </div>
                <div class="ep-mobile-hosts-scroll">
                    { for people.iter().map(|p| html! {
                        <div class="ep-mobile-host-chip" onclick={go_to_person(p.id)} title={p.name.clone()}>
                            <div class="pp-chip-avatar">
                                { avatar(&server_name, p, "ep-mobile-host-avatar", "ep-mobile-host-placeholder") }
                                { unverified_dot(p) }
                            </div>
                            <span class="ep-mobile-host-name">{ &p.name }</span>
                            <span class="ep-mobile-host-role">{ &p.role }</span>
                        </div>
                    }) }
                </div>
            </div>
        },
        PeopleLayout::Cards => html! {
            <div class={classes!(props.class.clone())}>
                <div class="ep-people">
                    { for people.iter().map(|p| {
                        let roles = p.roles.join(", ");
                        let roles = if roles.is_empty() { p.role.clone() } else { roles };
                        html! {
                            <div class="ep-person" onclick={go_to_person(p.id)} title={p.name.clone()}>
                                { avatar(&server_name, p, "ep-person-avatar", "ep-person-avatar-ph") }
                                <div class="ep-person-body">
                                    <div class="ep-person-name">{ &p.name }{ unverified_dot(p) }</div>
                                    <span class="ep-person-role">{ roles }</span>
                                </div>
                                <FollowButton person_id={p.id} followed={p.followed} compact={true} />
                            </div>
                        }
                    }) }
                </div>
                { if props.on_add.is_some() {
                    html! { <div class="pp-cards-foot">{ add_button(false) }</div> }
                } else { html! {} } }
            </div>
        },
    }
}

#[derive(Properties, PartialEq, Clone)]
pub struct FollowButtonProps {
    pub person_id: i32,
    pub followed: bool,
    /// Icon-only.
    #[prop_or_default]
    pub compact: bool,
    #[prop_or_default]
    pub on_change: Option<Callback<bool>>,
}

/// Follow / unfollow toggle. Optimistic; reverts with a toast if the server says no.
#[function_component(FollowButton)]
pub fn follow_button(props: &FollowButtonProps) -> Html {
    let (i18n, _) = use_translation();
    let auth = use_auth();
    let following = use_state(|| props.followed);
    let busy = use_state(|| false);
    {
        let following = following.clone();
        use_effect_with((props.person_id, props.followed), move |(_, followed)| {
            following.set(*followed);
            || ()
        });
    }

    let label_follow = i18n.t("people.follow").to_string();
    let label_following = i18n.t("people.following").to_string();
    let label_unfollow = i18n.t("people.unfollow").to_string();
    let failed = i18n.t("people.follow_failed").to_string();

    let onclick = {
        let following = following.clone();
        let busy = busy.clone();
        let person_id = props.person_id;
        let on_change = props.on_change.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            let Some((server, key)) = auth.clone() else { return };
            if *busy {
                return;
            }
            let target = !*following;
            following.set(target);
            busy.set(true);
            let following = following.clone();
            let busy = busy.clone();
            let on_change = on_change.clone();
            let failed = failed.clone();
            spawn_local(async move {
                match call_set_following(&server, &key, person_id, target).await {
                    Ok(()) => {
                        if let Some(cb) = on_change {
                            cb.emit(target);
                        }
                    }
                    Err(e) => {
                        following.set(!target);
                        notify::toast_error(&Dispatch::<NotificationState>::global(), format!("{}: {}", failed, e));
                    }
                }
                busy.set(false);
            });
        })
    };

    let is_following = *following;
    let (icon, text) = if is_following {
        ("ph-user-check", label_following.clone())
    } else {
        ("ph-user-plus", label_follow.clone())
    };
    let title = if is_following { label_unfollow } else { label_follow };
    html! {
        <button
            type="button"
            class={classes!("pp-follow", is_following.then_some("is-following"), props.compact.then_some("is-compact"))}
            title={title.clone()}
            aria-label={title}
            aria-pressed={is_following.to_string()}
            disabled={*busy}
            {onclick}
        >
            <i class={classes!("ph", icon)}></i>
            { if props.compact { html! {} } else { html! { <span>{ text }</span> } } }
        </button>
    }
}

// ---------------------------------------------------------------------------------------------
// Add people
// ---------------------------------------------------------------------------------------------

#[derive(Clone, PartialEq, Debug)]
struct Pending {
    name: String,
    person_id: Option<i32>,
    podpeople_id: Option<i64>,
    img: Option<String>,
    known_for: Option<String>,
}

impl Pending {
    fn typed(name: String) -> Self {
        Pending { name, person_id: None, podpeople_id: None, img: None, known_for: None }
    }

    fn from_hit(hit: &PersonHit) -> Self {
        Pending {
            name: hit.name.clone(),
            person_id: hit.person_id,
            podpeople_id: hit.podpeople_id,
            img: hit.img.clone(),
            known_for: hit.known_for.clone(),
        }
    }
}

/// Split typed or pasted text into names. Commas, semicolons and line breaks always separate;
/// " and " and " & " only when `all` (paste, Enter, submit), so a name being typed isn't cut.
/// Returns the complete names and the unfinished remainder.
fn split_names(text: &str, all: bool) -> (Vec<String>, String) {
    let mut normalized = text.replace(['\n', '\r', ';'], ",");
    if all {
        for sep in [" and ", " And ", " AND ", " & "] {
            normalized = normalized.replace(sep, ",");
        }
    }
    let mut parts: Vec<String> = normalized.split(',').map(|s| s.trim().to_string()).collect();
    let remainder = if all { String::new() } else { parts.pop().unwrap_or_default() };
    parts.retain(|s| !s.is_empty());
    (parts, if all { String::new() } else { remainder.trim_start().to_string() })
}

fn push_unique(list: &mut Vec<Pending>, item: Pending) {
    let lower = item.name.to_lowercase();
    if item.name.is_empty() || list.iter().any(|p| p.name.to_lowercase() == lower) {
        return;
    }
    list.push(item);
}

#[derive(Properties, PartialEq, Clone)]
pub struct AddPeopleSheetProps {
    pub podcast_id: i32,
    /// Credit the people on this episode instead of the whole show.
    #[prop_or_default]
    pub episode_id: Option<i32>,
    pub on_close: Callback<()>,
    /// Everyone on the podcast or episode after the contribution.
    pub on_added: Callback<Vec<CreditedPerson>>,
}

const ROLES: [&str; 3] = ["host", "co-host", "guest"];

#[function_component(AddPeopleSheet)]
pub fn add_people_sheet(props: &AddPeopleSheetProps) -> Html {
    let (i18n, _) = use_translation();
    let auth = use_auth();
    let input_ref = use_node_ref();
    let text = use_state(String::new);
    let chips = use_state(Vec::<Pending>::new);
    let role = use_state(|| if props.episode_id.is_some() { "guest" } else { "host" }.to_string());
    let hits = use_state(Vec::<PersonHit>::new);
    let highlighted = use_state(|| None::<usize>);
    let searching = use_state(|| false);
    let submitting = use_state(|| false);
    let error = use_state(|| None::<String>);
    let search_seq = use_mut_ref(|| Cell::new(0u32));

    {
        let input_ref = input_ref.clone();
        use_effect_with((), move |_| {
            if let Some(input) = input_ref.cast::<HtmlInputElement>() {
                let _ = input.focus();
            }
            || ()
        });
    }

    // Typeahead: debounced, and only the latest query's results are kept.
    {
        let hits = hits.clone();
        let highlighted = highlighted.clone();
        let searching = searching.clone();
        let search_seq = search_seq.clone();
        let auth = auth.clone();
        use_effect_with((*text).clone(), move |query| {
            let query = query.trim().to_string();
            let seq = search_seq.borrow().get().wrapping_add(1);
            search_seq.borrow().set(seq);
            if query.chars().count() < 2 {
                hits.set(Vec::new());
                highlighted.set(None);
                searching.set(false);
            } else if let Some((server, key)) = auth {
                searching.set(true);
                spawn_local(async move {
                    TimeoutFuture::new(220).await;
                    if search_seq.borrow().get() != seq {
                        return;
                    }
                    let result = call_search_people(&server, &key, &query).await;
                    if search_seq.borrow().get() != seq {
                        return;
                    }
                    hits.set(result.unwrap_or_default().into_iter().take(8).collect());
                    highlighted.set(None);
                    searching.set(false);
                });
            }
            || ()
        });
    }

    let add_names = {
        let chips = chips.clone();
        move |names: Vec<String>| {
            let mut list = (*chips).clone();
            for name in names {
                push_unique(&mut list, Pending::typed(name));
            }
            chips.set(list);
        }
    };

    let oninput = {
        let text = text.clone();
        let error = error.clone();
        let add_names = add_names.clone();
        Callback::from(move |e: InputEvent| {
            let Some(input) = e.target_dyn_into::<HtmlInputElement>() else { return };
            let value = input.value();
            let pasted = e.input_type() == "insertFromPaste";
            error.set(None);
            let (names, remainder) = split_names(&value, pasted);
            // A single pasted name stays in the box so it can still be matched.
            if pasted && names.len() < 2 {
                text.set(value);
                return;
            }
            if !names.is_empty() {
                add_names(names);
                input.set_value(&remainder);
            }
            text.set(remainder);
        })
    };

    let pick = {
        let chips = chips.clone();
        let text = text.clone();
        let hits = hits.clone();
        let input_ref = input_ref.clone();
        Callback::from(move |hit: PersonHit| {
            let mut list = (*chips).clone();
            push_unique(&mut list, Pending::from_hit(&hit));
            chips.set(list);
            text.set(String::new());
            hits.set(Vec::new());
            if let Some(input) = input_ref.cast::<HtmlInputElement>() {
                input.set_value("");
                let _ = input.focus();
            }
        })
    };

    let remove_chip = {
        let chips = chips.clone();
        Callback::from(move |index: usize| {
            let mut list = (*chips).clone();
            if index < list.len() {
                list.remove(index);
            }
            chips.set(list);
        })
    };

    let submit = {
        let auth = auth.clone();
        let chips = chips.clone();
        let text = text.clone();
        let role = role.clone();
        let submitting = submitting.clone();
        let error = error.clone();
        let on_added = props.on_added.clone();
        let on_close = props.on_close.clone();
        let podcast_id = props.podcast_id;
        let episode_id = props.episode_id;
        let need_name = i18n.t("people.need_a_name").to_string();
        let added_one = i18n.t("people.added_one").to_string();
        let added_many = i18n.t("people.added_many").to_string();
        Callback::from(move |_: ()| {
            if *submitting {
                return;
            }
            let Some((server, key)) = auth.clone() else { return };
            let mut list = (*chips).clone();
            let (names, _) = split_names(&text, true);
            for name in names {
                push_unique(&mut list, Pending::typed(name));
            }
            if list.is_empty() {
                error.set(Some(need_name.clone()));
                return;
            }
            chips.set(list.clone());
            text.set(String::new());
            submitting.set(true);
            error.set(None);
            let request = ContributeRequest {
                podcast_id,
                episode_id,
                people: list
                    .iter()
                    .map(|p| ContributePerson {
                        person_id: p.person_id,
                        podpeople_id: p.podpeople_id,
                        name: Some(p.name.clone()),
                        role: Some((*role).clone()),
                    })
                    .collect(),
            };
            let count = list.len();
            let submitting = submitting.clone();
            let error = error.clone();
            let on_added = on_added.clone();
            let on_close = on_close.clone();
            let toast = if count == 1 {
                added_one.replace("{name}", &list[0].name)
            } else {
                added_many.replace("{count}", &count.to_string())
            };
            spawn_local(async move {
                match call_contribute_people(&server, &key, &request).await {
                    Ok(people) => {
                        on_added.emit(people);
                        notify::toast_success(&Dispatch::<NotificationState>::global(), toast);
                        on_close.emit(());
                    }
                    Err(e) => {
                        error.set(Some(e.to_string()));
                        submitting.set(false);
                    }
                }
            });
        })
    };

    let onkeydown = {
        let hits = hits.clone();
        let highlighted = highlighted.clone();
        let pick = pick.clone();
        let text = text.clone();
        let chips = chips.clone();
        let submit = submit.clone();
        let add_names = add_names.clone();
        let on_close = props.on_close.clone();
        let input_ref = input_ref.clone();
        Callback::from(move |e: KeyboardEvent| match e.key().as_str() {
            "ArrowDown" | "ArrowUp" if !hits.is_empty() => {
                e.prevent_default();
                let len = hits.len();
                let next = match (*highlighted, e.key().as_str()) {
                    (None, "ArrowDown") => 0,
                    (None, _) => len - 1,
                    (Some(i), "ArrowDown") => (i + 1) % len,
                    (Some(i), _) => (i + len - 1) % len,
                };
                highlighted.set(Some(next));
            }
            "Enter" => {
                e.prevent_default();
                if let Some(hit) = (*highlighted).and_then(|i| hits.get(i)) {
                    pick.emit(hit.clone());
                } else if text.trim().is_empty() {
                    submit.emit(());
                } else {
                    let (names, _) = split_names(&text, true);
                    add_names(names);
                    text.set(String::new());
                    hits.set(Vec::new());
                    if let Some(input) = input_ref.cast::<HtmlInputElement>() {
                        input.set_value("");
                    }
                }
            }
            "Backspace" if text.is_empty() && !chips.is_empty() => {
                let mut list = (*chips).clone();
                list.pop();
                chips.set(list);
            }
            "Escape" => {
                if hits.is_empty() {
                    on_close.emit(());
                } else {
                    hits.set(Vec::new());
                }
            }
            _ => {}
        })
    };

    let close = {
        let on_close = props.on_close.clone();
        Callback::from(move |_: MouseEvent| on_close.emit(()))
    };
    let on_submit_click = {
        let submit = submit.clone();
        Callback::from(move |_: MouseEvent| submit.emit(()))
    };

    let server_name = auth.as_ref().map(|(s, _)| s.clone()).unwrap_or_default();
    let title = if props.episode_id.is_some() {
        i18n.t("people.add_title_episode")
    } else {
        i18n.t("people.add_title_podcast")
    };
    let pending_count = chips.len() + usize::from(!text.trim().is_empty());
    let submit_label = if *submitting {
        i18n.t("people.adding").to_string()
    } else if pending_count > 1 {
        i18n.t("people.add_count").replace("{count}", &pending_count.to_string())
    } else {
        i18n.t("people.add").to_string()
    };
    let role_label = |r: &str| match r {
        "host" => i18n.t("people.role_host"),
        "co-host" => i18n.t("people.role_cohost"),
        _ => i18n.t("people.role_guest"),
    };
    let typed = text.trim().to_string();
    let exact_match = hits.iter().any(|h| h.name.eq_ignore_ascii_case(&typed));

    html! {
        <div class="pp-sheet-scrim" onclick={close.clone()}>
            <div
                class="pp-sheet"
                role="dialog"
                aria-modal="true"
                aria-label={title.clone()}
                onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}
            >
                <div class="pp-sheet-grip"></div>
                <div class="pp-sheet-head">
                    <h3 class="pp-sheet-title">{ title }</h3>
                    <button type="button" class="pp-icon-btn" aria-label={i18n.t("people.cancel")} onclick={close.clone()}>
                        <i class="ph ph-x"></i>
                    </button>
                </div>

                <div class="pp-field">
                    { for chips.iter().enumerate().map(|(i, p)| {
                        let remove_chip = remove_chip.clone();
                        html! {
                            <span class={classes!("pp-token", (p.person_id.is_some() || p.podpeople_id.is_some()).then_some("is-known"))}
                                  title={p.known_for.clone().unwrap_or_default()}>
                                { match p.img.as_deref().filter(|s| !s.is_empty()) {
                                    Some(img) => html! { <img src={proxied_image(&server_name, img)} alt="" /> },
                                    None => html! {},
                                } }
                                <span>{ &p.name }</span>
                                <button type="button" aria-label={i18n.t("people.remove")}
                                        onclick={Callback::from(move |_: MouseEvent| remove_chip.emit(i))}>
                                    <i class="ph ph-x"></i>
                                </button>
                            </span>
                        }
                    }) }
                    <input
                        ref={input_ref.clone()}
                        type="text"
                        class="pp-input"
                        placeholder={if chips.is_empty() { i18n.t("people.input_placeholder") } else { i18n.t("people.input_placeholder_more") }}
                        autocomplete="off"
                        spellcheck="false"
                        value={(*text).clone()}
                        {oninput}
                        {onkeydown}
                        disabled={*submitting}
                    />
                </div>

                { if !hits.is_empty() || typed.chars().count() >= 2 {
                    html! {
                        <ul class="pp-suggest" role="listbox">
                            { for hits.iter().enumerate().map(|(i, hit)| {
                                let pick = pick.clone();
                                let h = hit.clone();
                                let on_pick = Callback::from(move |e: MouseEvent| { e.prevent_default(); pick.emit(h.clone()); });
                                html! {
                                    <li role="option" aria-selected={(Some(i) == *highlighted).to_string()}
                                        class={classes!("pp-suggest-item", (Some(i) == *highlighted).then_some("is-active"))}
                                        onmousedown={on_pick}>
                                        <PersonAvatar
                                            src={hit.img.as_deref().filter(|s| !s.is_empty()).map(|img| AttrValue::from(proxied_image(&server_name, img)))}
                                            class="pp-suggest-avatar"
                                            placeholder="is-ph"
                                        />
                                        <div class="pp-suggest-body">
                                            <span class="pp-suggest-name">{ &hit.name }</span>
                                            { match &hit.known_for {
                                                Some(k) if !k.is_empty() => html! { <span class="pp-suggest-meta">{ k }</span> },
                                                _ => html! {},
                                            } }
                                        </div>
                                    </li>
                                }
                            }) }
                            { if !typed.is_empty() && !exact_match && !*searching {
                                let add_typed = {
                                    let add_names = add_names.clone();
                                    let text = text.clone();
                                    let hits = hits.clone();
                                    let input_ref = input_ref.clone();
                                    let typed = typed.clone();
                                    Callback::from(move |e: MouseEvent| {
                                        e.prevent_default();
                                        add_names(split_names(&typed, true).0);
                                        text.set(String::new());
                                        hits.set(Vec::new());
                                        if let Some(input) = input_ref.cast::<HtmlInputElement>() {
                                            input.set_value("");
                                        }
                                    })
                                };
                                html! {
                                    <li class="pp-suggest-item is-new" onmousedown={add_typed}>
                                        <div class="pp-suggest-avatar is-ph"><i class="ph ph-plus"></i></div>
                                        <div class="pp-suggest-body">
                                            <span class="pp-suggest-name">{ i18n.t("people.new_person").replace("{name}", &typed) }</span>
                                            <span class="pp-suggest-meta">{ i18n.t("people.press_enter") }</span>
                                        </div>
                                    </li>
                                }
                            } else if *searching && hits.is_empty() {
                                html! { <li class="pp-suggest-item is-muted">{ i18n.t("people.searching") }</li> }
                            } else { html! {} } }
                        </ul>
                    }
                } else { html! {} } }

                <div class="pp-role-row">
                    <span class="pp-role-label">{ i18n.t("people.role") }</span>
                    <div class="pp-segmented" role="radiogroup">
                        { for ROLES.iter().map(|r| {
                            let role = role.clone();
                            let active = *role == *r;
                            let value = r.to_string();
                            html! {
                                <button type="button" role="radio" aria-checked={active.to_string()}
                                        class={classes!("pp-seg", active.then_some("is-active"))}
                                        onclick={Callback::from(move |_: MouseEvent| role.set(value.clone()))}>
                                    { role_label(r) }
                                </button>
                            }
                        }) }
                    </div>
                </div>

                { match &*error {
                    Some(message) => html! { <p class="pp-error" role="alert"><i class="ph ph-warning-circle"></i>{ message }</p> },
                    None => html! {},
                } }

                <p class="pp-note">{ i18n.t("people.shared_note") }</p>

                <div class="pp-sheet-actions">
                    <button type="button" class="pp-btn" onclick={close}>{ i18n.t("people.cancel") }</button>
                    <button type="button" class="pp-btn is-primary" onclick={on_submit_click}
                            disabled={*submitting || pending_count == 0}>
                        { submit_label }
                    </button>
                </div>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::split_names;

    #[test]
    fn splits_pasted_lists() {
        let (names, rest) = split_names("Alice, Bob and Carol", true);
        assert_eq!(names, vec!["Alice", "Bob", "Carol"]);
        assert_eq!(rest, "");
        let (names, _) = split_names("Tom & Jerry;Spike\nTyke", true);
        assert_eq!(names, vec!["Tom", "Jerry", "Spike", "Tyke"]);
    }

    #[test]
    fn typing_keeps_the_unfinished_name() {
        let (names, rest) = split_names("Alice, Bob and Ca", false);
        assert_eq!(names, vec!["Alice"]);
        assert_eq!(rest, "Bob and Ca");
        let (names, rest) = split_names("Alice", false);
        assert!(names.is_empty());
        assert_eq!(rest, "Alice");
    }
}
