use crate::components::context::AppState;
use crate::components::gen_funcs::format_error_message;
use crate::requests::setting_reqs::{
    call_get_theme, call_set_theme, call_get_custom_themes, call_delete_custom_theme,
    CustomTheme, DeleteCustomThemeRequest, SetThemeRequest,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::window;
use yew::prelude::*;
use yewdux::prelude::*;

#[derive(Clone, Copy)]
struct ThemeData {
    name: &'static str,
    bg: &'static str,
    text: &'static str,
    swatch1: &'static str,
    swatch2: &'static str,
}

const THEMES: &[ThemeData] = &[
    ThemeData { name: "Light",                  bg: "#F9F9F9", text: "#4A4A4A", swatch1: "#0099E1", swatch2: "#4A4A4A" },
    ThemeData { name: "Soft Lavender",           bg: "#F5F0FF", text: "#50456E", swatch1: "#9371D9", swatch2: "#50456E" },
    ThemeData { name: "Minty Fresh",             bg: "#F1F9F6", text: "#2D6E5B", swatch1: "#3D9D82", swatch2: "#2D6E5B" },
    ThemeData { name: "Warm Vanilla",            bg: "#FDF6E9", text: "#6D4922", swatch1: "#C6A06D", swatch2: "#6D4922" },
    ThemeData { name: "Coastal Blue",            bg: "#F0F5FA", text: "#2C5D8F", swatch1: "#4C87C5", swatch2: "#2C5D8F" },
    ThemeData { name: "Paper Cream",             bg: "#FAF7F2", text: "#4A4439", swatch1: "#A19788", swatch2: "#4A4439" },
    ThemeData { name: "Dark",                    bg: "#2A2B33", text: "#F6F5F4", swatch1: "#4A535E", swatch2: "#F6F5F4" },
    ThemeData { name: "Nordic Light",            bg: "#ECEFF4", text: "#656D76", swatch1: "#2984CE", swatch2: "#656D76" },
    ThemeData { name: "Nordic",                  bg: "#3C4252", text: "#F6F5F4", swatch1: "#5D80AA", swatch2: "#F6F5F4" },
    ThemeData { name: "Abyss",                   bg: "#000C18", text: "#F6F5F4", swatch1: "#326FEF", swatch2: "#F6F5F4" },
    ThemeData { name: "Dracula",                 bg: "#282A36", text: "#F6F5F4", swatch1: "#BD93F9", swatch2: "#F6F5F4" },
    ThemeData { name: "Catppuccin Mocha Mauve",  bg: "#1E1E2E", text: "#CDD6F4", swatch1: "#CBA6F7", swatch2: "#CDD6F4" },
    ThemeData { name: "Midnight Ocean",          bg: "#0F172A", text: "#E2E8F0", swatch1: "#38BDF8", swatch2: "#E2E8F0" },
    ThemeData { name: "Forest Depths",           bg: "#1A2F1F", text: "#C9E4CA", swatch1: "#7FB685", swatch2: "#C9E4CA" },
    ThemeData { name: "Sunset Horizon",          bg: "#2B1C2C", text: "#FFD9C0", swatch1: "#FF9E64", swatch2: "#FFD9C0" },
    ThemeData { name: "Arctic Frost",            bg: "#1A1D21", text: "#ECEFF4", swatch1: "#88C0D0", swatch2: "#ECEFF4" },
    ThemeData { name: "Cyber Synthwave",         bg: "#1A1721", text: "#EEE6FF", swatch1: "#F92AAD", swatch2: "#EEE6FF" },
    ThemeData { name: "Github Light",            bg: "#FFFFFF", text: "#70777E", swatch1: "#6590FD", swatch2: "#70777E" },
    ThemeData { name: "Neon",                    bg: "#120E16", text: "#9F9DA1", swatch1: "#F75C1D", swatch2: "#9F9DA1" },
    ThemeData { name: "Kimbie",                  bg: "#221A0F", text: "#B1AD86", swatch1: "#CA9858", swatch2: "#B1AD86" },
    ThemeData { name: "Gruvbox Light",           bg: "#F9F5D7", text: "#5F5750", swatch1: "#D1AC0E", swatch2: "#5F5750" },
    ThemeData { name: "Gruvbox Dark",            bg: "#32302F", text: "#868729", swatch1: "#424314", swatch2: "#868729" },
    ThemeData { name: "Greenie Meanie",          bg: "#142E28", text: "#489D50", swatch1: "#489D50", swatch2: "#489D50" },
    ThemeData { name: "Wildberries",             bg: "#240041", text: "#CF8B3E", swatch1: "#F55385", swatch2: "#CF8B3E" },
    ThemeData { name: "Hot Dog Stand - MY EYES", bg: "#670B0A", text: "#121215", swatch1: "#EEB911", swatch2: "#121215" },
];

#[derive(Properties, PartialEq)]
pub struct ThemeOptionsProps {
    #[prop_or_default]
    pub refresh_trigger: u32,
}

#[function_component(ThemeOptions)]
pub fn theme(props: &ThemeOptionsProps) -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let selected_theme = use_state(|| "".to_string());
    let loading = use_state(|| true);
    let expanded = use_state(|| false);
    let custom_themes = use_state(|| Vec::<CustomTheme>::new());

    // Shuffled theme order — computed once on mount so the "show more" section
    // doesn't reshuffle every time the user picks a theme.
    let shuffled_order = use_state(|| {
        let n = THEMES.len();
        let mut indices: Vec<usize> = (0..n).collect();
        for i in (1..n).rev() {
            let j = (js_sys::Math::random() * (i + 1) as f64) as usize;
            indices.swap(i, j);
        }
        indices
    });

    // Fetch current selected theme on mount
    {
        let selected_theme = selected_theme.clone();
        let loading = loading.clone();
        let state = state.clone();

        use_effect_with((), move |_| {
            if let (Some(api_key), Some(user_id), Some(server_name)) = (
                state.auth_details.as_ref().and_then(|d| d.api_key.clone()),
                state.user_details.as_ref().map(|d| d.UserID),
                state.auth_details.as_ref().map(|d| d.server_name.clone()),
            ) {
                spawn_local(async move {
                    match call_get_theme(server_name, api_key, &user_id).await {
                        Ok(theme) => {
                            selected_theme.set(theme);
                            loading.set(false);
                        }
                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Error fetching theme: {:?}", e).into(),
                            );
                            loading.set(false);
                        }
                    }
                });
            }
            || ()
        });
    }

    // Fetch custom themes on mount and whenever refresh_trigger changes
    {
        let custom_themes = custom_themes.clone();
        let state = state.clone();
        let refresh_trigger = props.refresh_trigger;

        use_effect_with(refresh_trigger, move |_| {
            if let (Some(api_key), Some(user_id), Some(server_name)) = (
                state.auth_details.as_ref().and_then(|d| d.api_key.clone()),
                state.user_details.as_ref().map(|d| d.UserID),
                state.auth_details.as_ref().map(|d| d.server_name.clone()),
            ) {
                spawn_local(async move {
                    match call_get_custom_themes(&server_name, &api_key, user_id).await {
                        Ok(themes) => custom_themes.set(themes),
                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Error fetching custom themes: {:?}", e).into(),
                            );
                        }
                    }
                });
            }
            || ()
        });
    }

    let on_select = {
        let selected_theme = selected_theme.clone();
        let state = state.clone();
        let dispatch = dispatch.clone();

        Callback::from(move |theme_name: String| {
            let dispatch = dispatch.clone();

            changeTheme(&theme_name);

            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    let _ = storage.set_item("selected_theme", &theme_name);
                }
            }

            selected_theme.set(theme_name.clone());

            if let (Some(api_key), Some(user_id), Some(server_name)) = (
                state.auth_details.as_ref().and_then(|d| d.api_key.clone()),
                state.user_details.as_ref().map(|d| d.UserID),
                state.auth_details.as_ref().map(|d| d.server_name.clone()),
            ) {
                let request = SetThemeRequest { user_id, new_theme: theme_name };
                spawn_local(async move {
                    if let Err(e) =
                        call_set_theme(&Some(server_name), &Some(api_key), &request).await
                    {
                        let formatted = format_error_message(&e.to_string());
                        dispatch.reduce_mut(|s| {
                            s.error_message =
                                Some(format!("Failed to update theme: {}", formatted));
                        });
                    }
                });
            }
        })
    };

    let on_select_custom = {
        let selected_theme = selected_theme.clone();
        let state = state.clone();
        let dispatch = dispatch.clone();
        let custom_themes = custom_themes.clone();

        Callback::from(move |(theme_name, theme_id): (String, i32)| {
            let dispatch = dispatch.clone();

            // Find the custom theme data and apply its colors directly
            if let Some(ct) = (*custom_themes).iter().find(|t| t.themeid == theme_id) {
                applyCustomTheme(
                    &ct.background_color,
                    &ct.button_color,
                    &ct.container_button_color,
                    &ct.button_text_color,
                    &ct.text_color,
                    &ct.text_secondary_color,
                    &ct.border_color,
                    &ct.accent_color,
                    &ct.prog_bar_color,
                    &ct.error_color,
                    &ct.bonus_color,
                    &ct.secondary_background,
                    &ct.container_background,
                    &ct.standout_color,
                    &ct.hover_color,
                    &ct.link_color,
                    &ct.thumb_color,
                    &ct.unfilled_color,
                    &ct.check_box_color,
                );
            }

            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    let _ = storage.set_item("selected_theme", &theme_name);
                }
            }

            selected_theme.set(theme_name.clone());

            if let (Some(api_key), Some(user_id), Some(server_name)) = (
                state.auth_details.as_ref().and_then(|d| d.api_key.clone()),
                state.user_details.as_ref().map(|d| d.UserID),
                state.auth_details.as_ref().map(|d| d.server_name.clone()),
            ) {
                let request = SetThemeRequest { user_id, new_theme: theme_name };
                spawn_local(async move {
                    if let Err(e) =
                        call_set_theme(&Some(server_name), &Some(api_key), &request).await
                    {
                        let formatted = format_error_message(&e.to_string());
                        dispatch.reduce_mut(|s| {
                            s.error_message =
                                Some(format!("Failed to update theme: {}", formatted));
                        });
                    }
                });
            }
        })
    };

    let on_delete_custom = {
        let custom_themes = custom_themes.clone();
        let selected_theme = selected_theme.clone();
        let state = state.clone();
        let dispatch = dispatch.clone();

        Callback::from(move |(theme_id, theme_name): (i32, String)| {
            let custom_themes = custom_themes.clone();
            let selected_theme = selected_theme.clone();
            let dispatch = dispatch.clone();

            // Optimistically remove from list
            let prev = (*custom_themes).clone();
            custom_themes.set(prev.iter().filter(|t| t.themeid != theme_id).cloned().collect());

            // If this was the active theme, switch to Nordic
            if *selected_theme == theme_name {
                changeTheme("Nordic");
                if let Some(window) = web_sys::window() {
                    if let Ok(Some(storage)) = window.local_storage() {
                        let _ = storage.set_item("selected_theme", "Nordic");
                    }
                }
                selected_theme.set("Nordic".to_string());
            }

            if let (Some(api_key), Some(user_id), Some(server_name)) = (
                state.auth_details.as_ref().and_then(|d| d.api_key.clone()),
                state.user_details.as_ref().map(|d| d.UserID),
                state.auth_details.as_ref().map(|d| d.server_name.clone()),
            ) {
                let req = DeleteCustomThemeRequest { user_id, theme_id };
                spawn_local(async move {
                    if let Err(e) = call_delete_custom_theme(&server_name, &api_key, &req).await {
                        let formatted = format_error_message(&e.to_string());
                        dispatch.reduce_mut(|s| {
                            s.error_message =
                                Some(format!("Failed to delete theme: {}", formatted));
                        });
                    }
                });
            }
        })
    };

    let on_toggle_expand = {
        let expanded = expanded.clone();
        Callback::from(move |_: MouseEvent| expanded.set(!*expanded))
    };

    let current = (*selected_theme).clone();
    let current_idx = THEMES.iter().position(|t| t.name == current.as_str());

    // Current theme first, then the rest in shuffled order.
    let ordered: Vec<usize> = if let Some(ci) = current_idx {
        let mut v = vec![ci];
        v.extend((*shuffled_order).iter().copied().filter(|&i| i != ci));
        v
    } else {
        (*shuffled_order).clone()
    };

    let visible: Vec<usize> = ordered.iter().take(3).copied().collect();
    let hidden: Vec<usize> = ordered.iter().skip(3).copied().collect();
    let hidden_count = hidden.len();

    let visible_cards: Html = {
        let current = current.clone();
        let on_select = on_select.clone();
        visible
            .into_iter()
            .map(move |i| theme_card(THEMES[i], &current, on_select.clone()))
            .collect()
    };

    let hidden_cards: Html = if *expanded {
        let current = current.clone();
        let on_select = on_select.clone();
        hidden
            .into_iter()
            .map(move |i| theme_card(THEMES[i], &current, on_select.clone()))
            .collect()
    } else {
        Html::default()
    };

    let custom_cards: Html = {
        let current = current.clone();
        let on_select_custom = on_select_custom.clone();
        let on_delete_custom = on_delete_custom.clone();
        (*custom_themes)
            .iter()
            .map(move |ct| {
                custom_theme_card(ct, &current, on_select_custom.clone(), on_delete_custom.clone())
            })
            .collect()
    };

    html! {
        <div class="p-6 space-y-4">
            if *loading {
                <div class="flex justify-center">
                    <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-gray-500"></div>
                </div>
            } else {
                <>
                    <div class="theme-card-grid">
                        { visible_cards }
                    </div>

                    if hidden_count > 0 {
                        <button class="theme-show-more-btn" onclick={on_toggle_expand}>
                            if *expanded {
                                <i class="ph ph-caret-up"></i>
                                {" Show fewer"}
                            } else {
                                <i class="ph ph-caret-down"></i>
                                {format!(" Show {} more", hidden_count)}
                            }
                        </button>
                    }

                    if *expanded {
                        <div class="theme-expanded-grid">
                            { hidden_cards }
                        </div>
                    }

                    if !(*custom_themes).is_empty() {
                        <>
                            <div class="custom-themes-divider">
                                <span>{"My Themes"}</span>
                            </div>
                            <div class="theme-card-grid">
                                { custom_cards }
                            </div>
                        </>
                    }
                </>
            }
        </div>
    }
}

