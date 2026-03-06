PRAGMA journal_mode=WAL;

CREATE TABLE IF NOT EXISTS tracks (
  id INTEGER PRIMARY KEY,
  path TEXT UNIQUE NOT NULL,
  title TEXT,
  artist TEXT,
  album TEXT,
  album_artist TEXT,
  duration_ms INTEGER,
  track_no INTEGER,
  disc_no INTEGER,
  year INTEGER,
  genre TEXT,
  file_mtime INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS replaygain (
  track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
  track_gain_db REAL,
  track_peak REAL,
  album_gain_db REAL,
  album_peak REAL,
  analyzed_at INTEGER
);

CREATE VIRTUAL TABLE IF NOT EXISTS tracks_fts USING fts5(
  title,
  artist,
  album,
  path,
  content='tracks',
  content_rowid='id'
);

CREATE TRIGGER IF NOT EXISTS tracks_ai AFTER INSERT ON tracks BEGIN
  INSERT INTO tracks_fts(rowid, title, artist, album, path)
  VALUES (new.id, new.title, new.artist, new.album, new.path);
END;

CREATE TRIGGER IF NOT EXISTS tracks_ad AFTER DELETE ON tracks BEGIN
  INSERT INTO tracks_fts(tracks_fts, rowid, title, artist, album, path)
  VALUES('delete', old.id, old.title, old.artist, old.album, old.path);
END;

CREATE TRIGGER IF NOT EXISTS tracks_au AFTER UPDATE ON tracks BEGIN
  INSERT INTO tracks_fts(tracks_fts, rowid, title, artist, album, path)
  VALUES('delete', old.id, old.title, old.artist, old.album, old.path);
  INSERT INTO tracks_fts(rowid, title, artist, album, path)
  VALUES (new.id, new.title, new.artist, new.album, new.path);
END;
