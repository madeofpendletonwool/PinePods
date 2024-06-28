// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use directories::ProjectDirs;
use reqwest::blocking::get;
use serde::{Deserialize, Serialize};
use std::fs;
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

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            list_dir,
            get_app_dir,
            download_file,
            delete_file,
            list_app_files
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
