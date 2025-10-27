use crate::components::context::AppState;
use crate::components::gen_funcs::format_error_message;
use crate::requests::setting_reqs::{
    call_add_oidc_provider, call_list_oidc_providers, call_remove_oidc_provider,
    call_update_oidc_provider, AddOIDCProviderRequest, OIDCProvider,
};
use gloo_events::EventListener;
use i18nrs::yew::use_translation;
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yewdux::prelude::*;

#[derive(Clone, PartialEq)]
enum PageState {
    Hidden,
    AddProvider,
    EditProvider(i32), // provider_id
}

#[derive(Clone, PartialEq, Debug)]
pub struct ScopeOption {
    pub id: String,
    pub name: String,
    pub description: String,
    pub value: String,
    pub provider_type: ProviderType,
}

#[derive(Clone, PartialEq, Debug, Eq)]
pub enum ProviderType {
    Standard,
    GitHub,
    Google,
    Microsoft,
}

// Create a helper function to get scope collections
// This should be defined at the module level, outside of any component
fn get_scope_collections() -> (Vec<ScopeOption>, Vec<ScopeOption>) {
    // Standard OIDC scopes
    let standard_scopes = vec![
        ScopeOption {
            id: "openid".to_string(),
            name: "OpenID".to_string(),
            description: "Provides verifiable identity".to_string(),
            value: "openid".to_string(),
            provider_type: ProviderType::Standard,
        },
        ScopeOption {
            id: "email".to_string(),
            name: "Email".to_string(),
            description: "Access to email address".to_string(),
            value: "email".to_string(),
            provider_type: ProviderType::Standard,
        },
        ScopeOption {
            id: "profile".to_string(),
            name: "Profile".to_string(),
            description: "Basic profile info like name".to_string(),
            value: "profile".to_string(),
            provider_type: ProviderType::Standard,
        },
    ];

    // GitHub-specific scopes
    let github_scopes = vec![
        ScopeOption {
            id: "read_user".to_string(),
            name: "Read User".to_string(),
            description: "Read user profile data".to_string(),
            value: "read:user".to_string(),
            provider_type: ProviderType::GitHub,
        },
        ScopeOption {
            id: "user_email".to_string(),
            name: "User Email".to_string(),
            description: "Access to email address(es)".to_string(),
            value: "user:email".to_string(),
            provider_type: ProviderType::GitHub,
        },
    ];

    (standard_scopes, github_scopes)
}

// Helper function to format scopes for the request - also at module level
pub fn format_scopes_for_request(scopes: &[String], provider_type: &ProviderType) -> String {
    let (standard_scopes, github_scopes) = get_scope_collections();

    if scopes.is_empty() {
        // Provide defaults based on provider
        match provider_type {
            ProviderType::GitHub => "read:user,user:email".to_string(),
            _ => "openid email profile".to_string(),
        }
    } else {
        // Map selected scope IDs to their values
        let scope_values: Vec<String> = scopes
            .iter()
            .filter_map(|id| {
                // Find the matching scope option - use a function to get the right iterator
                let mut iter = match provider_type {
                    ProviderType::GitHub => github_scopes.iter().chain(standard_scopes.iter()),
                    _ => standard_scopes.iter().chain([].iter()), // Chain with empty iterator for the default case
                };

                iter.find(|s| &s.id == id).map(|s| s.value.clone())
            })
            .collect();

        scope_values.join(" ")
    }
}

#[derive(Properties, PartialEq)]
pub struct ScopeSelectorProps {
    pub selected_scopes: Vec<String>,
    pub on_select: Callback<Vec<String>>,
    pub auth_url: String,
    pub token_url: String,
    pub user_info_url: String,
}

