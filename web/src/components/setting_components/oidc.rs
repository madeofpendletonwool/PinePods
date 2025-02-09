use crate::components::context::{AppState, UIState};
use crate::requests::setting_reqs::{
    call_add_oidc_provider, call_list_oidc_providers, call_remove_oidc_provider,
    AddOIDCProviderRequest, OIDCProvider,
};
use wasm_bindgen::JsCast;
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yewdux::prelude::*;

#[derive(Clone, PartialEq)]
enum PageState {
    Hidden,
    AddProvider,
}

#[function_component(OIDCSettings)]
pub fn oidc_settings() -> Html {
    let (state, _dispatch) = use_store::<AppState>();
    let (_audio_state, audio_dispatch) = use_store::<UIState>();
    let page_state = use_state(|| PageState::Hidden);
    let providers = use_state(|| Vec::<OIDCProvider>::new());
    let update_trigger = use_state(|| false);

    // Form states for the add provider modal
    let provider_name = use_state(|| String::new());
    let client_id = use_state(|| String::new());
    let client_secret = use_state(|| String::new());
    let auth_url = use_state(|| String::new());
    let token_url = use_state(|| String::new());
    let user_info_url = use_state(|| String::new());
    let redirect_url = use_state(|| String::new());
    let button_text = use_state(|| String::new());
    let button_color = use_state(|| String::from("#000000"));
    let button_text_color = use_state(|| String::from("#000000"));
    let icon_svg = use_state(|| String::new());

    // Fetch providers on component mount and when update_trigger changes
    let effect_state = state.clone();
    {
        let providers = providers.clone();
        let update_trigger = update_trigger.clone();
        let audio_dispatch = audio_dispatch.clone();

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
                            audio_dispatch.reduce_mut(|state| {
                                state.error_message =
                                    Some(format!("Failed to fetch OIDC providers: {}", e));
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
        Callback::from(move |_| {
            page_state.set(PageState::AddProvider);
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
        let audio_dispatch = audio_dispatch.clone();

        Callback::from(move |provider_id: i32| {
            let server_name = remove_state
                .auth_details
                .as_ref()
                .map(|ud| ud.server_name.clone());
            let api_key = remove_state
                .auth_details
                .as_ref()
                .and_then(|ud| ud.api_key.clone());
            let update_trigger = update_trigger.clone();
            let audio_dispatch = audio_dispatch.clone();

            if let (Some(server_name), Some(api_key)) = (server_name, api_key) {
                wasm_bindgen_futures::spawn_local(async move {
                    match call_remove_oidc_provider(server_name, api_key, provider_id).await {
                        Ok(_) => {
                            update_trigger.set(!*update_trigger);
                            audio_dispatch.reduce_mut(|state| {
                                state.info_message =
                                    Some("Provider successfully removed".to_string());
                            });
                        }
                        Err(e) => {
                            audio_dispatch.reduce_mut(|state| {
                                state.error_message =
                                    Some(format!("Failed to remove provider: {}", e));
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

    let on_redirect_url_change = {
        let redirect_url = redirect_url.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target_unchecked_into::<HtmlInputElement>();
            redirect_url.set(target.value());
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
        let page_state = page_state.clone();
        let update_trigger = update_trigger.clone();
        let audio_dispatch = audio_dispatch.clone();

        Callback::from(move |e: SubmitEvent| {
            let call_trigger = update_trigger.clone();
            let call_page_state = page_state.clone();
            let call_dispatch = audio_dispatch.clone();
            e.prevent_default();
            let provider = AddOIDCProviderRequest {
                provider_name: (*provider_name).clone(),
                client_id: (*client_id).clone(),
                client_secret: (*client_secret).clone(),
                authorization_url: (*auth_url).clone(),
                token_url: (*token_url).clone(),
                user_info_url: (*user_info_url).clone(),
                button_text: (*button_text).clone(),
                scope: Some("openid email profile".to_string()),
                button_color: Some((*button_color).clone()),
                button_text_color: Some((*button_text_color).clone()),
                icon_svg: Some((*icon_svg).clone()),
            };

            let server_name = submit_state
                .auth_details
                .as_ref()
                .map(|ud| ud.server_name.clone());
            let api_key = submit_state
                .auth_details
                .as_ref()
                .and_then(|ud| ud.api_key.clone());

            if let (Some(server_name), Some(api_key)) = (server_name, api_key) {
                wasm_bindgen_futures::spawn_local(async move {
                    match call_add_oidc_provider(server_name, api_key, provider).await {
                        Ok(_) => {
                            call_trigger.set(!*call_trigger);
                            call_page_state.set(PageState::Hidden);
                            call_dispatch.reduce_mut(|state| {
                                state.info_message =
                                    Some("OIDC Provider successfully added".to_string());
                            });
                        }
                        Err(e) => {
                            call_dispatch.reduce_mut(|state| {
                                state.error_message =
                                    Some(format!("Failed to add provider: {}", e));
                            });
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
                            {"Add OIDC Provider"}
                        </h3>
                        <button onclick={on_close_modal.clone()}
                            class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                            <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                            </svg>
                            <span class="sr-only">{"Close modal"}</span>
                        </button>
                    </div>
                    <div class="p-4 md:p-5">
                        <div class="bg-indigo-50 dark:bg-indigo-900/20 border border-indigo-200 dark:border-indigo-800 rounded-lg p-4 mb-6">
                            <div class="flex items-center gap-2 mb-2">
                                <i class="ph ph-info text-indigo-600 dark:text-indigo-400"></i>
                                <h3 class="font-medium text-indigo-900 dark:text-indigo-100">{"OIDC Redirect URL"}</h3>
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
                                {"Use this URL when configuring your OIDC provider's callback/redirect settings."}
                            </p>
                        </div>
                        <form class="space-y-4" action="#" onsubmit={on_submit}>
                            <div class="grid grid-cols-2 gap-4">
                                <div class="form-group">
                                    <label class="form-label">{"Provider Name"}</label>
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
                                    <label class="form-label">{"Client ID"}</label>
                                    <input
                                        type="text"
                                        class="form-input"
                                        value={(*client_id).clone()}
                                        oninput={on_client_id_change}
                                        required=true
                                    />
                                </div>
                                <div class="form-group">
                                    <label class="form-label">{"Client Secret"}</label>
                                    <input
                                        type="password"
                                        class="form-input"
                                        value={(*client_secret).clone()}
                                        oninput={on_client_secret_change}
                                        required=true
                                    />
                                </div>
                                <div class="form-group">
                                    <label class="form-label">{"Authorization URL"}</label>
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
                                    <label class="form-label">{"Token URL"}</label>
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
                                    <label class="form-label">{"User Info URL"}</label>
                                    <input
                                        type="url"
                                        class="form-input"
                                        value={(*user_info_url).clone()}
                                        oninput={on_user_info_url_change}
                                        placeholder="https://provider.com/oauth2/userinfo"
                                        required=true
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
                            </div>
                            <div class="flex justify-end mt-4">
                                <button type="submit" class="download-button focus:ring-4 focus:outline-none font-medium rounded-lg text-sm px-5 py-2.5 text-center">
                                    {"Submit"}
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
                    <h2 class="text-xl font-semibold">{"OIDC Provider Management"}</h2>
                </div>
            </div>

            <div class="mb-6">
                <button onclick={on_add_provider} class="settings-button">
                    <i class="ph ph-plus"></i>
                    {" Add Provider"}
                </button>
            </div>

            if (*providers).is_empty() {
                <div class="oidc-empty-state">
                    <p>{"No OIDC providers configured yet. Add one to enable single sign-on."}</p>
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

                        html! {
                            <div class="oidc-provider-card">
                                <div class="oidc-provider-header">
                                    <div>
                                        <h3 class="text-lg font-medium">{&provider.provider_name}</h3>
                                        <p class="text-sm opacity-70">{&provider.client_id}</p>
                                    </div>
                                    <button onclick={on_remove} class="oidc-remove-button">
                                        <i class="ph ph-trash"></i>
                                        {" Remove"}
                                    </button>
                                </div>
                                <div class="oidc-provider-info">
                                    <div class="oidc-info-group">
                                        <div class="oidc-info-label">{"Authorization URL"}</div>
                                        <div class="oidc-info-value">{&provider.authorization_url}</div>
                                    </div>
                                    <div class="oidc-info-group">
                                        <div class="oidc-info-label">{"Token URL"}</div>
                                        <div class="oidc-info-value">{&provider.token_url}</div>
                                    </div>
                                    <div class="oidc-info-group">
                                        <div class="oidc-info-label">{"User Info URL"}</div>
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
                    PageState::Hidden => html! {},
                }
            }
        </div>
    }
}
