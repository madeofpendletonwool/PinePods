//! The index of episodes downloaded to this computer (`library.json`) and the files it points at.
//!
//! Media and artwork live in `<data dir>/downloads/`. The index is replaced atomically on every
//! save and the previous copy is kept as `library.json.bak`, so a crash mid-save can't lose it.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const LIBRARY_FILE: &str = "library.json";
const VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DownloadedEpisode {
    pub episode_id: i32,
    pub podcast_id: i32,
    /// File name inside the downloads folder.
    pub file: String,
    #[serde(default)]
    pub artwork_file: Option<String>,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub downloaded_at: i64,
    /// The episode as the app sent it, returned as-is so new fields need no change here.
    pub episode: Value,
    /// Downloaded by automatic downloads, which may also delete it.
    #[serde(default)]
    pub automatic: bool,
}

#[derive(Serialize, Deserialize, Default)]
struct LibraryFile {
    version: u32,
    episodes: Vec<DownloadedEpisode>,
    #[serde(default)]
    podcasts: Vec<Value>,
}

pub struct Library {
    data_dir: PathBuf,
    downloads_dir: PathBuf,
    episodes: Vec<DownloadedEpisode>,
    podcasts: Vec<Value>,
}

impl Library {
    /// Load the index, moving downloads from the old layout into it the first time.
    pub fn open(data_dir: &Path) -> Library {
        let downloads_dir = data_dir.join("downloads");
        if let Err(e) = fs::create_dir_all(&downloads_dir) {
            log::error!("Could not create {}: {e}", downloads_dir.display());
        }
        let mut lib = Library {
            data_dir: data_dir.to_path_buf(),
            downloads_dir,
            episodes: Vec::new(),
            podcasts: Vec::new(),
        };
        match lib.read() {
            Some(file) => {
                lib.episodes = file.episodes;
                lib.podcasts = file.podcasts;
            }
            None => lib.migrate_legacy(),
        }
        lib.reconcile();
        lib
    }

    pub fn downloads_dir(&self) -> &Path {
        &self.downloads_dir
    }

    pub fn episodes(&self) -> &[DownloadedEpisode] {
        &self.episodes
    }

    pub fn podcasts(&self) -> &[Value] {
        &self.podcasts
    }

    pub fn contains(&self, episode_id: i32) -> bool {
        self.episodes.iter().any(|e| e.episode_id == episode_id)
    }

    pub fn upsert(&mut self, record: DownloadedEpisode, podcast: Option<Value>) {
        if let Some(pos) = self.episodes.iter().position(|e| e.episode_id == record.episode_id) {
            let old = self.episodes.remove(pos);
            let keep = [Some(&record.file), record.artwork_file.as_ref()];
            for name in std::iter::once(&old.file).chain(old.artwork_file.iter()) {
                if !keep.contains(&Some(name)) {
                    let _ = fs::remove_file(self.downloads_dir.join(name));
                }
            }
        }
        let podcast_id = record.podcast_id;
        self.episodes.push(record);
        if let Some(podcast) = podcast {
            self.podcasts.retain(|p| podcast_id_of(p) != Some(podcast_id));
            self.podcasts.push(podcast);
        }
        self.save();
    }

    /// Remove episodes and their files. Returns the ids that were removed.
    pub fn remove(&mut self, ids: &[i32]) -> Vec<i32> {
        let mut removed = Vec::new();
        let (gone, kept): (Vec<_>, Vec<_>) =
            self.episodes.drain(..).partition(|e| ids.contains(&e.episode_id));
        self.episodes = kept;
        for record in gone {
            self.delete_files(&record);
            removed.push(record.episode_id);
        }
        self.drop_unused_podcasts();
        self.save();
        removed
    }

    fn delete_files(&self, record: &DownloadedEpisode) {
        for name in std::iter::once(&record.file).chain(record.artwork_file.iter()) {
            let path = self.downloads_dir.join(name);
            if path.exists() {
                if let Err(e) = fs::remove_file(&path) {
                    log::warn!("Could not delete {}: {e}", path.display());
                }
            }
        }
    }

