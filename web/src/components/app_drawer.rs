use std::rc::Rc;
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};
use yew_router::prelude::Link;
use yewdux::Dispatch;
use crate::components::context::AppState;
use crate::requests::search_pods::{call_get_podcast_info, test_connection};
use super::routes::Route;


#[function_component(App_drawer)]
pub fn app_drawer() -> Html {
    let selection = use_state(|| "".to_string());

    let is_drawer_open = use_state(|| false);

    let toggle_drawer = {
        let is_drawer_open = is_drawer_open.clone();
        move |_event: MouseEvent| {
            is_drawer_open.set(!*is_drawer_open);
            if let Some(window) = web_sys::window() {
                let body = window.document().unwrap().body().unwrap();
                if !*is_drawer_open {
                    body.class_list().add_1("no-scroll").unwrap();
                } else {
                    body.class_list().remove_1("no-scroll").unwrap();
                }
            }
        }
    };

    // let close_drawer = {
    //     let toggle_drawer = toggle_drawer.clone();
    //     Callback::from(move |_| {
    //         toggle_drawer(());
    //     })
    // };

    let on_selection_change = {
        let selection = selection.clone();
        Callback::from(move |e: InputEvent| {
            selection.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
        })
    };

    // let on_select = {
    //     let selection = selection.clone();
    //     Callback::from(move |_| {
    //         // Handle the login logic here
    //         // For example, send the username and password to a server
    //         let message = format!("Selected: {}", *selection);
    //         console::log_1(&message.into());
    //     })
    // };

    html! {
        <div class="relative min-h-screen">
            // Drawer
            <div class={classes!("fixed", "drawer-background", "top-0", "left-0", "z-20", "h-full", "transition-all", "duration-500", "transform", "shadow-lg", "md:w-64", "w-full", (*is_drawer_open).then(|| "translate-x-0").unwrap_or("-translate-x-full"))}>
                <div class="flex flex-col justify-between h-full">
                    <div class="px-6 py-4 mt-16">
                        <h2 class="text-lg font-semibold">{"Pinepods"}</h2>
                        <div class="space-y-4">
                            // User Account with Gravatar
                            <div class="flex items-center space-x-3">
                                <Link<Route> to={Route::UserStats}>
                                    // Initially, use the placeholder image
                                    <img src={"/static/assets/favicon.png"} style="width: 25px; height: 25px;" class="icon-size" alt="User Avatar" />
                                    <span class="text-lg">{"User's Account"}</span>
                                </Link<Route>>
                            </div>
                            // Other Links
                            <div class="flex items-center space-x-3">
                                <div onclick={toggle_drawer.clone()} class="flex items-center space-x-3 cursor-pointer">
                                    <Link<Route> to={Route::Home}>
                                        <span class="material-icons">{"home"}</span>
                                        <span class="text-lg">{"Home"}</span>
                                    </Link<Route>>
                                </div>
                            </div>
                            <div class="flex items-center space-x-3">
                                <div onclick={toggle_drawer.clone()} class="flex items-center space-x-3 cursor-pointer">
                                    <Link<Route> to={Route::Search}>
                                        <span class="material-icons">{"search"}</span>
                                        <span class="text-lg">{"Search Podcasts"}</span>
                                    </Link<Route>>
                                </div>
                            </div>
                            <div class="flex items-center space-x-3">
                                <div onclick={toggle_drawer.clone()} class="flex items-center space-x-3 cursor-pointer">
                                    <Link<Route> to={Route::Queue}>
                                    <span class="material-icons">{"queue"}</span>
                                    <span class="text-lg">{"Queue"}</span>
                                    </Link<Route>>
                                </div>
                            </div>
                            <div class="flex items-center space-x-3">
                                <div onclick={toggle_drawer.clone()} class="flex items-center space-x-3 cursor-pointer">
                                    <Link<Route> to={Route::Saved}>
                                        <span class="material-icons">{"star"}</span>
                                        <span class="text-lg">{"Saved"}</span>
                                    </Link<Route>>
                                </div>
                            </div>
                            <div class="flex items-center space-x-3">
                                <div onclick={toggle_drawer.clone()} class="flex items-center space-x-3 cursor-pointer">
                                    <Link<Route> to={Route::PodHistory}>
                                        <span class="material-icons">{"history"}</span>
                                        <span class="text-lg">{"History"}</span>
                                    </Link<Route>>
                                </div>
                            </div>
                            <div class="flex items-center space-x-3">
                                <div onclick={toggle_drawer.clone()} class="flex items-center space-x-3 cursor-pointer">
                                    <Link<Route> to={Route::Downloads}>
                                        <span class="material-icons">{"download"}</span>
                                        <span class="text-lg">{"Downloads"}</span>
                                    </Link<Route>>
                                </div>
                            </div>
                            <div class="flex items-center space-x-3">
                                <div onclick={toggle_drawer.clone()} class="flex items-center space-x-3 cursor-pointer">
                                    <Link<Route> to={Route::Settings}>
                                        <span class="material-icons">{"settings"}</span>
                                        <span class="text-lg">{"Settings"}</span>
                                    </Link<Route>>
                                </div>
                            </div>
                            <div class="px-6 py-4">
                                <div class="flex items-center space-x-3">
                                    <div onclick={toggle_drawer.clone()} class="flex items-center space-x-3 cursor-pointer">
                                        <Link<Route> to={Route::LogOut}>
                                            <span class="material-icons">{"logout"}</span>
                                            <span class="text-lg">{"Sign Out"}</span>
                                        </Link<Route>>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            // Toggle button - Fixed Position
            <div class="drawer-icon" onclick={toggle_drawer.clone()}>
                // <div class="drawer-button">
                    <label for="drawer-toggle" class="bg-indigo-500 rounded-lg cursor-pointer">
                        <div class="flex flex-col items-center">
                            <div class="w-6 h-1 mb-1 bg-white rounded-lg"></div>
                            <div class="w-6 h-1 mb-1 bg-white rounded-lg"></div>
                            <div class="w-6 h-1 bg-white rounded-lg"></div>
                        </div>
                    </label>
                // </div>
            </div>

            // <input
            //     type="checkbox"
            //     id="drawer-toggle"
            //     class="sr-only"
            //     checked={*is_drawer_open}
            //     onclick={toggle_drawer.clone()}
            // />
        </div>
    }


}