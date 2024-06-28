// In components/mod.rs
pub(crate) mod app_drawer;
pub(crate) mod history;
pub(crate) mod home;
pub mod misc_func;
pub(crate) mod queue;
pub(crate) mod routes;
pub(crate) mod saved;
pub(crate) mod search;
pub(crate) mod settings;
pub(crate) mod user_stats;

mod audio;
mod click_events;
pub(crate) mod context;
pub(crate) mod desc_impl;
pub(crate) mod episode;
pub(crate) mod episodes_layout;
pub(crate) mod gen_components;
pub mod gen_funcs;
pub(crate) mod podcast_layout;
pub(crate) mod podcasts;
pub(crate) mod search_new;
pub mod setting_components;

#[cfg(feature = "server_build")]
pub mod downloads;
#[cfg(feature = "server_build")]
pub mod login;

#[cfg(not(feature = "server_build"))]
pub mod downloads_tauri;
#[cfg(not(feature = "server_build"))]
pub mod login_tauri;
