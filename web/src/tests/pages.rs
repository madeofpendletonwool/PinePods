//! Pages mounted in a real browser with the app's providers, the server's API answered by a
//! stand-in for `window.fetch`. Run with:
//! `wasm-pack test --headless --chrome -- --features server_build,browser_tests`

use gloo_timers::future::TimeoutFuture;
use i18nrs::yew::{I18nProvider, I18nProviderConfig};
use serde_json::{json, Value};
use std::collections::HashMap;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::*;

use crate::components::context::AppState;
use crate::pages::discover::Discover;
use crate::pages::downloads::Downloads;
use crate::pages::feed::Feed;
use crate::pages::history::PodHistory;
use crate::pages::home::Home;
use crate::pages::login::Login;
use crate::pages::not_found::NotFound;
use crate::pages::playlists::Playlists;
use crate::pages::podcasts::Podcasts;
use crate::pages::queue::Queue;
use crate::pages::saved::Saved;
use crate::pages::search_new::SearchNew;
use crate::pages::settings::Settings;
use crate::pages::user_stats::UserStats;
use crate::requests::login_requests::{GetUserDetails, LoginServerRequest};

const SERVER: &str = "http://pinepods.test";

#[derive(Clone, PartialEq)]
enum Page {
    Discover,
    Downloads,
    Feed,
    History,
    Home,
    Login,
    NotFound,
    Playlists,
    Podcasts,
    Queue,
    Saved,
    Search,
    Settings,
    Stats,
}

#[derive(Properties, PartialEq)]
struct HarnessProps {
    page: Page,
}

#[function_component(Harness)]
fn harness(props: &HarnessProps) -> Html {
    let translations = HashMap::from([("en", include_str!("../translations/en.json"))]);
    let config = I18nProviderConfig {
        translations,
        default_language: "en".to_string(),
        ..Default::default()
    };
    let page = match props.page {
        Page::Discover => html! { <Discover /> },
        Page::Downloads => html! { <Downloads /> },
        Page::Feed => html! { <Feed /> },
        Page::History => html! { <PodHistory /> },
        Page::Home => html! { <Home /> },
        Page::Login => html! { <Login /> },
        Page::NotFound => html! { <NotFound /> },
        Page::Playlists => html! { <Playlists /> },
        Page::Podcasts => html! { <Podcasts /> },
        Page::Queue => html! { <Queue /> },
        Page::Saved => html! { <Saved /> },
        Page::Search => html! { <SearchNew /> },
        Page::Settings => html! { <Settings /> },
        Page::Stats => html! { <UserStats /> },
    };
    html! {
        <I18nProvider ..config>
            <BrowserRouter>{ page }</BrowserRouter>
        </I18nProvider>
    }
}

/// Answer `fetch` from `routes` (pathname to JSON); anything else is a 404.
fn answer_api_with(routes: Value) {
    let script = format!(
        r#"window.__api = {routes};
           window.__apiCalls = [];
           window.fetch = async (input, init) => {{
             const url = new URL(typeof input === 'string' ? input : input.url, location.href);
             window.__apiCalls.push(url.href);
             const known = Object.prototype.hasOwnProperty.call(window.__api, url.pathname);
             return new Response(JSON.stringify(known ? window.__api[url.pathname] : {{}}),
               {{ status: known ? 200 : 404, headers: {{ 'Content-Type': 'application/json' }} }});
           }};"#
    );
    js_sys::eval(&script).expect("install fetch stand-in");
}

fn sign_in() {
    Dispatch::<AppState>::global().reduce_mut(|state| {
        state.auth_details = Some(LoginServerRequest {
            server_name: SERVER.to_string(),
            username: Some("listener".to_string()),
            password: None,
            api_key: Some("test-key".to_string()),
        });
        state.user_details = Some(GetUserDetails {
            UserID: 2,
            Fullname: Some("Test Listener".to_string()),
            Username: Some("listener".to_string()),
            Email: Some("listener@example.test".to_string()),
            Hashed_PW: None,
            Salt: None,
            UseGravatar: false,
        });
    });
}

fn sign_out() {
    Dispatch::<AppState>::global().reduce_mut(|state| {
        state.auth_details = None;
        state.user_details = None;
    });
}

/// Mount a page, let its effects and requests settle, and return the text it shows.
async fn render(page: Page) -> (web_sys::Element, yew::AppHandle<Harness>) {
    web_sys::window().unwrap().local_storage().unwrap().unwrap().clear().unwrap();
    crate::requests::cache::invalidate_prefix("");
    let document = web_sys::window().unwrap().document().unwrap();
    let root = document.create_element("div").unwrap();
    document.body().unwrap().append_child(&root).unwrap();
    let app = yew::Renderer::<Harness>::with_root_and_props(root.clone(), HarnessProps { page }).render();
    TimeoutFuture::new(300).await;
    (root, app)
}

