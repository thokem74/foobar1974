mod db;
mod library;
mod models;
mod mpris;
mod player;
mod replaygain;
mod state;

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use gtk::prelude::*;
use gtk::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, Entry, Grid, Label, ListBox,
    ListBoxRow, Orientation, Paned, Picture, ScrolledWindow, Separator,
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

#[derive(Clone)]
struct TrackDetails {
    track: Track,
    year: Option<i64>,
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

fn load_track_details(ctx: &AppCtx, track_id: i64) -> Result<TrackDetails, String> {
    let conn = ctx.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id,path,COALESCE(title,''),COALESCE(artist,''),COALESCE(album,''),\
             COALESCE(album_artist,''),duration_ms,year FROM tracks WHERE id=?1",
        )
        .map_err(|e| e.to_string())?;
    stmt.query_row(params![track_id], |r| {
        Ok(TrackDetails {
            track: Track {
                id: r.get(0)?,
                path: r.get(1)?,
                title: r.get(2)?,
                artist: r.get(3)?,
                album: r.get(4)?,
                album_artist: r.get(5)?,
                duration_ms: r.get(6)?,
            },
            year: r.get(7)?,
        })
    })
    .map_err(|e| e.to_string())
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

fn resume(ctx: &AppCtx) -> Result<(), String> {
    ensure_vlc(&ctx.vlc)?;
    if let Some(v) = ctx.vlc.lock().map_err(|e| e.to_string())?.as_mut() {
        v.cmd("play").map_err(|e| e.to_string())?;
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

fn label_value(text: &str) -> &str {
    if text.trim().is_empty() {
        ""
    } else {
        text
    }
}

fn codec_from_path(path: &str) -> String {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_uppercase())
        .unwrap_or_default()
}

fn build_ui(app: &Application, ctx: AppCtx) {
    if let Some(display) = gtk::gdk::Display::default() {
        let provider = gtk::CssProvider::new();
        provider.load_from_data(
            "* {
  font-family: Segoe UI, Noto Sans, sans-serif;
}
window, box, paned, scrolledwindow, listbox {
  background: #14161b;
  color: #d9dbe0;
}
entry, button {
  background: #1b1f27;
  color: #e8ebf2;
  border-radius: 0;
  border: 1px solid #2f3643;
}
label {
  color: #d3d6dc;
}
separator {
  background: #2a303c;
}
",
        );
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    let window = ApplicationWindow::builder()
        .application(app)
        .title("foobar1974")
        .default_width(1220)
        .default_height(780)
        .build();

    let root = GtkBox::new(Orientation::Vertical, 0);
    let selected_track_id = Arc::new(Mutex::new(None::<i64>));

    let menu_row = GtkBox::new(Orientation::Horizontal, 14);
    menu_row.set_margin_start(8);
    menu_row.set_margin_end(8);
    menu_row.set_margin_top(6);
    menu_row.set_margin_bottom(4);
    for item in ["File", "Edit", "View", "Playback", "Library", "Help"] {
        let lbl = Label::new(Some(item));
        lbl.set_halign(Align::Start);
        menu_row.append(&lbl);
    }

    let toolbar_row = GtkBox::new(Orientation::Horizontal, 8);
    toolbar_row.set_margin_start(8);
    toolbar_row.set_margin_end(8);
    toolbar_row.set_margin_top(2);
    toolbar_row.set_margin_bottom(6);
    let play_selected_btn = Button::with_label("◻");
    play_selected_btn.set_width_request(30);
    toolbar_row.append(&play_selected_btn);
    let play_btn = Button::with_label("▶");
    play_btn.set_width_request(30);
    toolbar_row.append(&play_btn);
    let pause_btn = Button::with_label("⏸");
    pause_btn.set_width_request(30);
    toolbar_row.append(&pause_btn);
    let prev_btn = Button::with_label("⏮");
    prev_btn.set_width_request(30);
    toolbar_row.append(&prev_btn);
    let next_btn = Button::with_label("⏭");
    next_btn.set_width_request(30);
    toolbar_row.append(&next_btn);
    let stop_btn = Button::with_label("⏹");
    stop_btn.set_width_request(30);
    toolbar_row.append(&stop_btn);
    let help_btn = Button::with_label("?");
    help_btn.set_width_request(30);
    toolbar_row.append(&help_btn);

    let split_main = Paned::new(Orientation::Horizontal);
    split_main.set_wide_handle(true);
    split_main.set_position(300);

    let left_panel = GtkBox::new(Orientation::Vertical, 0);
    let library_title = Label::new(Some("All Music"));
    library_title.set_halign(Align::Start);
    library_title.set_margin_start(8);
    library_title.set_margin_end(8);
    library_title.set_margin_top(6);
    library_title.set_margin_bottom(4);
    left_panel.append(&library_title);

    let library_list = ListBox::new();
    let empty_library_row = Label::new(Some("No albums loaded"));
    empty_library_row.set_halign(Align::Start);
    empty_library_row.set_margin_start(6);
    empty_library_row.set_margin_end(6);
    empty_library_row.set_margin_top(2);
    empty_library_row.set_margin_bottom(2);
    library_list.append(&empty_library_row);
    let library_scroll = ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .build();
    library_scroll.set_child(Some(&library_list));
    left_panel.append(&library_scroll);

    let left_filter_row = GtkBox::new(Orientation::Horizontal, 6);
    left_filter_row.set_margin_start(6);
    left_filter_row.set_margin_end(6);
    left_filter_row.set_margin_top(6);
    left_filter_row.set_margin_bottom(6);
    let view_entry = Entry::builder()
        .text("by artist/album")
        .hexpand(true)
        .build();
    let filter_entry = Entry::builder()
        .placeholder_text("Filter")
        .hexpand(true)
        .build();
    left_filter_row.append(&view_entry);
    left_filter_row.append(&filter_entry);
    left_panel.append(&left_filter_row);

    let spectrum = Picture::for_filename("icons/icon.png");
    spectrum.set_keep_aspect_ratio(false);
    spectrum.set_height_request(130);
    left_panel.append(&spectrum);

    split_main.set_start_child(Some(&left_panel));

    let right_panel = GtkBox::new(Orientation::Vertical, 0);

    let top_props = Paned::new(Orientation::Horizontal);
    top_props.set_wide_handle(true);
    top_props.set_position(410);

    let prop_col_1 = GtkBox::new(Orientation::Vertical, 0);
    let prop_h1 = GtkBox::new(Orientation::Horizontal, 0);
    prop_h1.set_margin_start(8);
    prop_h1.set_margin_end(8);
    prop_h1.set_margin_top(2);
    prop_h1.set_margin_bottom(2);
    let h1n = Label::new(Some("Name"));
    let h1v = Label::new(Some("Value"));
    h1n.set_hexpand(true);
    h1n.set_halign(Align::Start);
    h1v.set_hexpand(true);
    h1v.set_halign(Align::Start);
    prop_h1.append(&h1n);
    prop_h1.append(&h1v);
    prop_col_1.append(&prop_h1);
    prop_col_1.append(&Separator::new(Orientation::Horizontal));

    let mut metadata_artist_value = None;
    let mut metadata_title_value = None;
    let mut metadata_album_value = None;
    let mut metadata_date_value = None;
    let mut metadata_codec_value = None;
    let metadata_grid = Grid::new();
    metadata_grid.set_column_spacing(16);
    metadata_grid.set_row_spacing(4);
    metadata_grid.set_margin_start(8);
    metadata_grid.set_margin_end(8);
    metadata_grid.set_margin_top(8);
    metadata_grid.set_margin_bottom(8);
    let metadata_rows = [
        ("Metadata", ""),
        ("Artist Name", ""),
        ("Track Title", ""),
        ("Album Title", ""),
        ("Date", ""),
        ("Codec", ""),
    ];
    for (idx, (key, value)) in metadata_rows.iter().enumerate() {
        let key_lbl = Label::new(Some(key));
        key_lbl.set_halign(Align::Start);
        let value_lbl = Label::new(Some(value));
        value_lbl.set_halign(Align::Start);
        metadata_grid.attach(&key_lbl, 0, idx as i32, 1, 1);
        metadata_grid.attach(&value_lbl, 1, idx as i32, 1, 1);
        match *key {
            "Artist Name" => metadata_artist_value = Some(value_lbl.clone()),
            "Track Title" => metadata_title_value = Some(value_lbl.clone()),
            "Album Title" => metadata_album_value = Some(value_lbl.clone()),
            "Date" => metadata_date_value = Some(value_lbl.clone()),
            "Codec" => metadata_codec_value = Some(value_lbl.clone()),
            _ => {}
        }
    }
    prop_col_1.append(&metadata_grid);
    let metadata_artist_value = metadata_artist_value.expect("metadata artist label");
    let metadata_title_value = metadata_title_value.expect("metadata title label");
    let metadata_album_value = metadata_album_value.expect("metadata album label");
    let metadata_date_value = metadata_date_value.expect("metadata date label");
    let metadata_codec_value = metadata_codec_value.expect("metadata codec label");

    let prop_col_2 = GtkBox::new(Orientation::Vertical, 0);
    let prop_h2 = GtkBox::new(Orientation::Horizontal, 0);
    prop_h2.set_margin_start(8);
    prop_h2.set_margin_end(8);
    prop_h2.set_margin_top(2);
    prop_h2.set_margin_bottom(2);
    let h2n = Label::new(Some("Name"));
    let h2v = Label::new(Some("Value"));
    h2n.set_hexpand(true);
    h2n.set_halign(Align::Start);
    h2v.set_hexpand(true);
    h2v.set_halign(Align::Start);
    prop_h2.append(&h2n);
    prop_h2.append(&h2v);
    prop_col_2.append(&prop_h2);
    prop_col_2.append(&Separator::new(Orientation::Horizontal));

    let mut location_file_name_value = None;
    let mut location_folder_value = None;
    let mut location_path_value = None;
    let mut location_subsong_value = None;
    let location_grid = Grid::new();
    location_grid.set_column_spacing(16);
    location_grid.set_row_spacing(4);
    location_grid.set_margin_start(8);
    location_grid.set_margin_end(8);
    location_grid.set_margin_top(8);
    location_grid.set_margin_bottom(8);
    let location_rows = [
        ("Location", ""),
        ("File name", ""),
        ("Folder name", ""),
        ("File path", ""),
        ("Subsong index", ""),
    ];
    for (idx, (key, value)) in location_rows.iter().enumerate() {
        let key_lbl = Label::new(Some(key));
        key_lbl.set_halign(Align::Start);
        let value_lbl = Label::new(Some(value));
        value_lbl.set_halign(Align::Start);
        location_grid.attach(&key_lbl, 0, idx as i32, 1, 1);
        location_grid.attach(&value_lbl, 1, idx as i32, 1, 1);
        match *key {
            "File name" => location_file_name_value = Some(value_lbl.clone()),
            "Folder name" => location_folder_value = Some(value_lbl.clone()),
            "File path" => location_path_value = Some(value_lbl.clone()),
            "Subsong index" => location_subsong_value = Some(value_lbl.clone()),
            _ => {}
        }
    }
    prop_col_2.append(&location_grid);
    let location_file_name_value = location_file_name_value.expect("file name label");
    let location_folder_value = location_folder_value.expect("folder label");
    let location_path_value = location_path_value.expect("file path label");
    let location_subsong_value = location_subsong_value.expect("subsong label");

    top_props.set_start_child(Some(&prop_col_1));
    top_props.set_end_child(Some(&prop_col_2));
    right_panel.append(&top_props);

    right_panel.append(&Separator::new(Orientation::Horizontal));

    let playlist_header = GtkBox::new(Orientation::Horizontal, 8);
    playlist_header.set_margin_start(8);
    playlist_header.set_margin_end(8);
    playlist_header.set_margin_top(5);
    playlist_header.set_margin_bottom(5);
    playlist_header.append(&Label::new(Some("Default Playlist")));
    right_panel.append(&playlist_header);

    let columns_header = GtkBox::new(Orientation::Horizontal, 8);
    columns_header.set_margin_start(8);
    columns_header.set_margin_end(8);
    columns_header.set_margin_top(3);
    columns_header.set_margin_bottom(5);
    for title in [
        "Playi...",
        "Artist/album",
        "Track no",
        "Title / track artist",
        "Durat...",
        "Album cover",
    ] {
        let c = Label::new(Some(title));
        c.set_halign(Align::Start);
        c.set_hexpand(true);
        columns_header.append(&c);
    }
    right_panel.append(&columns_header);
    right_panel.append(&Separator::new(Orientation::Horizontal));

    let status_label = Label::new(None);
    status_label.set_halign(Align::Start);
    status_label.set_margin_start(8);
    status_label.set_margin_end(8);
    status_label.set_margin_top(4);
    status_label.set_margin_bottom(4);

    let listbox = ListBox::new();
    let scroll = ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .build();
    scroll.set_child(Some(&listbox));
    right_panel.append(&scroll);

    split_main.set_end_child(Some(&right_panel));

    let search_row = GtkBox::new(Orientation::Horizontal, 6);
    search_row.set_margin_start(8);
    search_row.set_margin_end(8);
    search_row.set_margin_top(6);
    search_row.set_margin_bottom(6);
    let folder_entry = Entry::builder()
        .placeholder_text("/path/to/music folder")
        .hexpand(true)
        .build();
    let add_folder_button = Button::with_label("Add");
    let scan_button = Button::with_label("Scan");
    let search_entry = Entry::builder()
        .placeholder_text("Search tracks")
        .hexpand(true)
        .build();
    let search_button = Button::with_label("Search");
    search_row.append(&folder_entry);
    search_row.append(&add_folder_button);
    search_row.append(&scan_button);
    search_row.append(&search_entry);
    search_row.append(&search_button);

    root.append(&menu_row);
    root.append(&Separator::new(Orientation::Horizontal));
    root.append(&toolbar_row);
    root.append(&Separator::new(Orientation::Horizontal));
    root.append(&split_main);
    root.append(&search_row);
    root.append(&status_label);
    window.set_child(Some(&root));

    let ctx_toolbar_play_selected = ctx.clone();
    let status_toolbar_play_selected = status_label.clone();
    let selected_track_for_play = selected_track_id.clone();
    play_selected_btn.connect_clicked(move |_| {
        let track_id = selected_track_for_play
            .lock()
            .map_err(|e| e.to_string())
            .and_then(|guard| {
                guard.ok_or_else(|| "Select a search result to play".to_string())
            });
        match track_id.and_then(|track_id| enqueue_and_play(&ctx_toolbar_play_selected, track_id)) {
            Ok(()) => status_toolbar_play_selected.set_text("Playing selected track"),
            Err(err) => status_toolbar_play_selected.set_text(&err),
        }
    });

    let ctx_toolbar_resume = ctx.clone();
    let status_toolbar_resume = status_label.clone();
    play_btn.connect_clicked(move |_| match resume(&ctx_toolbar_resume) {
        Ok(()) => status_toolbar_resume.set_text("Resumed playback"),
        Err(err) => status_toolbar_resume.set_text(&format!("Resume failed: {err}")),
    });

    let ctx_toolbar_pause = ctx.clone();
    let status_toolbar_pause = status_label.clone();
    pause_btn.connect_clicked(move |_| match play_pause(&ctx_toolbar_pause) {
        Ok(()) => status_toolbar_pause.set_text("Toggled pause"),
        Err(err) => status_toolbar_pause.set_text(&format!("Pause failed: {err}")),
    });

    let ctx_toolbar_prev = ctx.clone();
    let status_toolbar_prev = status_label.clone();
    prev_btn.connect_clicked(move |_| match previous(&ctx_toolbar_prev) {
        Ok(()) => status_toolbar_prev.set_text("Moved to previous track"),
        Err(err) => status_toolbar_prev.set_text(&format!("Previous failed: {err}")),
    });

    let ctx_toolbar_next = ctx.clone();
    let status_toolbar_next = status_label.clone();
    next_btn.connect_clicked(move |_| match next(&ctx_toolbar_next) {
        Ok(()) => status_toolbar_next.set_text("Moved to next track"),
        Err(err) => status_toolbar_next.set_text(&format!("Next failed: {err}")),
    });

    let ctx_toolbar_stop = ctx.clone();
    let status_toolbar_stop = status_label.clone();
    stop_btn.connect_clicked(move |_| match stop(&ctx_toolbar_stop) {
        Ok(()) => status_toolbar_stop.set_text("Stopped playback"),
        Err(err) => status_toolbar_stop.set_text(&format!("Stop failed: {err}")),
    });

    let status_toolbar_help = status_label.clone();
    help_btn.connect_clicked(move |_| {
        status_toolbar_help
            .set_text("Select a search result, click [ ] to play it, then use transport buttons");
    });

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
    let selected_track_for_search = selected_track_id.clone();
    let metadata_artist_search = metadata_artist_value.clone();
    let metadata_title_search = metadata_title_value.clone();
    let metadata_album_search = metadata_album_value.clone();
    let metadata_date_search = metadata_date_value.clone();
    let metadata_codec_search = metadata_codec_value.clone();
    let location_file_name_search = location_file_name_value.clone();
    let location_folder_search = location_folder_value.clone();
    let location_path_search = location_path_value.clone();
    let location_subsong_search = location_subsong_value.clone();
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
                if let Ok(mut selected) = selected_track_for_search.lock() {
                    *selected = None;
                }
                metadata_artist_search.set_text("");
                metadata_title_search.set_text("");
                metadata_album_search.set_text("");
                metadata_date_search.set_text("");
                metadata_codec_search.set_text("");
                location_file_name_search.set_text("");
                location_folder_search.set_text("");
                location_path_search.set_text("");
                location_subsong_search.set_text("");
                for track in items {
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
                    let row = ListBoxRow::new();
                    row.set_selectable(true);
                    row.set_activatable(true);
                    row.set_child(Some(&label));
                    row.set_widget_name(&format!("track-{}", track.id));
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

    let ctx_row_select = ctx.clone();
    let selected_track_for_rows = selected_track_id.clone();
    let status_row_select = status_label.clone();
    let metadata_artist_select = metadata_artist_value.clone();
    let metadata_title_select = metadata_title_value.clone();
    let metadata_album_select = metadata_album_value.clone();
    let metadata_date_select = metadata_date_value.clone();
    let metadata_codec_select = metadata_codec_value.clone();
    let location_file_name_select = location_file_name_value.clone();
    let location_folder_select = location_folder_value.clone();
    let location_path_select = location_path_value.clone();
    let location_subsong_select = location_subsong_value.clone();
    listbox.connect_row_selected(move |_, row| {
        let Some(row) = row else {
            return;
        };
        if let Some(track_id) = row
            .widget_name()
            .strip_prefix("track-")
            .and_then(|id| id.parse::<i64>().ok())
        {
            if let Ok(mut selected) = selected_track_for_rows.lock() {
                *selected = Some(track_id);
            }
            match load_track_details(&ctx_row_select, track_id) {
                Ok(details) => {
                    let file_path = Path::new(&details.track.path);
                    let file_name = file_path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("");
                    let folder_name = file_path
                        .parent()
                        .and_then(|parent| parent.file_name())
                        .and_then(|name| name.to_str())
                        .unwrap_or("");
                    let folder_path = file_path
                        .parent()
                        .and_then(|parent| parent.to_str())
                        .unwrap_or("");

                    metadata_artist_select.set_text(label_value(&details.track.artist));
                    metadata_title_select.set_text(label_value(&details.track.title));
                    metadata_album_select.set_text(label_value(&details.track.album));
                    metadata_date_select
                        .set_text(&details.year.map(|year| year.to_string()).unwrap_or_default());
                    metadata_codec_select.set_text(&codec_from_path(&details.track.path));
                    location_file_name_select.set_text(file_name);
                    location_folder_select.set_text(folder_name);
                    location_path_select.set_text(folder_path);
                    location_subsong_select.set_text("");
                    status_row_select.set_text("Track selected");
                }
                Err(err) => status_row_select.set_text(&format!("Failed to load track info: {err}")),
            }
        }
    });

    let ctx_row_activate = ctx.clone();
    let status_row_activate = status_label.clone();
    let selected_track_for_activate = selected_track_id.clone();
    listbox.connect_row_activated(move |_, row| {
        if let Some(track_id) = row
            .widget_name()
            .strip_prefix("track-")
            .and_then(|id| id.parse::<i64>().ok())
        {
            if let Ok(mut selected) = selected_track_for_activate.lock() {
                *selected = Some(track_id);
            }
            match enqueue_and_play(&ctx_row_activate, track_id) {
                Ok(()) => status_row_activate.set_text("Playing selected track"),
                Err(err) => status_row_activate.set_text(&format!("Playback failed: {err}")),
            }
        }
    });

    app.connect_shutdown(move |_| {
        if let Ok(mut vlc) = ctx.vlc.lock() {
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
