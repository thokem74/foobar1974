use std::{
    collections::VecDeque,
    io::Write,
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
};

use rand::{rngs::StdRng, seq::SliceRandom, SeedableRng};
use serde::Serialize;

use crate::models::{RepeatMode, Track};

#[derive(Debug, Clone, Serialize)]
pub struct PlaybackState {
    pub status: String,
    pub position_sec: u32,
    pub length_sec: u32,
    pub volume_percent: u8,
    pub shuffle: bool,
    pub repeat_mode: RepeatMode,
}

pub struct VlcController {
    child: Child,
    stdin: ChildStdin,
}

impl VlcController {
    pub fn new() -> anyhow::Result<Self> {
        let mut child = Command::new("cvlc")
            .args(["--intf", "rc", "--rc-fake-tty", "--quiet"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("failed to open VLC stdin"))?;
        Ok(Self { child, stdin })
    }

    pub fn cmd(&mut self, cmd: &str) -> anyhow::Result<()> {
        writeln!(self.stdin, "{cmd}")?;
        self.stdin.flush()?;
        Ok(())
    }

    pub fn play_file(&mut self, path: &str) -> anyhow::Result<()> {
        self.cmd("clear")?;
        self.cmd(&format!("add {}", file_uri(path)))
    }

    pub fn shutdown(&mut self) {
        let _ = self.cmd("shutdown");
        let _ = self.child.kill();
    }
}

fn file_uri(path: &str) -> String {
    let path = Path::new(path);
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    };
    format!("file://{}", absolute.to_string_lossy().replace(' ', "%20"))
}

pub struct QueueModel {
    pub items: VecDeque<Track>,
    pub current_index: Option<usize>,
    pub shuffle: bool,
    pub repeat_mode: RepeatMode,
    shuffle_order: Vec<usize>,
}

impl QueueModel {
    pub fn new() -> Self {
        Self {
            items: VecDeque::new(),
            current_index: None,
            shuffle: false,
            repeat_mode: RepeatMode::Off,
            shuffle_order: vec![],
        }
    }

    pub fn enqueue_and_play_index(&mut self, track: Track) -> usize {
        self.items.push_back(track);
        let idx = self.items.len() - 1;
        self.current_index = Some(idx);
        self.rebuild_shuffle_order();
        idx
    }

    pub fn next_index(&mut self) -> Option<usize> {
        let len = self.items.len();
        let current = self.current_index?;
        if len == 0 {
            return None;
        }

        if matches!(self.repeat_mode, RepeatMode::One) {
            return Some(current);
        }

        let candidate = if self.shuffle {
            let pos = self
                .shuffle_order
                .iter()
                .position(|x| *x == current)
                .unwrap_or(0);
            self.shuffle_order.get(pos + 1).copied()
        } else {
            Some(current + 1)
        };

        match candidate {
            Some(n) if n < len => {
                self.current_index = Some(n);
                Some(n)
            }
            _ if matches!(self.repeat_mode, RepeatMode::All) => {
                self.current_index = Some(0);
                Some(0)
            }
            _ => None,
        }
    }

    pub fn rebuild_shuffle_order(&mut self) {
        self.shuffle_order = (0..self.items.len()).collect();
        let mut rng = StdRng::seed_from_u64(1974);
        self.shuffle_order.shuffle(&mut rng);
    }
}
