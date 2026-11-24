//! Episode labels: one store for the user's labels and which episodes carry them, the chips shown
//! on episode cards, the picker overlay (mounted once at the app root) and the settings manager.
//! The store also keeps the inbox count shown in the drawer.

use crate::components::context::{AppState, NotificationState};
use crate::components::i18n::use_translation;
use crate::requests::inbox_req::{self, EpisodeLabel, EpisodeRef, LABEL_COLORS};
use crate::requests::notify::toast_error;
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yewdux::prelude::*;

#[derive(Default, Clone, PartialEq, Store)]
pub struct LabelState {
    /// The server and user the labels were loaded for.
    pub loaded_for: Option<(String, i32)>,
    pub labels: Rc<Vec<EpisodeLabel>>,
    /// Label ids on each `(episode_id, is_youtube)`.
    pub assignments: Rc<HashMap<(i32, bool), Vec<i32>>>,
    /// Episodes the picker overlay is open for.
    pub picker: Option<Vec<EpisodeRef>>,
    pub inbox_count: Option<i64>,
}

impl LabelState {
    pub fn label(&self, id: i32) -> Option<&EpisodeLabel> {
        self.labels.iter().find(|l| l.label_id == id)
    }
    pub fn on(&self, episode_id: i32, is_youtube: bool) -> &[i32] {
        self.assignments.get(&(episode_id, is_youtube)).map(|v| v.as_slice()).unwrap_or(&[])
    }
}

/// `(server, api_key, user_id)` of the signed-in user.
pub fn creds() -> Option<(String, String, i32)> {
    let app = Dispatch::<AppState>::global().get();
    let auth = app.auth_details.as_ref()?;
    Some((auth.server_name.clone(), auth.api_key.clone()?, app.user_details.as_ref()?.UserID))
}

/// The signed-in `(server, user id)`, which can arrive after a page's first render on a fresh
/// load; effects that fetch should depend on it.
#[hook]
pub fn use_signed_in() -> std::rc::Rc<Option<(String, i32)>> {
    use_selector(|s: &AppState| {
        let auth = s.auth_details.as_ref().filter(|a| a.api_key.is_some())?;
        Some((auth.server_name.clone(), s.user_details.as_ref()?.UserID))
    })
}

/// Load labels and assignments, unless they're already loaded for this user.
pub fn ensure_loaded() {
    let Some((server, _, user_id)) = creds() else { return };
    if Dispatch::<LabelState>::global().get().loaded_for == Some((server, user_id)) {
        return;
    }
    reload();
}

pub fn reload() {
    let Some((server, api_key, user_id)) = creds() else { return };
    spawn_local(async move {
        let labels = inbox_req::call_get_labels(&server, &api_key).await;
        let assignments = inbox_req::call_get_label_assignments(&server, &api_key).await;
        if let (Ok(labels), Ok(assignments)) = (labels, assignments) {
            let mut map: HashMap<(i32, bool), Vec<i32>> = HashMap::new();
            for a in assignments {
                map.entry((a.episode_id, a.is_youtube)).or_default().push(a.label_id);
            }
            Dispatch::<LabelState>::global().reduce_mut(move |s| {
                s.loaded_for = Some((server, user_id));
                s.labels = Rc::new(labels);
                s.assignments = Rc::new(map);
            });
        }
    });
}

pub fn refresh_inbox_count() {
    let Some((server, api_key, _)) = creds() else { return };
    spawn_local(async move {
        if let Ok(count) = inbox_req::call_get_inbox_count(&server, &api_key).await {
            Dispatch::<LabelState>::global().reduce_mut(move |s| s.inbox_count = Some(count));
        }
    });
}

/// Change the shown inbox count by `delta` without asking the server.
pub fn adjust_inbox_count(delta: i64) {
    Dispatch::<LabelState>::global().reduce_mut(move |s| {
        if let Some(c) = s.inbox_count.as_mut() {
            *c = (*c + delta).max(0);
        }
    });
}

pub fn open_picker(episodes: Vec<EpisodeRef>) {
    ensure_loaded();
    Dispatch::<LabelState>::global().reduce_mut(move |s| s.picker = Some(episodes));
}

