// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use directories::ProjectDirs;
use reqwest::blocking::get;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use tauri::command;

// Define the structure for the file entries
#[derive(Serialize, Deserialize)]
struct FileEntry {
    path: String,
}

// Function to list directory contents
#[command]
async fn list_dir(path: String) -> Result<Vec<FileEntry>, String> {
    let home_dir = dirs::home_dir().ok_or("Cannot find home directory")?;
    let target_path = if path == "~" {
        home_dir
    } else {
        PathBuf::from(path)
    };

    let mut entries = Vec::new();
    for entry in fs::read_dir(target_path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        entries.push(FileEntry {
            path: entry.path().display().to_string(),
        });
    }

    Ok(entries)
}

fn get_project_dirs() -> Result<ProjectDirs, String> {
    ProjectDirs::from("com", "gooseberrydevelopment", "pinepods")
        .ok_or_else(|| "Cannot determine project directories".to_string())
}

#[command]
fn get_app_dir() -> Result<String, String> {
    let proj_dirs = get_project_dirs()?;
    let app_dir = proj_dirs.data_dir();
    if !app_dir.exists() {
        fs::create_dir_all(app_dir).map_err(|e| e.to_string())?;
    }
    Ok(app_dir.display().to_string())
}

#[command]
async fn download_file(url: String, filename: String) -> Result<(), String> {
    let proj_dirs = get_project_dirs()?;
    let app_dir = proj_dirs.data_dir();
    if !app_dir.exists() {
        fs::create_dir_all(app_dir).map_err(|e| e.to_string())?;
    }

    let response = get(&url).map_err(|e| e.to_string())?;
    let mut file = fs::File::create(app_dir.join(filename)).map_err(|e| e.to_string())?;
    let content = response.bytes().map_err(|e| e.to_string())?;
    file.write_all(&content).map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Debug, Deserialize, Clone, PartialEq, Serialize)]
pub struct EpisodeInfo {
    pub episodetitle: String,
    pub podcastname: String,
    pub podcastid: i32,
    pub episodepubdate: String,
    pub episodedescription: String,
    pub episodeartwork: String,
    pub episodeurl: String,
    pub episodeduration: i32,
    pub listenduration: Option<i32>,
    pub episodeid: i32,
}

#[command]
async fn update_local_db(episode_info: EpisodeInfo) -> Result<(), String> {
    let proj_dirs = get_project_dirs().map_err(|e| e.to_string())?;
    let db_path = proj_dirs.data_dir().join("local_episodes.json");

    let mut episodes = if db_path.exists() {
        let data = std::fs::read_to_string(&db_path).map_err(|e| e.to_string())?;
        serde_json::from_str::<Vec<EpisodeInfo>>(&data).map_err(|e| e.to_string())?
    } else {
        Vec::new()
    };

    episodes.push(episode_info);

    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&db_path)
        .map_err(|e| e.to_string())?;
    serde_json::to_writer(file, &episodes).map_err(|e| e.to_string())?;

    Ok(())
}

#[command]
async fn remove_from_local_db(episode_id: i32) -> Result<(), String> {
    let proj_dirs = get_project_dirs().map_err(|e| e.to_string())?;
    let db_path = proj_dirs.data_dir().join("local_episodes.json");

    let mut episodes = if db_path.exists() {
        let data = std::fs::read_to_string(&db_path).map_err(|e| e.to_string())?;
        serde_json::from_str::<Vec<EpisodeInfo>>(&data).map_err(|e| e.to_string())?
    } else {
        return Ok(()); // No episodes to remove if file doesn't exist
    };

    episodes.retain(|episode| episode.episodeid != episode_id);

    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&db_path)
        .map_err(|e| e.to_string())?;
    serde_json::to_writer(file, &episodes).map_err(|e| e.to_string())?;

    Ok(())
}

