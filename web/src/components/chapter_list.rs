// Chapter picker shared by the local player and the Music Assistant speaker
// bar. The caller owns the position and decides what a seek or toggle does.

use std::rc::Rc;

use crate::components::i18n::use_translation;
use yew::prelude::*;

use crate::components::gen_funcs::format_time_rm_hour;
use crate::requests::pod_req::Chapter;

/// Start and end of each chapter in seconds. A chapter ends where the next
/// one starts, and the last one at `duration`.
pub(crate) fn chapter_bounds(chapters: &[Chapter], duration: f64) -> Vec<(f64, f64)> {
    chapters
        .iter()
        .enumerate()
        .map(|(index, chapter)| {
            let start = chapter.startTime.unwrap_or_default() as f64;
            let end = chapters
                .get(index + 1)
                .map(|next| next.startTime.unwrap_or_default() as f64)
                .unwrap_or(duration);
            (start, end)
        })
        .collect()
}

/// How far `position` is through a chapter, in percent: 100 once it is past,
/// 0 before it starts.
pub(crate) fn chapter_progress(position: f64, start: f64, end: f64) -> f64 {
    if position >= end {
        100.0
    } else if position >= start && end > start {
        ((position - start) / (end - start) * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    }
}

#[derive(Properties, PartialEq)]
pub(crate) struct ChapterListProps {
    pub(crate) chapters: Rc<Vec<Chapter>>,
    pub(crate) position: f64,
    pub(crate) duration: f64,
    pub(crate) playing: bool,
    /// Seek to a chapter start, in seconds.
    pub(crate) on_seek: Callback<f64>,
    /// Tapping the current chapter plays or pauses instead.
    pub(crate) on_toggle: Callback<()>,
    pub(crate) on_close: Callback<()>,
}

#[function_component(ChapterList)]
pub(crate) fn chapter_list(props: &ChapterListProps) -> Html {
    let (i18n, _) = use_translation();
    let close = props.on_close.reform(|_: MouseEvent| ());
    let stop_propagation = Callback::from(|e: MouseEvent| e.stop_propagation());
    let bounds = chapter_bounds(&props.chapters, props.duration);

    html! {
        <div id="chapter-select-modal" tabindex="-1" aria-hidden="true"
            class="chapter-select-modal fixed top-0 right-0 left-0 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25"
            onclick={close.clone()}>
            <div class="modal-container relative p-4 w-full max-w-md max-h-full rounded-lg shadow"
                onclick={stop_propagation}>
                <div class="modal-container relative rounded-lg shadow">
                    <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t">
                        <h3 class="text-xl font-semibold">{ i18n.t("audio.chapters") }</h3>
                        <button onclick={close}
                            class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center">
                            <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                            </svg>
                            <span class="sr-only">{ i18n.t("common.close_modal") }</span>
                        </button>
                    </div>

                    <div class="p-4 md:p-5 max-h-[70vh] overflow-y-auto">
                        if props.chapters.is_empty() {
                            <div class="text-center p-4">{ i18n.t("audio.no_chapters_available") }</div>
                        }
                        { for props.chapters.iter().zip(bounds).map(|(chapter, (start, end))| {
                            let is_current = props.position >= start && props.position < end;
                            let progress = chapter_progress(props.position, start, end);
                            let activate = {
                                let on_seek = props.on_seek.clone();
                                let on_toggle = props.on_toggle.clone();
                                Callback::from(move |e: MouseEvent| {
                                    e.stop_propagation();
                                    if is_current {
                                        on_toggle.emit(());
                                    } else {
                                        on_seek.emit(start);
                                    }
                                })
                            };
                            html! {
                                <div
                                    class={classes!("chapter-item", is_current.then_some("current-chapter"))}
                                    onclick={activate.clone()}
                                >
                                    <button class="chapter-play-button" onclick={activate}>
                                        if is_current && props.playing {
                                            <i class="ph ph-pause text-xl"></i>
                                        } else {
                                            <i class="ph ph-play text-xl"></i>
                                        }
                                    </button>
                                    <div class="chapter-info">
                                        <span class="chapter-title">{ &chapter.title }</span>
                                        <span class="chapter-time">{ format_time_rm_hour(start as i32) }</span>
                                        <div class="chapter-progress-container">
                                            <div class="chapter-progress-bar" style={format!("width: {}%", progress)} />
                                        </div>
                                    </div>
                                </div>
                            }
                        }) }
                    </div>
                </div>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    fn chapter(start: i32) -> Chapter {
        Chapter { startTime: Some(start), title: format!("at {start}"), url: None, img: None }
    }

    #[wasm_bindgen_test]
    fn a_chapter_ends_where_the_next_starts_and_the_last_at_the_duration() {
        let chapters = [chapter(0), chapter(60), chapter(200)];
        assert_eq!(chapter_bounds(&chapters, 360.0), vec![(0.0, 60.0), (60.0, 200.0), (200.0, 360.0)]);
        assert_eq!(chapter_bounds(&[], 360.0), vec![]);
    }

    #[wasm_bindgen_test]
    fn progress_is_empty_before_a_chapter_and_full_after_it() {
        assert_eq!(chapter_progress(30.0, 60.0, 200.0), 0.0);
        assert_eq!(chapter_progress(130.0, 60.0, 200.0), 50.0);
        assert_eq!(chapter_progress(250.0, 60.0, 200.0), 100.0);
        // A chapter with no length never divides by zero.
        assert_eq!(chapter_progress(60.0, 60.0, 60.0), 100.0);
    }
}
