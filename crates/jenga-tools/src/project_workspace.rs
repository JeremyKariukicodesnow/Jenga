//! Project discovery and native hierarchical page creation. No app-only schematic format.
use anyhow::{Context, Result, ensure};
use jenga_core::{
    document::{Document, property},
    export,
    sexpr::Node,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Page {
    pub file: PathBuf,
    pub name: String,
    pub parent: Option<PathBuf>,
    pub sheet_uuid: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Project {
    pub folder: PathBuf,
    pub name: String,
    pub project_file: Option<PathBuf>,
    pub pages: Vec<Page>,
}
pub fn discover(path: &Path) -> Result<Project> {
    let path = jenga_core::document::normalized_path(&path.canonicalize()?);
    let folder = if path.is_dir() {
        path.clone()
    } else {
        path.parent().context("Missing parent")?.to_owned()
    };
    let mut pro: Vec<_> = fs::read_dir(&folder)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "kicad_pro"))
        .collect();
    pro.sort();
    let project_file = if path.extension().is_some_and(|e| e == "kicad_pro") {
        Some(path.clone())
    } else if path.with_extension("kicad_pro").is_file() {
        Some(path.with_extension("kicad_pro"))
    } else if pro.len() == 1 {
        pro.pop()
    } else {
        None
    };
    let name = project_file
        .as_ref()
        .unwrap_or(&path)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let mut project = Project {
        folder: folder.clone(),
        name,
        project_file,
        pages: vec![],
    };
    let mut seen = HashSet::new();
    fn walk(
        path: &Path,
        parent: Option<PathBuf>,
        sheet_uuid: Option<String>,
        name: String,
        seen: &mut HashSet<PathBuf>,
        pages: &mut Vec<Page>,
    ) -> Result<()> {
        let canonical = jenga_core::document::normalized_path(&path.canonicalize()?);
        ensure!(pages.len() < 1000, "Project exceeds 1000 pages");
        if !seen.insert(canonical.clone()) {
            return Ok(());
        }
        let d = Document::open(&canonical)?;
        let title = jenga_core::page::TitleBlock::read(&d).title;
        let name = if parent.is_none() && !title.is_empty() {
            title
        } else {
            name
        };
        pages.push(Page {
            file: canonical.clone(),
            name,
            parent,
            sheet_uuid,
        });
        for sheet in d.root.children("sheet") {
            let file = property(sheet, "Sheetfile");
            if !file.is_empty() {
                walk(
                    &canonical.parent().unwrap().join(file),
                    Some(canonical.clone()),
                    sheet.child("uuid").map(|v| v.value(1).to_owned()),
                    property(sheet, "Sheetname").to_owned(),
                    seen,
                    pages,
                )?;
            }
        }
        Ok(())
    }
    let root = project
        .project_file
        .as_ref()
        .map(|p| p.with_extension("kicad_sch"))
        .unwrap_or(path.clone());
    if root.is_file() {
        walk(
            &root,
            None,
            None,
            project.name.clone(),
            &mut seen,
            &mut project.pages,
        )?;
    }
    // Independent schematics in the same project are also accessible, without
    // treating exported/backed-up copies in nested folders as project pages.
    let mut extras: Vec<_> = fs::read_dir(&folder)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "kicad_sch"))
        .collect();
    extras.sort();
    for p in extras {
        let name = p
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        walk(&p, None, None, name, &mut seen, &mut project.pages)?;
    }
    Ok(project)
}

