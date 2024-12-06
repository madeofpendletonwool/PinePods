use super::app_drawer::App_drawer;
use super::gen_components::{Search_nav, UseScrollToTop};
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, UIState};
use crate::components::episodes_layout::UIStateMsg;
use crate::components::setting_components;
use crate::requests::login_requests::use_check_authentication;
use crate::requests::setting_reqs::call_user_admin_check;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::window;
use yew::prelude::*;
use yewdux::prelude::*;
// use crate::components::gen_funcs::check_auth;

#[derive(Properties, PartialEq, Clone)]
pub struct TabProps {
    pub is_active: bool,
    pub label: String,
    pub onclick: Callback<MouseEvent>,
    pub class: String,
}

#[function_component(Tab)]
fn tab(props: &TabProps) -> Html {
    let TabProps {
        is_active,
        label,
        onclick,
        class,
    } = props.clone();

    let tab_class = if is_active {
        format!(
            "{} tab-hightlight-colors px-6 py-2 rounded-md transition-all duration-200",
            class
        )
    } else {
        format!("{} tab-unselect-colors px-6 py-2 rounded-md hover:bg-opacity-10 hover:bg-white transition-all duration-200", class)
    };

    html! {
        <button class={tab_class} onclick={onclick}>{ label }</button>
    }
}

#[derive(Properties, PartialEq, Clone)]
pub struct AccordionItemProps {
    pub title: String,
    pub content: Html,
    pub position: AccordionItemPosition, // Add this line
}

// Enum to represent the position of the accordion item
#[derive(PartialEq, Clone)]
pub enum AccordionItemPosition {
    First,
    Middle,
}

#[function_component(AccordionItem)]
pub fn accordion_item(
    AccordionItemProps {
        title,
        content,
        position,
    }: &AccordionItemProps,
) -> Html {
    let is_open = use_state(|| false);
    let toggle = {
        let is_open = is_open.clone();
        Callback::from(move |_| is_open.set(!*is_open))
    };

    let (border_class, button_class) = match position {
        AccordionItemPosition::First => ("rounded-t-xl", "border-b-0"),
        AccordionItemPosition::Middle => ("", "border-b-0"),
    };

    let arrow_rotation_class = if *is_open { "rotate-180" } else { "rotate-0" };

    html! {
        <div class={format!("accordion-container {}", border_class)}>
            <h2>
                <button
                    class={format!("accordion-button flex items-center justify-between w-full p-5 font-medium {} focus:ring-4 gap-3 relative", button_class)}
                    onclick={toggle}
                >
                    <span>{ title }</span>
                    <svg
                        class={format!("w-3 h-3 shrink-0 transition-transform duration-300 accordion-arrow {}", arrow_rotation_class)}
                        xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 10 6"
                    >
                        <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5 5 1 1 5"/>
                    </svg>
                </button>
            </h2>
            if *is_open {
                <div class="p-5 accordion-content">
                    { content.clone() }
                </div>
            }
        </div>
    }
}