fn theme_card(t: ThemeData, current: &str, on_select: Callback<String>) -> Html {
    let is_selected = t.name == current;
    let name = t.name.to_string();

    let shadow = if is_selected {
        format!("0 0 0 2px {}, 0 1px 4px rgba(0,0,0,.25)", t.swatch1)
    } else {
        "0 1px 2px rgba(0,0,0,.2)".to_string()
    };

    let card_style = format!(
        "background-color:{};border-radius:10px;padding:12px;min-height:74px;\
         position:relative;cursor:pointer;box-shadow:{};",
        t.bg, shadow
    );

    html! {
        <div
            style={card_style}
            class="theme-card-item"
            onclick={Callback::from(move |_: MouseEvent| on_select.emit(name.clone()))}
            role="button"
            tabindex="0"
        >
            if is_selected {
                <i
                    class="ph ph-check-circle"
                    style={format!("position:absolute;top:8px;right:8px;color:{};font-size:16px;", t.swatch1)}
                ></i>
            }
            <div style={format!("color:{};font-size:13px;font-weight:600;line-height:1.2;margin-bottom:6px;", t.text)}>
                {t.name}
            </div>
            <div style="display:flex;gap:4px;">
                <span style={format!("display:inline-block;width:18px;height:18px;border-radius:4px;background-color:{};", t.swatch1)}></span>
                <span style={format!("display:inline-block;width:18px;height:18px;border-radius:4px;background-color:{};opacity:0.6;", t.swatch2)}></span>
            </div>
        </div>
    }
}

