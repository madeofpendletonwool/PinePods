//! The saved login, so the app stays signed in across restarts.

use std::fs;
use std::path::{Path, PathBuf};

fn path_for(data_dir: &Path, key: &str) -> Result<PathBuf, String> {
    if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err("invalid credential key".into());
    }
    Ok(data_dir.join("credentials").join(format!("{key}.json")))
}

pub fn store(data_dir: &Path, key: &str, value: &str) -> Result<(), String> {
    let path = path_for(data_dir, key)?;
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    fs::write(&path, value).map_err(|e| e.to_string())?;
    restrict(&path);
    Ok(())
}

pub fn get(data_dir: &Path, key: &str) -> Result<String, String> {
    let path = path_for(data_dir, key)?;
    fs::read_to_string(path).map_err(|_| "Credential not found".to_string())
}

pub fn remove(data_dir: &Path, key: &str) -> Result<(), String> {
    let path = path_for(data_dir, key)?;
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn clear(data_dir: &Path) -> Result<(), String> {
    let dir = data_dir.join("credentials");
    if dir.exists() {
        fs::remove_dir_all(dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Readable by this user only.
#[cfg(unix)]
fn restrict(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict(_path: &Path) {}
