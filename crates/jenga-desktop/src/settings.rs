use directories::ProjectDirs;
use jenga_libraries::Source;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub schema_version: u32,
    pub recent: Vec<PathBuf>,
    pub libraries: Vec<Source>,
    pub grid: f64,
}
pub fn path() -> PathBuf {
    ProjectDirs::from("org", "Jenga", "Jenga")
        .map(|d| d.config_dir().join("settings.json"))
        .unwrap_or_else(|| PathBuf::from("jenga-settings.json"))
}
impl Settings {
    pub fn load() -> Self {
        std::fs::read_to_string(path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(Self {
                schema_version: 1,
                grid: 1.27,
                ..Self::default()
            })
    }
    pub fn save(&self) -> anyhow::Result<()> {
        jenga_core::document::atomic_write(
            &path(),
            serde_json::to_string_pretty(self)?.as_bytes(),
            false,
        )
    }
    pub fn remember(&mut self, p: PathBuf) {
        self.recent.retain(|r| *r != p);
        self.recent.insert(0, p);
        self.recent.truncate(12);
    }
}
