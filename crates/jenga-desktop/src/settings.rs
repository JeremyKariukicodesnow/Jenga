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
    pub reduced_motion: bool,
}
pub fn path() -> PathBuf {
    if let Some(folder) = std::env::var_os("JENGA_CONFIG_DIR") {
        return PathBuf::from(folder).join("settings.json");
    }
    ProjectDirs::from("org", "Jenga", "Jenga")
        .map(|d| d.config_dir().join("settings.json"))
        .unwrap_or_else(|| PathBuf::from("jenga-settings.json"))
}
impl Settings {
    pub fn load() -> Self {
        let mut settings: Self = std::fs::read_to_string(path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(Self {
                schema_version: 1,
                grid: 1.27,
                ..Self::default()
            });
        // Migrate old recent lists: discover sibling native projects after an old
        // CLI renamed their folder without publishing a desktop catalog event.
        let missing: Vec<_> = settings
            .recent
            .iter()
            .filter(|p| !p.exists())
            .cloned()
            .collect();
        for old in missing {
            if let Some(parent) = old.parent().and_then(|p| p.parent()) {
                for entry in std::fs::read_dir(parent)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .take(512)
                {
                    let folder = entry.path();
                    if !folder.is_dir() {
                        continue;
                    }
                    if let Some(name) = folder.file_name() {
                        let root = folder.join(name).with_extension("kicad_sch");
                        if root.is_file() && root.with_extension("kicad_pro").is_file() {
                            settings.remember(root);
                        }
                    }
                }
            }
        }
        jenga_tools::catalog::apply(&mut settings.recent, &jenga_tools::catalog::events());
        settings.recent.retain(|p| p.is_file());
        settings
    }
    pub fn save(&self) -> anyhow::Result<()> {
        jenga_core::document::atomic_write(
            &path(),
            serde_json::to_string_pretty(self)?.as_bytes(),
            false,
        )
    }
    pub fn remember(&mut self, p: PathBuf) {
        let p = jenga_core::document::normalized_path(&p);
        self.recent
            .retain(|r| !jenga_tools::catalog::same_path(r, &p));
        self.recent.insert(0, p);
        self.recent.truncate(12);
    }
}