fn custom_theme_card(
    ct: &CustomTheme,
    current: &str,
    on_select: Callback<(String, i32)>,
    on_delete: Callback<(i32, String)>,
) -> Html {
    let is_selected = ct.name == current;
    let name = ct.name.clone();
    let name_for_delete = ct.name.clone();
    let theme_id = ct.themeid;
    let bg = ct.background_color.clone();
    let text = ct.text_color.clone();
    let swatch1 = ct.prog_bar_color.clone();
    let swatch2 = ct.standout_color.clone();

    let shadow = if is_selected {
        format!("0 0 0 2px {}, 0 1px 4px rgba(0,0,0,.25)", swatch1)
    } else {
        "0 1px 2px rgba(0,0,0,.2)".to_string()
    };

    let card_style = format!(
        "background-color:{};border-radius:10px;padding:12px;min-height:74px;\
         position:relative;cursor:pointer;box-shadow:{};",
        bg, shadow
    );

    html! {
        <div
            style={card_style}
            class="theme-card-item"
            onclick={Callback::from(move |_: MouseEvent| on_select.emit((name.clone(), theme_id)))}
            role="button"
            tabindex="0"
        >
            // Delete button in top-left
            <button
                class="custom-theme-delete-btn"
                title="Delete theme"
                onclick={Callback::from(move |e: MouseEvent| {
                    e.stop_propagation();
                    on_delete.emit((theme_id, name_for_delete.clone()));
                })}
            >
                <i class="ph ph-trash"></i>
            </button>

            if is_selected {
                <i
                    class="ph ph-check-circle"
                    style={format!("position:absolute;top:8px;right:8px;color:{};font-size:16px;", swatch1)}
                ></i>
            }
            <div style={format!("color:{};font-size:13px;font-weight:600;line-height:1.2;margin-bottom:6px;padding-left:20px;", text)}>
                {ct.name.clone()}
            </div>
            <div style="display:flex;gap:4px;padding-left:20px;">
                <span style={format!("display:inline-block;width:18px;height:18px;border-radius:4px;background-color:{};", swatch1)}></span>
                <span style={format!("display:inline-block;width:18px;height:18px;border-radius:4px;background-color:{};opacity:0.6;", swatch2)}></span>
            </div>
        </div>
    }
}

