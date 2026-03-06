use std::{collections::HashSet, path::PathBuf, sync::Arc};

use rusqlite::{params, Connection};
use tauri::Emitter;
use walkdir::WalkDir;

const AUDIO_EXTS: &[&str] = &["mp3", "flac", "ogg", "wav", "m4a", "opus", "aac"];

pub fn scan_and_index(
    app: tauri::AppHandle,
    conn: Arc<std::sync::Mutex<Connection>>,
    roots: Vec<PathBuf>,
) -> anyhow::Result<()> {
    std::thread::spawn(move || {
        let mut discovered = 0usize;
        let mut indexed = 0usize;
        let mut seen_paths = HashSet::new();

        for root in roots {
            for entry in WalkDir::new(root)
                .into_iter()
                .flatten()
                .filter(|e| e.file_type().is_file())
            {
                let p = entry.path();
                let ext_ok = p
                    .extension()
                    .and_then(|x| x.to_str())
                    .map(|x| AUDIO_EXTS.contains(&x.to_ascii_lowercase().as_str()))
                    .unwrap_or(false);
                if !ext_ok {
                    continue;
                }
                discovered += 1;
                seen_paths.insert(p.to_string_lossy().to_string());

                let title = p
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Unknown title")
                    .to_string();
                let artist = "Unknown artist".to_string();
                let album = "Unknown album".to_string();
                let album_artist = "".to_string();
                let mtime = entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.elapsed().ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);

                if let Ok(c) = conn.lock() {
                    let _ = c.execute(
                        "INSERT INTO tracks(path,title,artist,album,album_artist,file_mtime) VALUES (?1,?2,?3,?4,?5,?6)
                         ON CONFLICT(path) DO UPDATE SET title=excluded.title, artist=excluded.artist, album=excluded.album, album_artist=excluded.album_artist, file_mtime=excluded.file_mtime",
                        params![p.to_string_lossy(), title, artist, album, album_artist, mtime],
                    );
                }
                indexed += 1;
                if indexed % 100 == 0 {
                    let _ = app.emit(
                        "scan_progress",
                        serde_json::json!({"discovered": discovered, "indexed": indexed}),
                    );
                }
            }
        }

        if let Ok(c) = conn.lock() {
            let mut stmt = c.prepare("SELECT path FROM tracks").ok();
            if let Some(stmt) = stmt.as_mut() {
                let iter = stmt.query_map([], |r| r.get::<_, String>(0)).ok();
                if let Some(iter) = iter {
                    for db_path in iter.flatten() {
                        if !seen_paths.contains(&db_path) {
                            let _ = c.execute("DELETE FROM tracks WHERE path=?1", params![db_path]);
                        }
                    }
                }
            }
        }

        let _ = app.emit("library_updated", serde_json::json!({"indexed": indexed}));
    });
    Ok(())
}
