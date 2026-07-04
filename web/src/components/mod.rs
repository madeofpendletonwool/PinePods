// In components/mod.rs
pub(crate) mod audio_player_bar;
pub(crate) mod app_drawer;
pub(crate) mod context_menu_button;
pub(crate) mod collection_picker_modal;
pub(crate) mod episode_list_item;
pub(crate) mod episode_list_view;
pub(crate) mod host_component;
pub(crate) mod loading;
pub mod misc_func;
pub(crate) mod navigation;
pub(crate) mod notification_center;
pub(crate) mod oauth_callback;
pub(crate) mod restore_overlay;
pub(crate) mod safehtml;
pub(crate) mod virtual_list;

pub(crate) mod audio;
pub(crate) mod click_events;
pub(crate) mod queue_panel;
pub(crate) mod queue_manage_modal;
pub(crate) mod context;
pub(crate) mod desc_impl;
pub(crate) mod gen_components;
pub mod gen_funcs;
// #[cfg(feature = "server_build")]
// pub(crate) mod login;
pub mod setting_components;

//#[cfg(not(feature = "server_build"))]
//pub mod downloads_tauri;
// #[cfg(not(feature = "server_build"))]
// pub mod login_tauri;
