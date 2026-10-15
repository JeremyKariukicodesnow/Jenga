use anyhow::{Context, Result, ensure};
use jenga_core::{
    document::{Document, atomic_write},
    sexpr::{Node, parse},
};
use serde_json::{Value, json};
pub fn manage(doc: Option<&Document>, p: &Value) -> Result<Value> {
    let scope = p["scope"].as_str().unwrap_or("project");
    let base = if scope == "personal" {
        crate::symbol_editor::personal_folder()?
    } else {
        doc.and_then(|d| d.path.as_ref())
            .and_then(|p| p.parent())
            .context("Open a saved project first")?
            .to_owned()
    };
    let lib = crate::engine::string(p, "library")?;
    crate::projects::valid_name(lib)?;
    let path = base.join("libraries").join(format!("{lib}.kicad_sym"));
    let mut root = parse(&std::fs::read_to_string(&path)?)?;
    let name = crate::engine::string(p, "name")?;
    let at = root
        .items()
        .iter()
        .position(|n| n.name() == "symbol" && n.value(1) == name)
        .context("Symbol not found")?;
    let action = crate::engine::string(p, "action")?;
    if action == "export" {
        let out = std::path::PathBuf::from(crate::engine::string(p, "path")?);
        ensure!(!out.exists(), "Export destination exists");
        let def = jenga_libraries::Libraries::symbol_from_file(&path, name)?;
        let mut export = root.clone();
        export.items_mut().retain(|n| n.name() != "symbol");
        export.items_mut().push(def);
        atomic_write(&out, export.document_text().as_bytes(), false)?;
        return Ok(json!({"path":out}));
    }
    ensure!(
        p["expected_revision"].as_str()
            == Some(crate::journal::revision_text(&root.encoded()).as_str()),
        "Library revision conflict; read_library first"
    );
    if action == "rename" {
        let new = crate::engine::string(p, "new_name")?;
        crate::projects::valid_name(new)?;
        ensure!(
            !root.children("symbol").any(|n| n.value(1) == new),
            "Name already exists"
        );
        root.items_mut()[at].items_mut()[1] = Node::string(new);
        for child in root.items_mut()[at]
            .items_mut()
            .iter_mut()
            .filter(|n| n.name() == "symbol")
        {
            let suffix = child
                .value(1)
                .rsplit('_')
                .take(2)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("_");
            child.items_mut()[1] = Node::string(format!("{new}_{suffix}"));
        }
        for n in root.items_mut().iter_mut().filter(|n| n.name() == "symbol") {
            if let Some(e) = n.child_mut("extends")
                && e.value(1) == name
            {
                e.items_mut()[1] = Node::string(new);
            }
        }
    } else if action == "delete" {
        ensure!(
            !root
                .children("symbol")
                .any(|n| n.child("extends").is_some_and(|n| n.value(1) == name)),
            "Other symbols inherit this symbol; edit their inheritance first"
        );
        root.items_mut().remove(at);
    } else {
        anyhow::bail!("Use rename, delete or export");
    }
    atomic_write(&path, root.document_text().as_bytes(), true)?;
    Ok(
        json!({"path":path,"action":action,"revision":crate::journal::revision_text(&root.encoded()),"note":"Embedded schematic symbols are retained; deleting or renaming the library definition does not rewrite placed instances"}),
    )
}
