//! Small immutable project events shared by desktop and headless processes.
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
#[derive(Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub time: u128,
    pub kind: String,
    pub old: Option<PathBuf>,
    pub path: Option<PathBuf>,
}
pub fn directory() -> PathBuf {
    std::env::var_os("JENGA_DATA_DIR")
        .or_else(|| std::env::var_os("LOCALAPPDATA"))
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Jenga")
}
pub fn record(kind: &str, old: Option<&Path>, path: Option<&Path>) -> Result<()> {
    let event = Event {
        id: uuid::Uuid::new_v4().to_string(),
        time: crate::journal::now(),
        kind: kind.into(),
        old: old.map(jenga_core::document::normalized_path),
        path: path.map(jenga_core::document::normalized_path),
    };
    let dir = directory().join("project-events");
    std::fs::create_dir_all(&dir)?;
    jenga_core::document::atomic_write(
        &dir.join(format!("{}-{}.json", event.time, event.id)),
        &serde_json::to_vec(&event)?,
        false,
    )
}
pub fn events() -> Vec<Event> {
    let mut out = vec![];
    for file in std::fs::read_dir(directory().join("project-events"))
        .into_iter()
        .flatten()
        .flatten()
    {
        if let Ok(bytes) = std::fs::read(file.path())
            && let Ok(event) = serde_json::from_slice(&bytes)
        {
            out.push(event);
        }
    }
    out.sort_by_key(|e: &Event| (e.time, e.id.clone()));
    out
}
/// Retain unrelated cards, replace renamed paths and include externally saved projects.
pub fn apply(paths: &mut Vec<PathBuf>, events: &[Event]) {
    for e in events {
        if let Some(old) = &e.old {
            if e.kind == "forget" && old.with_extension("kicad_pro").is_file() {
                if let Some(folder) = old.parent() {
                    paths.retain(|p| p.parent() != Some(folder));
                }
            }
            if let Some(folder) = old.parent() {
                if e.kind == "delete_project" {
                    paths.retain(|p| !p.starts_with(folder) || p.is_file());
                }
                if e.kind == "rename" {
                    if let Some(new_folder) = e.path.as_ref().and_then(|p| p.parent()) {
                        for path in paths.iter_mut() {
                            if !path.is_file() && path.starts_with(folder) {
                                if let Ok(relative) = path.strip_prefix(folder) {
                                    let candidate = new_folder.join(relative);
                                    if candidate.is_file() {
                                        *path = candidate;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            paths.retain(|p| !same_path(p, old));
        }
        if let Some(path) = &e.path {
            paths.retain(|p| !same_path(p, path));
            if path.is_file() {
                paths.insert(0, path.clone());
            }
        }
    }
    paths.truncate(24);
}
pub fn same_path(a: &Path, b: &Path) -> bool {
    let a = jenga_core::document::normalized_path(a);
    let b = jenga_core::document::normalized_path(b);
    if cfg!(windows) {
        a.to_string_lossy()
            .eq_ignore_ascii_case(&b.to_string_lossy())
    } else {
        a == b
    }
}
/// Historical delete events must not close a project that was later restored.
pub fn affects_active(event: &Event, active: &Path) -> bool {
    event.old.as_ref().is_some_and(|old| same_path(active, old))
        && (event.kind != "delete_project" || !active.is_file())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restored_project_survives_historical_delete_events() {
        let dir = tempfile::tempdir().unwrap();
        let active = dir.path().join("Restored.kicad_sch");
        let event = Event {
            id: "delete".into(),
            time: 1,
            kind: "delete_project".into(),
            old: Some(active.clone()),
            path: None,
        };
        assert!(affects_active(&event, &active));
        std::fs::write(&active, "restored").unwrap();
        assert!(!affects_active(&event, &active));
    }
    #[test]
    fn rename_and_delete_remove_old_cards() {
        let dir = tempfile::tempdir().unwrap();
        let new = dir.path().join("New.kicad_sch");
        std::fs::write(&new, "x").unwrap();
        let old = dir.path().join("Old.kicad_sch");
        let mut cards = vec![old.clone()];
        apply(
            &mut cards,
            &[Event {
                id: "1".into(),
                time: 1,
                kind: "rename".into(),
                old: Some(old),
                path: Some(new.clone()),
            }],
        );
        assert_eq!(cards, vec![new.clone()]);
        apply(
            &mut cards,
            &[Event {
                id: "2".into(),
                time: 2,
                kind: "delete".into(),
                old: Some(new),
                path: None,
            }],
        );
        assert!(cards.is_empty());
    }
}