pub fn create_page(parent: &mut Document, params: &Value) -> Result<Value> {
    ensure!(
        !parent.dirty,
        "Save the current page before adding another page"
    );
    let name = crate::engine::string(params, "name")?;
    crate::projects::valid_name(name)?;
    let parent_path = parent.path.clone().context("Save the project first")?;
    let path = parent_path
        .parent()
        .context("Missing project folder")?
        .join(format!("{name}.kicad_sch"));
    ensure!(!path.exists(), "Page already exists");
    let hierarchical = params
        .get("hierarchical")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let next_page = discover(&parent_path)?.pages.len() + 1;
    let mut child = Document::new();
    let mut title = jenga_core::page::TitleBlock::read(parent);
    title.title = name.into();
    title.write(&mut child);
    let id = uuid::Uuid::new_v4().to_string();
    if hierarchical {
        let at = params
            .get("at")
            .map(|_| crate::engine::point(params, "at"))
            .transpose()?
            .unwrap_or((25.4, 25.4));
        let sheet = Node::list(
            "sheet",
            vec![
                Node::list("at", vec![Node::atom(at.0), Node::atom(at.1)]),
                Node::list("size", vec![Node::atom(50.8), Node::atom(25.4)]),
                Node::list("fields_autoplaced", vec![]),
                Node::list(
                    "stroke",
                    vec![
                        Node::list("width", vec![Node::atom(0)]),
                        Node::list("type", vec![Node::atom("default")]),
                    ],
                ),
                Node::list(
                    "fill",
                    vec![Node::list(
                        "color",
                        vec![Node::atom(0), Node::atom(0), Node::atom(0), Node::atom(0)],
                    )],
                ),
                Node::list("uuid", vec![Node::string(&id)]),
                field("Sheetname", name, at.0, at.1 - 2.54),
                field(
                    "Sheetfile",
                    &format!("{name}.kicad_sch"),
                    at.0,
                    at.1 + 27.94,
                ),
            ],
        );
        parent.root.items_mut().push(sheet);
        child.root.set_child(Node::list(
            "sheet_instances",
            vec![Node::list(
                "path",
                vec![
                    Node::string(format!("/{}/{}", parent.root_uuid(), id)),
                    Node::list(
                        "page",
                        vec![Node::string(
                            params
                                .get("page_number")
                                .and_then(Value::as_str)
                                .map(str::to_owned)
                                .unwrap_or_else(|| next_page.to_string()),
                        )],
                    ),
                ],
            )],
        ));
        parent.dirty = true;
    }
    child.save(&path)?;
    if hierarchical && let Err(e) = parent.save(&parent_path) {
        let _ = fs::remove_file(&path);
        return Err(e);
    }
    let _ = crate::catalog::record("create_schematic", None, Some(&path));
    Ok(
        json!({"file":path,"parent":parent_path,"sheet_uuid":if hierarchical{Some(id)}else{None},"hierarchical":hierarchical}),
    )
}
fn field(key: &str, value: &str, x: f64, y: f64) -> Node {
    Node::list(
        "property",
        vec![
            Node::string(key),
            Node::string(value),
            Node::list("at", vec![Node::atom(x), Node::atom(y), Node::atom(0)]),
            Node::list(
                "effects",
                vec![
                    Node::list(
                        "font",
                        vec![Node::list("size", vec![Node::atom(1.27), Node::atom(1.27)])],
                    ),
                    Node::list("justify", vec![Node::atom("left")]),
                ],
            ),
        ],
    )
}
pub fn export_pages(project: &Project, p: &Value) -> Result<Value> {
    let selected = p.get("pages").and_then(Value::as_array);
    let pages: Vec<_> = project
        .pages
        .iter()
        .filter(|page| {
            selected.is_none_or(|ids| {
                ids.iter().any(|v| {
                    v.as_str()
                        .is_some_and(|s| s == page.name || s == page.file.to_string_lossy())
                })
            })
        })
        .collect();
    ensure!(!pages.is_empty(), "Select at least one project page");
    if let Some(ids) = selected {
        ensure!(
            ids.len() == pages.len(),
            "Unknown or duplicate page selection"
        );
    }
    let path = PathBuf::from(crate::engine::string(p, "path")?);
    let format = p.get("format").and_then(Value::as_str).unwrap_or("pdf");
    let mut docs: Vec<_> = pages
        .iter()
        .map(|p| Document::open(&p.file))
        .collect::<Result<_>>()?;
    for (page, doc) in pages.iter().zip(docs.iter_mut()) {
        doc.page_context = Some((
            project
                .pages
                .iter()
                .position(|p| p.file == page.file)
                .unwrap()
                + 1,
            project.pages.len(),
        ));
    }
    let separate = p.get("separate").and_then(Value::as_bool).unwrap_or(false);
    if format == "pdf" && !separate {
        export::pdf_pages(&docs, &path)?;
    } else {
        ensure!(
            ["pdf", "svg", "png", "kicad_sch"].contains(&format),
            "Project export supports PDF, SVG, PNG or native schematic"
        );
        fs::create_dir_all(&path)?;
        let mut names = HashSet::new();
        for (page, doc) in pages.iter().zip(&docs) {
            let name = page.file.file_stem().unwrap().to_string_lossy();
            ensure!(
                names.insert(name.to_string()),
                "Two pages have the same export filename"
            );
            let out = path.join(format!("{name}.{format}"));
            match format {
                "pdf" => export::pdf(doc, &out)?,
                "svg" => export::svg(doc, &out)?,
                "png" => crate::render::png(doc, &out, 2048)?,
                _ => export::schematic(doc, &out)?,
            }
        }
    }
    Ok(json!({"path":path,"format":format,"pages":pages.len(),"separate":separate||format!="pdf"}))
}
