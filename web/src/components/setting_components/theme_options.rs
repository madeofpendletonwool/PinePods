use crate::components::context::{AppState, UIState};
use crate::requests::setting_reqs::{call_get_theme, call_set_theme, SetThemeRequest};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::window;
use web_sys::HtmlSelectElement;
use yew::prelude::*;
use yewdux::prelude::*;

#[function_component(ThemeOptions)]
pub fn theme() -> Html {
    let (state, _dispatch) = use_store::<AppState>();
    let (_audio_state, audio_dispatch) = use_store::<UIState>();
    // Use state to manage the selected theme
    let selected_theme = use_state(|| "".to_string());
    let loading = use_state(|| true);
    // let selected_theme = state.selected_theme.as_ref();

    {
        let selected_theme = selected_theme.clone();
        let loading = loading.clone();
        let state = state.clone();

        use_effect_with((), move |_| {
            let selected_theme = selected_theme.clone();
            let loading = loading.clone();

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

    let on_change = {
        let selected_theme = selected_theme.clone();
        Callback::from(move |e: Event| {
            if let Some(select) = e.target_dyn_into::<HtmlSelectElement>() {
                selected_theme.set(select.value());
            }
        })
    };

    let on_submit = {
        let selected_theme = selected_theme.clone();
        let state = state.clone();

        Callback::from(move |_| {
            let audio_dispatch = audio_dispatch.clone();
            let theme = (*selected_theme).clone();

            if theme.is_empty() {
                return;
            }

            // Call JavaScript theme change function
            {
                changeTheme(&theme)
            };

            // Store in local storage
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    let _ = storage.set_item("selected_theme", &theme);
                }
            }

            // Update server
            if let (Some(api_key), Some(user_id), Some(server_name)) = (
                state.auth_details.as_ref().and_then(|d| d.api_key.clone()),
                state.user_details.as_ref().map(|d| d.UserID),
                state.auth_details.as_ref().map(|d| d.server_name.clone()),
            ) {
                let request = SetThemeRequest {
                    user_id,
                    new_theme: theme.clone(),
                };

                spawn_local(async move {
                    match call_set_theme(&Some(server_name), &Some(api_key), &request).await {
                        Ok(_) => {}
                        Err(e) => {
                            audio_dispatch.reduce_mut(|state| {
                                state.error_message =
                                    Some(format!("Failed to update theme: {}", e));
                            });
                        }
                    }
                });
            }
        })
    };

    let theme_options = vec![
        "Light",
        "Dark",
        "Nordic Light",
        "Nordic",
        "Abyss",
        "Dracula",
        "Midnight Ocean",
        "Forest Depths",
        "Sunset Horizon",
        "Arctic Frost",
        "Cyber Synthwave",
        "Github Light",
        "Neon",
        "Kimbie",
        "Gruvbox Light",
        "Gruvbox Dark",
        "Greenie Meanie",
        "Wildberries",
        "Hot Dog Stand - MY EYES",
        "Catppuccin Mocha Mauve",
    ];

    html! {
        <div class="p-6 space-y-4">
            <div class="flex items-center gap-3 mb-6">
                <i class="ph ph-paint-roller text-2xl"></i>
                <h2 class="text-xl font-semibold item_container-text">{"Theme Settings"}</h2>
            </div>

            <div class="mb-6">
                <p class="item_container-text mb-2">
                    {"Choose your preferred theme. Your selection will sync across all your Pinepods applications."}
                </p>
            </div>

            if *loading {
                <div class="flex justify-center">
                    <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-gray-500"></div>
                </div>
            } else {
                <div class="theme-select-container relative">
                    <select
                        onchange={on_change}
                        class="theme-select-dropdown w-full p-3 pr-10 rounded-lg border appearance-none cursor-pointer"
                        value={(*selected_theme).clone()}
                    >
                        <option value="" disabled=true>{"Select a theme"}</option>
                        {theme_options.into_iter().map(|theme| {
                            let current_theme = (*selected_theme).clone();
                            html! {
                                <option value={theme} selected={theme == current_theme}>
                                    {theme}
                                </option>
                            }
                        }).collect::<Html>()}
                    </select>
                    <div class="absolute inset-y-0 right-0 flex items-center px-3 pointer-events-none">
                        <i class="ph ph-caret-down text-2xl"></i>
                    </div>
                </div>

                <button
                    onclick={on_submit}
                    class="theme-submit-button mt-4 w-full p-3 rounded-lg transition-colors duration-200 flex items-center justify-center gap-2"
                >
                    <i class="ph ph-thumbs-up text-2xl"></i>
                    {"Apply Theme"}
                </button>
            }
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
                root.style.setProperty('--bonus-color', '#000000'); // Assuming black as bonus color
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
                root.style.setProperty('--bonus-color', '#d8dee9'); // Assuming black as bonus color
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
                root.style.setProperty('--bonus-color', '#000000'); // Assuming black as bonus color
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
                root.style.setProperty('--bonus-color', '#000000'); // Assuming black as bonus color
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
                root.style.setProperty('--bonus-color', '#000000'); // Assuming black as bonus color
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
                root.style.setProperty('--bonus-color', '#221A1F'); // Assuming black as bonus color
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
                root.style.setProperty('--bonus-color', '#1a171e'); // Assuming black as bonus color
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
                root.style.setProperty('--bonus-color', '#1a3c35'); // Assuming black as bonus color
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
                root.style.setProperty('--bonus-color', '#f2e5bc'); // Assuming black as bonus color
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
                root.style.setProperty('--bonus-color', '#363332'); // Assuming black as bonus color
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
                root.style.setProperty('--bonus-color', '#44433A'); // Assuming black as bonus color
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
                root.style.setProperty('--bonus-color', '#0f172a'); // Assuming black as bonus color
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
                root.style.setProperty('--bonus-color', '#D5BC5C'); // Assuming black as bonus color
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

            default:
                // Reset to default (perhaps the Light or Dark theme)
                break;
        }
    }
")]
extern "C" {
    pub fn changeTheme(theme: &str);
}

pub fn initialize_default_theme() {
    if let Some(window) = window() {
        if let Ok(Some(storage)) = window.local_storage() {
            // Check if a theme is already set
            match storage.get_item("selected_theme") {
                Ok(Some(theme)) => {
                    // Use existing theme
                    changeTheme(&theme);
                }
                _ => {
                    // No theme found, set Nordic as default
                    storage
                        .set_item("selected_theme", "Nordic")
                        .unwrap_or_default();
                    changeTheme("Nordic");
                }
            }
        }
    }
}
