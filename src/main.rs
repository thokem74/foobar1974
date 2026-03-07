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

use gtk::prelude::*;
use gtk::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, Entry, Grid, Label, ListBox,
    Orientation, Paned, Picture, ScrolledWindow, Separator,
};
use gtk4 as gtk;
use models::{RepeatMode, Track};
use player::{QueueModel, VlcController};
use rusqlite::{params, Connection};
use state::AppStateFile;

#[derive(Clone)]
struct AppCtx {
    db: Arc<Mutex<Connection>>,
    queue: Arc<Mutex<QueueModel>>,
    vlc: Arc<Mutex<Option<VlcController>>>,
    state_path: PathBuf,
    app_state: Arc<Mutex<AppStateFile>>,
}

fn add_library_folder(ctx: &AppCtx, path: String) -> Result<(), String> {
    let mut st = ctx.app_state.lock().map_err(|e| e.to_string())?;
    if !st.library_folders.contains(&path) {
        st.library_folders.push(path);
    }
    state::save(&ctx.state_path, &st).map_err(|e| e.to_string())
}

fn start_scan(ctx: &AppCtx) -> Result<(), String> {
    let roots: Vec<PathBuf> = ctx
        .app_state
        .lock()
        .map_err(|e| e.to_string())?
        .library_folders
        .iter()
        .map(PathBuf::from)
        .collect();
    let app = (); // placeholder to preserve scanner API adaptation below
    library::scan_and_index_no_events(app, ctx.db.clone(), roots).map_err(|e| e.to_string())
}

