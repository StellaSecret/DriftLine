//! The player's saved state: which levels are done and the display settings. Nothing here touches rendering,
//! so the format and the file handling are tested headlessly (`tests/save.rs`). The browser build keeps the same
//! text in `localStorage` (see `src/storage.rs` and `web/index.html`).
//!
//! Format (plain text, one record per line):
//!
//! ```text
//! vfield-save 1
//! done pos-1
//! done pos-2
//! grid 2
//! hc 0
//! ```
//!
//! Reading is forgiving, because a save outlives the build that wrote it: unknown records and malformed lines
//! are skipped, a level id this build does not have is kept (so a player who goes back and forth between builds
//! loses nothing), and text with no header is read as the old format (one finished level id per line, the
//! `vfield_progress.txt` of earlier builds).
use std::collections::BTreeSet;

pub const HEADER: &str = "vfield-save 1";
/// The arrow-density setting a new player starts with, and its range (`G` cycles 0..=3).
pub const DEFAULT_GRID: usize = 2;
pub const GRID_MODES: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Save {
    /// Ids of finished levels.
    pub done: BTreeSet<String>,
    pub grid: usize,
    pub hc: bool,
}

impl Default for Save {
    fn default() -> Self {
        Save { done: BTreeSet::new(), grid: DEFAULT_GRID, hc: false }
    }
}

fn valid_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 32 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

impl Save {
    pub fn parse(text: &str) -> Save {
        let mut s = Save::default();
        let text = text.trim_start_matches('\u{feff}');
        let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty()).peekable();
        let versioned = lines.peek().is_some_and(|l| l.starts_with("vfield-save"));
        if !versioned {
            // the old format: a bare level id on every line
            s.done = lines.filter(|l| valid_id(l)).map(str::to_string).collect();
            return s;
        }
        lines.next();
        for l in lines {
            let (key, val) = l.split_once(char::is_whitespace).map_or((l, ""), |(k, v)| (k, v.trim()));
            match key {
                "done" if valid_id(val) => {
                    s.done.insert(val.to_string());
                }
                "grid" => {
                    if let Ok(g) = val.parse::<usize>() {
                        s.grid = g.min(GRID_MODES - 1);
                    }
                }
                "hc" => s.hc = val == "1",
                _ => {}
            }
        }
        s
    }

    pub fn render(&self) -> String {
        let mut out = format!("{HEADER}\n");
        for id in &self.done {
            out.push_str(&format!("done {id}\n"));
        }
        out.push_str(&format!("grid {}\nhc {}\n", self.grid.min(GRID_MODES - 1), self.hc as u8));
        out
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub mod file {
    //! The desktop save: one small text file in the player's own data folder.
    use std::path::{Path, PathBuf};

    pub const FILE: &str = "vfield_save.txt";
    /// Where earlier builds wrote progress: the current directory, one level id per line.
    pub const LEGACY_FILE: &str = "vfield_progress.txt";

    /// The folder the save lives in, from the environment (`env` is `std::env::var` in the game, a map in tests):
    /// `VFIELD_SAVE_DIR` if set, else the platform's per-user data folder, else `None` (the caller then uses the
    /// current directory).
    pub fn save_dir(env: &dyn Fn(&str) -> Option<String>, windows: bool, mac: bool) -> Option<PathBuf> {
        let set = |k: &str| env(k).filter(|v| !v.is_empty());
        if let Some(d) = set("VFIELD_SAVE_DIR") {
            return Some(PathBuf::from(d));
        }
        if windows {
            return set("APPDATA").map(|d| PathBuf::from(d).join("vfield"));
        }
        let home = set("HOME").map(PathBuf::from);
        if mac {
            return home.map(|h| h.join("Library").join("Application Support").join("vfield"));
        }
        set("XDG_DATA_HOME").map(|d| PathBuf::from(d).join("vfield")).or_else(|| home.map(|h| h.join(".local").join("share").join("vfield")))
    }

    pub fn default_dir() -> PathBuf {
        save_dir(&|k| std::env::var(k).ok(), cfg!(windows), cfg!(target_os = "macos")).unwrap_or_else(|| PathBuf::from("."))
    }

    /// Reads `dir/FILE`; if there is none yet, the old progress file in the current directory (so a player who
    /// updates keeps what they had). `None` when neither exists or can be read.
    pub fn read_from(dir: &Path, legacy_dir: &Path) -> Option<String> {
        std::fs::read_to_string(dir.join(FILE)).or_else(|_| std::fs::read_to_string(legacy_dir.join(LEGACY_FILE))).ok()
    }

    /// Writes `dir/FILE` by writing a sibling and renaming it over the old one, so a crash mid-write cannot leave
    /// half a save. Creates the folder. Returns whether it worked.
    pub fn write_to(dir: &Path, text: &str) -> bool {
        let go = || -> std::io::Result<()> {
            std::fs::create_dir_all(dir)?;
            let tmp = dir.join(format!("{FILE}.tmp"));
            std::fs::write(&tmp, text)?;
            std::fs::rename(&tmp, dir.join(FILE))
        };
        go().is_ok()
    }
}
