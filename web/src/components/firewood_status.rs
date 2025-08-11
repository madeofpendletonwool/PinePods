// src/components/firewood_status.rs

use crate::components::context::AppState;
use crate::components::setting_components::firewood_players::{
    FirewoodPlaybackStatus, FirewoodServer, get_firewood_server_by_id,
    pause_firewood_playback, resume_firewood_playback, stop_firewood_playback,
    skip_firewood_playback, set_firewood_volume, poll_firewood_status_for_server,
    set_active_firewood_server,
};
use crate::components::gen_funcs::format_time;
use gloo_timers::callback::Interval;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yewdux::prelude::*;

#[derive(Properties, PartialEq)]
pub struct FirewoodStatusProps {
    #[prop_or_default]
    pub show_compact: bool, // Show compact version for mini-player
}

#[function_component(FirewoodStatus)]
pub fn firewood_status(props: &FirewoodStatusProps) -> Html {
    let (state, dispatch) = use_store::<AppState>();
    
    // Get active Firewood server status
    let active_server_status = if let (Some(active_server_id), Some(status_map)) = 
        (&state.active_firewood_server, &state.firewood_status) {
        status_map.get(active_server_id).cloned()
    } else {
        None
    };
    
    let active_server = if let (Some(active_server_id), Some(servers)) = 
        (&state.active_firewood_server, &state.firewood_servers) {
        get_firewood_server_by_id(servers, *active_server_id)
    } else {
        None
    };
    
    // Volume slider state
    let volume_slider_value = use_state(|| 70.0);
    
    // Real-time status update effect
    {
        let dispatch = dispatch.clone();
        let active_server_id = state.active_firewood_server;
        let status_map = state.firewood_status.clone();
        
        use_effect_with((active_server_id, status_map), move |(server_id, _)| {
            if let Some(server_id) = server_id {
                // Start periodic status updates every 3 seconds for the active server
                let dispatch = dispatch.clone();
                let server_id = *server_id;
                
                let interval = Interval::new(3000, move || {
                    let dispatch = dispatch.clone();
                    spawn_local(async move {
                        if let Some(status_map) = dispatch.get().firewood_status.as_ref() {
                            if let Some((server_address, _)) = status_map.get(&server_id) {
                                let _ = poll_firewood_status_for_server(
                                    server_id,
                                    server_address,
                                    &dispatch
                                ).await;
                            }
                        }
                    });
                });
                
                Box::new(move || {
                    interval.cancel();
                }) as Box<dyn FnOnce()>
            } else {
                Box::new(move || {
                }) as Box<dyn FnOnce()>
            }
        });
    }
    
    // Handle remote control actions
    let handle_pause_resume = {
        let dispatch = dispatch.clone();
        let server_address = active_server_status.as_ref().map(|(addr, _)| addr.clone());
        let is_playing = active_server_status.as_ref().map(|(_, status)| status.is_playing).unwrap_or(false);
        
        Callback::from(move |_: MouseEvent| {
            if let Some(address) = &server_address {
                let address = address.clone();
                let dispatch = dispatch.clone();
                
                spawn_local(async move {
                    let result = if is_playing {
                        pause_firewood_playback(&address).await
                    } else {
                        resume_firewood_playback(&address).await
                    };
                    
                    match result {
                        Ok(_) => {
                            // Status will be updated by the periodic polling
                        }
                        Err(e) => {
                            dispatch.reduce_mut(|state| {
                                state.error_message = Some(format!("Firewood control failed: {:?}", e));
                            });
                        }
                    }
                });
            }
        })
    };
    
    let handle_stop = {
        let dispatch = dispatch.clone();
        let server_address = active_server_status.as_ref().map(|(addr, _)| addr.clone());
        
        Callback::from(move |_: MouseEvent| {
            if let Some(address) = &server_address {
                let address = address.clone();
                let dispatch = dispatch.clone();
                
                spawn_local(async move {
                    match stop_firewood_playback(&address).await {
                        Ok(_) => {
                            // Clear active server since playback stopped
                            set_active_firewood_server(&dispatch, None);
                        }
                        Err(e) => {
                            dispatch.reduce_mut(|state| {
                                state.error_message = Some(format!("Firewood stop failed: {:?}", e));
                            });
                        }
                    }
                });
            }
        })
    };
    
    let handle_skip_forward = {
        let dispatch = dispatch.clone();
        let server_address = active_server_status.as_ref().map(|(addr, _)| addr.clone());
        
        Callback::from(move |_: MouseEvent| {
            if let Some(address) = &server_address {
                let address = address.clone();
                let dispatch = dispatch.clone();
                
                spawn_local(async move {
                    match skip_firewood_playback(&address, 15).await {
                        Ok(_) => {
                            // Status will be updated by the periodic polling
                        }
                        Err(e) => {
                            dispatch.reduce_mut(|state| {
                                state.error_message = Some(format!("Firewood skip failed: {:?}", e));
                            });
                        }
                    }
                });
            }
        })
    };
    
    let handle_skip_backward = {
        let dispatch = dispatch.clone();
        let server_address = active_server_status.as_ref().map(|(addr, _)| addr.clone());
        
        Callback::from(move |_: MouseEvent| {
            if let Some(address) = &server_address {
                let address = address.clone();
                let dispatch = dispatch.clone();
                
                spawn_local(async move {
                    match skip_firewood_playback(&address, -15).await {
                        Ok(_) => {
                            // Status will be updated by the periodic polling
                        }
                        Err(e) => {
                            dispatch.reduce_mut(|state| {
                                state.error_message = Some(format!("Firewood skip failed: {:?}", e));
                            });
                        }
                    }
                });
            }
        })
    };
    
    let handle_volume_change = {
        let dispatch = dispatch.clone();
        let server_address = active_server_status.as_ref().map(|(addr, _)| addr.clone());
        let volume_slider_value = volume_slider_value.clone();
        
        Callback::from(move |e: InputEvent| {
            let target = e.target_unchecked_into::<HtmlInputElement>();
            let volume = target.value().parse::<f32>().unwrap_or(50.0) / 100.0;
            volume_slider_value.set(volume * 100.0);
            
            if let Some(address) = &server_address {
                let address = address.clone();
                let dispatch = dispatch.clone();
                
                spawn_local(async move {
                    match set_firewood_volume(&address, volume).await {
                        Ok(_) => {
                            // Volume updated successfully
                        }
                        Err(e) => {
                            dispatch.reduce_mut(|state| {
                                state.error_message = Some(format!("Firewood volume failed: {:?}", e));
                            });
                        }
                    }
                });
            }
        })
    };
    
    // Show nothing if no active server
    if active_server_status.is_none() || active_server.is_none() {
        return html! {};
    }
    
    let (server_address, status) = active_server_status.unwrap();
    let server = active_server.unwrap();
    
    if props.show_compact {
        // Compact version for mini-player or sidebars
        html! {
            <div class="firewood-status-compact p-2 border rounded-lg mb-2">
                <div class="flex items-center space-x-2">
                    <i class="ph ph-broadcast text-lg firewood-status-icon"></i>
                    <div class="flex-1 min-w-0">
                        <div class="text-sm font-medium firewood-server-name truncate">{&server.server_name}</div>
                        if let Some(episode) = &status.current_episode {
                            <div class="text-xs firewood-episode-info opacity-80 truncate">{&episode.episode_title}</div>
                        }
                    </div>
                    <div class="flex items-center space-x-1">
                        <button onclick={handle_skip_backward} class="firewood-control-btn-sm">
                            <i class="ph ph-skip-back"></i>
                        </button>
                        <button onclick={handle_pause_resume} class="firewood-control-btn-sm">
                            <i class={if status.is_playing { "ph ph-pause" } else { "ph ph-play" }}></i>
                        </button>
                        <button onclick={handle_skip_forward} class="firewood-control-btn-sm">
                            <i class="ph ph-skip-forward"></i>
                        </button>
                    </div>
                </div>
            </div>
        }
    } else {
        // Full version for main audio player
        html! {
            <div class="firewood-status-full p-4 border rounded-lg mb-4">
                <div class="flex items-start space-x-4">
                    <div class="firewood-status-icon-large">
                        <i class="ph ph-broadcast text-2xl"></i>
                    </div>
                    
                    <div class="flex-1 min-w-0">
                        <div class="firewood-status-header mb-2">
                            <h4 class="text-lg font-medium firewood-server-name">{&server.server_name}</h4>
                            <div class="text-sm firewood-server-address opacity-70">{&server_address}</div>
                        </div>
                        
                        if let Some(episode) = &status.current_episode {
                            <div class="firewood-episode-info mb-3">
                                <div class="text-base font-medium firewood-episode-title">{&episode.episode_title}</div>
                                <div class="text-sm firewood-podcast-name opacity-80">{&episode.podcast_name}</div>
                            </div>
                        }
                        
                        <div class="firewood-playback-info mb-3">
                            <div class="flex items-center justify-between text-sm">
                                <span>{format_time(status.position as f64)}</span>
                                <span class={if status.is_playing { "text-green-500" } else { "text-gray-500" }}>
                                    {if status.is_playing { "Playing" } else { "Paused" }}
                                </span>
                                <span>{format_time(status.duration as f64)}</span>
                            </div>
                            <div class="firewood-progress-bar mt-1">
                                <div class="w-full bg-gray-200 rounded-full h-2">
                                    <div 
                                        class="bg-blue-600 h-2 rounded-full transition-all duration-300"
                                        style={format!("width: {}%", if status.duration > 0 { 
                                            (status.position as f64 / status.duration as f64 * 100.0) 
                                        } else { 
                                            0.0 
                                        })}
                                    ></div>
                                </div>
                            </div>
                        </div>
                        
                        <div class="firewood-controls flex items-center justify-center space-x-4 mb-3">
                            <button onclick={handle_skip_backward} class="firewood-control-btn">
                                <i class="ph ph-skip-back text-xl"></i>
                            </button>
                            <button onclick={handle_pause_resume} class="firewood-control-btn-primary">
                                <i class={format!("ph {} text-2xl", if status.is_playing { "ph-pause" } else { "ph-play" })}></i>
                            </button>
                            <button onclick={handle_skip_forward} class="firewood-control-btn">
                                <i class="ph ph-skip-forward text-xl"></i>
                            </button>
                            <button onclick={handle_stop} class="firewood-control-btn">
                                <i class="ph ph-stop text-xl"></i>
                            </button>
                        </div>
                        
                        <div class="firewood-volume-control flex items-center space-x-3">
                            <i class="ph ph-speaker-low"></i>
                            <input 
                                type="range" 
                                min="0" 
                                max="100" 
                                value={format!("{}", (status.volume * 100.0) as i32)}
                                oninput={handle_volume_change}
                                class="firewood-volume-slider flex-1"
                            />
                            <i class="ph ph-speaker-high"></i>
                            <span class="text-sm firewood-volume-display">{format!("{}%", (status.volume * 100.0) as i32)}</span>
                        </div>
                    </div>
                </div>
            </div>
        }
    }
}