fn text(root: &web_sys::Element) -> String {
    root.clone().dyn_into::<web_sys::HtmlElement>().unwrap().inner_text()
}

fn close(root: web_sys::Element, app: yew::AppHandle<Harness>) {
    app.destroy();
    root.remove();
}

/// Mount `page` signed in with an API that knows `routes`, and return what it shows.
async fn shown_when_signed_in(page: Page, routes: Value) -> String {
    answer_api_with(routes);
    sign_in();
    let (root, app) = render(page).await;
    let shown = text(&root);
    close(root, app);
    sign_out();
    shown
}

fn assert_shows(shown: &str, expected: &str) {
    assert!(shown.contains(expected), "expected {expected:?} on the page, got: {shown}");
}

// Each page, signed in with nothing to show yet.

#[wasm_bindgen_test]
async fn the_home_page_welcomes_a_new_listener() {
    assert_shows(&shown_when_signed_in(Page::Home, json!({})).await, "Welcome to Pinepods");
}

#[wasm_bindgen_test]
async fn the_feed_page_explains_how_to_add_podcasts() {
    assert_shows(&shown_when_signed_in(Page::Feed, json!({})).await, "No Recent Episodes Found");
}

#[wasm_bindgen_test]
async fn the_saved_page_offers_its_sort_options() {
    assert_shows(&shown_when_signed_in(Page::Saved, json!({})).await, "Newest First");
}

#[wasm_bindgen_test]
async fn the_history_page_has_an_empty_state() {
    assert_shows(&shown_when_signed_in(Page::History, json!({})).await, "No Episode History Found");
}

#[wasm_bindgen_test]
async fn the_downloads_page_has_an_empty_state() {
    assert_shows(&shown_when_signed_in(Page::Downloads, json!({})).await, "No Episode Downloads Found");
}

#[wasm_bindgen_test]
async fn the_queue_page_has_an_empty_state() {
    assert_shows(&shown_when_signed_in(Page::Queue, json!({})).await, "No Queued Episodes Found");
}

#[wasm_bindgen_test]
async fn the_podcasts_page_has_an_empty_state() {
    assert_shows(&shown_when_signed_in(Page::Podcasts, json!({})).await, "No Podcasts Fo");
}

#[wasm_bindgen_test]
async fn the_search_page_renders() {
    assert_shows(&shown_when_signed_in(Page::Search, json!({})).await, "Search");
}

#[wasm_bindgen_test]
async fn the_playlists_page_offers_creating_one() {
    assert_shows(&shown_when_signed_in(Page::Playlists, json!({})).await, "Create Playlist");
}

#[wasm_bindgen_test]
async fn the_discover_page_has_its_sections() {
    let shown = shown_when_signed_in(Page::Discover, json!({})).await;
    assert_shows(&shown, "For You");
    assert_shows(&shown, "Trending Now");
}

#[wasm_bindgen_test]
async fn the_settings_page_tells_you_when_the_server_cannot_be_reached() {
    assert_shows(&shown_when_signed_in(Page::Settings, json!({})).await, "Is the server reachable?");
}

#[wasm_bindgen_test]
async fn the_stats_page_has_its_heading() {
    assert_shows(&shown_when_signed_in(Page::Stats, json!({})).await, "User Statistics");
}

#[wasm_bindgen_test]
async fn the_not_found_page_links_home() {
    let shown = shown_when_signed_in(Page::NotFound, json!({})).await;
    assert_shows(&shown, "404");
    assert_shows(&shown, "Head back home");
}

// Data from the server reaches the screen.

#[wasm_bindgen_test]
async fn queued_episodes_are_listed() {
    let shown = shown_when_signed_in(
        Page::Queue,
        json!({ "/api/data/get_queued_episodes": { "data": [{
            "episodeid": 11,
            "podcastid": 3,
            "podcastname": "Queued Show",
            "episodetitle": "An Episode In The Queue",
            "episodeurl": "https://example.test/ep.mp3",
            "episodepubdate": "2026-01-02T03:04:05",
            "episodeduration": 1800,
        }] } }),
    )
    .await;
    assert_shows(&shown, "An Episode In The Queue");
}

#[wasm_bindgen_test]
async fn the_stats_page_shows_the_servers_numbers() {
    let shown = shown_when_signed_in(
        Page::Stats,
        json!({ "/api/data/get_stats": {
            "UserCreated": "2026-01-02T03:04:05",
            "PodcastsPlayed": 321,
            "TimeListened": 654,
            "PodcastsAdded": 987,
            "EpisodesSaved": 12,
            "EpisodesDownloaded": 34,
            "GpodderUrl": "",
            "Pod_Sync_Type": "None",
        } }),
    )
    .await;
    assert_shows(&shown, "321");
    assert_shows(&shown, "987");
}

