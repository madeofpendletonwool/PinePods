use std::rc::Rc;
use web_sys::MouseEvent;
use yew::{Callback, function_component, Html, html};
use yew_router::history::{BrowserHistory, History};
use yewdux::Dispatch;
use super::app_drawer::App_drawer;
use super::gen_components::Search_nav;
use crate::components::context::{AppState};
use crate::requests::search_pods::{call_get_podcast_info, call_parse_podcast_url, test_connection};

#[function_component(PodLayout)]
pub fn pod_layout() -> Html {
    let dispatch = Dispatch::<AppState>::global();
    let state: Rc<AppState> = dispatch.get();
    let search_results = state.search_results.clone();
    let history = BrowserHistory::new();
    let history_clone = history.clone();
    web_sys::console::log_1(&format!("Search Results: {:?}", search_results).into());

    html! {
    <div>
        <Search_nav />
        <h1 class="text-2xl font-bold my-4 center-text">{ "Podcast Search Results" }</h1>
        {
            if let Some(results) = search_results {
                html! {
                    <div>
                        { for results.feeds.iter().map(|podcast| {
                            let podcast_url = podcast.url.clone();
                            let dispatch_clone = dispatch.clone(); // Clone the dispatch here
                            let history = history_clone.clone();
                            let on_title_click = {
                                let dispatch = dispatch.clone();
                                let history = history.clone(); // Clone history for use inside the closure
                                Callback::from(move |e: MouseEvent| {
                                    e.prevent_default(); // Prevent the default anchor behavior
                                    let podcast_url = podcast_url.clone();
                                    let dispatch = dispatch.clone();
                                    let history = history.clone(); // Clone again for use inside async block
                                    wasm_bindgen_futures::spawn_local(async move {
                                        match call_parse_podcast_url(&podcast_url).await {
                                            Ok(podcast_feed_results) => {
                                                dispatch.reduce_mut(move |state| {
                                                    state.podcast_feed_results = Some(podcast_feed_results);
                                                });
                                                history.push("/episode_layout"); // Navigate to episode_layout
                                            },
                                            Err(e) => {
                                                web_sys::console::log_1(&format!("Error: {}", e).into());
                                            }
                                        }
                                    });
                                })
                            };
                            html! {
                                <div key={podcast.id.to_string()} class="flex items-center mb-4 bg-white shadow-md rounded-lg overflow-hidden">
                                    <img src={podcast.image.clone()} alt={format!("Cover for {}", &podcast.title)} class="w-1/4 object-cover"/>
                                    <div class="flex flex-col p-4 space-y-2 w-7/12">
                                        <a onclick={on_title_click} class="text-xl font-semibold hover:underline">{ &podcast.title }</a>
                                        <p class="text-gray-600">{ &podcast.description }</p>
                                    </div>
                                    <button class="w-1/4 bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded">
                                        {"Add"}
                                    </button>
                                </div>
                            }
                        })}
                    </div>
                }
            } else {
                html! {
                    <>
                        <div class="empty-episodes-container">
                            <img src="static/assets/favicon.png" alt="Logo" class="logo"/>
                            <h1>{ "No Podcast Search Results Found" }</h1>
                            <p>{"Try searching again with a different set of keywords."}</p>
                        </div>
                    </>
                }
            }
        }
        <App_drawer />
    </div>
}

}