#[function_component(ScopeSelector)]
pub fn scope_selector(props: &ScopeSelectorProps) -> Html {
    let is_open = use_state(|| false);
    let dropdown_ref = use_node_ref();

    let (standard_scopes, github_scopes) = get_scope_collections();

    // Combine all scopes based on the detected provider
    let detected_provider =
        detect_provider(&props.auth_url, &props.token_url, &props.user_info_url);
    let available_scopes = match detected_provider {
        ProviderType::GitHub => [&standard_scopes[..], &github_scopes[..]].concat(),
        _ => standard_scopes,
    };

    // Handle clicking outside to close dropdown
    {
        let is_open = is_open.clone();
        let dropdown_ref = dropdown_ref.clone();

        use_effect_with(dropdown_ref.clone(), move |dropdown_ref| {
            let document = web_sys::window().unwrap().document().unwrap();
            let dropdown_element = dropdown_ref.cast::<HtmlElement>();

            let listener = EventListener::new(&document, "click", move |event| {
                if let Some(target) = event.target() {
                    if let Some(dropdown) = &dropdown_element {
                        if let Ok(node) = target.dyn_into::<web_sys::Node>() {
                            if !dropdown.contains(Some(&node)) {
                                is_open.set(false);
                            }
                        }
                    }
                }
            });

            || drop(listener)
        });
    }

    let toggle_dropdown = {
        let is_open = is_open.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            is_open.set(!*is_open);
        })
    };

    let toggle_scope_selection = {
        let selected = props.selected_scopes.clone();
        let on_select = props.on_select.clone();

        Callback::from(move |scope_id: String| {
            let mut new_selection = selected.clone();
            if let Some(pos) = new_selection.iter().position(|id| id == &scope_id) {
                new_selection.remove(pos);
            } else {
                new_selection.push(scope_id);
            }
            on_select.emit(new_selection);
        })
    };

    // Format selected scopes for display
    let formatted_scopes = if props.selected_scopes.is_empty() {
        if detected_provider == ProviderType::GitHub {
            "GitHub: read:user,user:email".to_string()
        } else if detected_provider == ProviderType::Google {
            "OpenID Connect: openid email profile".to_string()
        } else {
            "OpenID Connect: openid email profile".to_string()
        }
    } else {
        let selected_names: Vec<String> = available_scopes
            .iter()
            .filter(|scope| props.selected_scopes.contains(&scope.id))
            .map(|scope| scope.name.clone())
            .collect();

        selected_names.join(", ")
    };

    html! {
        <div class="relative" ref={dropdown_ref}>
            <button
                type="button"
                onclick={toggle_dropdown.clone()}
                class="search-bar-input border text-sm rounded-lg block w-full p-2.5 flex items-center"
            >
                <div class="flex items-center flex-grow">
                    <span class="flex-grow text-left">
                        {formatted_scopes}
                    </span>
                    <i class={classes!(
                        "ph",
                        "ph-caret-down",
                        "transition-transform",
                        "duration-200",
                        if *is_open { "rotate-180" } else { "" }
                    )}></i>
                </div>
            </button>

            {
                if *is_open {
                    let standard_group = available_scopes.iter()
                        .filter(|s| s.provider_type == ProviderType::Standard)
                        .collect::<Vec<_>>();

                    let github_group = available_scopes.iter()
                        .filter(|s| s.provider_type == ProviderType::GitHub)
                        .collect::<Vec<_>>();

                    let google_group = available_scopes.iter()
                        .filter(|s| s.provider_type == ProviderType::Google)
                        .collect::<Vec<_>>();

                    html! {
                        <div
                            class="absolute z-50 mt-1 w-full rounded-lg shadow-lg modal-container"
                            onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}
                        >
                            <div class="max-h-[400px] overflow-y-auto p-2 space-y-1">
                                if detected_provider == ProviderType::GitHub {
                                    <div class="p-2 rounded bg-blue-900/20 mb-2">
                                        <div class="flex items-center gap-2">
                                            <i class="ph ph-info text-blue-400"></i>
                                            <span class="font-medium">{"GitHub Provider Detected"}</span>
                                        </div>
                                        <p class="text-sm opacity-80 mt-1">
                                            {"GitHub doesn't use standard OIDC scopes. We recommend using their specific scopes for best compatibility."}
                                        </p>
                                    </div>
                                }

                                // Render scope groups
                                if !standard_group.is_empty() {
                                    <div class="mb-2">
                                        <h3 class="text-sm font-medium opacity-70 mb-1 px-2">{"Standard OIDC Scopes"}</h3>
                                        { render_scope_group(standard_group, &props.selected_scopes, &toggle_scope_selection) }
                                    </div>
                                }

                                if !github_group.is_empty() {
                                    <div class="mb-2">
                                        <h3 class="text-sm font-medium opacity-70 mb-1 px-2">{"GitHub Scopes"}</h3>
                                        { render_scope_group(github_group, &props.selected_scopes, &toggle_scope_selection) }
                                    </div>
                                }

                                if !google_group.is_empty() {
                                    <div class="mb-2">
                                        <h3 class="text-sm font-medium opacity-70 mb-1 px-2">{"Google Scopes"}</h3>
                                        { render_scope_group(google_group, &props.selected_scopes, &toggle_scope_selection) }
                                    </div>
                                }
                            </div>
                        </div>
                    }
                } else {
                    html! {}
                }
            }
        </div>
    }
}

// Helper function to render a group of scopes
fn render_scope_group(
    scopes: Vec<&ScopeOption>,
    selected_scopes: &[String],
    toggle_callback: &Callback<String>,
) -> Html {
    scopes
        .iter()
        .map(|scope| {
            let is_selected = selected_scopes.contains(&scope.id);
            let onclick = {
                let toggle = toggle_callback.clone();
                let id = scope.id.clone();
                Callback::from(move |_| toggle.emit(id.clone()))
            };

            html! {
                <div
                    key={scope.id.clone()}
                    {onclick}
                    class={classes!(
                        "flex",
                        "items-start",
                        "p-2",
                        "rounded-lg",
                        "cursor-pointer",
                        "hover:bg-gray-700",
                        "transition-colors",
                        if is_selected { "bg-gray-700" } else { "" }
                    )}
                >
                    <div class="flex items-center h-5">
                        <input
                            type="checkbox"
                            class="w-4 h-4 accent-blue-500"
                            checked={is_selected}
                            readonly=true
                        />
                    </div>
                    <div class="ml-3 flex-grow">
                        <label class="font-medium">
                            {&scope.name}
                        </label>
                        <p class="text-sm opacity-70">
                            {&scope.description}
                        </p>
                        <code class="text-xs opacity-50 block mt-1">
                            {&scope.value}
                        </code>
                    </div>
                    if is_selected {
                        <i class="ph ph-check text-blue-500 text-xl"></i>
                    }
                </div>
            }
        })
        .collect::<Html>()
}

// Helper function to detect provider type based on URLs
fn detect_provider(auth_url: &str, token_url: &str, user_info_url: &str) -> ProviderType {
    let urls = [auth_url, token_url, user_info_url];

    for url in urls {
        if url.contains("github.com") {
            return ProviderType::GitHub;
        } else if url.contains("google") || url.contains("googleapis.com") {
            return ProviderType::Google;
        } else if url.contains("microsoft")
            || url.contains("azure")
            || url.contains("microsoftonline")
        {
            return ProviderType::Microsoft;
        }
    }

    ProviderType::Standard
}

