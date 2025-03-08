// In components/mod.rs
pub(crate) mod app_drawer;
pub(crate) mod feed;
pub(crate) mod history;
pub(crate) mod home;
pub(crate) mod host_component;
pub mod misc_func;
pub(crate) mod navigation;
pub(crate) mod oauth_callback;
pub(crate) mod playlist_detail;
pub(crate) mod playlists;
pub(crate) mod queue;
pub(crate) mod routes;
pub(crate) mod saved;
pub(crate) mod search;
pub(crate) mod settings;
pub(crate) mod user_stats;
pub(crate) mod virtual_list;
pub(crate) mod youtube_layout;

mod audio;
mod click_events;
pub(crate) mod context;
pub(crate) mod desc_impl;
pub mod downloads;
pub(crate) mod episode;
pub(crate) mod episodes_layout;
pub(crate) mod gen_components;
pub mod gen_funcs;
#[cfg(feature = "server_build")]
pub(crate) mod login;
pub(crate) mod people_subs;
pub(crate) mod person;
pub(crate) mod podcast_layout;
pub(crate) mod podcasts;
pub(crate) mod search_new;
pub mod setting_components;
pub(crate) mod shared_episode;

#[cfg(not(feature = "server_build"))]
pub mod downloads_tauri;
#[cfg(not(feature = "server_build"))]
pub mod login_tauri;