/// Put a label on episodes or take it off: the store changes at once, and back if the server
/// refuses.
pub fn set_label_on(label_id: i32, episodes: Vec<EpisodeRef>, assigned: bool) {
    let Some((server, api_key, _)) = creds() else { return };
    let targets = episodes.clone();
    let apply = move |s: &mut LabelState, assigned: bool| {
        let mut map = (*s.assignments).clone();
        for e in &targets {
            let ids = map.entry((e.episode_id, e.is_youtube)).or_default();
            ids.retain(|id| *id != label_id);
            if assigned {
                ids.push(label_id);
            }
        }
        map.retain(|_, v| !v.is_empty());
        let count = map.values().filter(|v| v.contains(&label_id)).count() as i64;
        s.assignments = Rc::new(map);
        let mut labels = (*s.labels).clone();
        if let Some(l) = labels.iter_mut().find(|l| l.label_id == label_id) {
            l.episode_count = count;
        }
        s.labels = Rc::new(labels);
    };
    let apply_now = apply.clone();
    Dispatch::<LabelState>::global().reduce_mut(move |s| apply_now(s, assigned));
    spawn_local(async move {
        if let Err(e) = inbox_req::call_set_label_episodes(&server, &api_key, label_id, &episodes, assigned).await {
            Dispatch::<LabelState>::global().reduce_mut(move |s| apply(s, !assigned));
            toast_error(&Dispatch::<NotificationState>::global(), e.to_string());
        }
    });
}

/// CSS modifier for a label colour.
pub fn color_class(color: &str) -> String {
    let c = if LABEL_COLORS.contains(&color) { color } else { "blue" };
    format!("ep-label--{c}")
}

#[derive(Properties, PartialEq)]
pub struct LabelChipsProps {
    pub episode_id: i32,
    pub is_youtube: bool,
    /// Clicking a chip opens the picker for this episode.
    #[prop_or(false)]
    pub editable: bool,
}

/// The labels on one episode; nothing when it has none.
#[function_component(LabelChips)]
pub fn label_chips(props: &LabelChipsProps) -> Html {
    let key = (props.episode_id, props.is_youtube);
    let chips = use_selector(move |s: &LabelState| {
        s.on(key.0, key.1)
            .iter()
            .filter_map(|id| s.label(*id).cloned())
            .collect::<Vec<_>>()
    });
    if chips.is_empty() {
        return html! {};
    }
    let onclick = props.editable.then(|| {
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            open_picker(vec![EpisodeRef { episode_id: key.0, is_youtube: key.1 }]);
        })
    });
    html! {
        <span class="ep-labels" {onclick}>
            { for chips.iter().map(|l| html! {
                <span class={classes!("ep-label", color_class(&l.color))}>{ l.name.clone() }</span>
            }) }
        </span>
    }
}