#[function_component(OIDCSettings)]
pub fn oidc_settings() -> Html {
    let (i18n, _) = use_translation();
    let (state, _dispatch) = use_store::<AppState>();
    let page_state = use_state(|| PageState::Hidden);
    let providers = use_state(|| Vec::<OIDCProvider>::new());
    let update_trigger = use_state(|| false);

    // Capture i18n strings before they get moved
    let i18n_failed_to_fetch_oidc_providers =
        i18n.t("oidc.failed_to_fetch_oidc_providers").to_string();
    let i18n_provider_successfully_removed =
        i18n.t("oidc.provider_successfully_removed").to_string();
    let i18n_failed_to_remove_provider = i18n.t("oidc.failed_to_remove_provider").to_string();
    let i18n_oidc_provider_successfully_added =
        i18n.t("oidc.oidc_provider_successfully_added").to_string();
    let i18n_failed_to_add_provider = i18n.t("oidc.failed_to_add_provider").to_string();
    let i18n_oidc_provider_successfully_updated = i18n
        .t("oidc.oidc_provider_successfully_updated")
        .to_string();
    let i18n_failed_to_update_provider = i18n.t("oidc.failed_to_update_provider").to_string();
    let i18n_add_oidc_provider = i18n.t("oidc.add_oidc_provider").to_string();
    let i18n_edit_oidc_provider = i18n.t("oidc.edit_oidc_provider").to_string();
    let i18n_close_modal = i18n.t("common.close_modal").to_string();
    let i18n_oidc_redirect_url = i18n.t("oidc.oidc_redirect_url").to_string();
    let i18n_use_this_url_when_configuring =
        i18n.t("oidc.use_this_url_when_configuring").to_string();
    let i18n_provider_name = i18n.t("oidc.provider_name").to_string();
    let i18n_client_id = i18n.t("oidc.client_id").to_string();
    let i18n_client_secret = i18n.t("oidc.client_secret").to_string();
    let i18n_authorization_url = i18n.t("oidc.authorization_url").to_string();
    let i18n_token_url = i18n.t("oidc.token_url").to_string();
    let i18n_user_info_url = i18n.t("oidc.user_info_url").to_string();
    let i18n_submit = i18n.t("common.submit").to_string();
    let i18n_add = i18n.t("common.add").to_string();
    let i18n_update = i18n.t("common.update").to_string();
    let i18n_oidc_provider_management = i18n.t("oidc.oidc_provider_management").to_string();
    let i18n_add_provider = i18n.t("oidc.add_provider").to_string();
    let i18n_no_oidc_providers_configured = i18n.t("oidc.no_oidc_providers_configured").to_string();
    let i18n_remove = i18n.t("common.remove").to_string();

    // Form states for the add/edit provider modal
    let provider_name = use_state(|| String::new());
    let client_id = use_state(|| String::new());
    let client_secret = use_state(|| String::new());
    let auth_url = use_state(|| String::new());
    let token_url = use_state(|| String::new());
    let user_info_url = use_state(|| String::new());
    let button_text = use_state(|| String::new());
    let button_color = use_state(|| String::from("#000000"));
    let button_text_color = use_state(|| String::from("#000000"));
    let icon_svg = use_state(|| String::new());
    let name_claim = use_state(|| String::new());
    let email_claim = use_state(|| String::new());
    let username_claim = use_state(|| String::new());
    let roles_claim = use_state(|| String::new());
    let user_role = use_state(|| String::new());
    let admin_role = use_state(|| String::new());
    let selected_scopes = use_state(|| Vec::<String>::new());
    let editing_provider_id = use_state(|| None::<i32>);

    // Function to populate form with provider data for editing
    let populate_form_for_edit = {
        let provider_name = provider_name.clone();
        let client_id = client_id.clone();
        let client_secret = client_secret.clone();
        let auth_url = auth_url.clone();
        let token_url = token_url.clone();
        let user_info_url = user_info_url.clone();
        let button_text = button_text.clone();
        let button_color = button_color.clone();
        let button_text_color = button_text_color.clone();
        let icon_svg = icon_svg.clone();
        let name_claim = name_claim.clone();
        let email_claim = email_claim.clone();
        let username_claim = username_claim.clone();
        let roles_claim = roles_claim.clone();
        let user_role = user_role.clone();
        let admin_role = admin_role.clone();
        let selected_scopes = selected_scopes.clone();
        let editing_provider_id = editing_provider_id.clone();

        move |provider: &OIDCProvider| {
            provider_name.set(provider.provider_name.clone());
            client_id.set(provider.client_id.clone());
            client_secret.set(String::new()); // Don't populate secret for security
            auth_url.set(provider.authorization_url.clone());
            token_url.set(provider.token_url.clone());
            user_info_url.set(provider.user_info_url.clone());
            button_text.set(provider.button_text.clone());
            button_color.set(provider.button_color.clone());
            button_text_color.set(provider.button_text_color.clone());
            icon_svg.set(provider.icon_svg.as_ref().unwrap_or(&String::new()).clone());
            name_claim.set(
                provider
                    .name_claim
                    .as_ref()
                    .unwrap_or(&String::new())
                    .clone(),
            );
            email_claim.set(
                provider
                    .email_claim
                    .as_ref()
                    .unwrap_or(&String::new())
                    .clone(),
            );
            username_claim.set(
                provider
                    .username_claim
                    .as_ref()
                    .unwrap_or(&String::new())
                    .clone(),
            );
            roles_claim.set(
                provider
                    .roles_claim
                    .as_ref()
                    .unwrap_or(&String::new())
                    .clone(),
            );
            user_role.set(
                provider
                    .user_role
                    .as_ref()
                    .unwrap_or(&String::new())
                    .clone(),
            );
            admin_role.set(
                provider
                    .admin_role
                    .as_ref()
                    .unwrap_or(&String::new())
                    .clone(),
            );
            editing_provider_id.set(Some(provider.provider_id));

            // Parse scopes from the provider's scope string
            let scopes_vec: Vec<String> = provider
                .scope
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();
            selected_scopes.set(scopes_vec);
        }
    };

    // Function to clear form for adding new provider
    let clear_form = {
        let provider_name = provider_name.clone();
        let client_id = client_id.clone();
        let client_secret = client_secret.clone();
        let auth_url = auth_url.clone();
        let token_url = token_url.clone();
        let user_info_url = user_info_url.clone();
        let button_text = button_text.clone();
        let button_color = button_color.clone();
        let button_text_color = button_text_color.clone();
        let icon_svg = icon_svg.clone();
        let name_claim = name_claim.clone();
        let email_claim = email_claim.clone();
        let username_claim = username_claim.clone();
        let roles_claim = roles_claim.clone();
        let user_role = user_role.clone();
        let admin_role = admin_role.clone();
        let selected_scopes = selected_scopes.clone();
        let editing_provider_id = editing_provider_id.clone();

        move || {
            provider_name.set(String::new());
            client_id.set(String::new());
            client_secret.set(String::new());
            auth_url.set(String::new());
            token_url.set(String::new());
            user_info_url.set(String::new());
            button_text.set(String::new());
            button_color.set(String::from("#000000"));
            button_text_color.set(String::from("#000000"));
            icon_svg.set(String::new());
            name_claim.set(String::new());
            email_claim.set(String::new());
            username_claim.set(String::new());
            roles_claim.set(String::new());
            user_role.set(String::new());
            admin_role.set(String::new());
            selected_scopes.set(Vec::new());
            editing_provider_id.set(None);
        }
    };

    // Add this callback after your other input handlers
    let scope_on_select = {
        let selected_scopes = selected_scopes.clone();
        Callback::from(move |scopes: Vec<String>| {
            selected_scopes.set(scopes);
        })
    };

    // Fetch providers on component mount and when update_trigger changes
    let effect_state = state.clone();
    {
        let providers = providers.clone();
        let update_trigger = update_trigger.clone();
        let _dispatch = _dispatch.clone();

        use_effect_with(*update_trigger, move |_| {
            let server_name = effect_state
                .auth_details
                .as_ref()
                .map(|ud| ud.server_name.clone());
            let api_key = effect_state
                .auth_details
                .as_ref()
                .and_then(|ud| ud.api_key.clone());

            if let (Some(server_name), Some(api_key)) = (server_name, api_key) {
                wasm_bindgen_futures::spawn_local(async move {
                    match call_list_oidc_providers(server_name, api_key).await {
                        Ok(fetched_providers) => {
                            providers.set(fetched_providers);
                        }
                        Err(e) => {
                            let formatted_error = format_error_message(&e.to_string());
                            _dispatch.reduce_mut(|state| {
                                state.error_message = Some(format!(
                                    "{}{}",
                                    i18n_failed_to_fetch_oidc_providers.clone(),
                                    formatted_error
                                ));
                            });
                        }
                    }
                });
            }
            || ()
        });
    }

    let on_add_provider = {
        let page_state = page_state.clone();
        let clear_form = clear_form.clone();
        Callback::from(move |_| {
            clear_form();
            page_state.set(PageState::AddProvider);
        })
    };

    let on_edit_provider = {
        let page_state = page_state.clone();
        let populate_form_for_edit = populate_form_for_edit.clone();
        let providers = providers.clone();
        Callback::from(move |provider_id: i32| {
            if let Some(provider) = providers.iter().find(|p| p.provider_id == provider_id) {
                populate_form_for_edit(provider);
                page_state.set(PageState::EditProvider(provider_id));
            }
        })
    };

    let on_close_modal = {
        let page_state = page_state.clone();
        Callback::from(move |_: MouseEvent| {
            page_state.set(PageState::Hidden);
        })
    };

    let on_background_click = {
        let on_close_modal = on_close_modal.clone();
        Callback::from(move |e: MouseEvent| {
            let target = e.target().unwrap();
            let element = target.dyn_into::<web_sys::Element>().unwrap();
            if element.tag_name() == "DIV" {
                on_close_modal.emit(e);
            }
        })
    };

    let stop_propagation = Callback::from(|e: MouseEvent| {
        e.stop_propagation();
    });

    let remove_state = state.clone();
    let on_remove_provider = {
        let update_trigger = update_trigger.clone();
        let _dispatch = _dispatch.clone();
        let i18n_provider_successfully_removed = i18n_provider_successfully_removed.clone();
        let i18n_failed_to_remove_provider = i18n_failed_to_remove_provider.clone();

        Callback::from(move |provider_id: i32| {
            let i18n_provider_successfully_removed = i18n_provider_successfully_removed.clone();
            let i18n_failed_to_remove_provider = i18n_failed_to_remove_provider.clone();
            let server_name = remove_state
                .auth_details
                .as_ref()
                .map(|ud| ud.server_name.clone());
            let api_key = remove_state
                .auth_details
                .as_ref()
                .and_then(|ud| ud.api_key.clone());
            let update_trigger = update_trigger.clone();
            let _dispatch = _dispatch.clone();

            if let (Some(server_name), Some(api_key)) = (server_name, api_key) {
                wasm_bindgen_futures::spawn_local(async move {
                    match call_remove_oidc_provider(server_name, api_key, provider_id).await {
                        Ok(_) => {
                            update_trigger.set(!*update_trigger);
                            _dispatch.reduce_mut(|state| {
                                state.info_message =
                                    Some(i18n_provider_successfully_removed.clone());
                            });
                        }
                        Err(e) => {
                            let formatted_error = format_error_message(&e.to_string());
                            _dispatch.reduce_mut(|state| {
                                state.error_message = Some(format!(
                                    "{}{}",
                                    i18n_failed_to_remove_provider, formatted_error
                                ));
                            });
                        }
                    }
                });
            }
        })
    };

    // Create provider modal
    // Input handlers
    let on_provider_name_change = {
        let provider_name = provider_name.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target_unchecked_into::<HtmlInputElement>();
            provider_name.set(target.value());
        })
    };

    let on_client_id_change = {
        let client_id = client_id.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target_unchecked_into::<HtmlInputElement>();
            client_id.set(target.value());
        })
    };

    let on_client_secret_change = {
        let client_secret = client_secret.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target_unchecked_into::<HtmlInputElement>();
            client_secret.set(target.value());
        })
    };

    let on_auth_url_change = {
        let auth_url = auth_url.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target_unchecked_into::<HtmlInputElement>();
            auth_url.set(target.value());
        })
    };

    let on_token_url_change = {
        let token_url = token_url.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target_unchecked_into::<HtmlInputElement>();
            token_url.set(target.value());
        })
    };

    let on_user_info_url_change = {
        let user_info_url = user_info_url.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target_unchecked_into::<HtmlInputElement>();
            user_info_url.set(target.value());
        })
    };

    let on_button_text_change = {
        let button_text = button_text.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target_unchecked_into::<HtmlInputElement>();
            button_text.set(target.value());
        })
    };

    let on_button_color_change = {
        let button_color = button_color.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target_unchecked_into::<HtmlInputElement>();
            button_color.set(target.value());
        })
    };

    let on_button_text_color_change = {
        let button_text_color = button_text_color.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target_unchecked_into::<HtmlInputElement>();
            button_text_color.set(target.value());
        })
    };

    let on_icon_svg_change = {
        let icon_svg = icon_svg.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target_unchecked_into::<HtmlInputElement>();
            icon_svg.set(target.value());
        })
    };

    let on_name_claim_change = {
        let name_claim = name_claim.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target_unchecked_into::<HtmlInputElement>();
            name_claim.set(target.value());
        })
    };

    let on_email_claim_change = {
        let email_claim = email_claim.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target_unchecked_into::<HtmlInputElement>();
            email_claim.set(target.value());
        })
    };

    let on_username_claim_change = {
        let username_claim = username_claim.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target_unchecked_into::<HtmlInputElement>();
            username_claim.set(target.value());
        })
    };

    let on_roles_claim_change = {
        let roles_claim = roles_claim.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target_unchecked_into::<HtmlInputElement>();
            roles_claim.set(target.value());
        })
    };

    let on_user_role_change = {
        let user_role = user_role.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target_unchecked_into::<HtmlInputElement>();
            user_role.set(target.value());
        })
    };

    let on_admin_role_change = {
        let admin_role = admin_role.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target_unchecked_into::<HtmlInputElement>();
            admin_role.set(target.value());
        })
    };

    let submit_state = state.clone();
    let on_submit = {
        let provider_name = provider_name.clone();
        let client_id = client_id.clone();
        let client_secret = client_secret.clone();
        let auth_url = auth_url.clone();
        let token_url = token_url.clone();
        let user_info_url = user_info_url.clone();
        let button_text = button_text.clone();
        let button_color = button_color.clone();
        let button_text_color = button_text_color.clone();
        let icon_svg = icon_svg.clone();
        let name_claim = name_claim.clone();
        let email_claim = email_claim.clone();
        let username_claim = username_claim.clone();
        let roles_claim = roles_claim.clone();
        let user_role = user_role.clone();
        let admin_role = admin_role.clone();
        let page_state = page_state.clone();
        let update_trigger = update_trigger.clone();
        let _dispatch = _dispatch.clone();
        let selected_scopes = selected_scopes.clone();
        let i18n_oidc_provider_successfully_added = i18n_oidc_provider_successfully_added.clone();
        let i18n_failed_to_add_provider = i18n_failed_to_add_provider.clone();
        let i18n_oidc_provider_successfully_updated =
            i18n_oidc_provider_successfully_updated.clone();
        let i18n_failed_to_update_provider = i18n_failed_to_update_provider.clone();

        Callback::from(move |e: SubmitEvent| {
            let i18n_oidc_provider_successfully_added =
                i18n_oidc_provider_successfully_added.clone();
            let i18n_failed_to_add_provider = i18n_failed_to_add_provider.clone();
            let i18n_oidc_provider_successfully_updated =
                i18n_oidc_provider_successfully_updated.clone();
            let i18n_failed_to_update_provider = i18n_failed_to_update_provider.clone();
            let call_trigger = update_trigger.clone();
            let call_page_state = page_state.clone();
            let call_dispatch = _dispatch.clone();
            e.prevent_default();

            // Calculate detected_provider inside the callback so it uses current values
            let detected_provider = detect_provider(&(*auth_url), &(*token_url), &(*user_info_url));

            let provider = AddOIDCProviderRequest {
                provider_name: (*provider_name).clone(),
                client_id: (*client_id).clone(),
                client_secret: (*client_secret).clone(),
                authorization_url: (*auth_url).clone(),
                token_url: (*token_url).clone(),
                user_info_url: (*user_info_url).clone(),
                button_text: (*button_text).clone(),
                scope: Some(format_scopes_for_request(
                    &*selected_scopes,
                    &detected_provider,
                )),
                button_color: Some((*button_color).clone()),
                button_text_color: Some((*button_text_color).clone()),
                icon_svg: Some((*icon_svg).clone()),
                name_claim: Some((*name_claim).clone()),
                email_claim: Some((*email_claim).clone()),
                username_claim: Some((*username_claim).clone()),
                roles_claim: Some((*roles_claim).clone()),
                user_role: Some((*user_role).clone()),
                admin_role: Some((*admin_role).clone()),
            };

            // Rest of your submission code...
            let server_name = submit_state
                .auth_details
                .as_ref()
                .map(|ud| ud.server_name.clone());
            let api_key = submit_state
                .auth_details
                .as_ref()
                .and_then(|ud| ud.api_key.clone());

            if let (Some(server_name), Some(api_key)) = (server_name, api_key) {
                let current_page_state = (*call_page_state).clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match current_page_state {
                        PageState::AddProvider => {
                            match call_add_oidc_provider(server_name, api_key, provider).await {
                                Ok(_) => {
                                    call_trigger.set(!*call_trigger);
                                    call_page_state.set(PageState::Hidden);
                                    call_dispatch.reduce_mut(|state| {
                                        state.info_message =
                                            Some(i18n_oidc_provider_successfully_added.clone());
                                    });
                                }
                                Err(e) => {
                                    let formatted_error = format_error_message(&e.to_string());
                                    call_dispatch.reduce_mut(|state| {
                                        state.error_message = Some(format!(
                                            "{}{}",
                                            i18n_failed_to_add_provider, formatted_error
                                        ));
                                    });
                                }
                            }
                        }
                        PageState::EditProvider(provider_id) => {
                            match call_update_oidc_provider(
                                server_name,
                                api_key,
                                provider_id,
                                provider,
                            )
                            .await
                            {
                                Ok(_) => {
                                    call_trigger.set(!*call_trigger);
                                    call_page_state.set(PageState::Hidden);
                                    call_dispatch.reduce_mut(|state| {
                                        state.info_message =
                                            Some(i18n_oidc_provider_successfully_updated.clone());
                                    });
                                }
                                Err(e) => {
                                    let formatted_error = format_error_message(&e.to_string());
                                    call_dispatch.reduce_mut(|state| {
                                        state.error_message = Some(format!(
                                            "{}{}",
                                            i18n_failed_to_update_provider, formatted_error
                                        ));
                                    });
                                }
                            }
                        }
                        PageState::Hidden => {
                            // This shouldn't happen, but handle gracefully
                        }
                    }
                });
            }
        })
    };

    let redirect_url = if let Some(auth_details) = &state.auth_details {
        format!("{}/api/auth/callback", auth_details.server_name)
    } else {
        "https://your-pinepods-instance/api/auth/callback".to_string()
    };

    let onclick_copy = {
        let url = redirect_url.clone();
        Callback::from(move |_| {
            if let Some(window) = web_sys::window() {
                let clipboard = window.navigator().clipboard();
                let _ = clipboard.write_text(&url);
            }
        })
    };

    let add_provider_modal = html! {
            <div id="add-provider-modal" tabindex="-1" aria-hidden="true"
                class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25"
                onclick={on_background_click.clone()}>
                <div class="modal-container relative p-4 w-full max-w-2xl max-h-[80vh] rounded-lg shadow overflow-y-auto"
                    onclick={stop_propagation.clone()}>
                    <div class="modal-container relative rounded-lg shadow">
                        <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t">
                            <h3 class="text-xl font-semibold">
    {match &*page_state {
        PageState::AddProvider => &i18n_add_oidc_provider,
        PageState::EditProvider(_) => &i18n_edit_oidc_provider,
        PageState::Hidden => &i18n_add_oidc_provider, // fallback
    }}
                            </h3>
                            <button onclick={on_close_modal.clone()}
                                class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                                <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                    <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                                </svg>
                                <span class="sr-only">{&i18n_close_modal}</span>
                            </button>
                        </div>
                        <div class="p-4 md:p-5">
                            <div class="bg-indigo-50 dark:bg-indigo-900/20 border border-indigo-200 dark:border-indigo-800 rounded-lg p-4 mb-6">
                                <div class="flex items-center gap-2 mb-2">
                                    <i class="ph ph-info text-indigo-600 dark:text-indigo-400"></i>
                                    <h3 class="font-medium text-indigo-900 dark:text-indigo-100">{&i18n_oidc_redirect_url}</h3>
                                </div>
                                <div class="flex items-center gap-2 bg-white dark:bg-gray-800 rounded p-2">
                                    <code class="text-sm text-gray-800 dark:text-gray-200 flex-grow">
                                        {redirect_url.clone()}
                                    </code>
                                    <button
                                        onclick={onclick_copy.clone()}
                                        class="text-indigo-600 dark:text-indigo-400 hover:text-indigo-700 dark:hover:text-indigo-300 p-1 rounded"
                                        title="Copy to clipboard"
                                    >
                                        <i class="ph ph-copy text-lg"></i>
                                    </button>
                                </div>
                                <p class="text-sm text-indigo-700 dark:text-indigo-300 mt-2">
    {&i18n_use_this_url_when_configuring}
                                </p>
                            </div>
                            <form class="space-y-4" action="#" onsubmit={on_submit}>
                                <div class="grid grid-cols-2 gap-4">
                                    <div class="form-group">
                                        <label class="form-label">{&i18n_provider_name}</label>
                                        <input
                                            type="text"
                                            class="form-input"
                                            value={(*provider_name).clone()}
                                            oninput={on_provider_name_change}
                                            placeholder="Google"
                                            required=true
                                        />
                                    </div>
                                    <div class="form-group">
                                        <label class="form-label">{&i18n_client_id}</label>
                                        <input
                                            type="text"
                                            class="form-input"
                                            value={(*client_id).clone()}
                                            oninput={on_client_id_change}
                                            required=true
                                        />
                                    </div>
                                    <div class="form-group">
                                        <label class="form-label">{&i18n_client_secret}
                                            {match &*page_state {
                                                PageState::EditProvider(_) => html! { <span class="text-sm opacity-70">{" (leave empty to keep current)"}</span> },
                                                _ => html! {}
                                            }}
                                        </label>
                                        <input
                                            type="password"
                                            class="form-input"
                                            value={(*client_secret).clone()}
                                            oninput={on_client_secret_change}
                                            required={match &*page_state {
                                                PageState::AddProvider => true,
                                                PageState::EditProvider(_) => false,
                                                PageState::Hidden => false,
                                            }}
                                        />
                                    </div>
                                    <div class="form-group">
                                        <label class="form-label">{&i18n_authorization_url}</label>
                                        <input
                                            type="url"
                                            class="form-input"
                                            value={(*auth_url).clone()}
                                            oninput={on_auth_url_change}
                                            placeholder="https://provider.com/oauth2/auth"
                                            required=true
                                        />
                                    </div>
                                    <div class="form-group">
                                        <label class="form-label">{&i18n_token_url}</label>
                                        <input
                                            type="url"
                                            class="form-input"
                                            value={(*token_url).clone()}
                                            oninput={on_token_url_change}
                                            placeholder="https://provider.com/oauth2/token"
                                            required=true
                                        />
                                    </div>
                                    <div class="form-group">
                                        <label class="form-label">{&i18n_user_info_url}</label>
                                        <input
                                            type="url"
                                            class="form-input"
                                            value={(*user_info_url).clone()}
                                            oninput={on_user_info_url_change}
                                            placeholder="https://provider.com/oauth2/userinfo"
                                            required=true
                                        />
                                    </div>
                                    <div class="form-group col-span-2">
                                        <label class="form-label">{"Scopes"}</label>
                                        <ScopeSelector
                                            selected_scopes={(*selected_scopes).clone()}
                                            on_select={scope_on_select}
                                            auth_url={(*auth_url).clone()}
                                            token_url={(*token_url).clone()}
                                            user_info_url={(*user_info_url).clone()}
                                        />
                                    </div>
                                    <div class="form-group">
                                        <label class="form-label">{"Button Text"}</label>
                                        <input
                                            type="text"
                                            class="form-input"
                                            value={(*button_text).clone()}
                                            oninput={on_button_text_change}
                                            placeholder="Login with Provider"
                                            required=true
                                        />
                                    </div>
                                    <div class="form-group">
                                        <label class="form-label">{"Button Color"}</label>
                                        <input
                                            type="color"
                                            class="form-input h-[42px]"
                                            value={(*button_color).clone()}
                                            oninput={on_button_color_change}
                                        />
                                    </div>
                                    <div class="form-group">
                                        <label class="form-label">{"Button Text Color"}</label>
                                        <input
                                            type="color"
                                            class="form-input h-[42px]"
                                            value={(*button_text_color).clone()}
                                            oninput={on_button_text_color_change}
                                        />
                                    </div>
                                    <div class="form-group col-span-2">
                                        <label class="form-label">{"Icon SVG (optional)"}</label>
                                        <textarea
                                            class="form-input min-h-[100px]"
                                            value={(*icon_svg).clone()}
                                            oninput={on_icon_svg_change}
                                            placeholder="<svg>...</svg>"
                                        />
                                    </div>
                                    <div class="form-group">
                                        <label class="form-label">{"Name Claim"}</label>
                                        <input
                                            type="text"
                                            class="form-input"
                                            value={(*name_claim).clone()}
                                            oninput={on_name_claim_change}
                                        />
                                    </div>
                                    <div class="form-group">
                                        <label class="form-label">{"Email Claim"}</label>
                                        <input
                                            type="text"
                                            class="form-input"
                                            value={(*email_claim).clone()}
                                            oninput={on_email_claim_change}
                                        />
                                    </div>
                                    <div class="form-group">
                                        <label class="form-label">{"Username Claim"}</label>
                                        <input
                                            type="text"
                                            class="form-input"
                                            value={(*username_claim).clone()}
                                            oninput={on_username_claim_change}
                                        />
                                    </div>
                                    <div class="form-group">
                                        <label class="form-label">{"Roles Claim"}</label>
                                        <input
                                            type="text"
                                            class="form-input"
                                            value={(*roles_claim).clone()}
                                            oninput={on_roles_claim_change}
                                        />
                                    </div>
                                    <div class="form-group">
                                        <label class="form-label">{"User Role"}</label>
                                        <input
                                            type="text"
                                            class="form-input"
                                            value={(*user_role).clone()}
                                            oninput={on_user_role_change}
                                        />
                                    </div>
                                    <div class="form-group">
                                        <label class="form-label">{"Admin Role"}</label>
                                        <input
                                            type="text"
                                            class="form-input"
                                            value={(*admin_role).clone()}
                                            oninput={on_admin_role_change}
                                        />
                                    </div>
                                </div>
                                <div class="flex justify-end mt-4">
                                    <button type="submit" class="download-button focus:ring-4 focus:outline-none font-medium rounded-lg text-sm px-5 py-2.5 text-center">
    {match &*page_state {
        PageState::AddProvider => &i18n_add,
        PageState::EditProvider(_) => &i18n_update,
        PageState::Hidden => &i18n_submit, // fallback
    }}
                                    </button>
                                </div>
                            </form>
                        </div>
                    </div>
                </div>
            </div>
        };

    html! {
            <div class="user-settings-container">

                <div class="bg-indigo-50 dark:bg-indigo-900/20 border border-indigo-200 dark:border-indigo-800 rounded-lg p-4 mb-6">
                    <div class="flex items-center gap-2 mb-2">
                        <i class="ph ph-info text-indigo-600 dark:text-indigo-400"></i>
                        <h3 class="font-medium text-indigo-900 dark:text-indigo-100">{"OIDC Redirect URL"}</h3>
                    </div>
                    <div class="flex items-center gap-2 bg-white dark:bg-gray-800 rounded p-2">
                        <code class="text-sm text-gray-800 dark:text-gray-200 flex-grow">
                            {redirect_url}
                        </code>
                        <button
                            onclick={onclick_copy}
                            class="text-indigo-600 dark:text-indigo-400 hover:text-indigo-700 dark:hover:text-indigo-300 p-1 rounded"
                            title="Copy to clipboard"
                        >
                            <i class="ph ph-copy text-lg"></i>
                        </button>
                    </div>
                    <p class="text-sm text-indigo-700 dark:text-indigo-300 mt-2">
                        {"Use this URL when configuring your OIDC provider's callback/redirect settings."}
                    </p>
                </div>

                <div class="settings-header">
                    <div class="flex items-center gap-4">
                        <i class="ph ph-key text-2xl"></i>
                        <h2 class="text-xl font-semibold">{&i18n_oidc_provider_management}</h2>
                    </div>
                </div>

                <div class="mb-6">
                    <button onclick={on_add_provider} class="settings-button">
                        <i class="ph ph-plus"></i>
    {&i18n_add_provider}
                    </button>
                </div>

                if (*providers).is_empty() {
                    <div class="oidc-empty-state">
                        <p>{&i18n_no_oidc_providers_configured}</p>
                    </div>
                } else {
                    {
                        (*providers).iter().map(|provider| {
                            let on_remove = {
                                let on_remove_provider = on_remove_provider.clone();
                                let provider_id = provider.provider_id;
                                Callback::from(move |_| {
                                    on_remove_provider.emit(provider_id);
                                })
                            };

                            let on_edit = {
                                let on_edit_provider = on_edit_provider.clone();
                                let provider_id = provider.provider_id;
                                Callback::from(move |_| {
                                    on_edit_provider.emit(provider_id);
                                })
                            };

                            html! {
                                <div class="oidc-provider-card">
                                    <div class="oidc-provider-header">
                                        <div>
                                            <div class="flex items-center gap-2">
                                                <h3 class="text-lg font-medium">{&provider.provider_name}</h3>
                                                if provider.initialized_from_env {
                                                    <span class="px-2 py-1 text-xs rounded-full bg-blue-900/20 text-blue-300 border border-blue-800/30">
                                                        {"Environment"}
                                                    </span>
                                                }
                                            </div>
                                            <p class="text-sm opacity-70">{&provider.client_id}</p>
                                        </div>
                                        <div class="flex items-center gap-2">
                                            <button onclick={on_edit} class="oidc-edit-button">
                                                <i class="ph ph-pencil"></i>
                                                {"Edit"}
                                            </button>
                                            if !provider.initialized_from_env {
                                                <button onclick={on_remove} class="oidc-remove-button">
                                                    <i class="ph ph-trash"></i>
                                                    {&i18n_remove}
                                                </button>
                                            } else {
                                                <button
                                                    class="oidc-remove-button opacity-50 cursor-not-allowed"
                                                    disabled=true
                                                    title="Cannot remove environment-initialized providers"
                                                >
                                                    <i class="ph ph-trash"></i>
                                                    {&i18n_remove}
                                                </button>
                                            }
                                        </div>
                                    </div>
                                    <div class="oidc-provider-info">
                                        <div class="oidc-info-group">
                                            <div class="oidc-info-label">{&i18n_authorization_url}</div>
                                            <div class="oidc-info-value">{&provider.authorization_url}</div>
                                        </div>
                                        <div class="oidc-info-group">
                                            <div class="oidc-info-label">{&i18n_token_url}</div>
                                            <div class="oidc-info-value">{&provider.token_url}</div>
                                        </div>
                                        <div class="oidc-info-group">
                                            <div class="oidc-info-label">{&i18n_user_info_url}</div>
                                            <div class="oidc-info-value">{&provider.user_info_url}</div>
                                        </div>
                                    </div>
                                </div>
                            }
                        }).collect::<Html>()
                    }
                }

                {
                    match *page_state {
                        PageState::AddProvider => add_provider_modal,
                        PageState::EditProvider(_) => add_provider_modal, // Reuse the same modal for editing
                        PageState::Hidden => html! {},
                    }
                }
            </div>
        }
}
