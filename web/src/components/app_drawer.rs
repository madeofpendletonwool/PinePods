use yew::prelude::*;
use yew_router::prelude::Link;
use yewdux::{use_store};
use crate::components::context::AppState;
use super::routes::Route;

#[allow(non_camel_case_types)]
#[function_component(App_drawer)]
pub fn app_drawer() -> Html {
    // let selection = use_state(|| "".to_string());
    // let (state, _dispatch) = use_store::<AppState>();

    let is_drawer_open = use_state(|| false);
    let (state, _dispatch) = use_store::<AppState>();
    let username = state.user_details.as_ref().map_or("Guest".to_string(), |ud| ud.Username.clone().unwrap());
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

    html! {
        <div class="relative min-h-screen">
            // Drawer
            <div class={classes!("fixed", "drawer-background", "top-0", "left-0", "z-20", "h-full", "transition-all", "duration-500", "transform", "shadow-lg", "md:w-64", "w-full", "border-solid", "border-b-2", "border-r-2", "border-color", (*is_drawer_open).then(|| "translate-x-0").unwrap_or("-translate-x-full"))}>
                <div class="flex flex-col justify-between h-full">
                    <div class="px-6 py-4 mt-16">
                        <h2 class="drawer-text text-lg font-semibold">{"Pinepods"}</h2>
                        <hr class="my-4 drawer-hr" />
                        <div class="space-y-4">
                            // User Account with Gravatar
                        <div class="flex items-center space-x-3">
                            <Link<Route> to={Route::UserStats}>
                                <div class="align-avatar space-x-3">
                                    <img
                                        src={state.gravatar_url.clone().unwrap_or_else(|| "/static/assets/favicon.png".to_string())}
                                        style="width: 25px; height: 25px;"
                                        class="icon-size rounded-full" // Added rounded-full for circular image
                                        alt="User Avatar"
                                    />
                                    <span class="drawer-text text-lg text-adjust" style="margin-top: 7px; margin-left: -2px;">
                                        {username} // Displaying the username
                                    </span>
                                </div>
                            </Link<Route>>
                        </div>

                            // Other Links
                            <div class="m-0 p-0 flex items-center space-x-3">
                                <div onclick={toggle_drawer.clone()} class="drawer-text flex items-center space-x-3 cursor-pointer">
                                    <Link<Route> to={Route::Home}>
                                        <span class="material-standard-color material-icons icon-space">{"home"}</span>
                                        <span class="text-lg">{"Home"}</span>
                                    </Link<Route>>
                                </div>
                            </div>
                            <div class="flex items-center space-x-3">
                                <div onclick={toggle_drawer.clone()} class="drawer-text flex items-center space-x-3 cursor-pointer">
                                    <Link<Route> to={Route::Search}>
                                        <span class="material-icons icon-space">{"search"}</span>
                                        <span class="text-lg">{"Search Podcasts"}</span>
                                    </Link<Route>>
                                </div>
                            </div>
                            <div class="flex items-center space-x-3">
                                <div onclick={toggle_drawer.clone()} class="drawer-text flex items-center space-x-3 cursor-pointer">
                                    <Link<Route> to={Route::Queue}>
                                    <span class="material-icons icon-space">{"queue"}</span>
                                    <span class="text-lg">{"Queue"}</span>
                                    </Link<Route>>
                                </div>
                            </div>
                            <div class="flex items-center space-x-3">
                                <div onclick={toggle_drawer.clone()} class="drawer-text flex items-center space-x-3 cursor-pointer">
                                    <Link<Route> to={Route::Saved}>
                                        <span class="material-icons icon-space">{"star"}</span>
                                        <span class="text-lg">{"Saved"}</span>
                                    </Link<Route>>
                                </div>
                            </div>
                            <div class="flex items-center space-x-3">
                                <div onclick={toggle_drawer.clone()} class="drawer-text flex items-center space-x-3 cursor-pointer">
                                    <Link<Route> to={Route::PodHistory}>
                                        <span class="material-icons icon-space">{"history"}</span>
                                        <span class="text-lg">{"History"}</span>
                                    </Link<Route>>
                                </div>
                            </div>
                            <div class="flex items-center space-x-3">
                            <div onclick={toggle_drawer.clone()} class="drawer-text flex items-center space-x-3 cursor-pointer">
                                <Link<Route> to={Route::Downloads}>
                                    <span class="material-icons icon-space">{"download"}</span>
                                    <span class="text-lg">{"Downloads"}</span>
                                </Link<Route>>
                            </div>
                        </div>
                            <div class="flex items-center space-x-3">
                                <div onclick={toggle_drawer.clone()} class="drawer-text flex items-center space-x-3 cursor-pointer">
                                    <Link<Route> to={Route::Podcasts}>
                                        <span class="material-icons icon-space">{"podcasts"}</span>
                                        <span class="text-lg">{"Podcasts"}</span>
                                    </Link<Route>>
                                </div>
                            </div>
                            <div class="flex items-center space-x-3">
                                <div onclick={toggle_drawer.clone()} class="drawer-text flex items-center space-x-3 cursor-pointer">
                                    <Link<Route> to={Route::Settings}>
                                        <span class="material-icons icon-space">{"settings"}</span>
                                        <span class="text-lg">{"Settings"}</span>
                                    </Link<Route>>
                                </div>
                            </div>
                            <div class="flex-grow"></div>
                                    </div>
                            <div class="px-1 py-4">
                <div class="flex items-center space-x-3">
                    <div onclick={toggle_drawer.clone()} class="drawer-text flex items-center space-x-3 cursor-pointer">
                        <Link<Route> to={Route::LogOut}>
                            <span class="material-icons icon-space">{"logout"}</span>
                            <span class="text-lg">{"Sign Out"}</span>
                        </Link<Route>>
                    </div>
                </div>

                        </div>
                    </div>
                </div>
            </div>

        <div class="drawer-icon flex items-center" onclick={toggle_drawer.clone()}>
            <label for="drawer-toggle" class="rounded-lg cursor-pointer">
                <div class="flex flex-col items-center">
                    <div class="w-6 h-1 mb-1 drawer-burger rounded-lg"></div>
                    <div class="w-6 h-1 mb-1 drawer-burger rounded-lg"></div>
                    <div class="w-6 h-1 drawer-burger rounded-lg"></div>
                </div>
            </label>

        <div class="w-8 h-8 ml-3">

            {
                match state.is_loading {
                    Some(true) => html! {
                        // <div class="spinner-border animate-spin inline-block w-8 h-8 border-4 custom-spinner-color rounded-full" role="status">
                        //     <span class="visually-hidden">{""}</span>
                        // </div>
                        <div role="status">
                            <svg aria-hidden="true" class="w-8 h-8 text-gray-200 animate-spin dark:text-gray-600 fill-blue-600" viewBox="0 0 100 101" fill="none" xmlns="http://www.w3.org/2000/svg">
                                <path d="M100 50.5908C100 78.2051 77.6142 100.591 50 100.591C22.3858 100.591 0 78.2051 0 50.5908C0 22.9766 22.3858 0.59082 50 0.59082C77.6142 0.59082 100 22.9766 100 50.5908ZM9.08144 50.5908C9.08144 73.1895 27.4013 91.5094 50 91.5094C72.5987 91.5094 90.9186 73.1895 90.9186 50.5908C90.9186 27.9921 72.5987 9.67226 50 9.67226C27.4013 9.67226 9.08144 27.9921 9.08144 50.5908Z" fill="currentColor"/>
                                <path d="M93.9676 39.0409C96.393 38.4038 97.8624 35.9116 97.0079 33.5539C95.2932 28.8227 92.871 24.3692 89.8167 20.348C85.8452 15.1192 80.8826 10.7238 75.2124 7.41289C69.5422 4.10194 63.2754 1.94025 56.7698 1.05124C51.7666 0.367541 46.6976 0.446843 41.7345 1.27873C39.2613 1.69328 37.813 4.19778 38.4501 6.62326C39.0873 9.04874 41.5694 10.4717 44.0505 10.1071C47.8511 9.54855 51.7191 9.52689 55.5402 10.0491C60.8642 10.7766 65.9928 12.5457 70.6331 15.2552C75.2735 17.9648 79.3347 21.5619 82.5849 25.841C84.9175 28.9121 86.7997 32.2913 88.1811 35.8758C89.083 38.2158 91.5421 39.6781 93.9676 39.0409Z" fill="currentFill"/>
                            </svg>
                            <span class="sr-only">{"Loading..."}</span>
                        </div>
                    },
                    _ => html! {}, // Covers both Some(false) and None
                }
            }
        </div>
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