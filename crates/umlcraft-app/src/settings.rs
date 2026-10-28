//! Small per-user settings, stored as JSON in `%APPDATA%\UmlCraft\settings.json`.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const MAX_RECENT: usize = 8;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// Most recently used project files, newest first.
    pub recent: Vec<PathBuf>,
    pub language: Option<String>,
    pub dark: Option<bool>,
    /// Draw members' visibility in a column of its own, so all names of a compartment start
    /// in one line. Off by default: members then look as in Astah.
    pub visibility_column: bool,
}

impl Settings {
    fn file() -> Option<PathBuf> {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".config")))?;
        Some(base.join("UmlCraft").join("settings.json"))
    }

    /// Loads the settings; missing or broken files give the defaults.
    pub fn load() -> Self {
        Self::file()
            .and_then(|f| fs::read_to_string(f).ok())
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    /// Best effort: a failure to write settings must never disturb the user's work.
    pub fn save(&self) {
        let Some(file) = Self::file() else { return };
        if let Some(dir) = file.parent()
            && fs::create_dir_all(dir).is_ok()
            && let Ok(json) = serde_json::to_string_pretty(self)
        {
            let _ = fs::write(file, json);
        }
    }

    pub fn add_recent(&mut self, path: &Path) {
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_path_buf());
        self.recent.truncate(MAX_RECENT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_files_are_unique_newest_first_and_bounded() {
        let mut s = Settings::default();
        for i in 0..10 {
            s.add_recent(Path::new(&format!("p{i}.ucraft")));
        }
        s.add_recent(Path::new("p5.ucraft"));
        assert_eq!(s.recent.len(), MAX_RECENT);
        assert_eq!(s.recent[0], Path::new("p5.ucraft"));
        assert_eq!(s.recent.iter().filter(|p| *p == Path::new("p5.ucraft")).count(), 1);
    }
}
