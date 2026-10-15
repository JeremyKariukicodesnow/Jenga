use crate::engine::{Engine, string};
use anyhow::{Context, Result, ensure};
use jenga_core::{
    document::atomic_write,
    pcb,
    sexpr::{Node, parse},
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
pub fn folder(e: &Engine) -> PathBuf {
    e.board
        .as_ref()
        .and_then(|b| b.path.as_ref())
        .or_else(|| e.doc.as_ref().and_then(|d| d.path.as_ref()))
        .and_then(|p| p.parent())
        .map(Path::to_owned)
        .unwrap_or_else(|| e.workspace.clone())
}
fn library(e: &Engine, p: &Value) -> Result<(PathBuf, PathBuf, String)> {
    let name = string(p, "library")?;
    crate::projects::valid_name(name)?;
    let personal = p["scope"] == "personal";
    let base = if personal {
        crate::symbol_editor::personal_folder()?
    } else {
        p["project_folder"]
            .as_str()
            .map(|p| e.resolve(p))
            .unwrap_or_else(|| folder(e))
    };
    let dir = base.join("libraries").join(format!("{name}.pretty"));
    let uri = if personal {
        dir.to_string_lossy().into_owned()
    } else {
        format!("${{KIPRJMOD}}/libraries/{name}.pretty")
    };
    Ok((dir, base.join("fp-lib-table"), uri))
}
pub fn save(e: &Engine, p: &Value) -> Result<Value> {
    let def = pcb::footprint_source(p)?;
    let name = string(p, "name")?;
    crate::projects::valid_name(name)?;
    let (dir, table_path, uri) = library(e, p)?;
    let path = dir.join(format!("{name}.kicad_mod"));
    let before = std::fs::read(&path).ok();
    if let Some(bytes) = &before {
        let old = parse(std::str::from_utf8(bytes)?)?;
        ensure!(
            p["expected_revision"].as_str()
                == Some(crate::journal::revision_text(&old.encoded()).as_str()),
            "Footprint revision conflict; read_library again before updating {}",
            path.display()
        );
    }
    let mut table = if table_path.exists() {
        parse(&std::fs::read_to_string(&table_path)?)?
    } else {
        Node::list(
            "fp_lib_table",
            vec![Node::list("version", vec![Node::atom(7)])],
        )
    };
    ensure!(
        table.name() == "fp_lib_table",
        "Invalid footprint library table"
    );
    let alias = string(p, "library")?;
    if let Some(existing) = table
        .children("lib")
        .find(|n| n.child("name").is_some_and(|n| n.value(1) == alias))
    {
        ensure!(
            existing.child("uri").is_some_and(|n| n.value(1) == uri),
            "Library alias {alias} is already registered at another location"
        );
    } else {
        table.items_mut().push(Node::list(
            "lib",
            vec![
                Node::list("name", vec![Node::string(alias)]),
                Node::list("type", vec![Node::string("KiCad")]),
                Node::list("uri", vec![Node::string(uri)]),
                Node::list("options", vec![Node::string("")]),
                Node::list("descr", vec![Node::string("Jenga custom footprints")]),
            ],
        ));
    }
    std::fs::create_dir_all(&dir)?;
    atomic_write(&path, def.document_text().as_bytes(), true)?;
    if let Err(err) = atomic_write(&table_path, table.document_text().as_bytes(), true) {
        if let Some(bytes) = before {
            let _ = atomic_write(&path, &bytes, false);
        } else {
            let _ = std::fs::remove_file(&path);
        }
        return Err(err);
    }
    Ok(
        json!({"path":path,"id":format!("{alias}:{name}"),"revision":crate::journal::revision_text(&def.encoded()),"pads":def.children("pad").count(),"models":def.children("model").count(),"scope":p["scope"]}),
    )
}
pub fn manage(e: &Engine, p: &Value) -> Result<Value> {
    let (dir, _, _) = library(e, p)?;
    let name = string(p, "name")?;
    crate::projects::valid_name(name)?;
    let path = dir.join(format!("{name}.kicad_mod"));
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("Custom footprint not found: {}", path.display()))?;
    let mut def = parse(&text)?;
    let revision = crate::journal::revision_text(&def.encoded());
    let action = string(p, "action")?;
    if action == "export" {
        let out = e.resolve(string(p, "path")?);
        ensure!(!out.exists(), "Export destination exists");
        atomic_write(&out, text.as_bytes(), false)?;
        return Ok(json!({"path":out,"exported":true}));
    }
    ensure!(
        p["expected_revision"] == revision,
        "Footprint revision conflict; read_library again"
    );
    let recovery = dir
        .join(".jenga-recovery")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&recovery)?;
    atomic_write(
        &recovery.join(format!("{name}.kicad_mod")),
        text.as_bytes(),
        false,
    )?;
    match action {
        "delete" => {
            std::fs::remove_file(&path)?;
            Ok(json!({"deleted":true,"recovery":recovery,"placed_instances_preserved":true}))
        }
        "rename" => {
            let new = string(p, "new_name")?;
            crate::projects::valid_name(new)?;
            let dest = dir.join(format!("{new}.kicad_mod"));
            ensure!(!dest.exists(), "Footprint {new} already exists");
            def.items_mut()[1] = Node::string(new);
            atomic_write(&dest, def.document_text().as_bytes(), false)?;
            if let Err(e) = std::fs::remove_file(&path) {
                let _ = std::fs::remove_file(&dest);
                return Err(e.into());
            }
            Ok(
                json!({"path":dest,"revision":crate::journal::revision_text(&def.encoded()),"recovery":recovery,"placed_instances_preserved":true}),
            )
        }
        _ => anyhow::bail!("Footprint action must be rename/delete/export"),
    }
}
pub fn copy(e: &mut Engine, p: &Value) -> Result<Value> {
    let libs = jenga_libraries::Libraries::discover(&[], Some(&folder(e)));
    let id = string(p, "id")?;
    let def = libs.footprint(id)?;
    let mut params = p.clone();
    params["name"] = json!(id.split_once(':').map(|p| p.1).unwrap_or(id));
    params["native_source"] = json!(def.document_text());
    if params["scope"].is_null() {
        params["scope"] = json!("personal");
    }
    let result = save(e, &params)?;
    e.libraries = None;
    Ok(result)
}
pub fn import_model(e: &Engine, p: &Value) -> Result<Value> {
    let source = e.resolve(string(p, "path")?);
    ensure!(
        source.is_file(),
        "Model source not found: {}",
        source.display()
    );
    ensure!(
        source
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(
                |e| ["step", "stp", "wrl", "stl", "obj", "glb", "igs", "iges"]
                    .contains(&e.to_lowercase().as_str())
            ),
        "Unsupported 3D model extension"
    );
    let base = if p["scope"] == "personal" {
        crate::symbol_editor::personal_folder()?
    } else {
        p["project_folder"]
            .as_str()
            .map(|p| e.resolve(p))
            .unwrap_or_else(|| folder(e))
    };
    let folder = base.join("models");
    std::fs::create_dir_all(&folder)?;
    let dest = folder.join(source.file_name().context("Missing model filename")?);
    ensure!(
        !dest.exists(),
        "Model destination already exists: {}",
        dest.display()
    );
    ensure!(
        std::fs::metadata(&source)?.len() <= 256 * 1024 * 1024,
        "Model exceeds 256 MiB"
    );
    std::fs::copy(&source, &dest)?;
    Ok(
        json!({"path":dest,"reference":if p["scope"]=="personal"{dest.to_string_lossy().into_owned()}else{format!("${{KIPRJMOD}}/models/{}",dest.file_name().unwrap().to_string_lossy())},"scope":p["scope"]}),
    )
}
