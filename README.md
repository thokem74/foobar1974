# foobar1974

A foobar2000-inspired Linux music player built with **GTK4 + Rust**.

## Stack

- UI: GTK4 (native Rust)
- Backend: Rust modules (DB/library/player state)
- Playback: `cvlc` (VLC RC interface)
- Database: SQLite (+ FTS5)
- ReplayGain: ffmpeg PCM decode + gain math helpers
- Media keys: MPRIS2 (DBus bootstrap)

## Repository layout

- Active app code: `src-tauri/` (Rust crate with GTK4 UI and core player logic)

## Storage layout

- Root: `~/.foobar1974/`
- Database: `~/.foobar1974/library.sqlite`
- State: `~/.foobar1974/state.json`
- Cache: `~/.foobar1974/cache/`

## Features in this implementation

- Native GTK4 desktop window with library folder, scan, search, and playback controls.
- SQLite-backed search (`offset/limit`) and FTS5 support.
- Library scanning and indexing for supported audio extensions.
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
- GTK 4 runtime libraries

Install Linux build requirements (needed for `cargo test` and GTK4 builds):

- `pkg-config`
- GTK4 + GLib development packages

Examples:

```bash
# Debian/Ubuntu
sudo apt update
sudo apt install -y \
  pkg-config \
  libglib2.0-dev \
  libgtk-4-dev

# Fedora
sudo dnf install -y pkgconf-pkg-config glib2-devel gtk4-devel

# Arch
sudo pacman -S --needed pkgconf glib2 gtk4
```

## Development

```bash
cd src-tauri
cargo run
```

## Packaging notes

- Ensure target system includes `cvlc` and `ffmpeg` in `PATH`.
- When packaging, include desktop integration metadata and DBus permissions as needed for MPRIS2.

## Error handling highlights

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
