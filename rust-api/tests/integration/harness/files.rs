//! The server's file storage during tests, and audio to put in it. `PINEPODS_DATA_DIR` and
//! `PINEPODS_WEB_ROOT` are process-wide, so all tests share one sandbox: downloads are told
//! apart by podcast name and ids, and local-media tests use a folder named after their tag.

use std::path::{Path, PathBuf};

use super::env;

/// Point the server's data and web directories at a fresh sandbox holding the real
/// translations. Resolved, because the local-media path checks compare resolved paths.
pub fn install_sandbox() {
    let base = std::env::temp_dir().join(format!("pinepods-files-{}", std::process::id()));
    let data = base.join("data");
    let web = base.join("web");
    let translations = web.join("static/translations");
    let assets = web.join("static/assets");
    for dir in [&data, &translations, &assets] {
        std::fs::create_dir_all(dir).expect("create test file sandbox");
    }
    let source = env::repo_root().join("web/src/translations");
    for entry in std::fs::read_dir(&source).expect("read translations").flatten() {
        std::fs::copy(entry.path(), translations.join(entry.file_name())).expect("copy translation");
    }
    std::fs::write(translations.join("broken.json"), b"{ not json").unwrap();
    std::fs::write(translations.join("notes.txt"), b"not a translation").unwrap();
    std::fs::write(assets.join("favicon.png"), b"fake-logo").unwrap();

    let data = data.canonicalize().unwrap();
    let web = web.canonicalize().unwrap();
    // SAFETY (edition 2021 API): runs once, before the server code under test starts.
    std::env::set_var("PINEPODS_DATA_DIR", &data);
    std::env::set_var("PINEPODS_WEB_ROOT", &web);
}

pub fn data_dir() -> PathBuf {
    PathBuf::from(std::env::var("PINEPODS_DATA_DIR").expect("sandbox installed"))
}

/// A folder under the local-media root.
pub fn local_media_folder(name: &str) -> PathBuf {
    let dir = data_dir().join("local-media").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Enough valid MPEG frames for tag libraries and ffmpeg to treat it as an MP3.
pub fn mp3_bytes() -> Vec<u8> {
    let mut bytes = Vec::with_capacity(417 * 40);
    for _ in 0..40 {
        bytes.extend_from_slice(&[0xFF, 0xFB, 0x90, 0x00]);
        bytes.extend(std::iter::repeat(0u8).take(413));
    }
    bytes
}

/// Mono 8 kHz WAV: `loud` seconds of a square wave, then `silent` seconds of silence, then
/// `loud` seconds again, so silence detection has one known gap to find.
pub fn wav_with_gap(loud: u32, silent: u32) -> Vec<u8> {
    let rate = 8000u32;
    let mut samples: Vec<u8> = Vec::new();
    let tone = |secs: u32, samples: &mut Vec<u8>| {
        for i in 0..(secs * rate) {
            let v: i16 = if (i / 8) % 2 == 0 { 12000 } else { -12000 };
            samples.extend_from_slice(&v.to_le_bytes());
        }
    };
    tone(loud, &mut samples);
    samples.extend(std::iter::repeat(0u8).take((silent * rate * 2) as usize));
    tone(loud, &mut samples);

    let mut wav = Vec::with_capacity(44 + samples.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + samples.len() as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(samples.len() as u32).to_le_bytes());
    wav.extend_from_slice(&samples);
    wav
}

pub fn assert_gone(path: &Path) {
    assert!(!path.exists(), "{} should have been removed", path.display());
}

static PROCESS_ENV: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Backups read `DB_*` from the process environment and write one shared folder, so tests
/// that use them take turns. Holds the lock until dropped.
pub struct BackupEnv {
    _guard: tokio::sync::MutexGuard<'static, ()>,
}

impl BackupEnv {
    pub async fn claim(app: &super::TestApp) -> BackupEnv {
        let guard = PROCESS_ENV.lock().await;
        let env = app.env;
        // SAFETY (edition 2021 API): only these tests read or write these variables.
        std::env::set_var("DB_TYPE", env.backend.db_type());
        std::env::set_var("DB_HOST", &env.db_host);
        std::env::set_var("DB_PORT", env.db_port.to_string());
        std::env::set_var("DB_USER", &env.db_user);
        std::env::set_var("DB_PASSWORD", &env.db_password);
        std::env::set_var("DB_NAME", &app.db_name);
        let dir = data_dir().join("backups");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        BackupEnv { _guard: guard }
    }

    pub fn dir(&self) -> PathBuf {
        data_dir().join("backups")
    }
}