fn search_tracks(
    ctx: &AppCtx,
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

fn enqueue_and_play(ctx: &AppCtx, track_id: i64) -> Result<(), String> {
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

    Ok(())
}

fn play_pause(ctx: &AppCtx) -> Result<(), String> {
    ensure_vlc(&ctx.vlc)?;
    if let Some(v) = ctx.vlc.lock().map_err(|e| e.to_string())?.as_mut() {
        v.cmd("pause").map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn stop(ctx: &AppCtx) -> Result<(), String> {
    ensure_vlc(&ctx.vlc)?;
    if let Some(v) = ctx.vlc.lock().map_err(|e| e.to_string())?.as_mut() {
        v.cmd("stop").map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn next(ctx: &AppCtx) -> Result<(), String> {
    ensure_vlc(&ctx.vlc)?;
    if let Some(v) = ctx.vlc.lock().map_err(|e| e.to_string())?.as_mut() {
        v.cmd("next").map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn previous(ctx: &AppCtx) -> Result<(), String> {
    ensure_vlc(&ctx.vlc)?;
    if let Some(v) = ctx.vlc.lock().map_err(|e| e.to_string())?.as_mut() {
        v.cmd("prev").map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn set_shuffle(ctx: &AppCtx, enabled: bool) -> Result<(), String> {
    let mut q = ctx.queue.lock().map_err(|e| e.to_string())?;
    q.shuffle = enabled;
    q.rebuild_shuffle_order();
    Ok(())
}

fn set_repeat_mode(ctx: &AppCtx, mode: &str) -> Result<(), String> {
    let mut q = ctx.queue.lock().map_err(|e| e.to_string())?;
    q.repeat_mode = match mode {
        "all" => RepeatMode::All,
        "one" => RepeatMode::One,
        _ => RepeatMode::Off,
    };
    Ok(())
}

fn home_root() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".foobar1974")
}

fn clear_listbox(listbox: &ListBox) {
    while let Some(child) = listbox.first_child() {
        listbox.remove(&child);
    }
}

fn build_ui(app: &Application, ctx: AppCtx) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Igorrr - [Savage Sinusoid #02] ieuD  [foobar2000]")
        .default_width(1100)
        .default_height(760)
        .build();

    let root = GtkBox::new(Orientation::Vertical, 0);

    let menu_row = GtkBox::new(Orientation::Horizontal, 12);
    menu_row.set_margin_start(8);
    menu_row.set_margin_end(8);
    menu_row.set_margin_top(6);
    menu_row.set_margin_bottom(6);
    for item in ["File", "Edit", "View", "Playback", "Library", "Help"] {
        let lbl = Label::new(Some(item));
        lbl.set_halign(Align::Start);
        menu_row.append(&lbl);
    }

    let tabs_row = GtkBox::new(Orientation::Horizontal, 12);
    tabs_row.set_margin_start(8);
    tabs_row.set_margin_end(8);
    tabs_row.set_margin_top(4);
    tabs_row.set_margin_bottom(4);
    for tab in ["Properties", "Playlists"] {
        tabs_row.append(&Label::new(Some(tab)));
    }

    let folder_row = GtkBox::new(Orientation::Horizontal, 6);
    folder_row.set_margin_start(8);
    folder_row.set_margin_end(8);
    folder_row.set_margin_top(2);
    folder_row.set_margin_bottom(4);
    let folder_entry = Entry::builder()
        .placeholder_text("/path/to/music folder")
        .hexpand(true)
        .build();
    let add_folder_button = Button::with_label("Add");
    let scan_button = Button::with_label("Scan");
    folder_row.append(&folder_entry);
    folder_row.append(&add_folder_button);
    folder_row.append(&scan_button);

    let search_row = GtkBox::new(Orientation::Horizontal, 6);
    search_row.set_margin_start(8);
    search_row.set_margin_end(8);
    search_row.set_margin_bottom(4);
    let search_entry = Entry::builder()
        .placeholder_text("Search tracks")
        .hexpand(true)
        .build();
    let search_button = Button::with_label("Search");
    search_row.append(&search_entry);
    search_row.append(&search_button);

    let split_main = Paned::new(Orientation::Horizontal);
    split_main.set_wide_handle(true);
    split_main.set_position(330);

    let left_panel = GtkBox::new(Orientation::Vertical, 0);
    let prop_header = GtkBox::new(Orientation::Horizontal, 0);
    prop_header.set_margin_start(8);
    prop_header.set_margin_end(8);
    prop_header.set_margin_top(6);
    prop_header.set_margin_bottom(6);
    let name_header = Label::new(Some("Name"));
    name_header.set_hexpand(true);
    name_header.set_halign(Align::Start);
    let value_header = Label::new(Some("Value"));
    value_header.set_hexpand(true);
    value_header.set_halign(Align::Start);
    prop_header.append(&name_header);
    prop_header.append(&value_header);
    left_panel.append(&prop_header);
    left_panel.append(&Separator::new(Orientation::Horizontal));

    let metadata_grid = Grid::new();
    metadata_grid.set_column_spacing(12);
    metadata_grid.set_row_spacing(4);
    metadata_grid.set_margin_start(8);
    metadata_grid.set_margin_end(8);
    metadata_grid.set_margin_top(8);
    metadata_grid.set_margin_bottom(8);
    let metadata_rows = [
        ("Metadata", ""),
        ("Artist Name", "Igorrr"),
        ("Track Title", "Viande"),
        ("Album Title", "Savage Sinusoid"),
        ("Date", "2017"),
        ("Album Artist", "Igorrr"),
        ("Track Number", "1"),
        ("Total Tracks", "11"),
        ("ReplayGain", ""),
        ("Track gain", "-9.90 dB"),
        ("Track peak", "1.123491"),
        ("Album gain", "-9.38 dB"),
        ("Album peak", "1.345638"),
    ];
    for (idx, (key, value)) in metadata_rows.iter().enumerate() {
        let key_lbl = Label::new(Some(key));
        key_lbl.set_halign(Align::Start);
        key_lbl.set_hexpand(true);
        let value_lbl = Label::new(Some(value));
        value_lbl.set_halign(Align::Start);
        value_lbl.set_hexpand(true);
        metadata_grid.attach(&key_lbl, 0, idx as i32, 1, 1);
        metadata_grid.attach(&value_lbl, 1, idx as i32, 1, 1);
    }
    left_panel.append(&metadata_grid);

    let album_art = Picture::for_filename("icons/icon.png");
    album_art.set_content_fit(gtk::ContentFit::Contain);
    album_art.set_width_request(300);
    album_art.set_height_request(300);
    album_art.set_margin_start(8);
    album_art.set_margin_end(8);
    album_art.set_margin_top(8);
    album_art.set_margin_bottom(8);
    left_panel.append(&album_art);

    let right_panel = Paned::new(Orientation::Vertical);
    right_panel.set_wide_handle(true);
    right_panel.set_position(460);

    let playlist_box = GtkBox::new(Orientation::Vertical, 0);
    let playlist_header = GtkBox::new(Orientation::Horizontal, 0);
    playlist_header.set_margin_start(8);
    playlist_header.set_margin_end(8);
    playlist_header.set_margin_top(6);
    playlist_header.set_margin_bottom(6);
    let playlist_label = Label::new(Some("Playlist:  Default"));
    playlist_label.set_halign(Align::Start);
    playlist_header.append(&playlist_label);
    playlist_box.append(&playlist_header);
    playlist_box.append(&Separator::new(Orientation::Horizontal));

    let columns_header = GtkBox::new(Orientation::Horizontal, 8);
    columns_header.set_margin_start(8);
    columns_header.set_margin_end(8);
    columns_header.set_margin_top(6);
    columns_header.set_margin_bottom(6);
    for title in [
        "Playi...",
        "Artist/album",
        "Track no",
        "Title / track artist",
        "Durat...",
    ] {
        let c = Label::new(Some(title));
        c.set_halign(Align::Start);
        c.set_hexpand(true);
        columns_header.append(&c);
    }
    playlist_box.append(&columns_header);
    playlist_box.append(&Separator::new(Orientation::Horizontal));

    let status_label = Label::new(None);
    status_label.set_margin_start(8);
    status_label.set_margin_end(8);
    status_label.set_margin_top(4);
    status_label.set_margin_bottom(4);
    status_label.set_halign(Align::Start);

    let listbox = ListBox::new();
    let scroll = ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .build();
    scroll.set_child(Some(&listbox));
    playlist_box.append(&scroll);

    let spectrum = Picture::for_filename("icons/icon.png");
    spectrum.set_content_fit(gtk::ContentFit::Fill);
    spectrum.set_height_request(160);
    right_panel.set_start_child(Some(&playlist_box));
    right_panel.set_end_child(Some(&spectrum));

    split_main.set_start_child(Some(&left_panel));
    split_main.set_end_child(Some(&right_panel));

    let controls = GtkBox::new(Orientation::Horizontal, 8);
    controls.set_margin_start(8);
    controls.set_margin_end(8);
    controls.set_margin_top(6);
    controls.set_margin_bottom(6);
    let previous_btn = Button::with_label("Previous");
    let play_pause_btn = Button::with_label("Play/Pause");
    let next_btn = Button::with_label("Next");
    let stop_btn = Button::with_label("Stop");
    let shuffle_btn = Button::with_label("Toggle Shuffle");
    let repeat_btn = Button::with_label("Repeat All");
    controls.append(&previous_btn);
    controls.append(&play_pause_btn);
    controls.append(&next_btn);
    controls.append(&stop_btn);
    controls.append(&shuffle_btn);
    controls.append(&repeat_btn);

    root.append(&menu_row);
    root.append(&Separator::new(Orientation::Horizontal));
    root.append(&tabs_row);
    root.append(&Separator::new(Orientation::Horizontal));
    root.append(&folder_row);
    root.append(&search_row);
    root.append(&split_main);
    root.append(&status_label);
    root.append(&controls);
    window.set_child(Some(&root));

    let ctx_add = ctx.clone();
    let status_add = status_label.clone();
    add_folder_button.connect_clicked(move |_| {
        let path = folder_entry.text().to_string();
        if path.trim().is_empty() {
            status_add.set_text("Please enter a library folder path");
            return;
        }

        match add_library_folder(&ctx_add, path.clone()) {
            Ok(()) => status_add.set_text(&format!("Added folder: {path}")),
            Err(err) => status_add.set_text(&format!("Failed to add folder: {err}")),
        }
    });

    let ctx_scan = ctx.clone();
    let status_scan = status_label.clone();
    scan_button.connect_clicked(move |_| match start_scan(&ctx_scan) {
        Ok(()) => status_scan.set_text("Scan completed"),
        Err(err) => status_scan.set_text(&format!("Scan failed: {err}")),
    });

    let ctx_search = ctx.clone();
    let status_search = status_label.clone();
    let listbox_search = listbox.clone();
    search_button.connect_clicked(move |_| {
        let q = search_entry.text().to_string();
        match search_tracks(
            &ctx_search,
            q.clone(),
            "artist".into(),
            "asc".into(),
            0,
            500,
        ) {
            Ok(items) => {
                clear_listbox(&listbox_search);
                for track in items {
                    let row = GtkBox::new(Orientation::Horizontal, 8);
                    let summary = format!(
                        "{} — {} ({})",
                        if track.artist.is_empty() {
                            "Unknown Artist"
                        } else {
                            &track.artist
                        },
                        if track.title.is_empty() {
                            "Unknown Title"
                        } else {
                            &track.title
                        },
                        if track.album.is_empty() {
                            "Unknown Album"
                        } else {
                            &track.album
                        }
                    );
                    let label = Label::new(Some(&summary));
                    label.set_halign(Align::Start);
                    label.set_hexpand(true);
                    let play_btn = Button::with_label("Play");
                    let ctx_play = ctx_search.clone();
                    let status_play = status_search.clone();
                    let track_id = track.id;
                    play_btn.connect_clicked(move |_| {
                        match enqueue_and_play(&ctx_play, track_id) {
                            Ok(()) => status_play.set_text("Playing selected track"),
                            Err(err) => status_play.set_text(&format!("Playback failed: {err}")),
                        }
                    });

                    row.append(&label);
                    row.append(&play_btn);
                    listbox_search.append(&row);
                }

                status_search.set_text(&format!(
                    "Loaded {} tracks",
                    listbox_search.observe_children().n_items()
                ));
            }
            Err(err) => status_search.set_text(&format!("Search failed: {err}")),
        }
    });

    let ctx_prev = ctx.clone();
    let status_prev = status_label.clone();
    previous_btn.connect_clicked(move |_| match previous(&ctx_prev) {
        Ok(()) => status_prev.set_text("Sent previous"),
        Err(err) => status_prev.set_text(&format!("Previous failed: {err}")),
    });

    let ctx_pp = ctx.clone();
    let status_pp = status_label.clone();
    play_pause_btn.connect_clicked(move |_| match play_pause(&ctx_pp) {
        Ok(()) => status_pp.set_text("Toggled play/pause"),
        Err(err) => status_pp.set_text(&format!("Play/pause failed: {err}")),
    });

    let ctx_next = ctx.clone();
    let status_next = status_label.clone();
    next_btn.connect_clicked(move |_| match next(&ctx_next) {
        Ok(()) => status_next.set_text("Sent next"),
        Err(err) => status_next.set_text(&format!("Next failed: {err}")),
    });

    let ctx_stop = ctx.clone();
    let status_stop = status_label.clone();
    stop_btn.connect_clicked(move |_| match stop(&ctx_stop) {
        Ok(()) => status_stop.set_text("Sent stop"),
        Err(err) => status_stop.set_text(&format!("Stop failed: {err}")),
    });

    let ctx_shuffle = ctx.clone();
    let status_shuffle = status_label.clone();
    shuffle_btn.connect_clicked(move |_| {
        let enabled = {
            let q = ctx_shuffle.queue.lock();
            match q {
                Ok(q) => !q.shuffle,
                Err(err) => {
                    status_shuffle.set_text(&format!("Shuffle toggle failed: {err}"));
                    return;
                }
            }
        };

        match set_shuffle(&ctx_shuffle, enabled) {
            Ok(()) => status_shuffle.set_text(if enabled { "Shuffle on" } else { "Shuffle off" }),
            Err(err) => status_shuffle.set_text(&format!("Shuffle toggle failed: {err}")),
        }
    });

    let ctx_repeat = ctx.clone();
    let status_repeat = status_label.clone();
    repeat_btn.connect_clicked(move |_| match set_repeat_mode(&ctx_repeat, "all") {
        Ok(()) => status_repeat.set_text("Repeat mode set: all"),
        Err(err) => status_repeat.set_text(&format!("Repeat mode failed: {err}")),
    });

    let ctx_shutdown = ctx.clone();
    app.connect_shutdown(move |_| {
        if let Ok(mut vlc) = ctx_shutdown.vlc.lock() {
            if let Some(controller) = vlc.as_mut() {
                controller.shutdown();
            }
        }
    });

    window.present();
}

fn main() {
    let base = home_root();
    let _ = std::fs::create_dir_all(base.join("cache"));
    let state_path = base.join("state.json");
    let db_path = base.join("library.sqlite");
    let conn = db::open_db(&db_path).expect("db init");
    let app_state = state::load(&state_path).unwrap_or_default();

    let ctx = AppCtx {
        db: Arc::new(Mutex::new(conn)),
        queue: Arc::new(Mutex::new(QueueModel::new())),
        vlc: Arc::new(Mutex::new(None)),
        state_path,
        app_state: Arc::new(Mutex::new(app_state)),
    };

    let app = Application::builder()
        .application_id("com.foobar1974.gtk")
        .build();

    app.connect_activate(move |app| build_ui(app, ctx.clone()));

    app.run();
}
