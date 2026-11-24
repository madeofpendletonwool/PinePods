//! The one-tap actions on inbox cards. Each updates the shared episode state the same way the
//! episode menu does, so the inbox (which hides handled episodes) and every badge follow.

use crate::components::context::{current_playing_anchor, EpisodeStatusState, NotificationState};
use crate::components::labels::{adjust_inbox_count, creds};
use crate::requests::episode::Episode;
use crate::requests::inbox_req::{self, EpisodeRef};
use crate::requests::notify::toast_error;
use crate::requests::pod_req::{
    self, MarkEpisodeCompletedRequest, QueuePodcastRequest, SavePodcastRequest,
};
use wasm_bindgen_futures::spawn_local;
use yewdux::prelude::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum InboxAction {
    /// Last in the queue.
    Queue,
    /// Next in the queue, under whatever is playing.
    PlayNext,
    Save,
    MarkPlayed,
    Dismiss,
}

fn error(e: impl ToString) {
    toast_error(&Dispatch::<NotificationState>::global(), e.to_string());
}

/// Run `action` on `episode`; `done` runs once the server agreed.
pub fn run(action: InboxAction, episode: Episode, done: impl FnOnce() + 'static) {
    let Some((server, api_key, user_id)) = creds() else { return };
    let key = Some(api_key.clone());
    spawn_local(async move {
        let ok = match action {
            InboxAction::Queue | InboxAction::PlayNext => {
                let (playing_episode_id, playing_is_youtube) = current_playing_anchor();
                let req = QueuePodcastRequest {
                    episode_id: episode.episodeid,
                    user_id,
                    is_youtube: episode.is_youtube,
                    playing_episode_id,
                    playing_is_youtube,
                    position: Some(if action == InboxAction::PlayNext { "top" } else { "bottom" }.to_string()),
                };
                match pod_req::call_queue_episode(&server, &key, &req).await {
                    Ok(_) => {
                        let id = episode.episodeid;
                        Dispatch::<EpisodeStatusState>::global().reduce_mut(move |s| {
                            let ids = s.queued_episode_ids.get_or_insert_with(Vec::new);
                            if !ids.contains(&id) {
                                ids.push(id);
                            }
                        });
                        if let Ok(mut eps) = pod_req::call_get_queued_episodes(&server, &key, &user_id).await {
                            eps.sort_by_key(|e| e.queueposition.unwrap_or(i32::MAX));
                            let ids: Vec<i32> = eps.iter().map(|e| e.episodeid).collect();
                            Dispatch::<EpisodeStatusState>::global().reduce_mut(move |s| {
                                s.queued_episodes = Some(pod_req::QueuedEpisodesResponse { episodes: eps });
                                s.queued_episode_ids = Some(ids);
                            });
                        }
                        true
                    }
                    Err(e) => {
                        error(e);
                        false
                    }
                }
            }
            InboxAction::Save => {
                let req = SavePodcastRequest { episode_id: episode.episodeid, user_id, is_youtube: episode.is_youtube };
                match pod_req::call_save_episode(&server, &key, &req).await {
                    Ok(_) => {
                        let ep = episode.clone();
                        Dispatch::<EpisodeStatusState>::global().reduce_mut(move |s| {
                            if !s.saved_episode_ids().any(|id| id == ep.episodeid) {
                                s.saved_episodes.push(ep);
                            }
                        });
                        true
                    }
                    Err(e) => {
                        error(e);
                        false
                    }
                }
            }
            InboxAction::MarkPlayed => {
                let req = MarkEpisodeCompletedRequest { episode_id: episode.episodeid, user_id, is_youtube: episode.is_youtube };
                match pod_req::call_mark_episode_completed(&server, &key, &req).await {
                    Ok(_) => {
                        let id = episode.episodeid;
                        Dispatch::<EpisodeStatusState>::global().reduce_mut(move |s| {
                            s.completed_episodes.insert(id);
                        });
                        true
                    }
                    Err(e) => {
                        error(e);
                        false
                    }
                }
            }
            InboxAction::Dismiss => match inbox_req::call_dismiss_inbox(&server, &api_key, &[EpisodeRef::from(&episode)]).await {
                Ok(_) => true,
                Err(e) => {
                    error(e);
                    false
                }
            },
        };
        if ok {
            adjust_inbox_count(-1);
            done();
        }
    });
}
