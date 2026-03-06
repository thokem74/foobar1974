# foobar1974

A foobar2000-inspired Linux music player built with **Tauri + Rust + React (TypeScript)**.

## Stack

- Frontend: React + TypeScript + `react-virtuoso`
- Backend: Rust + Tauri commands/events
- Playback: `cvlc` (VLC RC interface)
- Database: SQLite (+ FTS5)
- ReplayGain: ffmpeg PCM decode + gain math helpers
- Media keys: MPRIS2 (DBus bootstrap)

## Storage layout

- Root: `~/.foobar1974/`
- Database: `~/.foobar1974/library.sqlite`
- State: `~/.foobar1974/state.json`
- Cache: `~/.foobar1974/cache/`

## Features in this implementation

- Modern split UI with virtualized library + queue panes.
- Debounced/paginated backend search (`offset/limit`) and FTS5 support.
- Background scanner with progress events (`scan_progress`, `library_updated`).
- VLC controller (`cvlc --intf rc --rc-fake-tty --quiet`) with core transport commands.
- Queue model with shuffle + repeat state.
- ReplayGain math utilities (dB/linear conversion, clipping prevention, VLC volume mapping).
- MPRIS2 session connection bootstrap.
- Basic persistence for folders/volume/repeat/shuffle/replaygain settings.

## Dependencies

Install runtime requirements:

- `vlc` (must provide `cvlc`)
- `ffmpeg`
- Linux DBus session (for MPRIS2)

Install Linux build requirements (needed for `cargo test` and Tauri builds):

- `pkg-config`
- GLib dev package (`glib-2.0`, `gobject-2.0`, `gio-2.0`)

Examples:

```bash
# Debian/Ubuntu
sudo apt update
sudo apt install -y \
  pkg-config \
  libglib2.0-dev \
  libgtk-3-dev \
  libwebkit2gtk-4.1-dev \
  libsoup-3.0-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  patchelf

# Fedora
sudo dnf install -y pkgconf-pkg-config glib2-devel

# Arch
sudo pacman -S --needed pkgconf glib2
```

## Development

```bash
npm install
npm run dev
```

In another terminal:

```bash
npm run tauri dev
```

## Packaging notes

- Ensure target system includes `cvlc` and `ffmpeg` in `PATH`.
- When packaging, include desktop integration metadata and DBus permissions as needed for MPRIS2.

## Error handling highlights

- Emits `error` event if MPRIS initialization fails.
- Returns command errors for VLC spawn failure / DB issues.
- Scanner continues on unreadable or malformed files.

## Tests

Rust unit tests are provided for:

- DB migration + search CRUD path
- ReplayGain math + clipping prevention + volume cap

Run:

```bash
cd src-tauri
cargo test
```
