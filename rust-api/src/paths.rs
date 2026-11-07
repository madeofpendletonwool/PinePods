//! Where the server keeps files. The defaults are the container's layout; `PINEPODS_DATA_DIR`
//! and `PINEPODS_WEB_ROOT` move them, which lets tests run without touching those locations.

fn data_dir() -> String {
    std::env::var("PINEPODS_DATA_DIR").unwrap_or_else(|_| "/opt/pinepods".to_string())
}

fn web_root() -> String {
    std::env::var("PINEPODS_WEB_ROOT").unwrap_or_else(|_| "/var/www/html".to_string())
}

pub fn downloads_dir() -> String {
    format!("{}/downloads", data_dir())
}

pub fn youtube_downloads_dir() -> String {
    format!("{}/downloads/youtube", data_dir())
}

pub fn transcribe_tmp_dir() -> String {
    format!("{}/downloads/.transcribe-tmp", data_dir())
}

pub fn backups_dir() -> String {
    format!("{}/backups", data_dir())
}

pub fn local_media_dir() -> String {
    format!("{}/local-media", data_dir())
}

pub fn translations_dir() -> String {
    format!("{}/static/translations", web_root())
}

pub fn logo_file() -> String {
    format!("{}/static/assets/favicon.png", web_root())
}