    fn drop_unused_podcasts(&mut self) {
        let used: Vec<i32> = self.episodes.iter().map(|e| e.podcast_id).collect();
        self.podcasts
            .retain(|p| podcast_id_of(p).is_some_and(|id| used.contains(&id)));
    }

    /// Drop records whose media file is gone and clear out interrupted downloads.
    fn reconcile(&mut self) {
        let before = self.episodes.len();
        let dir = self.downloads_dir.clone();
        self.episodes.retain(|e| dir.join(&e.file).is_file());
        let lost = before - self.episodes.len();
        if lost > 0 {
            log::warn!("{lost} downloaded episode(s) had no file and were removed from the index");
        }
        if let Ok(entries) = fs::read_dir(&self.downloads_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "part") {
                    let _ = fs::remove_file(&path);
                }
            }
        }
        self.drop_unused_podcasts();
        if lost > 0 {
            self.save();
        }
    }

    fn read(&self) -> Option<LibraryFile> {
        let path = self.data_dir.join(LIBRARY_FILE);
        let backup = path.with_extension("json.bak");
        for candidate in [&path, &backup] {
            let Ok(data) = fs::read_to_string(candidate) else { continue };
            match serde_json::from_str::<LibraryFile>(&data) {
                Ok(file) => return Some(file),
                Err(e) => {
                    log::error!("{} is unreadable ({e}); keeping it as .corrupt", candidate.display());
                    let _ = fs::rename(candidate, candidate.with_extension("corrupt"));
                }
            }
        }
        None
    }

    fn save(&self) {
        let path = self.data_dir.join(LIBRARY_FILE);
        let file = LibraryFile {
            version: VERSION,
            episodes: self.episodes.clone(),
            podcasts: self.podcasts.clone(),
        };
        if let Err(e) = write_atomic(&path, &file) {
            log::error!("Could not save {}: {e}", path.display());
        }
    }

    /// Move downloads made by 0.7 (`local_episodes.json`, `episode_<id>.mp3` beside it).
    fn migrate_legacy(&mut self) {
        let old_index = self.data_dir.join("local_episodes.json");
        let Ok(data) = fs::read_to_string(&old_index) else { return };
        let old: Vec<Value> = serde_json::from_str(&data).unwrap_or_default();
        for mut episode in old {
            let Some(id) = episode.get("episodeid").and_then(Value::as_i64).map(|v| v as i32) else {
                continue;
            };
            let old_media = self.data_dir.join(format!("episode_{id}.mp3"));
            if !old_media.is_file() || self.contains(id) {
                continue;
            }
            let file = format!("{id}.mp3");
            if fs::rename(&old_media, self.downloads_dir.join(&file)).is_err() {
                continue;
            }
            let old_art = self.data_dir.join(format!("artwork_{id}.jpg"));
            let artwork_file = (old_art.is_file()
                && fs::rename(&old_art, self.downloads_dir.join(format!("{id}-art.jpg"))).is_ok())
            .then(|| format!("{id}-art.jpg"));
            // 0.7 stored these under different names; keep one name each (the app reads both
            // names into the same field and rejects a record that has both).
            if let Some(obj) = episode.as_object_mut() {
                for (old_key, new_key) in [("is_queued", "queued"), ("is_saved", "saved")] {
                    if let Some(v) = obj.remove(old_key) {
                        obj.insert(new_key.to_string(), v);
                    }
                }
            }
            let size = fs::metadata(self.downloads_dir.join(&file)).map(|m| m.len()).unwrap_or(0);
            self.episodes.push(DownloadedEpisode {
                episode_id: id,
                podcast_id: episode.get("podcastid").and_then(Value::as_i64).unwrap_or(0) as i32,
                file,
                artwork_file,
                size,
                downloaded_at: 0,
                episode,
                automatic: false,
            });
        }
        let old_podcasts = self.data_dir.join("local_podcasts.json");
        if let Ok(data) = fs::read_to_string(&old_podcasts) {
            self.podcasts = serde_json::from_str(&data).unwrap_or_default();
        }
        log::info!("Moved {} download(s) from the old layout", self.episodes.len());
        self.save();
        let _ = fs::rename(&old_index, old_index.with_extension("json.migrated"));
        let _ = fs::rename(&old_podcasts, old_podcasts.with_extension("json.migrated"));
    }
}