#[wasm_bindgen(inline_js = "
    export function changeTheme(theme) {
        const root = document.documentElement;
        switch (theme) {
            case 'Light':
                root.style.setProperty('--background-color', '#f9f9f9');
                root.style.setProperty('--button-color', '#0099e1');
                root.style.setProperty('--container-button-color', 'transparent');
                root.style.setProperty('--button-text-color', '#24292e');
                root.style.setProperty('--text-color', '#4a4a4a');
                root.style.setProperty('--text-secondary-color', '#4a4a4a');
                root.style.setProperty('--border-color', '#4a4a4a');
                root.style.setProperty('--accent-color', '#969797');
                root.style.setProperty('--prog-bar-color', '#0099e1');
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#0099e1');
                root.style.setProperty('--secondary-background', '#f1f1f1');
                root.style.setProperty('--container-background', '#e8e8e8');
                root.style.setProperty('--standout-color', '#705697');
                root.style.setProperty('--hover-color', '#0099e1');
                root.style.setProperty('--link-color', '#0099e1');
                root.style.setProperty('--thumb-color', '#666673');
                root.style.setProperty('--unfilled-color', '#d4d6d7');
                root.style.setProperty('--check-box-color', '#000000');
                break;

            case 'Github Light':
                root.style.setProperty('--background-color', '#ffffff');
                root.style.setProperty('--button-color', '#54a3ff');
                root.style.setProperty('--container-button-color', 'transparent');
                root.style.setProperty('--button-text-color', '#24292e');
                root.style.setProperty('--text-color', '#70777e');
                root.style.setProperty('--text-secondary-color', '#707378');
                root.style.setProperty('--border-color', '#000000');
                root.style.setProperty('--accent-color', '#666d76');
                root.style.setProperty('--prog-bar-color', '#f1f2f3');
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#d3dbcd');
                root.style.setProperty('--secondary-background', '#24292e');
                root.style.setProperty('--container-background', '#fafbfc');
                root.style.setProperty('--standout-color', '#705697');
                root.style.setProperty('--hover-color', '#d5d0e2');
                root.style.setProperty('--link-color', '#6590fd');
                root.style.setProperty('--thumb-color', '#666673');
                root.style.setProperty('--unfilled-color', '#d4d6d7');
                root.style.setProperty('--check-box-color', '#000000');
                break;

            case 'Dark':
                root.style.setProperty('--background-color', '#2a2b33');
                root.style.setProperty('--button-color', '#303648');
                root.style.setProperty('--button-text-color', '#f6f5f4');
                root.style.setProperty('--text-color', '#f6f5f4');
                root.style.setProperty('--text-secondary-color', '#f6f5f4');
                root.style.setProperty('--border-color', '#000000');
                root.style.setProperty('--accent-color', '#4a535e');
                root.style.setProperty('--prog-bar-color', '#4a535e');
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#000000');
                root.style.setProperty('--secondary-background', '#32333b');
                root.style.setProperty('--container-background', '#1b1d1e');
                root.style.setProperty('--standout-color', '#797b85');
                root.style.setProperty('--hover-color', '#4b5563');
                root.style.setProperty('--link-color', '#6590fd');
                root.style.setProperty('--thumb-color', '#1a1c1d');
                root.style.setProperty('--unfilled-color', '#e5e5e5');
                root.style.setProperty('--check-box-color', '#ffffff');
                break;

            case 'Nordic Light':
                root.style.setProperty('--background-color', '#eceff4');
                root.style.setProperty('--button-color', '#d8dee9');
                root.style.setProperty('--button-text-color', '#696c00');
                root.style.setProperty('--text-color', '#656d76');
                root.style.setProperty('--text-secondary-color', '#9aa2aa');
                root.style.setProperty('--border-color', '#000000');
                root.style.setProperty('--accent-color', '#878d95');
                root.style.setProperty('--prog-bar-color', '#2984ce');
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#d8dee9');
                root.style.setProperty('--secondary-background', '#e5e9f0');
                root.style.setProperty('--container-background', '#d8dee9');
                root.style.setProperty('--standout-color', '#2f363d');
                root.style.setProperty('--hover-color', '#2a85cf');
                root.style.setProperty('--link-color', '#2a85cf');
                root.style.setProperty('--thumb-color', '#2984ce');
                root.style.setProperty('--unfilled-color', '#d4d6d7');
                root.style.setProperty('--check-box-color', '#000000');
                break;

            case 'Nordic':
                root.style.setProperty('--background-color', '#3C4252');
                root.style.setProperty('--button-color', '#3e4555');
                root.style.setProperty('--button-text-color', '#f6f5f4');
                root.style.setProperty('--text-color', '#f6f5f4');
                root.style.setProperty('--text-secondary-color', '#f6f5f4');
                root.style.setProperty('--border-color', '#000000');
                root.style.setProperty('--accent-color', '#6d747f');
                root.style.setProperty('--prog-bar-color', '#3550af');
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#000000');
                root.style.setProperty('--secondary-background', '#2e3440');
                root.style.setProperty('--container-background', '#2b2f3a');
                root.style.setProperty('--standout-color', '#6e8e92');
                root.style.setProperty('--hover-color', '#5d80aa');
                root.style.setProperty('--link-color', '#5d80aa');
                root.style.setProperty('--thumb-color', '#3550af');
                root.style.setProperty('--unfilled-color', '#d4d6d7');
                root.style.setProperty('--check-box-color', '#ffffff');
                break;

            case 'Abyss':
                root.style.setProperty('--background-color', '#000C18');
                root.style.setProperty('--button-color', '#303648');
                root.style.setProperty('--button-text-color', '#f6f5f4');
                root.style.setProperty('--text-color', '#f6f5f4');
                root.style.setProperty('--text-secondary-color', '#f6f5f4');
                root.style.setProperty('--border-color', '#000000');
                root.style.setProperty('--accent-color', '#838385');
                root.style.setProperty('--prog-bar-color', '#326fef');
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#000000');
                root.style.setProperty('--secondary-background', '#051336');
                root.style.setProperty('--container-background', '#061940');
                root.style.setProperty('--standout-color', '#000000');
                root.style.setProperty('--hover-color', '#152037');
                root.style.setProperty('--link-color', '#c8aa7d');
                root.style.setProperty('--thumb-color', '#326fef');
                root.style.setProperty('--unfilled-color', '#d4d6d7');
                root.style.setProperty('--check-box-color', '#ffffff');
                break;

            case 'Dracula':
                root.style.setProperty('--background-color', '#282A36');
                root.style.setProperty('--button-color', '#292e42');
                root.style.setProperty('--button-text-color', '#f6f5f4');
                root.style.setProperty('--text-color', '#f6f5f4');
                root.style.setProperty('--text-secondary-color', '#f6f5f4');
                root.style.setProperty('--border-color', '#000000');
                root.style.setProperty('--accent-color', '#727580');
                root.style.setProperty('--prog-bar-color', '#bd93f9');
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#000000');
                root.style.setProperty('--secondary-background', '#262626');
                root.style.setProperty('--container-background', '#191a21');
                root.style.setProperty('--standout-color', '#575a68');
                root.style.setProperty('--hover-color', '#4b5563');
                root.style.setProperty('--link-color', '#6590fd');
                root.style.setProperty('--thumb-color', '#bd93f9');
                root.style.setProperty('--unfilled-color', '#d4d6d7');
                root.style.setProperty('--check-box-color', '#ffffff');
                break;

            case 'Kimbie':
                root.style.setProperty('--background-color', '#221a0f');
                root.style.setProperty('--button-color', '#65533c');
                root.style.setProperty('--button-text-color', '#B1AD86');
                root.style.setProperty('--text-color', '#B1AD86');
                root.style.setProperty('--text-secondary-color', '#B1AD86');
                root.style.setProperty('--border-color', '#000000');
                root.style.setProperty('--accent-color', '#4a535e');
                root.style.setProperty('--prog-bar-color', '#ca9858');
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#221A1F');
                root.style.setProperty('--secondary-background', '#131510');
                root.style.setProperty('--container-background', '#362712');
                root.style.setProperty('--standout-color', '#B1AD86');
                root.style.setProperty('--hover-color', '#d3af86');
                root.style.setProperty('--link-color', '#f6f5f4');
                root.style.setProperty('--thumb-color', '#ca9858');
                root.style.setProperty('--unfilled-color', '#d4d6d7');
                root.style.setProperty('--check-box-color', '#b1ad86');
                break;

            case 'Neon':
                root.style.setProperty('--background-color', '#120e16');
                root.style.setProperty('--button-color', '#303648');
                root.style.setProperty('--button-text-color', '#af565f');
                root.style.setProperty('--text-color', '#9F9DA1');
                root.style.setProperty('--text-secondary-color', '#92bb75');
                root.style.setProperty('--border-color', '#000000');
                root.style.setProperty('--accent-color', '#4a535e');
                root.style.setProperty('--prog-bar-color', '#f75c1d');
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#1a171e');
                root.style.setProperty('--secondary-background', '#120e16');
                root.style.setProperty('--container-background', '#1a171e');
                root.style.setProperty('--standout-color', '#797b85');
                root.style.setProperty('--hover-color', '#7000ff');
                root.style.setProperty('--link-color', '#7000ff');
                root.style.setProperty('--thumb-color', '#f75c1d');
                root.style.setProperty('--unfilled-color', '#d4d6d7');
                root.style.setProperty('--check-box-color', '#8a888c');
                break;

            case 'Greenie Meanie':
                root.style.setProperty('--background-color', '#142e28');
                root.style.setProperty('--button-color', '#489D50');
                root.style.setProperty('--button-text-color', '#f6f5f4');
                root.style.setProperty('--text-color', '#489D50');
                root.style.setProperty('--text-secondary-color', '#489D50');
                root.style.setProperty('--border-color', '#000000');
                root.style.setProperty('--accent-color', '#446448');
                root.style.setProperty('--prog-bar-color', '#224e44');
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#1a3c35');
                root.style.setProperty('--secondary-background', '#292A2E');
                root.style.setProperty('--container-background', '#292A2E');
                root.style.setProperty('--standout-color', '#797b85');
                root.style.setProperty('--hover-color', '#4b5563');
                root.style.setProperty('--link-color', '#6590fd');
                root.style.setProperty('--thumb-color', '#666673');
                root.style.setProperty('--unfilled-color', '#d4d6d7');
                root.style.setProperty('--check-box-color', '#489d50');
                break;

            case 'Gruvbox Light':
                root.style.setProperty('--background-color', '#f9f5d7');
                root.style.setProperty('--button-color', '#aca289');
                root.style.setProperty('--button-text-color', '#5f5750');
                root.style.setProperty('--text-color', '#5f5750');
                root.style.setProperty('--text-secondary-color', '#aca289');
                root.style.setProperty('--border-color', '#000000');
                root.style.setProperty('--accent-color', '#e0dbb2');
                root.style.setProperty('--prog-bar-color', '#d1ac0e');
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#f2e5bc');
                root.style.setProperty('--secondary-background', '#fbf1c7');
                root.style.setProperty('--container-background', '#fbf1c7');
                root.style.setProperty('--standout-color', '#797b85');
                root.style.setProperty('--hover-color', '#cfd2a8');
                root.style.setProperty('--link-color', '#a68738');
                root.style.setProperty('--thumb-color', '#d1ac0e');
                root.style.setProperty('--unfilled-color', '#d4d6d7');
                root.style.setProperty('--check-box-color', '#5f5750');
                break;

            case 'Gruvbox Dark':
                root.style.setProperty('--background-color', '#32302f');
                root.style.setProperty('--button-color', '#303648');
                root.style.setProperty('--button-text-color', '#868729');
                root.style.setProperty('--text-color', '#868729');
                root.style.setProperty('--text-secondary-color', '#868729');
                root.style.setProperty('--border-color', '#000000');
                root.style.setProperty('--accent-color', '#ebdbb2');
                root.style.setProperty('--prog-bar-color', '#424314');
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#363332');
                root.style.setProperty('--secondary-background', '#282828');
                root.style.setProperty('--container-background', '#302e2e');
                root.style.setProperty('--standout-color', '#ebdbb2');
                root.style.setProperty('--hover-color', '#59544a');
                root.style.setProperty('--link-color', '#6f701b');
                root.style.setProperty('--thumb-color', '#424314');
                root.style.setProperty('--unfilled-color', '#d4d6d7');
                root.style.setProperty('--check-box-color', '#868729');
                break;

            case 'Wildberries':
                root.style.setProperty('--background-color', '#240041');
                root.style.setProperty('--button-color', '#3a264a');
                root.style.setProperty('--button-text-color', '#F55385');
                root.style.setProperty('--text-color', '#CF8B3E');
                root.style.setProperty('--text-secondary-color', '#CF8B3E');
                root.style.setProperty('--border-color', '#000000');
                root.style.setProperty('--accent-color', '#C79BFF');
                root.style.setProperty('--prog-bar-color', '#4b246b');
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#44433A');
                root.style.setProperty('--secondary-background', '#19002E');
                root.style.setProperty('--container-background', '#19002E');
                root.style.setProperty('--standout-color', '#00FFB7');
                root.style.setProperty('--hover-color', '#44433A');
                root.style.setProperty('--link-color', '#5196B2');
                root.style.setProperty('--thumb-color', '#666673');
                root.style.setProperty('--unfilled-color', '#d4d6d7');
                root.style.setProperty('--check-box-color', '#cf8b3e');
                break;

            case 'Midnight Ocean':
                root.style.setProperty('--background-color', '#0f172a');
                root.style.setProperty('--button-color', '#1e293b');
                root.style.setProperty('--button-text-color', '#38bdf8');
                root.style.setProperty('--text-color', '#e2e8f0');
                root.style.setProperty('--text-secondary-color', '#94a3b8');
                root.style.setProperty('--border-color', '#1e293b');
                root.style.setProperty('--accent-color', '#38bdf8');
                root.style.setProperty('--prog-bar-color', '#0ea5e9');
                root.style.setProperty('--error-color', '#ef4444');
                root.style.setProperty('--bonus-color', '#0f172a');
                root.style.setProperty('--secondary-background', '#1e293b');
                root.style.setProperty('--container-background', '#1e293b');
                root.style.setProperty('--standout-color', '#38bdf8');
                root.style.setProperty('--hover-color', '#0ea5e9');
                root.style.setProperty('--link-color', '#60a5fa');
                root.style.setProperty('--thumb-color', '#38bdf8');
                root.style.setProperty('--unfilled-color', '#334155');
                root.style.setProperty('--check-box-color', '#ffffff');
                break;

            case 'Forest Depths':
                root.style.setProperty('--background-color', '#1a2f1f');
                root.style.setProperty('--button-color', '#2d4a33');
                root.style.setProperty('--button-text-color', '#7fb685');
                root.style.setProperty('--text-color', '#c9e4ca');
                root.style.setProperty('--text-secondary-color', '#8fbb91');
                root.style.setProperty('--border-color', '#2d4a33');
                root.style.setProperty('--accent-color', '#7fb685');
                root.style.setProperty('--prog-bar-color', '#5c8b61');
                root.style.setProperty('--error-color', '#e67c73');
                root.style.setProperty('--bonus-color', '#1a2f1f');
                root.style.setProperty('--secondary-background', '#2d4a33');
                root.style.setProperty('--container-background', '#2d4a33');
                root.style.setProperty('--standout-color', '#7fb685');
                root.style.setProperty('--hover-color', '#5c8b61');
                root.style.setProperty('--link-color', '#a1d0a5');
                root.style.setProperty('--thumb-color', '#7fb685');
                root.style.setProperty('--unfilled-color', '#3d5a43');
                root.style.setProperty('--check-box-color', '#c9e4ca');
                break;

            case 'Sunset Horizon':
                root.style.setProperty('--background-color', '#2b1c2c');
                root.style.setProperty('--button-color', '#432e44');
                root.style.setProperty('--button-text-color', '#ff9e64');
                root.style.setProperty('--text-color', '#ffd9c0');
                root.style.setProperty('--text-secondary-color', '#d4a5a5');
                root.style.setProperty('--border-color', '#432e44');
                root.style.setProperty('--accent-color', '#ff9e64');
                root.style.setProperty('--prog-bar-color', '#e8875c');
                root.style.setProperty('--error-color', '#ff6b6b');
                root.style.setProperty('--bonus-color', '#2b1c2c');
                root.style.setProperty('--secondary-background', '#432e44');
                root.style.setProperty('--container-background', '#432e44');
                root.style.setProperty('--standout-color', '#ff9e64');
                root.style.setProperty('--hover-color', '#e8875c');
                root.style.setProperty('--link-color', '#ffb088');
                root.style.setProperty('--thumb-color', '#ff9e64');
                root.style.setProperty('--unfilled-color', '#533a54');
                root.style.setProperty('--check-box-color', '#ffd9c0');
                break;

            case 'Arctic Frost':
                root.style.setProperty('--background-color', '#1a1d21');
                root.style.setProperty('--button-color', '#2a2f36');
                root.style.setProperty('--button-text-color', '#88c0d0');
                root.style.setProperty('--text-color', '#eceff4');
                root.style.setProperty('--text-secondary-color', '#aeb3bb');
                root.style.setProperty('--border-color', '#2a2f36');
                root.style.setProperty('--accent-color', '#88c0d0');
                root.style.setProperty('--prog-bar-color', '#5e81ac');
                root.style.setProperty('--error-color', '#bf616a');
                root.style.setProperty('--bonus-color', '#1a1d21');
                root.style.setProperty('--secondary-background', '#2a2f36');
                root.style.setProperty('--container-background', '#2a2f36');
                root.style.setProperty('--standout-color', '#88c0d0');
                root.style.setProperty('--hover-color', '#5e81ac');
                root.style.setProperty('--link-color', '#81a1c1');
                root.style.setProperty('--thumb-color', '#88c0d0');
                root.style.setProperty('--unfilled-color', '#3b4252');
                root.style.setProperty('--check-box-color', '#eceff4');
                break;

            case 'Cyber Synthwave':
                root.style.setProperty('--background-color', '#1a1721');
                root.style.setProperty('--button-color', '#2a1f3a');
                root.style.setProperty('--button-text-color', '#f92aad');
                root.style.setProperty('--text-color', '#eee6ff');
                root.style.setProperty('--text-secondary-color', '#c3b7d9');
                root.style.setProperty('--border-color', '#2a1f3a');
                root.style.setProperty('--accent-color', '#f92aad');
                root.style.setProperty('--prog-bar-color', '#b31777');
                root.style.setProperty('--error-color', '#ff2e63');
                root.style.setProperty('--bonus-color', '#1a1721');
                root.style.setProperty('--secondary-background', '#2a1f3a');
                root.style.setProperty('--container-background', '#2a1f3a');
                root.style.setProperty('--standout-color', '#f92aad');
                root.style.setProperty('--hover-color', '#b31777');
                root.style.setProperty('--link-color', '#ff71ce');
                root.style.setProperty('--thumb-color', '#f92aad');
                root.style.setProperty('--unfilled-color', '#3a2f4a');
                root.style.setProperty('--check-box-color', '#eee6ff');
                break;

            case 'Hot Dog Stand - MY EYES':
                root.style.setProperty('--background-color', '#670b0a');
                root.style.setProperty('--button-color', '#730B1B');
                root.style.setProperty('--button-text-color', '#121215');
                root.style.setProperty('--text-color', '#121215');
                root.style.setProperty('--text-secondary-color', '#D5BC5C');
                root.style.setProperty('--border-color', '#000000');
                root.style.setProperty('--accent-color', '#D5BC5C');
                root.style.setProperty('--prog-bar-color', '#D5BC5C');
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#D5BC5C');
                root.style.setProperty('--secondary-background', '#EEB911');
                root.style.setProperty('--container-background', '#C3590D');
                root.style.setProperty('--standout-color', '#797b85');
                root.style.setProperty('--hover-color', '#4b5563');
                root.style.setProperty('--link-color', '#6590fd');
                root.style.setProperty('--thumb-color', '#666673');
                root.style.setProperty('--unfilled-color', '#d4d6d7');
                root.style.setProperty('--check-box-color', '#000000');
                break;

            case 'Catppuccin Mocha Mauve':
                root.style.setProperty('--background-color', '#1e1e2e');
                root.style.setProperty('--button-color', '#313244');
                root.style.setProperty('--button-text-color', '#a6adc8');
                root.style.setProperty('--text-color', '#cdd6f4');
                root.style.setProperty('--text-secondary-color', '#bac2de');
                root.style.setProperty('--border-color', '#cba6f7');
                root.style.setProperty('--accent-color', '#cba6f7');
                root.style.setProperty('--prog-bar-color', '#a6e3a1');
                root.style.setProperty('--error-color', '#f38ba8');
                root.style.setProperty('--bonus-color', '#45475a');
                root.style.setProperty('--secondary-background', '#11111b');
                root.style.setProperty('--container-background', '#313244');
                root.style.setProperty('--standout-color', '#89b4fa');
                root.style.setProperty('--hover-color', '#6c7086');
                root.style.setProperty('--link-color', '#f5c2e7');
                root.style.setProperty('--thumb-color', '#b4befe');
                root.style.setProperty('--unfilled-color', '#74c7ec');
                root.style.setProperty('--check-box-color', '#cdd6f4');
                break;

            case 'Soft Lavender':
                root.style.setProperty('--background-color', '#f5f0ff');
                root.style.setProperty('--button-color', '#e0d5f5');
                root.style.setProperty('--container-button-color', 'transparent');
                root.style.setProperty('--button-text-color', '#614b8f');
                root.style.setProperty('--text-color', '#50456e');
                root.style.setProperty('--text-secondary-color', '#7c6a99');
                root.style.setProperty('--border-color', '#bdb4d1');
                root.style.setProperty('--accent-color', '#c9bcee');
                root.style.setProperty('--prog-bar-color', '#9371d9');
                root.style.setProperty('--error-color', '#e26295');
                root.style.setProperty('--bonus-color', '#a990e5');
                root.style.setProperty('--secondary-background', '#f0eaf8');
                root.style.setProperty('--container-background', '#e8e1f7');
                root.style.setProperty('--standout-color', '#8661c5');
                root.style.setProperty('--hover-color', '#a47ee9');
                root.style.setProperty('--link-color', '#7a58bf');
                root.style.setProperty('--thumb-color', '#9371d9');
                root.style.setProperty('--unfilled-color', '#d8d1e8');
                root.style.setProperty('--check-box-color', '#50456e');
                break;

            case 'Minty Fresh':
                root.style.setProperty('--background-color', '#f1f9f6');
                root.style.setProperty('--button-color', '#d9efe7');
                root.style.setProperty('--container-button-color', 'transparent');
                root.style.setProperty('--button-text-color', '#2d6e5b');
                root.style.setProperty('--text-color', '#2d6e5b');
                root.style.setProperty('--text-secondary-color', '#5ba192');
                root.style.setProperty('--border-color', '#b0d9cb');
                root.style.setProperty('--accent-color', '#85c2b0');
                root.style.setProperty('--prog-bar-color', '#3d9d82');
                root.style.setProperty('--error-color', '#e77670');
                root.style.setProperty('--bonus-color', '#65b7a1');
                root.style.setProperty('--secondary-background', '#e7f6f1');
                root.style.setProperty('--container-background', '#ddf0e8');
                root.style.setProperty('--standout-color', '#25b78f');
                root.style.setProperty('--hover-color', '#4dab92');
                root.style.setProperty('--link-color', '#2d9278');
                root.style.setProperty('--thumb-color', '#3d9d82');
                root.style.setProperty('--unfilled-color', '#c9e6dd');
                root.style.setProperty('--check-box-color', '#2d6e5b');
                break;

            case 'Warm Vanilla':
                root.style.setProperty('--background-color', '#fdf6e9');
                root.style.setProperty('--button-color', '#f2e3ca');
                root.style.setProperty('--container-button-color', 'transparent');
                root.style.setProperty('--button-text-color', '#865d30');
                root.style.setProperty('--text-color', '#6d4922');
                root.style.setProperty('--text-secondary-color', '#a08052');
                root.style.setProperty('--border-color', '#d8c7a7');
                root.style.setProperty('--accent-color', '#e6d1ac');
                root.style.setProperty('--prog-bar-color', '#c6a06d');
                root.style.setProperty('--error-color', '#d9684c');
                root.style.setProperty('--bonus-color', '#d9b77e');
                root.style.setProperty('--secondary-background', '#f8eede');
                root.style.setProperty('--container-background', '#f5e7d1');
                root.style.setProperty('--standout-color', '#b88c48');
                root.style.setProperty('--hover-color', '#d9b165');
                root.style.setProperty('--link-color', '#a17035');
                root.style.setProperty('--thumb-color', '#c6a06d');
                root.style.setProperty('--unfilled-color', '#e9dbc5');
                root.style.setProperty('--check-box-color', '#6d4922');
                break;

            case 'Coastal Blue':
                root.style.setProperty('--background-color', '#f0f5fa');
                root.style.setProperty('--button-color', '#dde9f3');
                root.style.setProperty('--container-button-color', 'transparent');
                root.style.setProperty('--button-text-color', '#2c5d8f');
                root.style.setProperty('--text-color', '#2c5d8f');
                root.style.setProperty('--text-secondary-color', '#5c89b7');
                root.style.setProperty('--border-color', '#b0cde3');
                root.style.setProperty('--accent-color', '#8cb0d1');
                root.style.setProperty('--prog-bar-color', '#4c87c5');
                root.style.setProperty('--error-color', '#e86f6f');
                root.style.setProperty('--bonus-color', '#71a0cc');
                root.style.setProperty('--secondary-background', '#e8f0f8');
                root.style.setProperty('--container-background', '#dee8f3');
                root.style.setProperty('--standout-color', '#2e78af');
                root.style.setProperty('--hover-color', '#5992ca');
                root.style.setProperty('--link-color', '#2b6fb0');
                root.style.setProperty('--thumb-color', '#4c87c5');
                root.style.setProperty('--unfilled-color', '#cde0ee');
                root.style.setProperty('--check-box-color', '#2c5d8f');
                break;

            case 'Paper Cream':
                root.style.setProperty('--background-color', '#faf7f2');
                root.style.setProperty('--button-color', '#ede9e1');
                root.style.setProperty('--container-button-color', 'transparent');
                root.style.setProperty('--button-text-color', '#5f584d');
                root.style.setProperty('--text-color', '#4a4439');
                root.style.setProperty('--text-secondary-color', '#847f74');
                root.style.setProperty('--border-color', '#d3cec3');
                root.style.setProperty('--accent-color', '#d8d0c0');
                root.style.setProperty('--prog-bar-color', '#a19788');
                root.style.setProperty('--error-color', '#d16c62');
                root.style.setProperty('--bonus-color', '#c1b8a3');
                root.style.setProperty('--secondary-background', '#f5f2ec');
                root.style.setProperty('--container-background', '#eee9e0');
                root.style.setProperty('--standout-color', '#847b6a');
                root.style.setProperty('--hover-color', '#b3a894');
                root.style.setProperty('--link-color', '#7d725f');
                root.style.setProperty('--thumb-color', '#a19788');
                root.style.setProperty('--unfilled-color', '#e3dfd5');
                root.style.setProperty('--check-box-color', '#4a4439');
                break;

            default:
                break;
        }
    }
