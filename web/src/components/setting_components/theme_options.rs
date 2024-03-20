use web_sys::{Element, HtmlSelectElement};
use yew::prelude::*;
use yewdux::prelude::*;
use crate::components::context::AppState;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::window;
use crate::requests::setting_reqs::{call_set_theme, SetThemeRequest};

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
                    <option value="Nordic" selected={(*selected_theme) == "Nordic"}>{"Nordic"}</option>
                    <option value="Abyss" selected={(*selected_theme) == "Abyss"}>{"Abyss"}</option>
                    <option value="Dracula" selected={(*selected_theme) == "Dracula"}>{"Dracula"}</option>
                    <option value="Neon" selected={(*selected_theme) == "Neon"}>{"Neon"}</option>
                    <option value="Kimbie" selected={(*selected_theme) == "Kimbie"}>{"Kimbie"}</option>
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
                root.style.setProperty('--background-color', '#32333b');
                root.style.setProperty('--button-color', '#2c3032');
                root.style.setProperty('--text-color', '#000000');
                root.style.setProperty('--text-secondary-color', '#000000');
                root.style.setProperty('--border-color', '#000000');
                root.style.setProperty('--accent-color', '#000000'); // Assuming black as accent color
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#000000'); // Assuming black as bonus color
                root.style.setProperty('--secondary-background', '#32333b');
                root.style.setProperty('--standout-color', '#304BFF');
                root.style.setProperty('--hover-color', '#304BFF');
                root.style.setProperty('--link-color', '#6590fd');
                root.style.setProperty('--transparent-background', 'rgba(63, 57, 90, 0.089');
                break;

            case 'Dark':
                root.style.setProperty('--background-color', '#2a2b33');
                root.style.setProperty('--button-color', '#303648');
                root.style.setProperty('--text-color', '#f6f5f4');
                root.style.setProperty('--text-secondary-color', '#f6f5f4');
                root.style.setProperty('--border-color', '#000000');
                root.style.setProperty('--accent-color', '#4a535e');
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#000000'); // Assuming black as bonus color
                root.style.setProperty('--secondary-background', '#32333b');
                root.style.setProperty('--container-background', '#1b1d1e');
                root.style.setProperty('--standout-color', '#797b85');
                root.style.setProperty('--hover-color', '#4b5563');
                root.style.setProperty('--link-color', '#6590fd');
                break;

            case 'Nordic':
                root.style.setProperty('--background-color', '#3C4252');
                root.style.setProperty('--button-color', '#FFFFFF');
                root.style.setProperty('--text-color', '#FFFFFF');
                root.style.setProperty('--accent-color', '#FFFFFF'); // Assuming white as accent color
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#000000'); // Assuming black as bonus color
                root.style.setProperty('--secondary-background', '#323542');
                root.style.setProperty('--standout-color', '#304BFF');
                root.style.setProperty('--hover-color', '#304BFF');
                break;

            case 'Abyss':
                root.style.setProperty('--background-color', '#000C18');
                root.style.setProperty('--button-color', '#FFFFFF'); // White
                root.style.setProperty('--text-color', '#42A5F5'); // Light blue
                root.style.setProperty('--accent-color', '#FFFFFF'); // White
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#000000'); // Black
                root.style.setProperty('--secondary-background', '#13326A'); // Dark blue
                root.style.setProperty('--standout-color', '#42A5F5'); // Light blue
                root.style.setProperty('--hover-color', '#42A5F5'); // Light blue
                break;

            case 'Dracula':
                root.style.setProperty('--background-color', '#282A36');
                root.style.setProperty('--button-color', '#5196B2'); // Light blue
                root.style.setProperty('--text-color', '#FFFFFF'); // White
                root.style.setProperty('--accent-color', '#5196B2'); // Light blue
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#D5BC5C'); // Yellow
                root.style.setProperty('--secondary-background', '#262626'); // Dark gray
                root.style.setProperty('--standout-color', '#5196B2'); // Light blue
                root.style.setProperty('--hover-color', '#5196B2'); // Light blue
                break;

            case 'Kimbie':
                root.style.setProperty('--background-color', '#221A0F'); // Dark brown
                root.style.setProperty('--button-color', '#B23958'); // Pink
                root.style.setProperty('--text-color', '#B1AD86'); // Beige
                root.style.setProperty('--accent-color', '#B23958'); // Pink
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#221A1F'); // Dark purple
                root.style.setProperty('--secondary-background', '#AC8E2F'); // Gold
                root.style.setProperty('--standout-color', '#B23958'); // Pink
                root.style.setProperty('--hover-color', '#B23958'); // Pink
                break;

            case 'Neon':
                root.style.setProperty('--background-color', '#120E16');
                root.style.setProperty('--button-color', '#7000FF'); // Purple
                root.style.setProperty('--text-color', '#9F9DA1'); // Grey
                root.style.setProperty('--accent-color', '#7000FF'); // Purple
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#01FFF4'); // Neon Blue
                root.style.setProperty('--secondary-background', '#161C26'); // Dark Blue
                root.style.setProperty('--standout-color', '#FF1178'); // Neon Pink
                root.style.setProperty('--hover-color', '#FF1178'); // Neon Pink
                break;

            case 'Greenie Meanie':
                root.style.setProperty('--background-color', '#1E1F21');
                root.style.setProperty('--button-color', '#737373'); // Grey
                root.style.setProperty('--text-color', '#489D50'); // Green
                root.style.setProperty('--accent-color', '#737373'); // Grey
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#849CA0'); // Blue-Grey
                root.style.setProperty('--secondary-background', '#292A2E'); // Dark Grey
                root.style.setProperty('--standout-color', '#446448'); // Dark Green
                root.style.setProperty('--hover-color', '#43603D'); // Darker Green
                break;

            case 'Wildberries':
                root.style.setProperty('--background-color', '#240041');
                root.style.setProperty('--button-color', '#F55385'); // Pink
                root.style.setProperty('--text-color', '#CF8B3E'); // Orange
                root.style.setProperty('--accent-color', '#F55385'); // Pink
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#C79BFF'); // Light Purple
                root.style.setProperty('--secondary-background', '#19002E'); // Dark Purple
                root.style.setProperty('--standout-color', '#00FFB7'); // Bright Green
                root.style.setProperty('--hover-color', '#44433A'); // Dark Grey
                break;

            case 'Hot Dog Stand - MY EYES':
                root.style.setProperty('--background-color', '#E31836');
                root.style.setProperty('--button-color', '#C3590D'); // Orange
                root.style.setProperty('--text-color', '#FFFFFF'); // White
                root.style.setProperty('--accent-color', '#EEB911'); // Yellow
                root.style.setProperty('--error-color', 'red');
                root.style.setProperty('--bonus-color', '#D5BC5C'); // Gold
                root.style.setProperty('--secondary-background', '#730B1B'); // Dark Red
                root.style.setProperty('--standout-color', '#D5BC5C'); // Gold
                root.style.setProperty('--hover-color', '#C3590D'); // Orange
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