#[command]
async fn get_local_episodes() -> Result<Vec<EpisodeInfo>, String> {
    let proj_dirs = get_project_dirs().map_err(|e| e.to_string())?;
    let db_path = proj_dirs.data_dir().join("local_episodes.json");

    if !db_path.exists() {
        return Ok(Vec::new());
    }

    let data = std::fs::read_to_string(&db_path).map_err(|e| e.to_string())?;
    let episodes = serde_json::from_str::<Vec<EpisodeInfo>>(&data).map_err(|e| e.to_string())?;

    Ok(episodes)
}

#[command]
fn delete_file(filename: String) -> Result<(), String> {
    let proj_dirs = get_project_dirs()?;
    let app_dir = proj_dirs.data_dir();
    let file_path = app_dir.join(filename);
    if file_path.exists() {
        fs::remove_file(file_path).map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("File does not exist".to_string())
    }
}

#[command]
fn list_app_files() -> Result<Vec<FileEntry>, String> {
    let proj_dirs = get_project_dirs()?;
    let app_dir = proj_dirs.data_dir();
    let mut entries = Vec::new();
    for entry in fs::read_dir(app_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        entries.push(FileEntry {
            path: entry.path().display().to_string(),
        });
    }
    Ok(entries)
}

#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct PodcastDetails {
    #[serde(rename = "PodcastName")]
    pub podcast_name: String,
    #[serde(rename = "ArtworkURL")]
    pub artwork_url: String,
    #[serde(rename = "Author")]
    pub author: String,
    #[serde(rename = "Categories")]
    pub categories: String,
    #[serde(rename = "Description")]
    pub description: String,
    #[serde(rename = "EpisodeCount")]
    pub episode_count: i32,
    #[serde(rename = "FeedURL")]
    pub feed_url: String,
    #[serde(rename = "WebsiteURL")]
    pub website_url: String,
    #[serde(rename = "Explicit")]
    pub explicit: bool,
    #[serde(rename = "UserID")]
    pub user_id: i32,
}

#[command]
async fn update_podcast_db(podcast_details: PodcastDetails) -> Result<(), String> {
    let proj_dirs = get_project_dirs().map_err(|e| e.to_string())?;
    let db_path = proj_dirs.data_dir().join("local_podcasts.json");

    let mut podcasts = if db_path.exists() {
        let data = std::fs::read_to_string(&db_path).map_err(|e| e.to_string())?;
        serde_json::from_str::<Vec<PodcastDetails>>(&data).map_err(|e| e.to_string())?
    } else {
        Vec::new()
    };

    if !podcasts
        .iter()
        .any(|p| p.user_id == podcast_details.user_id)
    {
        podcasts.push(podcast_details);
    }

    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&db_path)
        .map_err(|e| e.to_string())?;
    serde_json::to_writer(file, &podcasts).map_err(|e| e.to_string())?;

    Ok(())
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[allow(non_snake_case)]
pub struct Podcast {
    pub podcastid: i32,
    pub podcastname: String,
    pub artworkurl: Option<String>,
    pub description: Option<String>,
    pub episodecount: i32,
    pub websiteurl: Option<String>,
    pub feedurl: String,
    pub author: Option<String>,
    pub categories: String, // Keeping as String since it's handled as empty string "{}" or "{}"
    pub explicit: bool,
}

#[command]
async fn get_local_podcasts() -> Result<Vec<Podcast>, String> {
    let proj_dirs = get_project_dirs().map_err(|e| e.to_string())?;
    let db_path = proj_dirs.data_dir().join("local_podcasts.json");

    if !db_path.exists() {
        return Ok(Vec::new());
    }

    let data = std::fs::read_to_string(&db_path).map_err(|e| e.to_string())?;
    let podcasts = serde_json::from_str::<Vec<Podcast>>(&data).map_err(|e| e.to_string())?;

    Ok(podcasts)
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            list_dir,
            get_app_dir,
            download_file,
            delete_file,
            update_local_db,
            remove_from_local_db,
            update_podcast_db,
            get_local_podcasts,
            get_local_episodes,
            list_app_files
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