#[function_component(Settings)]
pub fn settings() -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let effect_dispatch = dispatch.clone();

    let session_dispatch = effect_dispatch.clone();
    let session_state = state.clone();

    use_effect_with((), move |_| {
        // Check if the page reload action has already occurred to prevent redundant execution
        if session_state.reload_occured.unwrap_or(false) {
            // Logic for the case where reload has already been processed
        } else {
            // Normal effect logic for handling page reload
            let window = web_sys::window().expect("no global `window` exists");
            let performance = window.performance().expect("should have performance");
            let navigation_type = performance.navigation().type_();

            if navigation_type == 1 {
                // 1 stands for reload
                let session_storage = window.session_storage().unwrap().unwrap();
                session_storage
                    .set_item("isAuthenticated", "false")
                    .unwrap();
            }

            // Always check authentication status
            let current_route = window.location().href().unwrap_or_default();
            use_check_authentication(session_dispatch.clone(), &current_route);

            // Mark that the page reload handling has occurred
            session_dispatch.reduce_mut(|state| {
                state.reload_occured = Some(true);
                state.clone() // Return the modified state
            });
        }

        || ()
    });

    let (_post_state, _post_dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();

    let error_message = audio_state.error_message.clone();
    let info_message = audio_state.info_message.clone();
    let active_tab = use_state(|| "user");

    let api_key = _post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.api_key.clone());
    let user_id = _post_state
        .user_details
        .as_ref()
        .map(|ud| ud.UserID.clone());
    let server_name = _post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.server_name.clone());

    let is_admin = use_state(|| false);
    let audio_admin = audio_dispatch.clone();

    {
        let is_admin = is_admin.clone();

        use_effect_with(
            (api_key.clone(), user_id.clone(), server_name.clone()),
            move |_| {
                if let (Some(api_key), Some(user_id), Some(server_name)) =
                    (api_key.clone(), user_id.clone(), server_name.clone())
                {
                    wasm_bindgen_futures::spawn_local(async move {
                        match call_user_admin_check(&server_name, &api_key.unwrap(), user_id).await
                        {
                            Ok(response) => {
                                is_admin.set(response.is_admin);
                            }
                            Err(e) => {
                                audio_admin.reduce_mut(|state| {
                                    state.error_message =
                                        Some(format!("Failed to check admin status: {:?}", e))
                                });
                                // console::log_1(&format!("Failed to check admin status: {:?}", e).into());
                            }
                        }
                    });
                }
                || ()
            },
        );
    }

    {
        let ui_dispatch = audio_dispatch.clone();
        use_effect(move || {
            let window = window().unwrap();
            let document = window.document().unwrap();

            let closure = Closure::wrap(Box::new(move |_event: Event| {
                ui_dispatch.apply(UIStateMsg::ClearErrorMessage);
                ui_dispatch.apply(UIStateMsg::ClearInfoMessage);
            }) as Box<dyn Fn(_)>);

            document
                .add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())
                .unwrap();

            // Return cleanup function
            move || {
                document
                    .remove_event_listener_with_callback("click", closure.as_ref().unchecked_ref())
                    .unwrap();
                closure.forget(); // Prevents the closure from being dropped
            }
        });
    }

    let on_user_tab_click = {
        let active_tab = active_tab.clone();
        Callback::from(move |_| active_tab.set("user"))
    };

    let on_admin_tab_click = {
        let active_tab = active_tab.clone();
        Callback::from(move |_| active_tab.set("admin"))
    };

    html! {
        <>
        <div class="main-container">
            <Search_nav />
            <UseScrollToTop />
            <div class="my-4">
                <h1 class="item_container-text text-2xl font-bold mb-3">{ "Settings" }</h1>
                <div class="inline-flex tab-background p-1 rounded-lg bg-opacity-10 mb-6">
                    <Tab
                        is_active={*active_tab == "user"}
                        class="text-base"
                        label={"User Settings".to_string()}
                        onclick={on_user_tab_click.clone()}
                    />
                    {
                        if *is_admin {
                            html! {
                                <Tab
                                    is_active={*active_tab == "admin"}
                                    class="text-base"
                                    label={"Admin Settings".to_string()}
                                    onclick={on_admin_tab_click.clone()}
                                />
                            }
                        } else {
                            html! {}
                        }
                    }
                </div>
                <div class="rounded-xl theme-dropdown-arrow overflow-hidden">
                {
                    if *active_tab == "user" {
                        html! {
                        <div id="accordion-collapse" data-accordion="collapse" class="bg-custom-light">
                            <AccordionItem title="Change Theme" content={html!{ <setting_components::theme_options::ThemeOptions /> }} position={AccordionItemPosition::First}/>
                            <AccordionItem title="MFA Settings" content={html!{ <setting_components::mfa_settings::MFAOptions /> }} position={AccordionItemPosition::Middle}/>
                            <AccordionItem title="Export/Backup Podcasts" content={html!{ <setting_components::export_settings::ExportOptions /> }} position={AccordionItemPosition::Middle}/>
                            <AccordionItem title="Import Podcasts" content={html!{ <setting_components::import_options::ImportOptions /> }} position={AccordionItemPosition::Middle}/>
                            <AccordionItem title="Add Custom Feed" content={html!{ <setting_components::custom_feed::CustomFeed /> }} position={AccordionItemPosition::Middle}/>
                            <AccordionItem title="Connect Nextcloud/Gpodder Podcast Sync" content={html!{ <setting_components::nextcloud_options::NextcloudOptions /> }} position={AccordionItemPosition::Middle}/>
                            <AccordionItem title="Api Keys" content={html!{ <setting_components::api_keys::APIKeys /> }} position={AccordionItemPosition::Middle}/>
                        </div>
                        }
                    } else if *active_tab == "admin" {
                        html! {
                        <div id="accordion-collapse" data-accordion="collapse" class="bg-custom-light">
                            <AccordionItem title="User Management" content={html!{ <setting_components::user_settings::UserSettings /> }} position={AccordionItemPosition::First}/>
                            // <AccordionItem title="Guest Settings" content={html!{ <setting_components::guest_settings::GuestSettings /> }} position={AccordionItemPosition::Middle}/>
                            <AccordionItem title="Download Settings" content={html!{ <setting_components::download_settings::DownloadSettings /> }} position={AccordionItemPosition::Middle}/>
                            <AccordionItem title="User Self Service Settings" content={html!{ <setting_components::user_self_service::SelfServiceSettings /> }} position={AccordionItemPosition::Middle}/>
                            <AccordionItem title="Email Settings" content={html!{ <setting_components::email_settings::EmailSettings /> }} position={AccordionItemPosition::Middle}/>
                            <AccordionItem title="Backup Server" content={html!{ <setting_components::backup_server::BackupServer /> }} position={AccordionItemPosition::Middle}/>
                            // <AccordionItem title="Restore Server" content={html!{ <setting_components::restore_server::RestoreServer /> }} position={AccordionItemPosition::Middle}/>
                        </div>
                        }
                    } else {
                        html! {}
                    }
                }
                </div>
            </div>
            // Conditional rendering for the error banner
            if let Some(error) = error_message {
                <div class="error-snackbar">{ error }</div>
            }
            if let Some(info) = info_message {
                <div class="info-snackbar">{ info }</div>
            }
            {
                if let Some(audio_props) = &audio_state.currently_playing {
                    html! { <AudioPlayer src={audio_props.src.clone()} title={audio_props.title.clone()} artwork_url={audio_props.artwork_url.clone()} duration={audio_props.duration.clone()} episode_id={audio_props.episode_id.clone()} duration_sec={audio_props.duration_sec.clone()} start_pos_sec={audio_props.start_pos_sec.clone()} end_pos_sec={audio_props.end_pos_sec.clone()} offline={audio_props.offline.clone()} /> }
                } else {
                    html! {}
                }
            }
        </div>
        <App_drawer />
        </>
    }
}
