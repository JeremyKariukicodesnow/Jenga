//! Explicit native project/schematic rename, with staged metadata and rollback on failure.
use anyhow::{Context, Result, ensure};
use jenga_core::{
    document::{Document, atomic_write},
    sexpr::Node,
};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};
pub fn valid_name(name: &str) -> Result<()> {
    ensure!(
        !name.trim().is_empty()
            && name == name.trim()
            && !name.ends_with('.')
            && !name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|'])
            && name != "."
            && name != "..",
        "Use a simple nonempty Windows-safe name"
    );
    let first = name.split('.').next().unwrap().to_uppercase();
    ensure!(
        ![
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
            "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"
        ]
        .contains(&first.as_str()),
        "Reserved Windows name"
    );
    Ok(())
}
fn metadata(node: &mut Node, old: &str, new: &str, oldfile: &str, newfile: &str) {
    if node.name() == "project" && node.value(1) == old {
        node.items_mut()[1] = Node::string(new);
    }
    if node.name() == "property" && node.value(1) == "Sheetfile" && node.value(2) == oldfile {
        node.items_mut()[2] = Node::string(newfile);
    }
    if let Node::List(items) = node {
        for n in items {
            metadata(n, old, new, oldfile, newfile);
        }
    }
}
pub fn rename(doc: &mut Document, params: &Value) -> Result<Value> {
    ensure!(!doc.dirty, "Save unsaved changes before renaming");
    let name = super::engine::string(params, "name")?;
    valid_name(name)?;
    let original = doc
        .path
        .as_ref()
        .context("Save the schematic before renaming")?
        .canonicalize()?;
    let dir = original
        .parent()
        .context("Missing project directory")?
        .to_owned();
    let kind = params
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("schematic");
    ensure!(
        kind == "schematic" || kind == "project",
        "kind must be schematic or project"
    );
    let mut moves: Vec<(PathBuf, PathBuf)> = vec![];
    let mut writes: Vec<(PathBuf, Vec<u8>, Vec<u8>)> = vec![];
    let mut project_file = None;
    let mut old = original.file_stem().unwrap().to_string_lossy().into_owned();
    if kind == "project" {
        let project = if let Some(p) = params.get("project_path").and_then(Value::as_str) {
            let p = PathBuf::from(p);
            if p.is_absolute() { p } else { dir.join(p) }
        } else {
            let candidates: Vec<_> = fs::read_dir(&dir)?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|e| e == "kicad_pro"))
                .collect();
            if original.with_extension("kicad_pro").exists() {
                original.with_extension("kicad_pro")
            } else {
                ensure!(
                    candidates.len() == 1,
                    "Specify project_path when the folder has multiple projects"
                );
                candidates[0].clone()
            }
        }
        .canonicalize()?;
        ensure!(
            project.parent() == Some(dir.as_path()),
            "Project must share the open schematic's directory"
        );
        old = project.file_stem().unwrap().to_string_lossy().into_owned();
        project_file = Some(project);
    } else {
        ensure!(
            !original.with_extension("kicad_pro").exists(),
            "This is a project's root schematic; rename kind:'project' to keep the project usable"
        );
    }
    ensure!(name != old, "New name is unchanged");
    let move_folder = params
        .get("move_folder")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    ensure!(
        !move_folder || kind == "project",
        "move_folder applies only to projects"
    );
    let newdir = if move_folder {
        ensure!(
            dir.file_name().is_some_and(|n| n.to_string_lossy() == old),
            "Folder move requires a directory named exactly like the project"
        );
        let parent = dir
            .parent()
            .context("Cannot rename a filesystem root")?
            .canonicalize()?;
        let target = parent.join(name);
        ensure!(
            target.parent() == Some(parent.as_path()) && !target.exists(),
            "Destination directory already exists or leaves the named parent"
        );
        target
    } else {
        dir.clone()
    };
    let oldroot = if kind == "project" {
        dir.join(format!("{old}.kicad_sch"))
    } else {
        original.clone()
    };
    ensure!(oldroot.is_file(), "Root schematic not found");
    let newroot = newdir.join(format!("{name}.kicad_sch"));
    let rootname = format!("{old}.kicad_sch");
    let newrootname = format!("{name}.kicad_sch");
    let old_inside = newdir.join(oldroot.file_name().unwrap());
    ensure!(
        move_folder || !newroot.exists(),
        "Destination schematic already exists"
    );
    moves.push((old_inside, newroot.clone()));
    if let Some(project) = project_file {
        let mut value: Value = serde_json::from_slice(&fs::read(&project)?)?;
        if let Some(meta) = value.get_mut("meta").and_then(Value::as_object_mut) {
            meta.insert("filename".into(), json!(format!("{name}.kicad_pro")));
        }
        let dest = newdir.join(format!("{name}.kicad_pro"));
        ensure!(
            move_folder || !dest.exists(),
            "Destination project already exists"
        );
        moves.push((newdir.join(project.file_name().unwrap()), dest.clone()));
        writes.push((
            dest,
            fs::read(&project)?,
            serde_json::to_vec_pretty(&value)?,
        ));
        let prl = project.with_extension("kicad_prl");
        if prl.exists() {
            let dest = newdir.join(format!("{name}.kicad_prl"));
            ensure!(
                move_folder || !dest.exists(),
                "Destination preferences already exist"
            );
            moves.push((newdir.join(prl.file_name().unwrap()), dest));
        }
    }
    let candidates = if kind == "project" {
        super::engine::files(&dir, 0)?
    } else {
        vec![original.clone()]
    };
    let mut updated_active = None;
    for path in candidates
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "kicad_sch"))
    {
        let mut d = Document::open(&path)?;
        let previous = d.root.clone();
        metadata(&mut d.root, &old, name, &rootname, &newrootname);
        let destination = if path == oldroot {
            newroot.clone()
        } else {
            newdir.join(path.strip_prefix(&dir)?)
        };
        if d.root != previous {
            writes.push((
                destination.clone(),
                fs::read(&path)?,
                d.root.document_text().into_bytes(),
            ));
        }
        if path == original {
            d.path = Some(destination);
            d.dirty = false;
            updated_active = Some(d);
        }
    }
    let active = updated_active.context("Open schematic was not included in project rename")?;
    let mut done = vec![];
    let mut changed = vec![];
    let mut moved_folder = false;
    let result = (|| -> Result<()> {
        if move_folder {
            fs::rename(&dir, &newdir)?;
            moved_folder = true;
        }
        for (from, to) in &moves {
            ensure!(
                !to.exists(),
                "Rename destination appeared concurrently: {}",
                to.display()
            );
            fs::rename(from, to)?;
            done.push((from.clone(), to.clone()));
        }
        for (path, before, after) in &writes {
            atomic_write(path, after, true)?;
            changed.push((path.clone(), before.clone()));
        }
        Ok(())
    })();
    if let Err(error) = result {
        let mut rollback = vec![];
        for (path, before) in changed.iter().rev() {
            if let Err(e) = atomic_write(path, before, false) {
                rollback.push(e.to_string());
            }
        }
        for (from, to) in done.iter().rev() {
            if let Err(e) = fs::rename(to, from) {
                rollback.push(e.to_string());
            }
        }
        if moved_folder && let Err(e) = fs::rename(&newdir, &dir) {
            rollback.push(e.to_string());
        }
        anyhow::bail!(
            "Rename failed: {error}; rollback: {}",
            if rollback.is_empty() {
                "completed".into()
            } else {
                rollback.join("; ")
            }
        );
    }
    *doc = active;
    Ok(
        json!({"kind":kind,"old_path":original,"path":doc.path,"project_name":name,"moved_folder":move_folder,"revision":super::journal::revision(doc)}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_rename_preserves_uuid_and_native_files() {
        let temp = tempfile::tempdir().unwrap();
        let mut d = jenga_core::document::create_project(temp.path(), "Before").unwrap();
        let id = d.root_uuid();
        let result = rename(
            &mut d,
            &json!({"name":"After","kind":"project","move_folder":true}),
        )
        .unwrap();
        assert!(temp.path().join("After/After.kicad_pro").exists());
        assert!(!temp.path().join("Before").exists());
        assert_eq!(d.root_uuid(), id);
        assert_eq!(result["project_name"], "After");
        let previous = d.path.clone();
        assert!(rename(&mut d, &json!({"name":"CON","kind":"project"})).is_err());
        assert_eq!(d.path, previous);
    }
}
