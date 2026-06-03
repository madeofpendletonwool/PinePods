//! Shared episode-list infrastructure: IntersectionObserver pagination, the fixed-bottom
//! loading spinner, and selection/delete-mode wiring. Every page that renders a list of
//! [`EpisodeListItem`]s should mount through this view so the perf contract lives in one
//! place. The actual windowing — bounding DOM cost to the viewport — is delegated to
//! [`VirtualList`](crate::components::virtual_list).
//!
//! ## Parent-side load-more pattern
//!
//! `on_load_more` is the only callback this view emits. The parent's handler is responsible
//! for firing the backend fetch, appending the result onto the source `Rc<Vec<Episode>>`, and
//! flipping `loading_more`. Use this shape — the two `TimeoutFuture::new(0).await` yields let
//! the spinner paint before the work and let the new cards paint after, so other interactions
//! stay responsive during the fetch:
//!
//! ```ignore
//! let on_load_more = {
//!     let loading_more = loading_more.clone();
//!     let episodes = episodes.clone();
//!     use_callback((), move |_, _| {
//!         if *loading_more { return; }
//!         loading_more.set(true);
//!         let loading_more = loading_more.clone();
//!         let episodes = episodes.clone();
//!         let offset = episodes.len() as i64;
//!         spawn_local(async move {
//!             match call_get_<page>_paged(.., offset, 50).await {
//!                 Ok(more) => {
//!                     TimeoutFuture::new(0).await;
//!                     let mut next = (**episodes).clone();
//!                     next.extend(more);
//!                     episodes.set(Rc::new(next));
//!                     TimeoutFuture::new(0).await;
//!                 }
//!                 Err(e) => web_sys::console::log_1(&format!("{}", e).into()),
//!             }
//!             loading_more.set(false);
//!         })
//!     })
//! };
//! ```
//!
//! ## Stable callbacks
//!
//! Every `Callback` you pass into this view — `on_load_more`, `on_checkbox_change`,
//! `on_select_above`, `on_select_below` — **must** be constructed with `use_callback` with
//! stable dependencies. The view forwards them straight into `EpisodeListItem`, whose
//! `PartialEq` impl compares them. Fresh `Callback::from(...)` per render would force every
//! already-mounted card to re-run its function body on every parent re-render — exactly the
//! regression the prior perf round eliminated.
//!
//! ## Reset on filter / sort / podcast change
//!
//! The view does **not** detect filter changes internally — earlier attempts to do so were
//! the root cause of the "cards disappear after backend append" bug. Parents reset by passing
//! a `key` prop that encodes the filter signature; when it changes, Yew unmounts and remounts
//! the view cleanly.

use crate::components::context_menu_button::PageType;
use crate::components::episode_list_item::EpisodeListItem;
use crate::components::virtual_list::{ScrollSource, VirtualList};
use crate::requests::episode::Episode;
use js_sys::Array;
use std::collections::HashSet;
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::{IntersectionObserver, IntersectionObserverEntry, IntersectionObserverInit};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct EpisodeListViewProps {
    pub episodes: Rc<Vec<Episode>>,
    #[prop_or(false)]
    pub backend_can_load_more: bool,
    #[prop_or(false)]
    pub loading_more: bool,
    #[prop_or_default]
    pub on_load_more: Callback<()>,
    #[prop_or(PageType::Default)]
    pub page_type: PageType,
    #[prop_or(false)]
    pub is_delete_mode: bool,
    #[prop_or_default]
    pub on_checkbox_change: Callback<i32>,
    #[prop_or_default]
    pub on_select_above: Callback<i32>,
    #[prop_or_default]
    pub on_select_below: Callback<i32>,
    #[prop_or_default]
    pub selected_episodes: Rc<HashSet<i32>>,
    #[prop_or_else(|| AttrValue::from("1500px"))]
    pub io_root_margin: AttrValue,
    /// Skip the IntersectionObserver entirely. Use this on pages where the view sits inside
    /// a larger "list of lists" (e.g. each subscribed-person's expanded dropdown) — there a
    /// sentinel firing would auto-reveal cards while the user is trying to scroll *past* this
    /// section to the next one.
    #[prop_or(false)]
    pub disable_sentinel: bool,
    /// Where to read scroll position from. Defaults to [`ScrollSource::Window`]; paginated
    /// pages that wrap the view in a `flex-grow overflow-y-auto` div should pass that div's
    /// `NodeRef` via [`ScrollSource::Container`].
    #[prop_or(ScrollSource::Window)]
    pub scroll_source: ScrollSource,
}

// Mirror of the latest props that the IntersectionObserver callback needs to read on every
// fire. The IO effect deliberately omits `episodes` from its deps (a change in episodes would
// recreate the observer and snap scroll position when backend appends arrive), so the closure
// captures this ref instead and reads through it. Without this, appending to `episodes` would
// leave the closure looking at a stale `Rc<Vec<Episode>>`.
struct LatestProps {
    loading_more: bool,
    backend_can_load_more: bool,
    on_load_more: Callback<()>,
}