/// Global overlay: tick labels on or off for the episodes in `LabelState::picker`; a new label
/// can be made from here and goes straight onto them.
#[function_component(LabelPickerModal)]
pub fn label_picker_modal() -> Html {
    let (i18n, _) = use_translation();
    let (state, dispatch) = use_store::<LabelState>();
    let new_name = use_state(String::new);
    let new_color = use_state(|| "blue".to_string());
    let busy = use_state(|| false);

    let Some(episodes) = state.picker.clone() else {
        return html! {};
    };
    let close = {
        let dispatch = dispatch.clone();
        let new_name = new_name.clone();
        Callback::from(move |_: MouseEvent| {
            new_name.set(String::new());
            dispatch.reduce_mut(|s| s.picker = None);
        })
    };
    let stop = Callback::from(|e: MouseEvent| e.stop_propagation());

    // A label counts as on when every episode in the picker has it.
    let is_on = |label_id: i32| {
        !episodes.is_empty() && episodes.iter().all(|e| state.on(e.episode_id, e.is_youtube).contains(&label_id))
    };

    let create = {
        let new_name = new_name.clone();
        let new_color = new_color.clone();
        let busy = busy.clone();
        let episodes = episodes.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let name = new_name.trim().to_string();
            if name.is_empty() || *busy {
                return;
            }
            let Some((server, api_key, _)) = creds() else { return };
            let color = (*new_color).clone();
            let episodes = episodes.clone();
            let new_name = new_name.clone();
            let busy = busy.clone();
            busy.set(true);
            spawn_local(async move {
                match inbox_req::call_create_label(&server, &api_key, &name, &color).await {
                    Ok(label) => {
                        let id = label.label_id;
                        Dispatch::<LabelState>::global().reduce_mut(move |s| {
                            let mut labels = (*s.labels).clone();
                            labels.push(label);
                            labels.sort_by_key(|l| l.name.to_lowercase());
                            s.labels = Rc::new(labels);
                        });
                        set_label_on(id, episodes, true);
                        new_name.set(String::new());
                    }
                    Err(e) => toast_error(&Dispatch::<NotificationState>::global(), e.to_string()),
                }
                busy.set(false);
            });
        })
    };

    html! {
        <div class="fixed inset-0 z-50 overflow-y-auto bg-black bg-opacity-25" onclick={close.clone()}>
            <div class="flex min-h-full items-center justify-center p-4">
                <div class="modal-container relative w-full max-w-md rounded-lg shadow" onclick={stop}>
                    <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t">
                        <h3 class="text-xl font-semibold">{
                            if episodes.len() > 1 {
                                i18n.t("labels.label_episodes_count").replace("{count}", &episodes.len().to_string())
                            } else {
                                i18n.t("labels.labels").to_string()
                            }
                        }</h3>
                        <button onclick={close.clone()} class="text-gray-400 bg-transparent rounded-lg text-sm w-8 h-8 inline-flex justify-center items-center">
                            <i class="ph ph-x text-xl"></i>
                        </button>
                    </div>
                    <div class="p-4 md:p-5">
                        if state.labels.is_empty() {
                            <p class="item_container-text text-sm mb-3">{ i18n.t("labels.none_yet") }</p>
                        }
                        <ul class="max-h-64 overflow-y-auto mb-3">
                            { for state.labels.iter().map(|l| {
                                let on = is_on(l.label_id);
                                let id = l.label_id;
                                let episodes = episodes.clone();
                                let onclick = Callback::from(move |e: MouseEvent| {
                                    e.stop_propagation();
                                    set_label_on(id, episodes.clone(), !on);
                                });
                                html! {
                                    <li class="dropdown-option flex items-center gap-2" {onclick}>
                                        <i class={classes!("ph", if on { "ph-check-square" } else { "ph-square" }, "text-xl")}></i>
                                        <span class={classes!("ep-label", color_class(&l.color))}>{ l.name.clone() }</span>
                                    </li>
                                }
                            }) }
                        </ul>
                        <form class="label-new" onsubmit={create}>
                            <input
                                type="text"
                                class="search-bar-input border text-sm rounded-lg block w-full p-2.5"
                                maxlength="64"
                                placeholder={i18n.t("labels.new_label")}
                                value={(*new_name).clone()}
                                oninput={let new_name = new_name.clone(); Callback::from(move |e: InputEvent| {
                                    let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                                    new_name.set(input.value());
                                })}
                            />
                            <ColorPicker value={(*new_color).clone()} onchange={let c = new_color.clone(); Callback::from(move |v: String| c.set(v))} />
                            <button type="submit" class="btn btn-secondary" disabled={*busy || new_name.trim().is_empty()}>
                                { i18n.t("labels.add") }
                            </button>
                        </form>
                    </div>
                </div>
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct ColorPickerProps {
    pub value: String,
    pub onchange: Callback<String>,
}

#[function_component(ColorPicker)]
pub fn color_picker(props: &ColorPickerProps) -> Html {
    html! {
        <span class="label-colors" role="radiogroup">
            { for LABEL_COLORS.iter().map(|c| {
                let selected = props.value == *c;
                let onchange = props.onchange.clone();
                let value = c.to_string();
                html! {
                    <button
                        type="button"
                        class={classes!("label-swatch", color_class(c), selected.then_some("is-selected"))}
                        role="radio"
                        aria-checked={selected.to_string()}
                        title={c.to_string()}
                        onclick={Callback::from(move |e: MouseEvent| { e.stop_propagation(); onchange.emit(value.clone()); })}
                    ></button>
                }
            }) }
        </span>
    }
}

/// Settings: create, rename, recolour and delete labels.
#[function_component(LabelManager)]
pub fn label_manager() -> Html {
    let (i18n, _) = use_translation();
    let state = use_store_value::<LabelState>();
    let editing = use_state(|| None::<(i32, String, String)>);
    let new_name = use_state(String::new);
    let new_color = use_state(|| "blue".to_string());

    let signed_in = use_signed_in();
    use_effect_with(signed_in, |_| {
        ensure_loaded();
        || ()
    });

    let save_new = {
        let new_name = new_name.clone();
        let new_color = new_color.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let name = new_name.trim().to_string();
            let Some((server, api_key, _)) = creds() else { return };
            if name.is_empty() {
                return;
            }
            let color = (*new_color).clone();
            let new_name = new_name.clone();
            spawn_local(async move {
                match inbox_req::call_create_label(&server, &api_key, &name, &color).await {
                    Ok(_) => {
                        new_name.set(String::new());
                        reload();
                    }
                    Err(e) => toast_error(&Dispatch::<NotificationState>::global(), e.to_string()),
                }
            });
        })
    };

    let i18n_delete_confirm = i18n.t("labels.delete_confirm").to_string();
    let i18n_episodes = i18n.t("labels.episode_count").to_string();

    html! {
        <div class="label-manager">
            if state.labels.is_empty() {
                <p class="item_container-text text-sm mb-3">{ i18n.t("labels.none_yet") }</p>
            }
            <ul class="label-manager-list">
                { for state.labels.iter().map(|l| {
                    let id = l.label_id;
                    let is_editing = editing.as_ref().map(|(e, _, _)| *e == id).unwrap_or(false);
                    if is_editing {
                        let (_, name, color) = (*editing).clone().unwrap();
                        let editing_name = editing.clone();
                        let editing_color = editing.clone();
                        let editing_save = editing.clone();
                        let editing_cancel = editing.clone();
                        let onsubmit = Callback::from(move |e: SubmitEvent| {
                            e.prevent_default();
                            let Some((server, api_key, _)) = creds() else { return };
                            let Some((id, name, color)) = (*editing_save).clone() else { return };
                            let editing_save = editing_save.clone();
                            spawn_local(async move {
                                match inbox_req::call_update_label(&server, &api_key, id, name.trim(), &color).await {
                                    Ok(()) => {
                                        editing_save.set(None);
                                        reload();
                                    }
                                    Err(e) => toast_error(&Dispatch::<NotificationState>::global(), e.to_string()),
                                }
                            });
                        });
                        html! {
                            <li class="label-manager-row">
                                <form class="label-new" {onsubmit}>
                                    <input
                                        type="text"
                                        class="search-bar-input border text-sm rounded-lg block w-full p-2.5"
                                        maxlength="64"
                                        value={name.clone()}
                                        oninput={Callback::from(move |e: InputEvent| {
                                            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                                            if let Some((id, _, color)) = (*editing_name).clone() {
                                                editing_name.set(Some((id, input.value(), color)));
                                            }
                                        })}
                                    />
                                    <ColorPicker value={color.clone()} onchange={Callback::from(move |c: String| {
                                        if let Some((id, name, _)) = (*editing_color).clone() {
                                            editing_color.set(Some((id, name, c)));
                                        }
                                    })} />
                                    <button type="submit" class="btn btn-secondary">{ i18n.t("labels.save") }</button>
                                    <button type="button" class="ico" onclick={Callback::from(move |_| editing_cancel.set(None))}>
                                        <i class="ph ph-x"></i>
                                    </button>
                                </form>
                            </li>
                        }
                    } else {
                        let start_edit = {
                            let editing = editing.clone();
                            let l = l.clone();
                            Callback::from(move |_| editing.set(Some((l.label_id, l.name.clone(), l.color.clone()))))
                        };
                        let confirm = i18n_delete_confirm.replace("{name}", &l.name);
                        let delete = Callback::from(move |_| {
                            let ok = web_sys::window().and_then(|w| w.confirm_with_message(&confirm).ok()).unwrap_or(false);
                            if !ok {
                                return;
                            }
                            let Some((server, api_key, _)) = creds() else { return };
                            spawn_local(async move {
                                match inbox_req::call_delete_label(&server, &api_key, id).await {
                                    Ok(()) => reload(),
                                    Err(e) => toast_error(&Dispatch::<NotificationState>::global(), e.to_string()),
                                }
                            });
                        });
                        html! {
                            <li class="label-manager-row">
                                <span class={classes!("ep-label", color_class(&l.color))}>{ l.name.clone() }</span>
                                <span class="label-manager-count">{ i18n_episodes.replace("{count}", &l.episode_count.to_string()) }</span>
                                <span class="label-manager-actions">
                                    <button class="ico" title={i18n.t("labels.rename")} onclick={start_edit}><i class="ph ph-pencil-simple"></i></button>
                                    <button class="ico" title={i18n.t("labels.delete")} onclick={delete}><i class="ph ph-trash"></i></button>
                                </span>
                            </li>
                        }
                    }
                }) }
            </ul>
            <form class="label-new" onsubmit={save_new}>
                <input
                    type="text"
                    class="search-bar-input border text-sm rounded-lg block w-full p-2.5"
                    maxlength="64"
                    placeholder={i18n.t("labels.new_label")}
                    value={(*new_name).clone()}
                    oninput={let new_name = new_name.clone(); Callback::from(move |e: InputEvent| {
                        let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                        new_name.set(input.value());
                    })}
                />
                <ColorPicker value={(*new_color).clone()} onchange={let c = new_color.clone(); Callback::from(move |v: String| c.set(v))} />
                <button type="submit" class="btn btn-secondary" disabled={new_name.trim().is_empty()}>{ i18n.t("labels.add") }</button>
            </form>
        </div>
    }
}
