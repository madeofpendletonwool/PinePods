use web_sys::{Element, HtmlSelectElement};
use yew::prelude::*;
use yewdux::prelude::*;
use crate::components::context::AppState;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::window;
use crate::requests::setting_reqs::{call_set_theme, SetThemeRequest};
use web_sys::console;

#[function_component(ThemeOptions)]
pub fn theme() -> Html {
    let (state, _dispatch) = use_store::<AppState>();
    // Use state to manage the selected theme
    let selected_theme = use_state(|| "Light".to_string());
    // let selected_theme = state.selected_theme.as_ref();


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
            let theme = (*selected_theme).to_string();
            web_sys::console::log_1(&format!("Submitting theme: {}", theme).into());
            changeTheme(&theme);
            if let Some(window) = web_sys::window() {
                if let Ok(Some(local_storage)) = window.local_storage() {
                    match local_storage.set_item("selected_theme", &theme) {
                        Ok(_) => console::log_1(&"Updated theme in local storage".into()),
                        Err(e) => console::log_1(&format!("Error updating theme in local storage: {:?}", e).into()),
                    }
                }
            }
            log_css_variables();

            // Optionally, store in local storage
            if let Some(window) = window() {
                let _ = window.local_storage().unwrap().unwrap().set_item("theme", &theme);
            }

            let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone()).flatten().unwrap();
            let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone()).unwrap();
            let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());

            let request = SetThemeRequest {
                user_id,
                new_theme: theme.clone(),
            };

            spawn_local(async move {
                if let Ok(_) = call_set_theme(&server_name, &Some(api_key), &request).await {
                    web_sys::console::log_1(&"Theme updated successfully".into());
                } else {
                    web_sys::console::log_1(&"Error updating theme".into());
                }
            });
        })
    };

    html! {
        <div class="p-4"> // You can adjust the padding as needed
            <p class="item_container-text text-lg font-bold mb-4">{"Theme Select:"}</p> // Styled paragraph
            <p class="item_container-text text-md mb-4">{"You can select your application theme here. Choosing a theme will follow you to any official Pinepods application as your theme preference gets saved to your user settings."}</p> // Styled paragraph

            <div class="theme-select-dropdown relative inline-block">
                <select onchange={on_change} class="theme-select-dropdown appearance-none w-full border px-4 py-2 pr-8 rounded shadow leading-tight focus:outline-none focus:shadow-outline">
                    <option value="Light" selected={(*selected_theme) == "Light"}>{"Light"}</option>
                    <option value="Dark" selected={(*selected_theme) == "Dark"}>{"Dark"}</option>
                    <option value="Github Light" selected={(*selected_theme) == "Github Light"}>{"Github Light"}</option>   
                    <option value="Nordic Light" selected={(*selected_theme) == "Nordic Light"}>{"Nordic Light"}</option>                                     
                    <option value="Nordic" selected={(*selected_theme) == "Nordic"}>{"Nordic"}</option>
                    <option value="Abyss" selected={(*selected_theme) == "Abyss"}>{"Abyss"}</option>
                    <option value="Dracula" selected={(*selected_theme) == "Dracula"}>{"Dracula"}</option>
                    <option value="Neon" selected={(*selected_theme) == "Neon"}>{"Neon"}</option>
                    <option value="Kimbie" selected={(*selected_theme) == "Kimbie"}>{"Kimbie"}</option>
                    <option value="Gruvbox Light" selected={(*selected_theme) == "Gruvbox Light"}>{"Gruvbox Light"}</option>
                    <option value="Gruvbox Dark" selected={(*selected_theme) == "Gruvbox Dark"}>{"Gruvbox Dark"}</option>
                    <option value="Greenie Meanie" selected={(*selected_theme) == "Greenie Meanie"}>{"Greenie Meanie"}</option>
                    <option value="Wildberries" selected={(*selected_theme) == "Wildberries"}>{"Wildberries"}</option>
                    <option value="Hot Dog Stand - MY EYES" selected={(*selected_theme) == "Hot Dog Stand - MY EYES"}>{"Hot Dog Stand - MY EYES"}</option>
                </select>
                <div class="theme-dropdown-arrow pointer-events-none absolute inset-y-0 right-0 flex items-center px-2">
                    <svg class="fill-current h-4 w-4" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><path d="M6.293 9.293a1 1 0 0 1 1.414 0L10 10.586l2.293-2.293a1 1 0 1 1 1.414 1.414l-3 3a1 1 0 0 1-1.414 0l-3-3a1 1 0 0 1 0-1.414z"/></svg>
                </div>
            </div>

            <button onclick={on_submit} class="theme-submit-button mt-4 font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" type="button">
                {"Submit"}
            </button>
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
                root.style.setProperty('--text-secondary-color', '#ababab');
                root.style.setProperty('--border-color', '#4a4a4a');
                root.style.setProperty('--accent-color', '#969797');
                root.style.setProperty('--prog-bar-color', '#d5d7d8');
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#0099e1');
                root.style.setProperty('--secondary-background', '#f1f1f1');
                root.style.setProperty('--container-background', '#e8e8e8');
                root.style.setProperty('--standout-color', '#705697');
                root.style.setProperty('--hover-color', '#0099e1');
                root.style.setProperty('--link-color', '#0099e1');
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
                break;

            case 'Nordic Light':
                root.style.setProperty('--background-color', '#eceff4');
                root.style.setProperty('--button-color', '#d8dee9');
                root.style.setProperty('--button-text-color', '#696c00');
                root.style.setProperty('--text-color', '#656d76');
                root.style.setProperty('--text-secondary-color', '#9aa2aa');
                root.style.setProperty('--border-color', '#000000');
                root.style.setProperty('--accent-color', '#878d95');
                root.style.setProperty('--prog-bar-color', '#cbdbf0');
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#d8dee9'); // Assuming black as bonus color
                root.style.setProperty('--secondary-background', '#e5e9f0');
                root.style.setProperty('--container-background', '#d8dee9');
                root.style.setProperty('--standout-color', '#2f363d');
                root.style.setProperty('--hover-color', '#2a85cf');
                root.style.setProperty('--link-color', '#2a85cf');
                break;

            case 'Nordic':
                root.style.setProperty('--background-color', '#3C4252');
                root.style.setProperty('--button-color', '#3e4555');
                root.style.setProperty('--button-text-color', '#f6f5f4');
                root.style.setProperty('--text-color', '#f6f5f4');
                root.style.setProperty('--text-secondary-color', '#f6f5f4');
                root.style.setProperty('--border-color', '#000000');
                root.style.setProperty('--accent-color', '#6d747f');
                root.style.setProperty('--prog-bar-color', '#323542');
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#000000'); // Assuming black as bonus color
                root.style.setProperty('--secondary-background', '#2e3440');
                root.style.setProperty('--container-background', '#2b2f3a');
                root.style.setProperty('--standout-color', '#6e8e92');
                root.style.setProperty('--hover-color', '#5d80aa');
                root.style.setProperty('--link-color', '#5d80aa');
                break;

            case 'Abyss':
                root.style.setProperty('--background-color', '#000C18');
                root.style.setProperty('--button-color', '#303648');
                root.style.setProperty('--button-text-color', '#f6f5f4');
                root.style.setProperty('--text-color', '#f6f5f4');
                root.style.setProperty('--text-secondary-color', '#f6f5f4');
                root.style.setProperty('--border-color', '#000000');
                root.style.setProperty('--accent-color', '#838385');
                root.style.setProperty('--prog-bar-color', '#051336');
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#000000'); // Assuming black as bonus color
                root.style.setProperty('--secondary-background', '#051336');
                root.style.setProperty('--container-background', '#061940');
                root.style.setProperty('--standout-color', '#000000');
                root.style.setProperty('--hover-color', '#152037');
                root.style.setProperty('--link-color', '#c8aa7d');
                break;

            case 'Dracula':
                root.style.setProperty('--background-color', '#282A36');
                root.style.setProperty('--button-color', '#292e42');
                root.style.setProperty('--button-text-color', '#f6f5f4');
                root.style.setProperty('--text-color', '#f6f5f4');
                root.style.setProperty('--text-secondary-color', '#f6f5f4');
                root.style.setProperty('--border-color', '#000000');
                root.style.setProperty('--accent-color', '#727580');
                root.style.setProperty('--prog-bar-color', '#282a36');
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#000000'); // Assuming black as bonus color
                root.style.setProperty('--secondary-background', '#262626');
                root.style.setProperty('--container-background', '#191a21');
                root.style.setProperty('--standout-color', '#575a68');
                root.style.setProperty('--hover-color', '#4b5563');
                root.style.setProperty('--link-color', '#6590fd');
                break;

            case 'Kimbie':
                root.style.setProperty('--background-color', '#221a0f');
                root.style.setProperty('--button-color', '#65533c');
                root.style.setProperty('--button-text-color', '#B1AD86');
                root.style.setProperty('--text-color', '#B1AD86');
                root.style.setProperty('--text-secondary-color', '#B1AD86');
                root.style.setProperty('--border-color', '#000000');
                root.style.setProperty('--accent-color', '#4a535e');
                root.style.setProperty('--prog-bar-color', '#453928');
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#221A1F'); // Assuming black as bonus color
                root.style.setProperty('--secondary-background', '#131510');
                root.style.setProperty('--container-background', '#362712');
                root.style.setProperty('--standout-color', '#B1AD86');
                root.style.setProperty('--hover-color', '#d3af86');
                root.style.setProperty('--link-color', '#f6f5f4');
                break;

            case 'Neon':
                root.style.setProperty('--background-color', '#120e16');
                root.style.setProperty('--button-color', '#303648');
                root.style.setProperty('--button-text-color', '#af565f');
                root.style.setProperty('--text-color', '#9F9DA1');
                root.style.setProperty('--text-secondary-color', '#92bb75');
                root.style.setProperty('--border-color', '#000000');
                root.style.setProperty('--accent-color', '#4a535e');
                root.style.setProperty('--prog-bar-color', '#39363b');
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#1a171e'); // Assuming black as bonus color
                root.style.setProperty('--secondary-background', '#120e16');
                root.style.setProperty('--container-background', '#1a171e');
                root.style.setProperty('--standout-color', '#797b85');
                root.style.setProperty('--hover-color', '#7000ff');
                root.style.setProperty('--link-color', '#7000ff');
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
                break;

            case 'Gruvbox Light':
                root.style.setProperty('--background-color', '#f9f5d7');
                root.style.setProperty('--button-color', '#aca289');
                root.style.setProperty('--button-text-color', '#5f5750');
                root.style.setProperty('--text-color', '#5f5750');
                root.style.setProperty('--text-secondary-color', '#aca289');
                root.style.setProperty('--border-color', '#000000');
                root.style.setProperty('--accent-color', '#e0dbb2');
                root.style.setProperty('--prog-bar-color', '#f2e5bc');
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#f2e5bc'); // Assuming black as bonus color
                root.style.setProperty('--secondary-background', '#fbf1c7');
                root.style.setProperty('--container-background', '#fbf1c7');
                root.style.setProperty('--standout-color', '#797b85');
                root.style.setProperty('--hover-color', '#cfd2a8');
                root.style.setProperty('--link-color', '#a68738');
                break;

            case 'Gruvbox Dark':
                root.style.setProperty('--background-color', '#32302f');
                root.style.setProperty('--button-color', '#303648');
                root.style.setProperty('--button-text-color', '#868729');
                root.style.setProperty('--text-color', '#868729');
                root.style.setProperty('--text-secondary-color', '#868729');
                root.style.setProperty('--border-color', '#000000');
                root.style.setProperty('--accent-color', '#ebdbb2');
                root.style.setProperty('--prog-bar-color', '#1d2021');
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#363332'); // Assuming black as bonus color
                root.style.setProperty('--secondary-background', '#282828');
                root.style.setProperty('--container-background', '#302e2e');
                root.style.setProperty('--standout-color', '#ebdbb2');
                root.style.setProperty('--hover-color', '#59544a');
                root.style.setProperty('--link-color', '#6f701b');
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

pub fn log_css_variables() {
    let window = window().expect("no global `window` exists");
    let document = window.document().expect("should have a document on window");
    let root: Element = document
        .document_element()
        .expect("document should have a root element");

    let computed_style = window
        .get_computed_style(&root)
        .expect("should be able to get computed style")
        .expect("computed style should not be null");

    let variable_names = vec![
        "--background-color",
        "--button-color",
        "--text-color",
        "--accent-color",
        "--error-color",
        "--bonus-color",
        "--secondary-background",
        "--standout-color",
        "--hover-color",
    ];

    web_sys::console::log_1(&"Current CSS Variable Values:".into());
    for var_name in variable_names {
        let value = computed_style
            .get_property_value(var_name)
            .expect("should get property value");
        web_sys::console::log_2(&var_name.into(), &value.into());
    }
}
