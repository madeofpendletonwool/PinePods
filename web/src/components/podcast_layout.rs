use std::rc::Rc;
use yew::{function_component, Html, html};
use yewdux::Dispatch;
use super::app_drawer::App_drawer;
use super::gen_components::Search_nav;
use crate::components::context::{AppState};

#[function_component(PodLayout)]
pub fn pod_layout() -> Html {
    let dispatch = Dispatch::<AppState>::global();
    let state: Rc<AppState> = dispatch.get();
    let search_results = state.search_results.clone();
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
                            html! {
                                <div key={podcast.id.to_string()} class="flex items-center mb-4 bg-white shadow-md rounded-lg overflow-hidden">
                                    <img src={podcast.image.clone()} alt={format!("Cover for {}", &podcast.title)} class="w-1/4 object-cover"/>
                                    <div class="flex flex-col p-4 space-y-2 w-7/12">
                                        <a href={format!("#{}", podcast.id)} class="text-xl font-semibold hover:underline">{ &podcast.title }</a>
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