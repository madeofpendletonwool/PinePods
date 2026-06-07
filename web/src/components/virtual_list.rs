//! Pure windowing for an episode list. Bounds DOM cost by viewport regardless of list length.
//!
//! ## What this is (and isn't)
//!
//! `VirtualList` is dumb. It takes a `Rc<Vec<Episode>>`, a `render_item` callback, and a scroll
//! source. It renders a top spacer, a windowed slice of items, and a bottom spacer. It does NOT
//! own the IntersectionObserver, the load-more sentinel, the spinner overlay, selection state,
//! or anything Episode-specific beyond the prop type — Episode-specific chrome lives in
//! [`EpisodeListView`](crate::components::episode_list_view). The split keeps the windowing
//! math testable in isolation.
//!
//! ## Scroll source
//!
//! Two modes, chosen by the parent page:
//!
//! - [`ScrollSource::Window`] — read scroll from `window.scrollY` (via the list root's
//!   `getBoundingClientRect`). Use this for inline lists embedded inside a longer page
//!   that the document itself scrolls (e.g. `person`, `subscribed_people`).
//! - [`ScrollSource::Container(node_ref)`] — read scroll from a specific container element's
//!   `scrollTop`. Use this when the parent renders the list inside an `overflow-y-auto` div
//!   (every paginated page: `episode_layout`, `feed`, `saved`, `history`, `playlist_detail`,
//!   `search`; also each expanded podcast's `.podcast-episodes-inner` box on `downloads`).
//!
//! We intentionally do NOT auto-detect by walking up the DOM looking for an overflow ancestor
//! — too magical, breaks silently when CSS is reorganized.
//!
//! ## Item height (self-tuning)
//!
//! The window math needs the per-card vertical footprint (card height + outer margin). We
//! seed it from a breakpoint table on mount, then **measure the real rendered spacing** after
//! each render with ≥ 2 visible items (`children[2].top − children[1].top` on the list root,
//! which captures the card's bounding rect plus its `mb-4` in one read). If the measured
//! value differs from the current estimate by more than 1px we update, which triggers a
//! re-render with corrected spacers. Convergence is one or two renders.
//!
//! This means [`default_item_height`] is only a startup seed — it doesn't need to track
//! [`EpisodeListItem`](crate::components::episode_list_item)'s CSS exactly; measurement
//! corrects any drift. The cost is a single layout-glitch frame on first mount where the
//! window slice is sized off the seed. Acceptable.
//!
//! A page that needs a different initial seed can pass `item_height_fn`.
//! [`EpisodeListView`] doesn't expose this — it's a VirtualList-internal escape hatch.
//!
//! ## Known limitations (documented; do not call these bugs)
//!
//! - **No per-item measurement.** A card whose description is expanded grows beyond the fixed
//!   height; the spacer math doesn't know, so the cards below it shift by a few px until the
//!   description collapses again. Acceptable for now.
//! - **No scroll position restoration across page navigation.**
//! - **No smooth scroll-to-episode for deep linking.**

use crate::requests::episode::Episode;
use gloo_events::EventListener;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::{window, Element, HtmlElement, ResizeObserver};
use yew::prelude::*;

/// Where to read scroll position from. Pages opt into [`Container`](Self::Container) explicitly.
#[derive(Clone, PartialEq)]
pub enum ScrollSource {
    /// Read scroll from `window.scrollY`. The list's position within the document is computed
    /// via `getBoundingClientRect`, so this works correctly even when the list is one of many
    /// sections on a longer page.
    Window,
    /// Read scroll from a specific scroll container's `scrollTop`. The container's
    /// `clientHeight` is the viewport. Pass the same `NodeRef` you attached to the
    /// `overflow-y-auto` div.
    Container(NodeRef),
}

impl Default for ScrollSource {
    fn default() -> Self {
        Self::Window
    }
}

/// Default item-height table. See module docs for the coupling to `EpisodeListItem` CSS.
fn default_item_height(window_width: f64) -> f64 {
    if window_width <= 530.0 {
        122.0 + 16.0
    } else if window_width <= 768.0 {
        150.0 + 16.0
    } else {
        221.0 + 16.0
    }
}

#[derive(Properties, PartialEq)]
pub struct VirtualListProps {
    pub episodes: Rc<Vec<Episode>>,
    /// Builds the html for one card. Called once per item in the visible window per render.
    /// The parent should build this via `use_callback` with stable deps — a fresh
    /// `Callback::from(...)` per render would break `EpisodeListItem`'s PartialEq and force
    /// every visible card to re-run its body on every parent re-render.
    pub render_item: Callback<(Episode, usize), Html>,
    #[prop_or(ScrollSource::Window)]
    pub scroll_source: ScrollSource,
    /// Optional override of the default breakpoint table. `f64 → f64` (window inner width to
    /// item height in px). Defaults to [`default_item_height`].
    #[prop_or_default]
    pub item_height_fn: Option<Callback<f64, f64>>,
    #[prop_or(3)]
    pub buffer_items: usize,
}

