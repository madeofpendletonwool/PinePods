// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::copy;
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
    println!(
        "Starting download_file with url: {}, filename: {}",
        url, filename
    );
    let proj_dirs = get_project_dirs()?;
    let app_dir: PathBuf = proj_dirs.data_dir().to_path_buf();
    println!("App dir path: {:?}", app_dir);
    if !app_dir.exists() {
        println!("Creating app directory");
        fs::create_dir_all(&app_dir).map_err(|e| e.to_string())?;
    }

    let url = url.clone();
    let filename = filename.clone();

    // Use tokio::task::spawn_blocking for blocking operations
    tokio::task::spawn_blocking(move || {
        let agent = ureq::Agent::config_builder()
            .max_redirects(20)
            .build()
            .new_agent();
            
        let mut response = agent.get(&url).call().map_err(|e| e.to_string())?;
        let mut reader = response.body_mut().with_config().reader(); // Alternative approach
        let mut file = File::create(app_dir.join(&filename)).map_err(|e| e.to_string())?;
        copy(&mut reader, &mut file).map_err(|e| e.to_string())?;
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Debug, Deserialize, Default, Clone, PartialEq, Serialize)]
#[serde(default)] 
pub struct EpisodeInfo {
    pub episodetitle: String,
    pub podcastname: String,
    pub podcastid: i32,
    pub podcastindexid: Option<i64>,
    pub feedurl: String,  // This field exists in the response
    pub episodepubdate: String,
    pub episodedescription: String,
    pub episodeartwork: String,
    pub episodeurl: String,
    pub episodeduration: i32,
    pub listenduration: Option<i32>,
    pub episodeid: i32,
    pub completed: bool,
    pub is_queued: bool,
    pub is_saved: bool,
    pub is_downloaded: bool,
    pub downloadedlocation: Option<String>,
    pub is_youtube: bool,
}

#[command]
async fn update_local_db(mut episode_info: EpisodeInfo) -> Result<(), String> {
    let proj_dirs = get_project_dirs().map_err(|e| e.to_string())?;
    let db_path = proj_dirs.data_dir().join("local_episodes.json");

    // Calculate the downloaded location
    let download_dir = proj_dirs
        .data_dir()
        .join(format!("episode_{}.mp3", episode_info.episodeid));
    episode_info.downloadedlocation = Some(download_dir.to_string_lossy().into_owned());

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
async fn remove_from_local_db(episodeid: i32) -> Result<(), String> {
    let proj_dirs = get_project_dirs().map_err(|e| e.to_string())?;
    let db_path = proj_dirs.data_dir().join("local_episodes.json");

    let mut episodes = if db_path.exists() {
        let data = std::fs::read_to_string(&db_path).map_err(|e| e.to_string())?;
        serde_json::from_str::<Vec<EpisodeInfo>>(&data).map_err(|e| e.to_string())?
    } else {
        return Ok(()); // No episodes to remove if file doesn't exist
    };

    episodes.retain(|episode| episode.episodeid != episodeid);

    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&db_path)
        .map_err(|e| e.to_string())?;
    serde_json::to_writer(file, &episodes).map_err(|e| e.to_string())?;

    // Delete the audio file and artwork
    let audio_file_path = proj_dirs
        .data_dir()
        .join(format!("episode_{}.mp3", episodeid));
    let artwork_file_path = proj_dirs
        .data_dir()
        .join(format!("artwork_{}.jpg", episodeid));

    if audio_file_path.exists() {
        std::fs::remove_file(audio_file_path).map_err(|e| e.to_string())?;
    }

    if artwork_file_path.exists() {
        std::fs::remove_file(artwork_file_path).map_err(|e| e.to_string())?;
    }

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
    pub podcastid: i32,
    pub podcastindexid: Option<i64>,
    pub artworkurl: String,
    pub author: String,
    pub categories: String,
    pub description: String,
    pub episodecount: i32,
    pub explicit: bool,
    pub feedurl: String,
    pub podcastname: String,
    pub userid: i32,
    pub websiteurl: String,
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
        .any(|p| p.podcastid == podcast_details.podcastid)
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
    pub podcastindexid: Option<i64>,
    pub podcastname: String,
    pub artworkurl: Option<String>,
    pub description: Option<String>,
    pub episodecount: i32,
    pub websiteurl: Option<String>,
    pub feedurl: String,
    pub author: Option<String>,
    pub categories: String, // Keeping as String since it's handled as empty string "{}" or "{}"
    pub explicit: bool,
    // pub is_youtube: bool,
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

#[tauri::command]
async fn get_local_file(filepath: String) -> Result<Vec<u8>, String> {
    use std::fs::File;
    use std::io::Read;
    use std::path::PathBuf;

    let path = PathBuf::from(filepath);
    let mut file = File::open(&path).map_err(|e| e.to_string())?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).map_err(|e| e.to_string())?;
    Ok(buffer)
}

#[tauri::command]
async fn start_file_server(filepath: String) -> Result<String, String> {
    // Log the file path to ensure it's correct
    println!("Starting file server with path: {}", filepath);

    // Ensure the path exists and is accessible
    if !std::path::Path::new(&filepath).exists() {
        return Err(format!("File path does not exist: {}", filepath));
    }

    // Get the directory of the file
    let file_dir = std::path::Path::new(&filepath)
        .parent()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();

    // Log the directory being served
    println!("Serving files from directory: {}", file_dir);

    // Create the warp filter to serve the directory containing the file
    let file_route = warp::fs::dir(file_dir);

    // Start the warp server
    tokio::spawn(warp::serve(file_route).run(([127, 0, 0, 1], 3030)));

    Ok("http://127.0.0.1:3030".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
            list_app_files,
            get_local_file,
            start_file_server
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