pub fn podcast_id_of(podcast: &Value) -> Option<i32> {
    podcast.get("podcastid").and_then(Value::as_i64).map(|v| v as i32)
}

/// Write `value` to a temp file, then swap it in, keeping the old file as `.bak`.
pub fn write_atomic(path: &Path, value: &impl Serialize) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&serde_json::to_vec(value)?)?;
        f.sync_all()?;
    }
    if path.exists() {
        fs::rename(path, path.with_extension("json.bak"))?;
    }
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tmpdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pinepods-lib-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn record(lib: &Library, id: i32) -> DownloadedEpisode {
        let file = format!("{id}.mp3");
        fs::write(lib.downloads_dir().join(&file), b"audio").unwrap();
        DownloadedEpisode {
            episode_id: id,
            podcast_id: 7,
            file,
            artwork_file: None,
            size: 5,
            downloaded_at: 1,
            episode: json!({ "episodeid": id, "podcastid": 7 }),
            automatic: false,
        }
    }

    #[test]
    fn survives_restart_and_removes_files() {
        let dir = tmpdir("restart");
        let mut lib = Library::open(&dir);
        let r = record(&lib, 1);
        lib.upsert(r, Some(json!({ "podcastid": 7 })));
        let lib2 = Library::open(&dir);
        assert_eq!(lib2.episodes().len(), 1);
        assert_eq!(lib2.podcasts().len(), 1);
        let mut lib2 = lib2;
        assert_eq!(lib2.remove(&[1]), vec![1]);
        assert!(!dir.join("downloads/1.mp3").exists());
        assert!(lib2.podcasts().is_empty());
    }

    #[test]
    fn corrupt_index_falls_back_to_backup() {
        let dir = tmpdir("corrupt");
        let mut lib = Library::open(&dir);
        let (a, b) = (record(&lib, 1), record(&lib, 2));
        lib.upsert(a, None);
        lib.upsert(b, None); // .bak now holds the one-episode index
        fs::write(dir.join(LIBRARY_FILE), b"{ not json").unwrap();
        let lib = Library::open(&dir);
        assert_eq!(lib.episodes().len(), 1);
        assert!(dir.join("library.corrupt").exists());
    }

    #[test]
    fn missing_files_and_partials_are_cleaned() {
        let dir = tmpdir("reconcile");
        let mut lib = Library::open(&dir);
        let r = record(&lib, 3);
        lib.upsert(r, None);
        fs::remove_file(dir.join("downloads/3.mp3")).unwrap();
        fs::write(dir.join("downloads/4.part"), b"half").unwrap();
        let lib = Library::open(&dir);
        assert!(lib.episodes().is_empty());
        assert!(!dir.join("downloads/4.part").exists());
    }

    #[test]
    fn migrates_old_layout() {
        let dir = tmpdir("legacy");
        fs::write(dir.join("episode_9.mp3"), b"audio").unwrap();
        fs::write(dir.join("artwork_9.jpg"), b"art").unwrap();
        fs::write(
            dir.join("local_episodes.json"),
            json!([{ "episodeid": 9, "podcastid": 2, "is_saved": true }, { "episodeid": 10 }]).to_string(),
        )
        .unwrap();
        fs::write(dir.join("local_podcasts.json"), json!([{ "podcastid": 2 }]).to_string()).unwrap();
        let lib = Library::open(&dir);
        assert_eq!(lib.episodes().len(), 1);
        let ep = &lib.episodes()[0];
        assert_eq!(ep.file, "9.mp3");
        assert_eq!(ep.artwork_file.as_deref(), Some("9-art.jpg"));
        assert_eq!(ep.episode["saved"], json!(true));
        assert!(ep.episode.get("is_saved").is_none());
        assert!(dir.join("downloads/9.mp3").exists());
        assert!(dir.join("local_episodes.json.migrated").exists());
        assert_eq!(Library::open(&dir).episodes().len(), 1);
    }
}