/// Read the current scroll position relative to the list's top, in pixels. Returns 0 when the
/// list is below the viewport / container top (not yet scrolled to).
fn read_scroll_position(source: &ScrollSource, root: &NodeRef) -> f64 {
    let root_el = match root.cast::<Element>() {
        Some(el) => el,
        None => return 0.0,
    };
    match source {
        ScrollSource::Window => (-root_el.get_bounding_client_rect().top()).max(0.0),
        ScrollSource::Container(nr) => match nr.cast::<Element>() {
            Some(cont) => {
                let c_top = cont.get_bounding_client_rect().top();
                let r_top = root_el.get_bounding_client_rect().top();
                (c_top - r_top).max(0.0)
            }
            None => 0.0,
        },
    }
}

/// Read the current viewport height — window inner height in `Window` mode, container's
/// `clientHeight` in `Container` mode.
fn read_viewport_height(source: &ScrollSource) -> f64 {
    let win = window().expect("no global window");
    match source {
        ScrollSource::Window => win.inner_height().unwrap().as_f64().unwrap_or(0.0),
        ScrollSource::Container(nr) => match nr.cast::<Element>() {
            Some(el) => el.client_height() as f64,
            None => win.inner_height().unwrap().as_f64().unwrap_or(0.0),
        },
    }
}