fn podcast_with_counts() -> Value {
    json!({ "pods": [{
        "podcastid": 5,
        "podcastname": "Counted Show",
        "artworkurl": null,
        "description": "A show",
        "episodecount": 742,
        "websiteurl": "",
        "feedurl": "https://example.test/feed.xml",
        "author": "Someone",
        "categories": null,
        "explicit": false,
        "podcastindexid": 0,
        "unplayed_count": 19,
    }] })
}

#[wasm_bindgen_test]
async fn the_podcasts_page_counts_every_episode_by_default() {
    let shown = shown_when_signed_in(
        Page::Podcasts,
        json!({ "/api/data/return_pods/2": podcast_with_counts() }),
    )
    .await;
    assert_shows(&shown, "742");
    assert!(!shown.contains("19"), "unplayed count shown without the setting: {shown}");
}

#[wasm_bindgen_test]
async fn the_podcasts_page_counts_unplayed_episodes_when_chosen() {
    let shown = shown_when_signed_in(
        Page::Podcasts,
        json!({
            "/api/data/return_pods/2": podcast_with_counts(),
            "/api/data/get_podcast_count_display/2": { "user_id": 2, "display": "unplayed" },
        }),
    )
    .await;
    assert_shows(&shown, "19");
    assert!(!shown.contains("742"), "total shown instead of unplayed: {shown}");
}

// Signing in.

async fn set_input(input: &web_sys::HtmlInputElement, value: &str) {
    input.set_value(value);
    let event = web_sys::InputEvent::new("input").unwrap();
    input.dispatch_event(&event).unwrap();
    TimeoutFuture::new(20).await;
}

#[wasm_bindgen_test]
async fn the_login_page_asks_for_credentials() {
    answer_api_with(json!({}));
    sign_out();
    let (root, app) = render(Page::Login).await;
    assert_shows(&text(&root), "A Forest of Podcasts");
    assert!(root.query_selector("input[type=password]").unwrap().is_some(), "a password field");
    assert!(root.query_selector("input[type=text]").unwrap().is_some(), "a username field");
    close(root, app);
}

#[wasm_bindgen_test]
async fn a_rejected_login_stays_on_the_page_and_says_so() {
    answer_api_with(json!({ "/api/pinepods_check": { "status_code": 200, "pinepods_instance": true } }));
    sign_out();
    let (root, app) = render(Page::Login).await;
    let username: web_sys::HtmlInputElement =
        root.query_selector("input[type=text]").unwrap().unwrap().dyn_into().unwrap();
    let password: web_sys::HtmlInputElement =
        root.query_selector("input[type=password]").unwrap().unwrap().dyn_into().unwrap();
    set_input(&username, "listener").await;
    set_input(&password, "wrong-password").await;

    let buttons = root.query_selector_all("button").unwrap();
    let login = (0..buttons.length())
        .filter_map(|i| buttons.item(i))
        .filter_map(|b| b.dyn_into::<web_sys::HtmlElement>().ok())
        .find(|b| b.inner_text().trim() == "Login")
        .expect("a Login button");
    login.click();
    TimeoutFuture::new(300).await;

    let calls = js_sys::eval("window.__apiCalls.join(' ')").unwrap().as_string().unwrap();
    assert!(calls.contains("/api/pinepods_check"), "the server was checked: {calls}");
    let origin = web_sys::window().unwrap().location().origin().unwrap();
    assert!(
        calls.contains(&format!("{origin}/api/data/get_key")),
        "whatever follows the address in the page's URL stays out of the server address: {calls}"
    );
    assert!(Dispatch::<AppState>::global().get().auth_details.is_none(), "still signed out");
    assert!(root.query_selector("input[type=password]").unwrap().is_some(), "still on the login page");
    close(root, app);
}

// Helpers that need a DOM.

#[wasm_bindgen_test]
fn opml_files_list_their_podcasts() {
    use crate::components::gen_funcs::parse_opml;
    let podcasts = parse_opml(
        r#"<?xml version="1.0"?><opml version="2.0"><body>
           <outline title="With Title" xmlUrl="https://a.test/feed"/>
           <outline text="Only Text" xmlUrl="https://b.test/feed"/>
           </body></opml>"#,
    );
    assert_eq!(
        podcasts,
        [
            ("With Title".to_string(), "https://a.test/feed".to_string()),
            ("Only Text".to_string(), "https://b.test/feed".to_string()),
        ]
    );
}

#[wasm_bindgen_test]
fn images_can_be_stripped_from_descriptions() {
    use crate::components::gen_funcs::strip_images_from_html;
    let clean = strip_images_from_html(r#"<p>Before<img src="x.png">After</p>"#);
    assert_eq!(clean, "<p>BeforeAfter</p>");
}
