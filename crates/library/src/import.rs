use std::path::{Path, PathBuf};

use deckforge_analysis::decode::is_supported;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::{Accessor, ItemKey};
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use serde::Serialize;
use walkdir::WalkDir;

use crate::unix_now;

#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct ImportResult {
    pub added: usize,
    pub skipped: usize,
    pub failed: usize,
}

struct Meta {
    title: String,
    artist: String,
    album: String,
    genre: String,
    duration: f64,
    bpm: Option<f64>,
    key: Option<String>,
    artwork: Option<(Option<String>, Vec<u8>)>,
}

fn read_meta(path: &Path) -> Meta {
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let mut meta = Meta {
        title: stem,
        artist: String::new(),
        album: String::new(),
        genre: String::new(),
        duration: 0.0,
        bpm: None,
        key: None,
        artwork: None,
    };
    let Ok(file) = lofty::read_from_path(path) else { return meta };
    meta.duration = file.properties().duration().as_secs_f64();
    if let Some(tag) = file.primary_tag().or_else(|| file.first_tag()) {
        if let Some(t) = tag.title().filter(|t| !t.trim().is_empty()) {
            meta.title = t.to_string();
        }
        meta.artist = tag.artist().map(|s| s.to_string()).unwrap_or_default();
        meta.album = tag.album().map(|s| s.to_string()).unwrap_or_default();
        meta.genre = tag.genre().map(|s| s.to_string()).unwrap_or_default();
        meta.bpm = tag
            .get_string(ItemKey::Bpm)
            .or_else(|| tag.get_string(ItemKey::IntegerBpm))
            .and_then(|s| s.trim().replace(',', ".").parse::<f64>().ok())
            .filter(|b| *b > 20.0 && *b < 400.0);
        meta.key = tag.get_string(ItemKey::InitialKey).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        meta.artwork = tag
            .pictures()
            .first()
            .map(|p| (p.mime_type().map(|m| m.as_str().to_string()), p.data().to_vec()));
    }
    meta
}

fn collect(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for p in paths {
        if p.is_dir() {
            for entry in WalkDir::new(p).follow_links(true).into_iter().filter_map(Result::ok) {
                let path = entry.path();
                let hidden = path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("._"));
                if entry.file_type().is_file() && is_supported(path) && !hidden {
                    files.push(path.to_path_buf());
                }
            }
        } else if p.is_file() && is_supported(p) {
            files.push(p.clone());
        }
    }
    files.sort();
    files.dedup();
    files
}

pub fn import_paths(conn: &Mutex<Connection>, paths: &[PathBuf]) -> ImportResult {
    let mut result = ImportResult::default();
    for path in collect(paths) {
        let path_str = path.to_string_lossy().to_string();
        let exists = conn
            .lock()
            .query_row("SELECT 1 FROM tracks WHERE path = ?1", params![path_str], |_| Ok(()))
            .is_ok();
        if exists {
            result.skipped += 1;
            continue;
        }
        let meta = read_meta(&path);
        let fs_meta = std::fs::metadata(&path).ok();
        let size = fs_meta.as_ref().map(|m| m.len() as i64);
        let mtime = fs_meta
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64);
        let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
        let conn = conn.lock();
        let inserted = conn.execute(
            "INSERT INTO tracks (path, title, artist, album, genre, duration_secs, tag_bpm, tag_key, file_type, file_size, mtime, added_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![path_str, meta.title, meta.artist, meta.album, meta.genre, meta.duration, meta.bpm, meta.key, ext, size, mtime, unix_now()],
        );
        match inserted {
            Ok(_) => {
                result.added += 1;
                if let Some((mime, data)) = meta.artwork {
                    let id = conn.last_insert_rowid();
                    let _ = conn.execute(
                        "INSERT INTO artwork (track_id, mime, data) VALUES (?1, ?2, ?3)",
                        params![id, mime, data],
                    );
                }
            }
            Err(e) => {
                log::warn!("import failed for {path_str}: {e}");
                result.failed += 1;
            }
        }
    }
    result
}