#[function_component(VirtualList)]
pub fn virtual_list(props: &VirtualListProps) -> Html {
    let scroll_top = use_state(|| 0.0_f64);
    let viewport_height = use_state(|| 0.0_f64);
    let item_height = use_state(|| 138.0_f64);
    let root_ref = use_node_ref();

    // Mount + resize: measure viewport height and item height. The first render uses the
    // initial state defaults; this effect fires after the first paint and triggers a re-render
    // with correct values. A one-frame glitch is acceptable — the spacers still compute to
    // something sensible.
    {
        let viewport_height = viewport_height.clone();
        let item_height = item_height.clone();
        let scroll_source = props.scroll_source.clone();
        let item_height_fn = props.item_height_fn.clone();
        use_effect_with(
            (props.scroll_source.clone(), props.item_height_fn.clone()),
            move |_| {
                let win = window().expect("no global window");
                let measure: Rc<dyn Fn()> = {
                    let viewport_height = viewport_height.clone();
                    let item_height = item_height.clone();
                    let scroll_source = scroll_source.clone();
                    let item_height_fn = item_height_fn.clone();
                    let win = win.clone();
                    Rc::new(move || {
                        let width = win.inner_width().unwrap().as_f64().unwrap_or(0.0);
                        let new_item_h = match &item_height_fn {
                            Some(cb) => cb.emit(width),
                            None => default_item_height(width),
                        };
                        item_height.set(new_item_h.max(1.0));
                        viewport_height.set(read_viewport_height(&scroll_source));
                    })
                };
                measure();
                let listener = {
                    let measure = measure.clone();
                    EventListener::new(&win, "resize", move |_| measure())
                };
                move || drop(listener)
            },
        );
    }

    // Mount + scroll listener (RAF-coalesced). Reads scroll position from the source on each
    // animation frame, at most once per paint. Skips the `is_updating` dance from the prior
    // implementation — that was working around a feedback loop caused by un-coalesced events
    // re-firing within the same frame; RAF coalescing solves it directly.
    {
        let scroll_top = scroll_top.clone();
        let scroll_source = props.scroll_source.clone();
        let root_ref = root_ref.clone();
        use_effect_with(
            (props.scroll_source.clone(), root_ref.clone()),
            move |_| {
                let win = window().expect("no global window");
                let raf_scheduled: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

                let on_scroll: Rc<dyn Fn()> = {
                    let scroll_top = scroll_top.clone();
                    let scroll_source = scroll_source.clone();
                    let root_ref = root_ref.clone();
                    let raf_scheduled = raf_scheduled.clone();
                    let win = win.clone();
                    Rc::new(move || {
                        if *raf_scheduled.borrow() {
                            return;
                        }
                        *raf_scheduled.borrow_mut() = true;
                        let scroll_top = scroll_top.clone();
                        let scroll_source = scroll_source.clone();
                        let root_ref = root_ref.clone();
                        let raf_scheduled = raf_scheduled.clone();
                        let cb = Closure::once_into_js(move || {
                            *raf_scheduled.borrow_mut() = false;
                            scroll_top.set(read_scroll_position(&scroll_source, &root_ref));
                        });
                        let _ = win.request_animation_frame(cb.as_ref().unchecked_ref());
                    })
                };

                // Attach to window or container depending on scroll source. The container
                // element may not be cast-able yet on first effect run if the parent hasn't
                // commited the ref — fall back to window in that case (harmless, the effect
                // re-runs if scroll_source / root_ref changes).
                let container_el = match &scroll_source {
                    ScrollSource::Container(nr) => nr.cast::<HtmlElement>(),
                    ScrollSource::Window => None,
                };
                let listener = match container_el.as_ref() {
                    Some(el) => {
                        let on_scroll = on_scroll.clone();
                        EventListener::new(el, "scroll", move |_| on_scroll())
                    }
                    None => {
                        let on_scroll = on_scroll.clone();
                        EventListener::new(&win, "scroll", move |_| on_scroll())
                    }
                };

                move || drop(listener)
            },
        );
    }

    // Container resize observer. In `Container` mode the scroll element's `clientHeight` can
    // change *after* the one-shot mount measurement: `downloads` animates its episode box from
    // `max-height: 0` to full height (0.5s) and loads episodes async, so the mount read often
    // captures a box that's still tens of pixels tall and `viewport_height` sticks there —
    // the window then only fills the top sliver of the eventual 850px box. `window.innerHeight`
    // (Window mode) is stable and already covered by the resize listener above, so we only
    // observe in Container mode. The observer fires an initial callback on `observe`, then on
    // every size change, so `viewport_height` converges to the box's real height without
    // requiring the user to scroll.
    {
        let viewport_height = viewport_height.clone();
        let scroll_source = props.scroll_source.clone();
        use_effect_with(props.scroll_source.clone(), move |_| {
            let mut cleanup: Option<(ResizeObserver, Closure<dyn Fn()>)> = None;
            if let ScrollSource::Container(nr) = &scroll_source {
                if let Some(el) = nr.cast::<Element>() {
                    let viewport_height = viewport_height.clone();
                    let scroll_source = scroll_source.clone();
                    let cb = Closure::<dyn Fn()>::wrap(Box::new(move || {
                        let measured = read_viewport_height(&scroll_source);
                        if measured > 1.0 && (measured - *viewport_height).abs() > 1.0 {
                            viewport_height.set(measured);
                        }
                    }));
                    if let Ok(observer) = ResizeObserver::new(cb.as_ref().unchecked_ref()) {
                        observer.observe(&el);
                        cleanup = Some((observer, cb));
                    }
                }
            }
            move || {
                if let Some((observer, _cb)) = cleanup {
                    observer.disconnect();
                }
            }
        });
    }

    let item_h = (*item_height).max(1.0);
    let total = props.episodes.len();
    let buffer = props.buffer_items;

    let start_unbuffered = (*scroll_top / item_h).floor().max(0.0) as usize;
    let start = start_unbuffered.saturating_sub(buffer);
    let end_unbuffered =
        ((*scroll_top + *viewport_height) / item_h).ceil().max(0.0) as usize + buffer;
    let end = end_unbuffered.min(total).max(start);

    let top_h = (start as f64) * item_h;
    let bot_h = ((total - end) as f64) * item_h;

    let items_html = (start..end)
        .map(|i| props.render_item.emit((props.episodes[i].clone(), i)))
        .collect::<Html>();

    // Self-tuning measurement: after each render where the slice has ≥ 2 cards, read the
    // y-distance between the first two and feed it back into `item_height`. Threshold of 1px
    // avoids feedback from sub-pixel rect rounding. The effect re-runs whenever start/end
    // change (which is whenever we scroll or whenever item_height changes — so each correction
    // gets revalidated by the next render).
    {
        let item_height = item_height.clone();
        let root_ref = root_ref.clone();
        use_effect_with((start, end), move |&(start, end)| {
            if end - start >= 2 {
                if let Some(root_el) = root_ref.cast::<Element>() {
                    let children = root_el.children();
                    if let (Some(c1), Some(c2)) = (children.item(1), children.item(2)) {
                        let measured = c2.get_bounding_client_rect().top()
                            - c1.get_bounding_client_rect().top();
                        if measured > 1.0 && (measured - *item_height).abs() > 1.0 {
                            item_height.set(measured);
                        }
                    }
                }
            }
            || ()
        });
    }

    html! {
        <div ref={root_ref}>
            <div style={format!("height: {}px;", top_h)} />
            { items_html }
            <div style={format!("height: {}px;", bot_h)} />
        </div>
    }
}
