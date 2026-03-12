use std::path::Path;

use anyhow::Context;
use rusqlite::{params, Connection};

use crate::models::Track;

#[derive(Debug, Clone)]
pub struct LibraryTrackRow {
    pub artist: String,
    pub album: String,
    pub path: String,
}

pub fn open_db(path: &Path) -> anyhow::Result<Connection> {
    let conn =
        Connection::open(path).with_context(|| format!("opening DB at {}", path.display()))?;
    apply_migrations(&conn)?;
    Ok(conn)
}

pub fn apply_migrations(conn: &Connection) -> anyhow::Result<()> {
    let migration = include_str!("../../migrations/001_init.sql");
    conn.execute_batch(migration)?;
    Ok(())
}

pub fn search_tracks(
    conn: &Connection,
    query: &str,
    sort: &str,
    direction: &str,
    offset: i64,
    limit: i64,
) -> anyhow::Result<Vec<Track>> {
    let sort_field = match sort {
        "title" | "artist" | "album" => sort,
        _ => "artist",
    };
    let dir = if direction.eq_ignore_ascii_case("desc") {
        "DESC"
    } else {
        "ASC"
    };

    let sql = if query.trim().is_empty() {
        format!(
            "SELECT id, path, COALESCE(title,''), COALESCE(artist,''), COALESCE(album,''), COALESCE(album_artist,''), duration_ms \
             FROM tracks ORDER BY {sort_field} {dir} LIMIT ?1 OFFSET ?2"
        )
    } else {
        format!(
            "SELECT t.id, t.path, COALESCE(t.title,''), COALESCE(t.artist,''), COALESCE(t.album,''), COALESCE(t.album_artist,''), t.duration_ms \
             FROM tracks_fts f JOIN tracks t ON t.id = f.rowid \
             WHERE tracks_fts MATCH ?1 ORDER BY t.{sort_field} {dir} LIMIT ?2 OFFSET ?3"
        )
    };

    let mut stmt = conn.prepare(&sql)?;
    let rows = if query.trim().is_empty() {
        stmt.query_map(params![limit, offset], map_track)?
    } else {
        let q = format!("{}*", query.replace('"', ""));
        stmt.query_map(params![q, limit, offset], map_track)?
    };

    Ok(rows.flatten().collect())
}

fn map_track(row: &rusqlite::Row<'_>) -> rusqlite::Result<Track> {
    Ok(Track {
        id: row.get(0)?,
        path: row.get(1)?,
        title: row.get(2)?,
        artist: row.get(3)?,
        album: row.get(4)?,
        album_artist: row.get(5)?,
        duration_ms: row.get(6)?,
    })
}

pub fn list_library_rows(conn: &Connection) -> anyhow::Result<Vec<LibraryTrackRow>> {
    let mut stmt = conn.prepare(
        "SELECT COALESCE(artist,''), COALESCE(album,''), path
         FROM tracks
         ORDER BY artist ASC, album ASC, path ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(LibraryTrackRow {
            artist: row.get(0)?,
            album: row.get(1)?,
            path: row.get(2)?,
        })
    })?;

    Ok(rows.flatten().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_and_search_work() {
        let conn = Connection::open_in_memory().unwrap();
        apply_migrations(&conn).unwrap();
        conn.execute("INSERT INTO tracks(path,title,artist,album,album_artist,file_mtime) VALUES (?1,?2,?3,?4,?5,?6)",
            params!["/tmp/a.flac","Song A","Artist A","Album A","Artist A",1_i64]).unwrap();
        conn.execute("INSERT INTO tracks(path,title,artist,album,album_artist,file_mtime) VALUES (?1,?2,?3,?4,?5,?6)",
            params!["/tmp/b.flac","Song B","Artist B","Album B","Artist B",1_i64]).unwrap();

        let results = search_tracks(&conn, "Song", "title", "asc", 0, 20).unwrap();
        assert_eq!(results.len(), 2);

        let results2 = search_tracks(&conn, "", "artist", "asc", 0, 1).unwrap();
        assert_eq!(results2.len(), 1);
    }
}
