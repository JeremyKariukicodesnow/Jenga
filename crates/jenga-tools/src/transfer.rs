//! Staged, non-destructive project import and recoverable relocation.
use crate::engine::{Engine, string};
use anyhow::{Context, Result, ensure};
use jenga_core::{
    document::{Document, atomic_write},
    pcb::Board,
    sexpr::{Node, parse},
};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};
fn tree(from: &Path, to: &Path, depth: usize) -> Result<()> {
    ensure!(
        depth < 32 && !from.is_symlink(),
        "Import does not follow symlinks or deeply nested folders"
    );
    if from.is_dir() {
        fs::create_dir_all(to)?;
        for item in fs::read_dir(from)? {
            let item = item?;
            let n = item.file_name();
            if [".git", "target", ".jenga", "Final"]
                .iter()
                .any(|s| n == *s)
            {
                continue;
            }
            tree(&item.path(), &to.join(n), depth + 1)?;
        }
    } else {
        ensure!(
            fs::metadata(from)?.len() <= 512 * 1024 * 1024,
            "Project asset exceeds 512 MiB"
        );
        fs::copy(from, to)?;
    }
    Ok(())
}
fn clean(e: &Engine) -> Result<()> {
    ensure!(
        !e.doc.as_ref().is_some_and(|d| d.dirty) && !e.board.as_ref().is_some_and(|b| b.dirty),
        "Save schematic and PCB changes before project import or relocation"
    );
    Ok(())
}
fn local_asset(value: &str, original: &Path, stage: &Path) -> Result<Option<String>> {
    let expanded = value
        .replace("${KIPRJMOD}", &original.to_string_lossy())
        .replace("$(KIPRJMOD)", &original.to_string_lossy());
    if expanded.contains("${") || expanded.contains("$(") {
        return Ok(None);
    }
    #[cfg(windows)]
    let expanded = expanded.replace('/', "\\");
    let p = PathBuf::from(&expanded);
    let p = if p.is_absolute() { p } else { original.join(p) };
    if !p.exists() {
        return Ok(None);
    }
    let p = jenga_core::document::normalized_path(&p.canonicalize()?);
    if let Ok(rel) = p.strip_prefix(original) {
        let out = stage.join(rel);
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        tree(&p, &out, 0)?;
        Ok(Some(format!(
            "${{KIPRJMOD}}/{}",
            rel.to_string_lossy().replace('\\', "/")
        )))
    } else {
        Ok(Some(p.to_string_lossy().replace('\\', "/")))
    }
}
fn model_assets(n: &mut Node, original: &Path, stage: &Path, prefix: &str) -> Result<()> {
    if n.name() == "model" {
        if let Some(path) = local_asset(n.value(1), original, stage)? {
            let path = path;
            n.items_mut()[1] = Node::string(path);
        }
    }
    if let Node::List(items) = n {
        for child in items {
            model_assets(child, original, stage, prefix)?;
        }
    }
    Ok(())
}
fn project_folder(e: &Engine) -> Result<PathBuf> {
    e.doc
        .as_ref()
        .and_then(|d| d.path.as_ref())
        .or_else(|| e.board.as_ref()?.path.as_ref())
        .and_then(|p| p.parent())
        .map(Path::to_owned)
        .context("Open a saved project first")
}
fn remap(n: &mut Node, old: &str, new: &str) {
    match n {
        Node::String(s) | Node::Atom(s) => {
            if s.starts_with(old) {
                *s = s.replacen(old, new, 1);
            }
        }
        Node::List(xs) => {
            for x in xs {
                remap(x, old, new);
            }
        }
    }
}
fn data_files(folder: &Path) -> Result<Vec<PathBuf>> {
    fn walk(p: &Path, out: &mut Vec<PathBuf>, depth: usize) -> Result<()> {
        ensure!(
            depth < 32 && !p.is_symlink(),
            "Unsupported project nesting/symlink"
        );
        for entry in fs::read_dir(p)? {
            let p = entry?.path();
            if p.is_dir() {
                if !p.file_name().is_some_and(|n| n == ".git" || n == "target") {
                    walk(&p, out, depth + 1)?;
                }
            } else {
                out.push(p);
            }
        }
        Ok(())
    }
    let mut out = vec![];
    walk(folder, &mut out, 0)?;
    Ok(out)
}
fn validate(folder: &Path) -> Result<()> {
    for p in crate::engine::files(folder, 0)? {
        match p.extension().and_then(|s| s.to_str()) {
            Some("kicad_sch") => {
                Document::open(&p)?;
            }
            Some("kicad_pcb") => {
                Board::open(&p)?;
            }
            Some("kicad_pro") => {
                serde_json::from_slice::<Value>(&fs::read(p)?)?;
            }
            _ => {}
        }
    }
    Ok(())
}
pub fn request(e: &mut Engine, method: &str, p: &Value) -> Result<Value> {
    clean(e)?;
    if method == "import_document" {
        return import_document(e, p);
    }
    let source = if method == "import_project" {
        let path =
            jenga_core::document::normalized_path(&e.resolve(string(p, "path")?).canonicalize()?);
        ensure!(
            path.extension().is_some_and(|s| s == "kicad_pro"),
            "Choose a native KiCad .kicad_pro project"
        );
        path.parent().unwrap().to_owned()
    } else {
        jenga_core::document::normalized_path(&project_folder(e)?.canonicalize()?)
    };
    let projects: Vec<_> = fs::read_dir(&source)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|s| s == "kicad_pro"))
        .collect();
    ensure!(
        projects.len() == 1,
        "Use a dedicated folder containing exactly one KiCad project"
    );
    let name = p["name"].as_str().unwrap_or(
        projects[0]
            .file_stem()
            .unwrap()
            .to_str()
            .context("Invalid project name")?,
    );
    crate::projects::valid_name(name)?;
    let parent = e.resolve(string(p, "folder")?);
    fs::create_dir_all(&parent)?;
    let parent = jenga_core::document::normalized_path(&parent.canonicalize()?);
    let dest = parent.join(name);
    ensure!(
        !dest.exists() && !dest.starts_with(&source),
        "Choose a new destination outside the source project"
    );
    // Keep filenames unchanged: project folder names need not rename native documents.
    let stage = tempfile::Builder::new()
        .prefix(".jenga-import-")
        .tempdir_in(&parent)?;
    if method == "move_project" {
        ensure!(
            source.file_name() == projects[0].file_stem(),
            "Relocation requires a dedicated folder matching the project name"
        );
    }
    tree(&source, stage.path(), 0)?;
    for rel in [".jenga/engineering.json", ".jenga/boards"] {
        let input = source.join(rel);
        if input.exists() {
            if let Some(parent) = stage.path().join(rel).parent() {
                fs::create_dir_all(parent)?;
            }
            tree(&input, &stage.path().join(rel), 0)?;
        }
    }
    for file in data_files(stage.path())? {
        if matches!(
            file.extension().and_then(|s| s.to_str()),
            Some("kicad_sch" | "kicad_pcb" | "kicad_mod" | "kicad_sym")
        ) || file
            .file_name()
            .is_some_and(|s| s == "fp-lib-table" || s == "sym-lib-table")
        {
            let mut root = parse(&fs::read_to_string(&file)?)?;
            remap(
                &mut root,
                &source.to_string_lossy(),
                &dest.to_string_lossy(),
            );
            remap(
                &mut root,
                &source.to_string_lossy().replace('\\', "/"),
                &dest.to_string_lossy().replace('\\', "/"),
            );
            atomic_write(&file, root.document_text().as_bytes(), false)?;
        } else if file.extension().is_some_and(|s| s == "json")
            && file.starts_with(stage.path().join(".jenga/boards"))
        {
            let mut value: Value = serde_json::from_slice(&fs::read(&file)?)?;
            if let Some(link) = value["schematic"].as_str().map(PathBuf::from) {
                if let Ok(rel) = link.strip_prefix(&source) {
                    value["schematic"] = json!(dest.join(rel));
                }
            }
            atomic_write(&file, &serde_json::to_vec_pretty(&value)?, false)?;
        }
    }
    validate(stage.path())?;
    fs::rename(stage.path(), &dest)?;
    let mut recovery = None;
    if method == "move_project" {
        ensure!(
            source.file_name() == projects[0].file_stem(),
            "Relocation requires the dedicated project folder to match the project name; copied destination is preserved and source was not removed"
        );
        let archive = source.with_file_name(format!(
            "{}.jenga-moved-{}",
            source.file_name().unwrap().to_string_lossy(),
            uuid::Uuid::new_v4()
        ));
        fs::rename(&source,&archive).context("Destination is verified, but original folder could not be archived; both copies preserved")?;
        recovery = Some(archive);
    }
    let pro = dest.join(projects[0].file_name().unwrap());
    e.doc = if pro.with_extension("kicad_sch").is_file() {
        Some(Document::open(&pro.with_extension("kicad_sch"))?)
    } else {
        None
    };
    e.board = if pro.with_extension("kicad_pcb").is_file() {
        Some(Board::open(&pro.with_extension("kicad_pcb"))?)
    } else {
        None
    };
    e.active_kind = if e.doc.is_some() { "schematic" } else { "pcb" }.into();
    e.libraries = None;
    let _ = crate::catalog::record(
        if method == "move_project" {
            "rename"
        } else {
            "save_project_as"
        },
        Some(&projects[0]),
        Some(&pro),
    );
    Ok(
        json!({"path":pro,"folder":dest,"recovery_folder":recovery,"native_files_validated":true,"source_preserved":method!="move_project","note":"Project-native relative paths preserved. External assets remain external; inspect model/library findings after import."}),
    )
}
fn import_document(e: &mut Engine, p: &Value) -> Result<Value> {
    let source =
        jenga_core::document::normalized_path(&e.resolve(string(p, "path")?).canonicalize()?);
    let ext = source.extension().and_then(|s| s.to_str()).unwrap_or("");
    ensure!(
        matches!(ext, "kicad_sch" | "kicad_pcb"),
        "Choose .kicad_sch or .kicad_pcb"
    );
    let name = p["name"].as_str().unwrap_or(
        source
            .file_stem()
            .unwrap()
            .to_str()
            .context("Invalid filename")?,
    );
    crate::projects::valid_name(name)?;
    let folder = project_folder(e)?;
    let dest = folder.join(format!("{name}.{ext}"));
    ensure!(!dest.exists(), "Document destination already exists");
    let original = source.parent().unwrap();
    let asset_dir = folder.join("imports").join(name);
    ensure!(
        !asset_dir.exists(),
        "Imported asset namespace exists; choose another name"
    );
    fs::create_dir_all(asset_dir.parent().unwrap())?;
    let stage = tempfile::tempdir_in(asset_dir.parent().unwrap())?;
    // Copy dependencies into an isolated namespace, then register private aliases.
    for sub in ["libraries", "models"] {
        if original.join(sub).is_dir() {
            tree(&original.join(sub), &stage.path().join(sub), 0)?;
        }
    }
    let mut root = parse(&fs::read_to_string(&source)?)?;
    ensure!(
        root.children("sheet").next().is_none(),
        "Hierarchical schematics must be imported as a complete project so child links remain intact"
    );
    let prefix = format!("${{KIPRJMOD}}/imports/{name}");
    model_assets(&mut root, original, stage.path(), &prefix)?;
    remap(&mut root, "${KIPRJMOD}", &prefix);
    let mut tables = vec![];
    for table in ["sym-lib-table", "fp-lib-table"] {
        let input = original.join(table);
        if !input.is_file() {
            continue;
        }
        let mut src = parse(&fs::read_to_string(input)?)?;
        let target = folder.join(table);
        let mut dst = if target.is_file() {
            parse(&fs::read_to_string(&target)?)?
        } else {
            Node::list(
                if table == "sym-lib-table" {
                    "sym_lib_table"
                } else {
                    "fp_lib_table"
                },
                vec![],
            )
        };
        for lib in src.items_mut().iter_mut().filter(|n| n.name() == "lib") {
            if let Some(uri) = lib.child("uri").map(|n| n.value(1).to_owned()) {
                if let Some(path) = local_asset(&uri, original, stage.path())? {
                    lib.set_child(Node::list("uri", vec![Node::string(path)]));
                }
            }
            let old = lib
                .child("name")
                .context("Library alias missing")?
                .value(1)
                .to_owned();
            let new = format!("{name}__{old}");
            ensure!(
                !dst.children("lib")
                    .any(|l| l.child("name").is_some_and(|n| n.value(1) == new)),
                "Imported library alias conflicts: {new}"
            );
            lib.set_child(Node::list("name", vec![Node::string(&new)]));
            remap(lib, "${KIPRJMOD}", &prefix);
            remap(&mut root, &format!("{old}:"), &format!("{new}:"));
            dst.items_mut().push(lib.clone());
        }
        tables.push((target, dst.document_text()));
    }
    for file in data_files(stage.path())? {
        if matches!(
            file.extension().and_then(|s| s.to_str()),
            Some("kicad_mod" | "kicad_sym")
        ) {
            let mut n = parse(&fs::read_to_string(&file)?)?;
            model_assets(&mut n, original, stage.path(), &prefix)?;
            remap(&mut n, "${KIPRJMOD}", &prefix);
            atomic_write(&file, n.document_text().as_bytes(), false)?;
        }
    }
    fs::rename(stage.path(), &asset_dir)?;
    // Native data is staged and aliases are conflict-checked before publishing.
    for (path, text) in tables {
        atomic_write(&path, text.as_bytes(), true)?;
    }
    atomic_write(&dest, root.document_text().as_bytes(), false)?;
    e.libraries = None;
    Ok(
        json!({"path":dest,"assets":asset_dir,"linked_schematic":false,"page_semantics":"independent_drawing_or_physical_board","note":"Imported as an independent document. Review external absolute model paths and intentionally link schematic/PCB only when required."}),
    )
}