")]
extern "C" {
    pub fn changeTheme(theme: &str);
}

#[wasm_bindgen(inline_js = "
    export function applyCustomTheme(bg, btn, containerBtn, btnText, text, textSec,
        border, accent, progBar, error, bonus, secBg, containerBg,
        standout, hover, link, thumb, unfilled, checkbox) {
        const root = document.documentElement;
        root.style.setProperty('--background-color', bg);
        root.style.setProperty('--button-color', btn);
        root.style.setProperty('--container-button-color', containerBtn);
        root.style.setProperty('--button-text-color', btnText);
        root.style.setProperty('--text-color', text);
        root.style.setProperty('--text-secondary-color', textSec);
        root.style.setProperty('--border-color', border);
        root.style.setProperty('--accent-color', accent);
        root.style.setProperty('--prog-bar-color', progBar);
        root.style.setProperty('--error-color', error);
        root.style.setProperty('--bonus-color', bonus);
        root.style.setProperty('--secondary-background', secBg);
        root.style.setProperty('--container-background', containerBg);
        root.style.setProperty('--standout-color', standout);
        root.style.setProperty('--hover-color', hover);
        root.style.setProperty('--link-color', link);
        root.style.setProperty('--thumb-color', thumb);
        root.style.setProperty('--unfilled-color', unfilled);
        root.style.setProperty('--check-box-color', checkbox);
    }
")]
extern "C" {
    pub fn applyCustomTheme(
        bg: &str,
        btn: &str,
        container_btn: &str,
        btn_text: &str,
        text: &str,
        text_sec: &str,
        border: &str,
        accent: &str,
        prog_bar: &str,
        error: &str,
        bonus: &str,
        sec_bg: &str,
        container_bg: &str,
        standout: &str,
        hover: &str,
        link: &str,
        thumb: &str,
        unfilled: &str,
        checkbox: &str,
    );
}

pub fn initialize_default_theme() {
    if let Some(window) = window() {
        if let Ok(Some(storage)) = window.local_storage() {
            match storage.get_item("selected_theme") {
                Ok(Some(theme)) => {
                    changeTheme(&theme);
                }
                _ => {
                    storage
                        .set_item("selected_theme", "Nordic")
                        .unwrap_or_default();
                    changeTheme("Nordic");
                }
            }
        }
    }
}