#[function_component(EpisodeListView)]
pub fn episode_list_view(props: &EpisodeListViewProps) -> Html {
    let sentinel_ref = use_node_ref();

    let latest = use_mut_ref(|| LatestProps {
        loading_more: props.loading_more,
        backend_can_load_more: props.backend_can_load_more,
        on_load_more: props.on_load_more.clone(),
    });
    *latest.borrow_mut() = LatestProps {
        loading_more: props.loading_more,
        backend_can_load_more: props.backend_can_load_more,
        on_load_more: props.on_load_more.clone(),
    };

    // IntersectionObserver effect. The sentinel fires when it intersects the scroll source's
    // viewport; on intersect (and only when not already mid-fetch), we ask the parent to
    // load more. With virtualization, the sentinel sits below the bottom spacer — it
    // physically resides at `total_items * item_height` pixels down regardless of how many
    // cards are currently mounted, so the IO math is unaffected by which cards are in the
    // window.
    //
    // When `scroll_source` is `Container`, we set the observer's `root` to that element. The
    // prior implementation relied on the document-root math happening to coincide with the
    // container's overflow region "well enough" today; with windowing in the mix that
    // coincidence is fragile (the spacer divs can put the sentinel where the document-root
    // math considers it "in viewport" while the container has actually clipped it). Setting
    // `root` explicitly aligns the observer with the actual scroll source.
    {
        let sentinel_ref_inner = sentinel_ref.clone();
        let latest = latest.clone();
        let io_root_margin = props.io_root_margin.clone();
        let disable_sentinel = props.disable_sentinel;
        let scroll_source = props.scroll_source.clone();
        use_effect_with(
            (
                sentinel_ref_inner,
                disable_sentinel,
                scroll_source.clone(),
            ),
            move |(sentinel_ref, disable_sentinel, scroll_source)| {
                if *disable_sentinel {
                    return Box::new(|| ()) as Box<dyn FnOnce()>;
                }
                let sentinel_el = match sentinel_ref.cast::<web_sys::Element>() {
                    Some(el) => el,
                    None => return Box::new(|| ()) as Box<dyn FnOnce()>,
                };

                let latest = latest.clone();
                let callback = Closure::<dyn Fn(Array)>::wrap(Box::new(move |entries: Array| {
                    let entry: IntersectionObserverEntry = entries.get(0).unchecked_into();
                    let latest = latest.borrow();
                    if !entry.is_intersecting() || latest.loading_more {
                        return;
                    }
                    if latest.backend_can_load_more {
                        latest.on_load_more.emit(());
                    }
                }));

                let opts = IntersectionObserverInit::new();
                opts.set_root_margin(io_root_margin.as_ref());
                if let ScrollSource::Container(node_ref) = scroll_source {
                    if let Some(root_el) = node_ref.cast::<web_sys::Element>() {
                        opts.set_root(Some(&root_el));
                    }
                }
                let observer = IntersectionObserver::new_with_options(
                    callback.as_ref().unchecked_ref(),
                    &opts,
                )
                .expect("IntersectionObserver creation failed");
                observer.observe(&sentinel_el);
                callback.forget();

                let observer_clone = observer.clone();
                Box::new(move || {
                    observer_clone.disconnect();
                }) as Box<dyn FnOnce()>
            },
        );
    }

    // Per-card render callback. Stable across renders via `use_callback` keyed on the props
    // that change card output. EpisodeListItem's PartialEq compares its props (including the
    // forwarded callbacks); using a fresh `Callback::from` per render would force every
    // visible card to re-run its function body on every parent re-render.
    let render_item: Callback<(Episode, usize), Html> = {
        let page_type = props.page_type.clone();
        let is_delete_mode = props.is_delete_mode;
        let on_checkbox_change = props.on_checkbox_change.clone();
        let on_select_above = props.on_select_above.clone();
        let on_select_below = props.on_select_below.clone();
        let selected_episodes = props.selected_episodes.clone();
        use_callback(
            (
                props.page_type.clone(),
                props.is_delete_mode,
                props.on_checkbox_change.clone(),
                props.on_select_above.clone(),
                props.on_select_below.clone(),
                props.selected_episodes.clone(),
            ),
            move |(ep, _i): (Episode, usize), _| {
                let ep_id = ep.episodeid;
                // Combine id + url so external-API episodes (which all have episodeid=0)
                // still produce unique keys.
                let key = format!("{}-{}", ep_id, &ep.episodeurl);
                let is_selected = selected_episodes.contains(&ep_id);
                html! {
                    <EpisodeListItem
                        key={key}
                        episode={ep}
                        page_type={page_type.clone()}
                        is_delete_mode={is_delete_mode}
                        on_checkbox_change={on_checkbox_change.clone()}
                        is_selected={Some(is_selected)}
                        on_select_above={on_select_above.clone()}
                        on_select_below={on_select_below.clone()}
                    />
                }
            },
        )
    };

    html! {
        <>
            <VirtualList
                episodes={props.episodes.clone()}
                render_item={render_item}
                scroll_source={props.scroll_source.clone()}
            />
            <div ref={sentinel_ref} style="height: 1px; overflow-anchor: none;" />
            if props.loading_more {
                <div style="position: fixed; bottom: 1.5rem; left: 0; right: 0; display: flex; justify-content: center; pointer-events: none; z-index: 50;">
                    <div class="animate-spin rounded-full h-6 w-6 border-b-2 border-current"></div>
                </div>
            }
        </>
    }
}
