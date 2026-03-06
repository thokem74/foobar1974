mod db;
mod library;
mod models;
mod mpris;
mod player;
mod replaygain;
mod state;

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use models::{QueueItem, RepeatMode, Track};
use player::{QueueModel, VlcController};
use rusqlite::{params, Connection};
use serde::Serialize;
use state::AppStateFile;
use tauri::{Emitter, State};

#[derive(Clone)]
struct AppCtx {
    db: Arc<Mutex<Connection>>,
    queue: Arc<Mutex<QueueModel>>,
    vlc: Arc<Mutex<Option<VlcController>>>,
    state_path: PathBuf,
    app_state: Arc<Mutex<AppStateFile>>,
}

#[derive(Clone, Debug, Serialize)]
struct PlaybackNow {
    track: Option<Track>,
}

#[tauri::command]
fn add_library_folder(ctx: State<AppCtx>, path: String) -> Result<(), String> {
    let mut st = ctx.app_state.lock().map_err(|e| e.to_string())?;
    if !st.library_folders.contains(&path) {
        st.library_folders.push(path);
    }
    state::save(&ctx.state_path, &st).map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_library_folder(ctx: State<AppCtx>, path: String) -> Result<(), String> {
    let mut st = ctx.app_state.lock().map_err(|e| e.to_string())?;
    st.library_folders.retain(|p| p != &path);
    state::save(&ctx.state_path, &st).map_err(|e| e.to_string())
}

#[tauri::command]
fn start_scan(app: tauri::AppHandle, ctx: State<AppCtx>) -> Result<(), String> {
    let roots: Vec<PathBuf> = ctx
        .app_state
        .lock()
        .map_err(|e| e.to_string())?
        .library_folders
        .iter()
        .map(PathBuf::from)
        .collect();
    library::scan_and_index(app, ctx.db.clone(), roots).map_err(|e| e.to_string())
}

#[tauri::command]
fn rescan(app: tauri::AppHandle, ctx: State<AppCtx>) -> Result<(), String> {
    start_scan(app, ctx)
}

#[tauri::command]
fn search_tracks(
    ctx: State<AppCtx>,
    query: String,
    sort: String,
    dir: String,
    offset: i64,
    limit: i64,
) -> Result<Vec<Track>, String> {
    let conn = ctx.db.lock().map_err(|e| e.to_string())?;
    db::search_tracks(&conn, &query, &sort, &dir, offset, limit).map_err(|e| e.to_string())
}

fn ensure_vlc(vlc: &Arc<Mutex<Option<VlcController>>>) -> Result<(), String> {
    let mut guard = vlc.lock().map_err(|e| e.to_string())?;
    if guard.is_none() {
        *guard = Some(VlcController::new().map_err(|e| format!("Failed to spawn cvlc: {e}"))?);
    }
    Ok(())
}

#[tauri::command]
fn enqueue_and_play(
    app: tauri::AppHandle,
    ctx: State<AppCtx>,
    track_id: i64,
) -> Result<(), String> {
    let track = {
        let conn = ctx.db.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT id,path,COALESCE(title,''),COALESCE(artist,''),COALESCE(album,''),COALESCE(album_artist,''),duration_ms FROM tracks WHERE id=?1")
            .map_err(|e| e.to_string())?;
        stmt.query_row(params![track_id], |r| {
            Ok(Track {
                id: r.get(0)?,
                path: r.get(1)?,
                title: r.get(2)?,
                artist: r.get(3)?,
                album: r.get(4)?,
                album_artist: r.get(5)?,
                duration_ms: r.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?
    };

    ensure_vlc(&ctx.vlc)?;
    {
        let mut q = ctx.queue.lock().map_err(|e| e.to_string())?;
        q.enqueue_and_play_index(track.clone());
    }
    {
        let mut vg = ctx.vlc.lock().map_err(|e| e.to_string())?;
        if let Some(v) = vg.as_mut() {
            v.play_file(&track.path).map_err(|e| e.to_string())?;
        }
    }

    app.emit("now_playing_changed", PlaybackNow { track: Some(track) })
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn enqueue(ctx: State<AppCtx>, track_id: i64) -> Result<(), String> {
    let conn = ctx.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("SELECT id,path,COALESCE(title,''),COALESCE(artist,''),COALESCE(album,''),COALESCE(album_artist,''),duration_ms FROM tracks WHERE id=?1").map_err(|e| e.to_string())?;
    let track = stmt
        .query_row(params![track_id], |r| {
            Ok(Track {
                id: r.get(0)?,
                path: r.get(1)?,
                title: r.get(2)?,
                artist: r.get(3)?,
                album: r.get(4)?,
                album_artist: r.get(5)?,
                duration_ms: r.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?;
    ctx.queue
        .lock()
        .map_err(|e| e.to_string())?
        .items
        .push_back(track);
    Ok(())
}

#[tauri::command]
fn remove_from_queue(ctx: State<AppCtx>, index: usize) -> Result<(), String> {
    let mut q = ctx.queue.lock().map_err(|e| e.to_string())?;
    if index < q.items.len() {
        q.items.remove(index);
    }
    Ok(())
}

#[tauri::command]
fn move_queue_item(ctx: State<AppCtx>, from: usize, to: usize) -> Result<(), String> {
    let mut q = ctx.queue.lock().map_err(|e| e.to_string())?;
    if from >= q.items.len() || to >= q.items.len() {
        return Ok(());
    }
    if let Some(item) = q.items.remove(from) {
        q.items.insert(to, item);
    }
    Ok(())
}

#[tauri::command]
fn clear_queue(ctx: State<AppCtx>) -> Result<(), String> {
    ctx.queue.lock().map_err(|e| e.to_string())?.items.clear();
    Ok(())
}

#[tauri::command]
fn play_pause(ctx: State<AppCtx>) -> Result<(), String> {
    ensure_vlc(&ctx.vlc)?;
    if let Some(v) = ctx.vlc.lock().map_err(|e| e.to_string())?.as_mut() {
        v.cmd("pause").map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[tauri::command]
fn stop(ctx: State<AppCtx>) -> Result<(), String> {
    ensure_vlc(&ctx.vlc)?;
    if let Some(v) = ctx.vlc.lock().map_err(|e| e.to_string())?.as_mut() {
        v.cmd("stop").map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[tauri::command]
fn next(ctx: State<AppCtx>) -> Result<(), String> {
    ensure_vlc(&ctx.vlc)?;
    if let Some(v) = ctx.vlc.lock().map_err(|e| e.to_string())?.as_mut() {
        v.cmd("next").map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[tauri::command]
fn previous(ctx: State<AppCtx>) -> Result<(), String> {
    ensure_vlc(&ctx.vlc)?;
    if let Some(v) = ctx.vlc.lock().map_err(|e| e.to_string())?.as_mut() {
        v.cmd("prev").map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[tauri::command]
fn seek(ctx: State<AppCtx>, seconds: u32) -> Result<(), String> {
    ensure_vlc(&ctx.vlc)?;
    if let Some(v) = ctx.vlc.lock().map_err(|e| e.to_string())?.as_mut() {
        v.cmd(&format!("seek {}", seconds))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[tauri::command]
fn set_volume(ctx: State<AppCtx>, percent: u8) -> Result<(), String> {
    ensure_vlc(&ctx.vlc)?;
    let vol = ((percent as f64 / 100.0) * 256.0).round() as i32;
    if let Some(v) = ctx.vlc.lock().map_err(|e| e.to_string())?.as_mut() {
        v.cmd(&format!("volume {}", vol))
            .map_err(|e| e.to_string())?;
    }
    ctx.app_state.lock().map_err(|e| e.to_string())?.volume = percent;
    Ok(())
}
#[tauri::command]
fn set_shuffle(ctx: State<AppCtx>, enabled: bool) -> Result<(), String> {
    let mut q = ctx.queue.lock().map_err(|e| e.to_string())?;
    q.shuffle = enabled;
    q.rebuild_shuffle_order();
    Ok(())
}
#[tauri::command]
fn set_repeat_mode(ctx: State<AppCtx>, mode: String) -> Result<(), String> {
    let mut q = ctx.queue.lock().map_err(|e| e.to_string())?;
    q.repeat_mode = match mode.as_str() {
        "all" => RepeatMode::All,
        "one" => RepeatMode::One,
        _ => RepeatMode::Off,
    };
    Ok(())
}
#[tauri::command]
fn get_now_playing(ctx: State<AppCtx>) -> Result<PlaybackNow, String> {
    let q = ctx.queue.lock().map_err(|e| e.to_string())?;
    let track = q.current_index.and_then(|i| q.items.get(i).cloned());
    Ok(PlaybackNow { track })
}
#[tauri::command]
fn get_album_art(_ctx: State<AppCtx>, _track_id: i64) -> Result<Option<String>, String> {
    Ok(None)
}
#[tauri::command]
fn replaygain_analyze_selected(_ctx: State<AppCtx>, _track_ids: Vec<i64>) -> Result<(), String> {
    Ok(())
}
#[tauri::command]
fn replaygain_analyze_all(_ctx: State<AppCtx>) -> Result<(), String> {
    Ok(())
}
#[tauri::command]
fn set_replaygain_settings(
    ctx: State<AppCtx>,
    mode: String,
    preamp_db: f64,
    prevent_clipping: bool,
) -> Result<(), String> {
    let mut st = ctx.app_state.lock().map_err(|e| e.to_string())?;
    st.replaygain.mode = mode;
    st.replaygain.preamp_db = preamp_db;
    st.replaygain.prevent_clipping = prevent_clipping;
    state::save(&ctx.state_path, &st).map_err(|e| e.to_string())
}

#[tauri::command]
fn queue_items(ctx: State<AppCtx>) -> Result<Vec<QueueItem>, String> {
    let q = ctx.queue.lock().map_err(|e| e.to_string())?;
    Ok(q.items
        .iter()
        .enumerate()
        .map(|(i, t)| QueueItem {
            queue_id: format!("q-{i}"),
            track_id: t.id,
            track: t.clone(),
        })
        .collect())
}

fn home_root() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".foobar1974")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let base = home_root();
    let _ = std::fs::create_dir_all(base.join("cache"));
    let state_path = base.join("state.json");
    let db_path = base.join("library.sqlite");
    let conn = db::open_db(&db_path).expect("db init");
    let app_state = state::load(&state_path).unwrap_or_default();

    tauri::Builder::default()
        .setup(|app| {
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(err) = mpris::start_mpris().await {
                    let _ = app_handle.emit("error", format!("MPRIS init failed: {err}"));
                }
            });
            Ok(())
        })
        .manage(AppCtx {
            db: Arc::new(Mutex::new(conn)),
            queue: Arc::new(Mutex::new(QueueModel::new())),
            vlc: Arc::new(Mutex::new(None)),
            state_path,
            app_state: Arc::new(Mutex::new(app_state)),
        })
        .invoke_handler(tauri::generate_handler![
            add_library_folder,
            remove_library_folder,
            start_scan,
            rescan,
            search_tracks,
            enqueue_and_play,
            enqueue,
            remove_from_queue,
            move_queue_item,
            clear_queue,
            play_pause,
            stop,
            next,
            previous,
            seek,
            set_volume,
            set_shuffle,
            set_repeat_mode,
            get_now_playing,
            get_album_art,
            replaygain_analyze_selected,
            replaygain_analyze_all,
            set_replaygain_settings,
            queue_items
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn main() {
    run();
}
