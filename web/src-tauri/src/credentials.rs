//! The saved login, so the app stays signed in across restarts.
//!
//! Kept in the system keychain (macOS Keychain, Windows Credential Manager, the Secret
//! Service on Linux). Where that isn't available (no Secret Service, some sandboxes) it falls
//! back to files readable only by this user. Logins saved as files by earlier versions move
//! into the keychain the first time they're read. `PINEPODS_CREDENTIALS=file` keeps them in
//! files (portable installs).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const SERVICE: &str = "com.gooseberrydevelopment.pinepods";
/// Names of the entries in the keychain, so signing out can remove them all.
const KEYCHAIN_INDEX: &str = "keychain_keys.json";
/// Entries the app writes, removed on sign-out even if the index is lost.
const KNOWN_KEYS: [&str; 3] = ["userState", "userAuthState", "serverState"];

#[cfg(test)]
thread_local! {
    /// Tests use files unless they opt in (against keyring's mock store).
    static TEST_KEYCHAIN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn check_key(key: &str) -> Result<(), String> {
    if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err("invalid credential key".into());
    }
    Ok(())
}

fn dir(data_dir: &Path) -> PathBuf {
    data_dir.join("credentials")
}

fn file_for(data_dir: &Path, key: &str) -> PathBuf {
    dir(data_dir).join(format!("{key}.json"))
}

fn keychain_enabled() -> bool {
    #[cfg(test)]
    return TEST_KEYCHAIN.with(|t| t.get());
    #[cfg(not(test))]
    std::env::var_os("PINEPODS_CREDENTIALS").is_none_or(|v| v != "file")
}

fn entry(key: &str) -> Option<keyring::Entry> {
    if !keychain_enabled() {
        return None;
    }
    keyring::Entry::new(SERVICE, key)
        .map_err(|e| log::warn!("keychain unavailable: {e}"))
        .ok()
}

fn index(data_dir: &Path) -> BTreeSet<String> {
    fs::read_to_string(dir(data_dir).join(KEYCHAIN_INDEX))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn update_index(data_dir: &Path, change: impl FnOnce(&mut BTreeSet<String>)) {
    let mut keys = index(data_dir);
    change(&mut keys);
    let _ = fs::create_dir_all(dir(data_dir));
    if let Ok(text) = serde_json::to_string(&keys) {
        let _ = fs::write(dir(data_dir).join(KEYCHAIN_INDEX), text);
    }
}

/// The value is now in the keychain: drop any file copy and remember the entry.
fn moved_to_keychain(data_dir: &Path, key: &str) {
    let _ = fs::remove_file(file_for(data_dir, key));
    update_index(data_dir, |k| {
        k.insert(key.to_string());
    });
}

fn write_file(data_dir: &Path, key: &str, value: &str) -> Result<(), String> {
    let path = file_for(data_dir, key);
    fs::create_dir_all(dir(data_dir)).map_err(|e| e.to_string())?;
    fs::write(&path, value).map_err(|e| e.to_string())?;
    restrict(&path);
    Ok(())
}

/// Save into the keychain, falling back to a file.
pub fn store(data_dir: &Path, key: &str, value: &str) -> Result<(), String> {
    check_key(key)?;
    if let Some(entry) = entry(key) {
        match entry.set_password(value) {
            Ok(()) => {
                moved_to_keychain(data_dir, key);
                return Ok(());
            }
            Err(e) => log::warn!("could not save {key} in the keychain, using a file: {e}"),
        }
    }
    write_file(data_dir, key, value)
}

pub fn get(data_dir: &Path, key: &str) -> Result<String, String> {
    check_key(key)?;
    let entry = entry(key);
    if let Some(entry) = &entry {
        match entry.get_password() {
            Ok(value) => return Ok(value),
            Err(keyring::Error::NoEntry) => {}
            Err(e) => log::warn!("could not read {key} from the keychain: {e}"),
        }
    }
    let value = fs::read_to_string(file_for(data_dir, key)).map_err(|_| "Credential not found".to_string())?;
    // Saved as a file by an earlier version, or while the keychain was unavailable.
    if entry.is_some_and(|e| e.set_password(&value).is_ok()) {
        moved_to_keychain(data_dir, key);
        log::info!("moved {key} into the keychain");
    }
    Ok(value)
}

pub fn remove(data_dir: &Path, key: &str) -> Result<(), String> {
    check_key(key)?;
    if let Some(entry) = entry(key) {
        let _ = entry.delete_credential();
        update_index(data_dir, |k| {
            k.remove(key);
        });
    }
    let path = file_for(data_dir, key);
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn clear(data_dir: &Path) -> Result<(), String> {
    let mut keys = index(data_dir);
    keys.extend(KNOWN_KEYS.iter().map(|k| k.to_string()));
    for key in keys {
        if let Some(entry) = entry(&key) {
            let _ = entry.delete_credential();
        }
    }
    let dir = dir(data_dir);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Against keyring's in-memory mock, which keeps a value only in the `Entry` that set
    /// it: what's checked is that files move out and sign-out clears everything.
    #[test]
    fn keychain_takes_over_from_files() {
        TEST_KEYCHAIN.with(|t| t.set(true));
        keyring::set_default_credential_builder(keyring::mock::default_credential_builder());
        let dir = std::env::temp_dir().join(format!("pinepods-creds-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);

        write_file(&dir, "userAuthState", "old").unwrap();
        assert_eq!(get(&dir, "userAuthState").unwrap(), "old");
        assert!(!file_for(&dir, "userAuthState").exists(), "moved out of the file");
        assert!(index(&dir).contains("userAuthState"));

        store(&dir, "serverState", "{}").unwrap();
        assert!(!file_for(&dir, "serverState").exists());
        assert!(store(&dir, "../x", "v").is_err());

        clear(&dir).unwrap();
        assert!(!dir.join("credentials").exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
